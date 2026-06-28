//! 录制服务

use crate::{
    core::event::{
        Event, EventSender, TaskProgressEvent, TaskStatus as EventTaskStatus, TaskUpdateEvent,
    },
    core::process::{ProcessManager, RecordingConfig, RecordingEngine},
    models::{Channel, ManualRecordRequest, Task},
    services::{
        notification::{
            category as notif_cat, level as notif_lvl, NotificationService, NotifyRequest,
        },
        PostProcessor, ServiceContext,
    },
};
use anyhow::Result;
use chrono::Utc;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// 进程级录制准入锁。
///
/// 用于把「并发检查 → INSERT running」这段 check-then-act 串行化，
/// 避免 cron 触发与 HTTP 手动录制、或多个 cron 同时进入时产生竞态：
///   - 突破 `max_concurrent`
///   - 同频道 / 同定时任务出现重复 running 记录
///
/// 注意：`RecordingService` 是每次请求现 new 的临时对象，没有跨请求共享状态，
/// 因此这把锁必须是进程级静态单例，而不是挂在 service 实例上。
///
/// 临界区只覆盖「检查 + 插入任务记录」，不覆盖后续录制进程的生命周期；
/// 插入完成后立即释放锁，不同频道仍可并发录制，吞吐不受影响。
///
/// 双重保险：DB 层的部分唯一索引（见 migration 0006）在锁失效或多实例时
/// 仍会拒绝重复 running 记录。
static ADMISSION_LOCK: TokioMutex<()> = TokioMutex::const_new(());

/// 录制服务
#[derive(Clone)]
pub struct RecordingService {
    process_manager: Arc<ProcessManager>,
    ctx: ServiceContext,
    event_sender: Option<EventSender>,
}

#[derive(Clone)]
struct RuntimeRecordingSettings {
    recorder_executable: PathBuf,
    ffmpeg_executable: PathBuf,
    recordings_dir: PathBuf,
    default_duration_seconds: i64,
    default_thread_count: i32,
    max_concurrent: usize,
    min_free_space_bytes: u64,
}

impl RecordingService {
    pub fn new(
        process_manager: Arc<ProcessManager>,
        ctx: ServiceContext,
        event_sender: Option<EventSender>,
    ) -> Self {
        Self {
            process_manager,
            ctx,
            event_sender,
        }
    }

    /// 手动启动录制
    pub async fn start_manual(&self, req: ManualRecordRequest) -> Result<Task> {
        let task_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let runtime_settings = self.load_runtime_settings().await?;
        let duration_seconds = req
            .duration_seconds
            .filter(|duration| *duration > 0)
            .unwrap_or(runtime_settings.default_duration_seconds);
        let thread_count = req
            .thread_count
            .filter(|threads| *threads > 0)
            .unwrap_or(runtime_settings.default_thread_count);
        let post_processor = self.build_post_processor(&req, &runtime_settings);

        // 获取频道信息
        let channel = self.get_channel(&req.channel_id).await?;
        let recording_engine = select_recording_engine(&channel.url);
        let recorder_executable = match recording_engine {
            RecordingEngine::NM3u8dlRe => runtime_settings.recorder_executable.clone(),
            RecordingEngine::Ffmpeg => runtime_settings.ffmpeg_executable.clone(),
        };
        self.validate_recorder_executable(recording_engine, &recorder_executable)?;

        // 构建输出文件路径（支持自定义目录和模板）—— 锁外完成，避免在临界区
        // 内做文件系统 I/O。准入串行化只覆盖「检查 → 插入 running」。
        let output_path = self
            .build_output_path(
                &channel,
                &task_id,
                &req.output_name,
                &req.output_dir,
                req.output_template.as_deref(),
                req.transcode_mode.as_deref(),
                &runtime_settings.recordings_dir,
            )
            .await?;

        // 并发准入临界区：锁内完成「检查 → INSERT running」，错误已映射为业务错误。
        // DB 部分唯一索引（migration 0006）为最终兜底。详见 admit_recording 注释。
        self.admit_recording(&task_id, &now, &output_path, &req, &runtime_settings)
            .await?;

        // 构建录制配置
        let config = RecordingConfig {
            recorder_executable: Some(recorder_executable),
            engine: recording_engine,
            url: channel.url.clone(),
            output_path: output_path.clone(),
            duration_seconds: Some(duration_seconds as u64),
            headers: vec![],
            user_agent: Some(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36".to_string(),
            ),
            proxy: None,
            threads: Some(thread_count as usize),
            video_quality: req.video_quality.clone(),
            audio_quality: req.audio_quality.clone(),
            max_speed: req.max_speed.clone(),
            task_id: task_id.clone(),
            channel_name: channel.name.clone(),
        };

        // 启动录制进程
        let handle = match self.process_manager.start_recording(config).await {
            Ok(h) => h,
            Err(e) => {
                // 进程启动失败，更新任务状态为 failed
                let now = Utc::now().to_rfc3339();
                let error_msg = format!("启动录制进程失败: {}", e);
                sqlx::query(
                    r#"
                    UPDATE tasks
                    SET status = 'failed', ended_at = ?, error_message = ?, updated_at = ?
                    WHERE id = ?
                    "#,
                )
                .bind(&now)
                .bind(&error_msg)
                .bind(&now)
                .bind(&task_id)
                .execute(&self.ctx.db)
                .await
                .ok();

                error!("❌ 录制任务启动失败: task_id={}, error={}", task_id, e);

                // 通知：录制启动失败（受 notification.on_failure 开关控制）
                let svc = NotificationService::new(self.ctx.clone(), self.event_sender.clone());
                if let Err(notif_err) = svc
                    .notify(
                        Some("notification.on_failure"),
                        NotifyRequest {
                            category: notif_cat::RECORDING_FAILED.to_string(),
                            level: notif_lvl::ERROR.to_string(),
                            title: format!("录制启动失败: {}", channel.name),
                            message: format!("频道「{}」启动录制失败：{}", channel.name, error_msg),
                            details: Some(
                                serde_json::json!({
                                    "channel": channel.name,
                                    "error": error_msg,
                                })
                                .to_string(),
                            ),
                            task_id: Some(task_id.clone()),
                        },
                    )
                    .await
                {
                    warn!("发送录制启动失败通知失败: {}", notif_err);
                }

                return Err(e);
            }
        };

        // 启动监控任务
        let db = self.ctx.db.clone();
        let output_path_clone = output_path.clone();
        let task_id_clone = task_id.clone();
        let mut status_rx = handle.status_rx;
        let post_processor = post_processor.clone();
        let start_time = std::time::Instant::now();
        let event_sender_clone = self.event_sender.clone();
        // 通知所需上下文：频道名 + 服务上下文（闭包内构造 NotificationService）
        let channel_name_clone = channel.name.clone();
        let notify_ctx = self.ctx.clone();

        tokio::spawn(async move {
            // 进度更新循环
            let progress_update_interval = tokio::time::Duration::from_secs(3);

            loop {
                tokio::select! {
                    // 定期更新进度
                    _ = tokio::time::sleep(progress_update_interval) => {
                        let status = status_rx.borrow().clone();
                        if status == crate::core::process::ProcessStatus::Running {
                            // 计算已录制时长
                            let elapsed = start_time.elapsed().as_secs() as i64;

                            // 计算进度百分比
                            let progress = if duration_seconds > 0 {
                                ((elapsed as f64 / duration_seconds as f64) * 100.0).min(99.0) as i32
                            } else {
                                0
                            };

                            // 获取文件大小 - 查找实际的输出文件
                            let file_size = match find_actual_output_file(&output_path_clone).await {
                                Some(actual_path) => {
                                    tokio::fs::metadata(&actual_path)
                                        .await
                                        .map(|m| m.len() as i64)
                                        .unwrap_or(0)
                                }
                                None => 0,
                            };

                            // 更新数据库
                            let now = chrono::Utc::now().to_rfc3339();
                            let _ = sqlx::query(
                                r#"
                                UPDATE tasks
                                SET progress_percent = ?, duration_recorded = ?, file_size = ?, updated_at = ?
                                WHERE id = ?
                                "#,
                            )
                            .bind(progress)
                            .bind(elapsed)
                            .bind(file_size)
                            .bind(&now)
                            .bind(&task_id_clone)
                            .execute(&db)
                            .await;

                            info!("📊 进度更新: task_id={}, progress={}%, elapsed={}s, size={}",
                                task_id_clone, progress, elapsed, file_size);

                            // 发布进度事件
                            if let Some(ref sender) = event_sender_clone {
                                let eta = if duration_seconds > 0 && elapsed < duration_seconds {
                                    Some((duration_seconds - elapsed) as u64)
                                } else {
                                    None
                                };
                                let _ = sender.send(Event::TaskProgress(TaskProgressEvent {
                                    task_id: task_id_clone.clone(),
                                    percent: progress.clamp(0, 99) as u8,
                                    downloaded_bytes: file_size as u64,
                                    speed: String::new(),
                                    eta_seconds: eta,
                                }));
                            }
                        }
                    }

                    // 等待状态变化
                    _ = status_rx.changed() => {
                        let status = status_rx.borrow().clone();

                        // 如果进程已完成，更新数据库
                        match status {
                            crate::core::process::ProcessStatus::Starting => continue,
                            crate::core::process::ProcessStatus::Running => continue,
                            crate::core::process::ProcessStatus::Stopping => continue,
                            crate::core::process::ProcessStatus::Completed { exit_code: _ } => {
                                break;
                            }
                            crate::core::process::ProcessStatus::Failed { error: _ } => {
                                break;
                            }
                            crate::core::process::ProcessStatus::Cancelled => {
                                break;
                            }
                        }
                    }
                }
            }

            // 最终状态更新
            let status = status_rx.borrow().clone();
            let final_elapsed = start_time.elapsed().as_secs() as i64;

            match status {
                crate::core::process::ProcessStatus::Completed { exit_code } => {
                    info!("录制进程已完成，开始查找输出文件...");

                    // 打印期望的输出路径信息
                    if let Some(parent) = output_path_clone.parent() {
                        info!("期望输出目录: {}", parent.display());
                        // 列出目录中的文件
                        if let Ok(mut entries) = tokio::fs::read_dir(parent).await {
                            info!("目录中的文件:");
                            while let Ok(Some(entry)) = entries.next_entry().await {
                                if let Ok(meta) = entry.metadata().await {
                                    info!(
                                        "  - {} ({} bytes)",
                                        entry.file_name().to_string_lossy(),
                                        meta.len()
                                    );
                                }
                            }
                        }
                    }
                    if let Some(stem) = output_path_clone.file_stem() {
                        info!("期望文件名前缀: {}", stem.to_string_lossy());
                    }

                    // 查找实际的输出文件（N_m3u8DL-RE 可能输出不同扩展名和文件名）
                    let actual_output_path = match find_actual_output_file(&output_path_clone).await
                    {
                        Some(path) => {
                            info!("✅ 找到实际输出文件: {}", path.display());
                            path
                        }
                        None => {
                            warn!(
                                "❌ 未找到输出文件，使用预期路径: {}",
                                output_path_clone.display()
                            );
                            output_path_clone.clone()
                        }
                    };

                    // 检查文件名是否符合预期，如果不符合则重命名
                    let final_output_path = if actual_output_path != output_path_clone {
                        // 获取实际文件的扩展名
                        let actual_ext = actual_output_path
                            .extension()
                            .and_then(|e| e.to_str())
                            .unwrap_or("ts");

                        // 构建预期的最终路径（使用实际扩展名）
                        let expected_stem = output_path_clone
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("");
                        let expected_dir = output_path_clone
                            .parent()
                            .unwrap_or_else(|| std::path::Path::new("."));

                        if !expected_stem.is_empty() {
                            let new_path =
                                expected_dir.join(format!("{}.{}", expected_stem, actual_ext));

                            // 重命名文件
                            match tokio::fs::rename(&actual_output_path, &new_path).await {
                                Ok(_) => {
                                    info!(
                                        "文件已重命名: {} -> {}",
                                        actual_output_path.display(),
                                        new_path.display()
                                    );
                                    new_path
                                }
                                Err(e) => {
                                    warn!(
                                        "重命名文件失败: {}, 使用原始路径: {}",
                                        e,
                                        actual_output_path.display()
                                    );
                                    actual_output_path
                                }
                            }
                        } else {
                            actual_output_path
                        }
                    } else {
                        actual_output_path
                    };

                    // 后处理（转码）
                    let final_path = if post_processor.is_enabled() {
                        match post_processor
                            .process(&final_output_path, &task_id_clone)
                            .await
                        {
                            Ok(path) => path,
                            Err(e) => {
                                error!("后处理失败: {}", e);
                                final_output_path.clone()
                            }
                        }
                    } else {
                        final_output_path.clone()
                    };

                    let now = Utc::now().to_rfc3339();

                    // 获取文件大小
                    let file_size = tokio::fs::metadata(&final_path)
                        .await
                        .map(|m| m.len() as i64)
                        .unwrap_or(0);

                    // 更新输出路径
                    let final_path_str = final_path.to_string_lossy().to_string();

                    let result = sqlx::query(
                        r#"
                        UPDATE tasks
                        SET status = ?, ended_at = ?, exit_code = ?, file_size = ?,
                            duration_recorded = ?, progress_percent = 100, output_path = ?, updated_at = ?
                        WHERE id = ? AND status = 'running'
                        "#,
                    )
                    .bind("completed")
                    .bind(&now)
                    .bind(exit_code)
                    .bind(file_size)
                    .bind(final_elapsed)
                    .bind(&final_path_str)
                    .bind(&now)
                    .bind(&task_id_clone)
                    .execute(&db)
                    .await;

                    if !matches!(result, Ok(done) if done.rows_affected() > 0) {
                        info!(
                            "跳过完成态写回，任务已提前进入其他终态: task_id={}",
                            task_id_clone
                        );
                        return;
                    }

                    info!(
                        "✅ 录制任务完成: task_id={}, output={}, duration={}s, size={}",
                        task_id_clone, final_path_str, final_elapsed, file_size
                    );

                    if let Some(ref sender) = event_sender_clone {
                        let _ = sender.send(Event::TaskUpdate(TaskUpdateEvent {
                            task_id: task_id_clone.clone(),
                            status: EventTaskStatus::Completed,
                            error_message: None,
                        }));
                    }

                    // 通知：录制完成（受 notification.on_complete 开关控制）
                    notify_terminal_task(
                        notify_ctx.clone(),
                        event_sender_clone.clone(),
                        Some("notification.on_complete"),
                        notif_cat::RECORDING_COMPLETE,
                        notif_lvl::INFO,
                        format!("录制完成: {}", channel_name_clone),
                        format!(
                            "频道「{}」录制完成，时长 {}，文件大小 {:.2} MB",
                            channel_name_clone,
                            format_duration(final_elapsed),
                            file_size as f64 / 1024.0 / 1024.0
                        ),
                        Some(
                            serde_json::json!({
                                "channel": channel_name_clone,
                                "duration_seconds": final_elapsed,
                                "file_size": file_size,
                                "output_path": final_path_str,
                                "exit_code": exit_code,
                            })
                            .to_string(),
                        ),
                        task_id_clone.clone(),
                    )
                    .await;
                }
                crate::core::process::ProcessStatus::Failed { error } => {
                    let now = Utc::now().to_rfc3339();

                    // 查找实际的输出文件并获取文件大小
                    let file_size = match find_actual_output_file(&output_path_clone).await {
                        Some(actual_path) => tokio::fs::metadata(&actual_path)
                            .await
                            .map(|m| m.len() as i64)
                            .unwrap_or(0),
                        None => 0,
                    };

                    let result = sqlx::query(
                        r#"
                        UPDATE tasks
                        SET status = ?, ended_at = ?, error_message = ?, file_size = ?,
                            duration_recorded = ?, updated_at = ?
                        WHERE id = ? AND status = 'running'
                        "#,
                    )
                    .bind("failed")
                    .bind(&now)
                    .bind(&error)
                    .bind(file_size)
                    .bind(final_elapsed)
                    .bind(&now)
                    .bind(&task_id_clone)
                    .execute(&db)
                    .await;

                    if !matches!(result, Ok(done) if done.rows_affected() > 0) {
                        info!(
                            "跳过失败态写回，任务已提前进入其他终态: task_id={}",
                            task_id_clone
                        );
                        return;
                    }

                    error!(
                        "❌ 录制任务失败: task_id={}, error={}",
                        task_id_clone, error
                    );

                    if let Some(ref sender) = event_sender_clone {
                        let _ = sender.send(Event::TaskUpdate(TaskUpdateEvent {
                            task_id: task_id_clone.clone(),
                            status: EventTaskStatus::Failed,
                            error_message: Some(error.clone()),
                        }));
                    }

                    // 通知：录制失败（受 notification.on_failure 开关控制）
                    notify_terminal_task(
                        notify_ctx.clone(),
                        event_sender_clone.clone(),
                        Some("notification.on_failure"),
                        notif_cat::RECORDING_FAILED,
                        notif_lvl::ERROR,
                        format!("录制失败: {}", channel_name_clone),
                        format!(
                            "频道「{}」录制失败：{}（已录制 {}）",
                            channel_name_clone,
                            error,
                            format_duration(final_elapsed)
                        ),
                        Some(
                            serde_json::json!({
                                "channel": channel_name_clone,
                                "duration_seconds": final_elapsed,
                                "file_size": file_size,
                                "error": error,
                            })
                            .to_string(),
                        ),
                        task_id_clone.clone(),
                    )
                    .await;
                }
                crate::core::process::ProcessStatus::Cancelled => {
                    let now = Utc::now().to_rfc3339();

                    // 查找实际的输出文件并获取文件大小
                    let file_size = match find_actual_output_file(&output_path_clone).await {
                        Some(actual_path) => tokio::fs::metadata(&actual_path)
                            .await
                            .map(|m| m.len() as i64)
                            .unwrap_or(0),
                        None => 0,
                    };

                    let result = sqlx::query(
                        r#"
                        UPDATE tasks
                        SET status = ?, ended_at = ?, file_size = ?, duration_recorded = ?, updated_at = ?
                        WHERE id = ? AND status = 'running'
                        "#,
                    )
                    .bind("cancelled")
                    .bind(&now)
                    .bind(file_size)
                    .bind(final_elapsed)
                    .bind(&now)
                    .bind(&task_id_clone)
                    .execute(&db)
                    .await;

                    if !matches!(result, Ok(done) if done.rows_affected() > 0) {
                        info!(
                            "跳过取消态写回，任务已提前进入其他终态: task_id={}",
                            task_id_clone
                        );
                        return;
                    }

                    info!(
                        "🚫 录制任务已取消: task_id={}, duration={}s",
                        task_id_clone, final_elapsed
                    );

                    if let Some(ref sender) = event_sender_clone {
                        let _ = sender.send(Event::TaskUpdate(TaskUpdateEvent {
                            task_id: task_id_clone.clone(),
                            status: EventTaskStatus::Cancelled,
                            error_message: None,
                        }));
                    }
                }
                _ => {
                    // 其他状态不应该出现在这里
                    warn!(
                        "⚠️ 录制任务异常结束: task_id={}, status={:?}",
                        task_id_clone, status
                    );
                }
            }
        });

        self.get_task(&task_id).await
    }

    /// 获取任务
    pub async fn get_task(&self, id: &str) -> Result<Task> {
        let task = sqlx::query_as::<_, Task>("SELECT * FROM tasks WHERE id = ?")
            .bind(id)
            .fetch_one(&self.ctx.db)
            .await?;

        Ok(task)
    }

    /// 获取所有任务
    pub async fn list_tasks(&self) -> Result<Vec<Task>> {
        let tasks = sqlx::query_as::<_, Task>("SELECT * FROM tasks ORDER BY created_at DESC")
            .fetch_all(&self.ctx.db)
            .await?;

        Ok(tasks)
    }

    /// 获取运行中的任务
    #[allow(dead_code)]
    pub async fn list_running(&self) -> Result<Vec<Task>> {
        let tasks = sqlx::query_as::<_, Task>("SELECT * FROM tasks WHERE status = 'running'")
            .fetch_all(&self.ctx.db)
            .await?;

        Ok(tasks)
    }

    /// 取消任务
    pub async fn cancel(&self, id: &str) -> Result<()> {
        info!("取消录制任务: task_id={}", id);

        // 先检查任务状态
        let task = self.get_task(id).await?;
        if task.status != "running" {
            warn!("任务不在运行状态: {}, 当前状态: {}", id, task.status);
            // 仍然更新数据库，清理可能的状态不一致
        }

        // 先将状态从 running 原子切到 cancelled，避免后台收尾协程覆盖终态
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query(
            r#"
            UPDATE tasks
            SET status = 'cancelled', ended_at = ?, updated_at = ?
            WHERE id = ? AND status = 'running'
            "#,
        )
        .bind(&now)
        .bind(&now)
        .bind(id)
        .execute(&self.ctx.db)
        .await?;

        // 尝试停止进程（可能不存在，比如僵尸任务）
        match self.process_manager.stop_by_task_id(id).await {
            Ok(_) => {
                info!("进程已停止: task_id={}", id);
            }
            Err(e) => {
                warn!("停止进程失败（可能已结束）: task_id={}, error={}", id, e);
            }
        }

        if result.rows_affected() > 0 {
            info!("任务已取消: task_id={}", id);

            // 通知：录制已取消（不受开关控制，主动操作仍记录以便追溯）
            let channel = self.get_channel(&task.channel_id).await.ok();
            let channel_name = channel
                .as_ref()
                .map(|c| c.name.as_str())
                .unwrap_or("未知频道")
                .to_string();
            let svc = NotificationService::new(self.ctx.clone(), self.event_sender.clone());
            if let Err(e) = svc
                .notify(
                    None,
                    NotifyRequest {
                        category: notif_cat::SYSTEM.to_string(),
                        level: notif_lvl::INFO.to_string(),
                        title: format!("录制已取消: {}", channel_name),
                        message: format!("频道「{}」的录制任务已被手动取消", channel_name),
                        details: Some(
                            serde_json::json!({
                                "channel": channel_name,
                                "task_id": id,
                            })
                            .to_string(),
                        ),
                        task_id: Some(id.to_string()),
                    },
                )
                .await
            {
                warn!("发送取消通知失败: {}", e);
            }
        } else {
            info!("任务状态已更新或不存在: task_id={}", id);
        }

        Ok(())
    }

    /// 清除已完成的任务记录（completed, failed, cancelled）
    pub async fn clear_completed_tasks(&self) -> Result<u64> {
        let result = sqlx::query(
            r#"
            DELETE FROM tasks
            WHERE status IN ('completed', 'failed', 'cancelled')
            "#,
        )
        .execute(&self.ctx.db)
        .await?;

        let count = result.rows_affected();
        if count > 0 {
            info!("已清除 {} 条已完成任务记录", count);
        }

        Ok(count)
    }

    /// 删除单条任务记录
    pub async fn delete_task(&self, id: &str) -> Result<()> {
        // 检查任务是否存在且不在运行中
        let task = self.get_task(id).await?;
        if task.status == "running" {
            return Err(anyhow::anyhow!("无法删除运行中的任务，请先取消任务"));
        }

        sqlx::query("DELETE FROM tasks WHERE id = ?")
            .bind(id)
            .execute(&self.ctx.db)
            .await?;

        info!("已删除任务记录: task_id={}", id);
        Ok(())
    }

    /// 获取频道信息
    async fn get_channel(&self, id: &str) -> Result<Channel> {
        let channel = sqlx::query_as::<_, Channel>("SELECT * FROM channels WHERE id = ?")
            .bind(id)
            .fetch_one(&self.ctx.db)
            .await?;

        Ok(channel)
    }

    async fn validate_schedule_id(&self, schedule_id: Option<&str>) -> Result<()> {
        let Some(schedule_id) = schedule_id else {
            return Ok(());
        };

        let exists: Option<(String,)> =
            sqlx::query_as("SELECT id FROM schedules WHERE id = ? LIMIT 1")
                .bind(schedule_id)
                .fetch_optional(&self.ctx.db)
                .await?;
        if exists.is_none() {
            return Err(anyhow::anyhow!("关联的录制计划不存在: {}", schedule_id));
        }

        Ok(())
    }

    /// 构建输出文件路径
    async fn build_output_path(
        &self,
        channel: &Channel,
        _task_id: &str,
        custom_name: &Option<String>,
        custom_dir: &Option<String>,
        output_template: Option<&str>,
        transcode_mode: Option<&str>,
        default_recordings_dir: &std::path::Path,
    ) -> Result<PathBuf> {
        // 确定输出目录：优先使用自定义目录，否则使用系统默认
        let output_dir: String = if let Some(dir) = custom_dir {
            if !dir.is_empty() {
                // 安全校验:自定义输出目录必须位于录制根目录之下,防止越界写文件。
                // 把 custom_dir 规范化后与 default_recordings_dir 比对前缀。
                let custom_path = std::path::PathBuf::from(dir);
                // 相对路径基于录制根解析,确保不会逃逸到别处
                let resolved = if custom_path.is_absolute() {
                    custom_path
                } else {
                    default_recordings_dir.join(&custom_path)
                };
                // 规范化(目录可能尚不存在,用 lexical 规范化而非 canonicalize)
                let normalized_custom = normalize_path_lexical(&resolved);
                let normalized_root = normalize_path_lexical(default_recordings_dir);
                if !normalized_custom.starts_with(&normalized_root) {
                    anyhow::bail!(
                        "自定义输出目录必须在录制根目录({})之下,拒绝越界路径: {}",
                        default_recordings_dir.display(),
                        dir
                    );
                }
                info!("使用自定义输出目录: {}", dir);
                resolved.to_string_lossy().to_string()
            } else {
                default_recordings_dir.to_string_lossy().to_string()
            }
        } else {
            default_recordings_dir.to_string_lossy().to_string()
        };

        // 确保输出目录存在
        tokio::fs::create_dir_all(&output_dir).await?;

        // 生成文件名（不带后缀，N_m3u8DL-RE 会自动添加）
        // 优先级：output_template > custom_name > 默认模板
        let filename = if let Some(template) = output_template {
            if !template.is_empty() {
                let filename = Self::render_output_template(template, channel);
                // 去掉可能的后缀
                let filename = filename.trim();
                let filename = filename
                    .strip_suffix(".mp4")
                    .or_else(|| filename.strip_suffix(".ts"))
                    .or_else(|| filename.strip_suffix(".mkv"))
                    .unwrap_or(filename);
                let filename = Self::sanitize_output_filename(filename);
                info!("使用模板生成文件名: {} -> {}", template, filename);
                filename
            } else {
                // 模板为空，使用默认
                Self::generate_default_filename(channel)
            }
        } else if let Some(name) = custom_name {
            // 如果指定了 output_name，直接使用（去掉可能的后缀）
            let name = name.trim();
            let name = name
                .strip_suffix(".mp4")
                .or_else(|| name.strip_suffix(".ts"))
                .or_else(|| name.strip_suffix(".mkv"))
                .unwrap_or(name);
            let name = Self::sanitize_output_filename(name);
            info!("使用自定义文件名: {}", name);
            name
        } else {
            // 默认模板
            Self::generate_default_filename(channel)
        };

        let filename = Self::ensure_recording_extension(&filename, transcode_mode);

        info!("最终输出路径: {}/{}", output_dir, filename);
        Ok(PathBuf::from(&output_dir).join(filename))
    }

    /// 渲染输出文件名模板，替换可用变量。
    ///
    /// 支持的变量（均先经过文件名安全清洗）：
    ///   - `{channel_name}` 频道名
    ///   - `{date}`         日期 YYYYMMDD
    ///   - `{time}`         时间 HHMMSS
    ///   - `{datetime}`     日期时间 YYYYMMDD_HHMMSS
    ///   - `{source}`       来源类型：`public`(公网源) / `private`(私有源)
    ///   - `{group}`        分组名
    ///   - `{source_url}`   源地址（URL，非法文件名字符会被替换为 `_`）
    ///
    /// 注意：未识别的 `{xxx}` 占位符会原样保留，随后由 sanitize_output_filename 兜底清洗。
    fn render_output_template(template: &str, channel: &Channel) -> String {
        let now = Utc::now();
        let channel_name = Self::sanitize_filename_part(&channel.name);
        let source = if channel.source_visibility == "private_server_only" {
            "private"
        } else {
            "public"
        };
        let group = Self::sanitize_filename_part(&channel.group_name);
        let source_url = Self::sanitize_filename_part(&channel.url);

        template
            .replace("{channel_name}", &channel_name)
            .replace("{date}", &now.format("%Y%m%d").to_string())
            .replace("{time}", &now.format("%H%M%S").to_string())
            .replace("{datetime}", &now.format("%Y%m%d_%H%M%S").to_string())
            .replace("{source}", source)
            .replace("{group}", &group)
            .replace("{source_url}", &source_url)
    }

    /// 生成默认文件名
    fn generate_default_filename(channel: &Channel) -> String {
        let now = Utc::now();
        let channel_name = Self::sanitize_filename_part(&channel.name);
        format!("{}_{}", channel_name, now.format("%Y%m%d_%H%M%S"))
    }

    fn sanitize_filename_part(input: &str) -> String {
        let sanitized = input
            .chars()
            .map(|ch| {
                if ch.is_control()
                    || matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
                {
                    '_'
                } else if ch.is_whitespace() {
                    '_'
                } else {
                    ch
                }
            })
            .collect::<String>()
            .trim_matches('_')
            .to_string();

        if sanitized.is_empty() {
            "recording".to_string()
        } else {
            sanitized
        }
    }

    fn sanitize_output_filename(input: &str) -> String {
        let file_name = Path::new(input)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(input);
        let sanitized = Self::sanitize_filename_part(file_name)
            .trim_matches('.')
            .trim_matches('_')
            .to_string();

        if sanitized.is_empty() {
            "recording".to_string()
        } else {
            sanitized
        }
    }

    fn ensure_recording_extension(filename: &str, transcode_mode: Option<&str>) -> String {
        let path = Path::new(filename);
        if path.extension().is_some() {
            filename.to_string()
        } else if let Some(mode) = transcode_mode {
            if mode == "off" || mode.is_empty() {
                format!("{filename}.ts")
            } else {
                format!("{filename}.mp4")
            }
        } else {
            format!("{filename}.ts")
        }
    }

    async fn load_runtime_settings(&self) -> Result<RuntimeRecordingSettings> {
        let recorder_executable = self
            .get_system_value_string(
                "recording.n_m3u8dl_re_path",
                &self.ctx.config.recorder.executable.to_string_lossy(),
            )
            .await?;
        let recorder_executable = normalize_recorder_executable(&recorder_executable);
        let recordings_dir = self
            .get_system_value_string(
                "storage.recordings_path",
                &self.ctx.config.storage.recordings_dir.to_string_lossy(),
            )
            .await?;
        let ffmpeg_default = if self.ctx.config.recorder.post_process.ffmpeg_path.is_empty() {
            "ffmpeg".to_string()
        } else {
            self.ctx.config.recorder.post_process.ffmpeg_path.clone()
        };
        let ffmpeg_executable = self
            .get_system_value_string("recorder.post_process.ffmpeg_path", &ffmpeg_default)
            .await?;
        let ffmpeg_executable = normalize_ffmpeg_executable(&ffmpeg_executable);
        let min_free_space_gb = self
            .get_system_value("storage.min_free_space_gb", 0u64)
            .await?;

        Ok(RuntimeRecordingSettings {
            recorder_executable,
            ffmpeg_executable,
            recordings_dir: PathBuf::from(recordings_dir),
            default_duration_seconds: self
                .get_system_value("recording.default_duration_minutes", 60u32)
                .await? as i64
                * 60,
            default_thread_count: self
                .get_system_value("recording.thread_count", 4u32)
                .await? as i32,
            max_concurrent: self.ctx.config.recorder.max_concurrent.max(1),
            min_free_space_bytes: if min_free_space_gb > 0 {
                min_free_space_gb.saturating_mul(1024 * 1024 * 1024)
            } else {
                self.ctx.config.storage.min_free_space_mb * 1024 * 1024
            },
        })
    }

    async fn get_system_value<T>(&self, key: &str, default: T) -> Result<T>
    where
        T: std::str::FromStr,
    {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT value FROM system_config WHERE key = ?")
                .bind(key)
                .fetch_optional(&self.ctx.db)
                .await?;

        if let Some((value,)) = row {
            value.parse().or(Ok(default))
        } else {
            Ok(default)
        }
    }

    async fn get_system_value_string(&self, key: &str, default: &str) -> Result<String> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT value FROM system_config WHERE key = ?")
                .bind(key)
                .fetch_optional(&self.ctx.db)
                .await?;

        Ok(row
            .map(|(value,)| value)
            .unwrap_or_else(|| default.to_string()))
    }

    fn build_post_processor(
        &self,
        req: &ManualRecordRequest,
        runtime_settings: &RuntimeRecordingSettings,
    ) -> PostProcessor {
        let mut config = self.ctx.config.recorder.post_process.clone();
        if let Some(mode) = &req.transcode_mode {
            if !mode.is_empty() {
                config.mode = mode.clone();
            }
        }
        if let Some(preset) = &req.transcode_preset {
            if !preset.is_empty() {
                config.preset = preset.clone();
            }
        }

        PostProcessor::new(config, runtime_settings.recordings_dir.clone())
    }

    fn validate_recorder_executable(
        &self,
        engine: RecordingEngine,
        executable: &Path,
    ) -> Result<()> {
        if executable.is_absolute() {
            if executable.is_file() {
                return Ok(());
            }
        } else if command_exists(executable) {
            return Ok(());
        }

        let (tool_name, config_key) = match engine {
            RecordingEngine::NM3u8dlRe => ("N_m3u8DL-RE", "recording.n_m3u8dl_re_path"),
            RecordingEngine::Ffmpeg => ("FFmpeg", "recorder.post_process.ffmpeg_path"),
        };

        Err(anyhow::anyhow!(
            "录制工具未找到: {} (当前配置: {}). 请在设置里配置 {}，或把它加入系统 PATH。",
            tool_name,
            executable.display(),
            config_key
        ))
    }

    async fn ensure_recording_capacity(
        &self,
        req: &ManualRecordRequest,
        runtime_settings: &RuntimeRecordingSettings,
    ) -> Result<()> {
        if self.count_running_tasks().await? >= runtime_settings.max_concurrent as i64 {
            return Err(anyhow::anyhow!(
                "当前运行中的录制任务已达到上限 ({})",
                runtime_settings.max_concurrent
            ));
        }

        if self.has_running_task_for_channel(&req.channel_id).await? {
            return Err(anyhow::anyhow!("该频道当前已有正在运行的录制任务"));
        }

        if let Some(schedule_id) = req.schedule_id.as_deref() {
            if self.has_running_task_for_schedule(schedule_id).await? {
                return Err(anyhow::anyhow!("该定时任务当前已有正在运行的录制任务"));
            }
        }

        self.ensure_min_free_space(runtime_settings).await
    }

    /// 并发准入临界区：在 `ADMISSION_LOCK` 保护下完成「检查 → 插入 running 记录」。
    ///
    /// 这是并发安全的核心。把这段 check-then-act 串行化，避免以下竞态
    /// （cron 触发 / HTTP 手动录制 / 多 cron 同秒触发并发进入时）：
    ///   - `max_concurrent` 被 COUNT 读取过期值而突破
    ///   - 同频道 / 同定时任务出现重复 running 记录
    ///
    /// 设计要点：
    ///   - **锁是进程级静态单例**（`ADMISSION_LOCK`），而非实例字段。因为
    ///     `RecordingService` 是每次请求现 new 的临时对象，实例字段无法跨请求共享。
    ///   - **临界区只覆盖「检查 + INSERT」**，不覆盖进程启动与监控循环；
    ///     INSERT 完成立即释放锁。不同频道仍可并发录制，吞吐不受影响。
    ///   - **DB 部分唯一索引（migration 0006）是兜底**：即便锁失效或多实例，
    ///     DB 仍会拒绝重复 running 记录，触发 UNIQUE 冲突；此处映射为业务错误。
    ///
    /// 输入约定：调用方在锁外算好 `task_id` / `now` / `output_path`，
    /// 本方法只负责锁内的纯 DB 操作，便于并发测试直接驱动。
    async fn admit_recording(
        &self,
        task_id: &str,
        now: &str,
        output_path: &Path,
        req: &ManualRecordRequest,
        runtime_settings: &RuntimeRecordingSettings,
    ) -> Result<()> {
        let admission = ADMISSION_LOCK.lock().await;

        self.validate_schedule_id(req.schedule_id.as_deref())
            .await?;
        self.ensure_recording_capacity(req, runtime_settings)
            .await?;

        let insert_result = sqlx::query(
            r#"
            INSERT INTO tasks (id, schedule_id, channel_id, status, started_at, output_path, created_at, updated_at)
            VALUES (?, ?, ?, 'running', ?, ?, ?, ?)
            "#,
        )
        .bind(task_id)
        .bind(&req.schedule_id)
        .bind(&req.channel_id)
        .bind(now)
        .bind(output_path.to_str().unwrap_or(task_id))
        .bind(now)
        .bind(now)
        .execute(&self.ctx.db)
        .await;

        // 任务记录已落库（或失败），临界区结束，释放锁。
        drop(admission);

        if let Err(e) = insert_result {
            // DB 部分唯一索引（migration 0006）兜底：把 SQLite UNIQUE 冲突翻译成
            // 友好的业务错误，避免裸 SQL 错误泄漏给前端。
            let msg = e.to_string();
            if is_unique_constraint_violation(&e) {
                if msg.contains("uniq_running_per_schedule") {
                    return Err(anyhow::anyhow!("该定时任务当前已有正在运行的录制任务"));
                }
                return Err(anyhow::anyhow!("该频道当前已有正在运行的录制任务"));
            }
            return Err(anyhow::anyhow!("创建录制任务失败: {}", msg));
        }

        Ok(())
    }

    async fn count_running_tasks(&self) -> Result<i64> {
        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM tasks WHERE status = 'running'")
                .fetch_one(&self.ctx.db)
                .await?;
        Ok(count)
    }

    async fn has_running_task_for_channel(&self, channel_id: &str) -> Result<bool> {
        let existing: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM tasks WHERE channel_id = ? AND status = 'running' LIMIT 1",
        )
        .bind(channel_id)
        .fetch_optional(&self.ctx.db)
        .await?;
        Ok(existing.is_some())
    }

    async fn has_running_task_for_schedule(&self, schedule_id: &str) -> Result<bool> {
        let existing: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM tasks WHERE schedule_id = ? AND status = 'running' LIMIT 1",
        )
        .bind(schedule_id)
        .fetch_optional(&self.ctx.db)
        .await?;
        Ok(existing.is_some())
    }

    async fn ensure_min_free_space(
        &self,
        runtime_settings: &RuntimeRecordingSettings,
    ) -> Result<()> {
        if runtime_settings.min_free_space_bytes == 0 {
            return Ok(());
        }

        // `df` 需要目标路径已存在；首次启动时录制目录可能还未创建。
        tokio::fs::create_dir_all(&runtime_settings.recordings_dir).await?;

        // 磁盘空间探测可能失败(如网络共享不可达、权限不足)。探测失败时不应阻塞录制,
        // 而是跳过空间预检并记录告警——录制本身仍可尝试进行。
        let available = match get_available_space(&runtime_settings.recordings_dir).await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    "跳过录制目录空间预检(探测失败,可能为网络路径): {} - {}",
                    runtime_settings.recordings_dir.display(),
                    e
                );
                return Ok(());
            }
        };
        if available < runtime_settings.min_free_space_bytes {
            return Err(anyhow::anyhow!(
                "录制目录剩余空间不足: 当前 {:.2} GB，要求至少 {:.2} GB",
                available as f64 / 1024_f64 / 1024_f64 / 1024_f64,
                runtime_settings.min_free_space_bytes as f64 / 1024_f64 / 1024_f64 / 1024_f64
            ));
        }

        Ok(())
    }
}

fn normalize_ffmpeg_executable(configured_path: &str) -> PathBuf {
    let configured_path = configured_path.trim();
    if configured_path.is_empty() {
        return PathBuf::from("ffmpeg");
    }

    let path = PathBuf::from(configured_path);
    if path.is_absolute() && !path.is_file() && command_exists(Path::new("ffmpeg")) {
        tracing::warn!(
            "Configured FFmpeg path does not exist in this runtime: {}. Falling back to ffmpeg from PATH.",
            path.display()
        );
        return PathBuf::from("ffmpeg");
    }

    path
}

fn normalize_recorder_executable(configured_path: &str) -> PathBuf {
    let configured_path = configured_path.trim();
    if configured_path.is_empty() {
        return PathBuf::from("N_m3u8DL-RE");
    }

    let path = PathBuf::from(configured_path);
    if path.is_absolute() && !path.is_file() && command_exists(Path::new("N_m3u8DL-RE")) {
        tracing::warn!(
            "Configured N_m3u8DL-RE path does not exist in this runtime: {}. Falling back to N_m3u8DL-RE from PATH.",
            path.display()
        );
        return PathBuf::from("N_m3u8DL-RE");
    }

    path
}

/// 查找实际的输出文件（N_m3u8DL-RE 可能输出不同扩展名和文件名）
/// 仅接受期望文件或同名前缀文件，避免误认领同目录下的其他录制文件。
async fn find_actual_output_file(expected_path: &PathBuf) -> Option<PathBuf> {
    // 首先检查期望的路径是否存在
    if tokio::fs::metadata(expected_path).await.is_ok() {
        debug!("找到期望的输出文件: {}", expected_path.display());
        return Some(expected_path.clone());
    }

    // 获取目录和文件名前缀
    let parent = expected_path.parent()?;
    let stem = expected_path.file_stem()?.to_str()?;

    debug!(
        "查找输出文件: 目录={}, 文件名前缀={}",
        parent.display(),
        stem
    );

    // 读取目录中的文件
    let mut entries = match tokio::fs::read_dir(parent).await {
        Ok(e) => e,
        Err(e) => {
            warn!("读取目录失败: {}", e);
            return None;
        }
    };

    // 收集所有匹配的文件，按修改时间排序
    let mut matching_files: Vec<(PathBuf, std::time::SystemTime, u64)> = Vec::new();
    // 支持的视频扩展名
    let video_extensions = ["ts", "mp4", "mkv", "flv", "avi", "mov"];

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();

        // 只匹配文件，排除目录
        if let Ok(metadata) = tokio::fs::metadata(&path).await {
            if metadata.is_dir() {
                continue; // 跳过目录
            }
        } else {
            continue;
        }

        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
            // 排除临时文件
            let lower_name = file_name.to_lowercase();
            if file_name.ends_with(".part")
                || file_name.ends_with(".tmp")
                || lower_name.contains("_temp")
                || lower_name.contains("_tmpl")
                || lower_name.ends_with(".json")
                || lower_name.ends_with(".log")
                || lower_name.ends_with(".m3u8")
            {
                continue;
            }

            // 检查是否是视频文件
            let is_video = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|ext| video_extensions.contains(&ext.to_lowercase().as_str()))
                .unwrap_or(false);

            if !is_video {
                continue;
            }

            // 获取文件修改时间和大小
            if let Ok(metadata) = tokio::fs::metadata(&path).await {
                if let Ok(modified) = metadata.modified() {
                    let size = metadata.len();
                    // 只考虑非空文件
                    if size > 0 {
                        if file_name.starts_with(stem) {
                            matching_files.push((path, modified, size));
                        }
                    }
                }
            }
        }
    }

    matching_files.sort_by(|a, b| b.1.cmp(&a.1));
    if let Some((path, _, size)) = matching_files.first() {
        info!(
            "找到前缀匹配的输出文件: {} (大小: {} bytes)",
            path.display(),
            size
        );
        return Some(path.clone());
    }

    warn!("未找到输出文件: 期望={}", expected_path.display());
    None
}

fn select_recording_engine(url: &str) -> RecordingEngine {
    let lower = url.to_ascii_lowercase();
    if lower.contains(".m3u8") || lower.contains(".mpd") {
        RecordingEngine::NM3u8dlRe
    } else {
        RecordingEngine::Ffmpeg
    }
}

/// 判断 sqlx 错误是否为 SQLite 的 UNIQUE 约束冲突。
///
/// 用于把 INSERT running 触发的部分唯一索引冲突（migration 0006）翻译成
/// 友好的业务错误。SQLITE_CONSTRAINT_UNIQUE 的扩展错误码为 2067，
/// SQLITE_CONSTRAINT 为 19。sqlx 的 SQLite 错误仅暴露消息字符串，
/// 故同时匹配主码（19）与扩展码（2067）并辅以消息特征，提高鲁棒性。
fn is_unique_constraint_violation(err: &sqlx::Error) -> bool {
    match err {
        sqlx::Error::Database(db_err) => {
            let code = db_err.code().map(|c| c.into_owned()).unwrap_or_default();
            let msg = db_err.message();
            code == "2067" || code == "19" || msg.contains("UNIQUE")
        }
        _ => false,
    }
}

fn command_exists(executable: &Path) -> bool {
    if executable.components().count() > 1 {
        return executable.is_file();
    }

    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };
    let candidates = command_name_candidates(executable);
    std::env::split_paths(&paths).any(|dir| candidates.iter().any(|name| dir.join(name).is_file()))
}

fn command_name_candidates(executable: &Path) -> Vec<std::ffi::OsString> {
    let name = executable.as_os_str().to_os_string();
    #[cfg(windows)]
    {
        let lower = executable.to_string_lossy().to_ascii_lowercase();
        if lower.ends_with(".exe") {
            vec![name]
        } else {
            vec![
                name,
                std::ffi::OsString::from(format!("{}.exe", executable.display())),
            ]
        }
    }
    #[cfg(not(windows))]
    {
        vec![name]
    }
}

async fn get_available_space(path: &Path) -> Result<u64> {
    #[cfg(windows)]
    {
        return get_available_space_windows(path).await;
    }

    #[cfg(not(windows))]
    {
        let output = tokio::process::Command::new("df")
            .arg("-B1")
            .arg(path)
            .output()
            .await?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "检查磁盘剩余空间失败: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let line = stdout
            .lines()
            .nth(1)
            .ok_or_else(|| anyhow::anyhow!("无法解析 df 输出"))?;
        let available = line
            .split_whitespace()
            .nth(3)
            .ok_or_else(|| anyhow::anyhow!("无法解析 df 可用空间字段"))?
            .parse::<u64>()?;

        Ok(available)
    }
}

#[cfg(windows)]
async fn get_available_space_windows(path: &Path) -> Result<u64> {
    let canonical = tokio::fs::canonicalize(path)
        .await
        .unwrap_or_else(|_| path.to_path_buf());
    let path_str = canonical.to_string_lossy().to_string();

    // 处理网络路径(UNC \\server\share,canonicalize 后形如 \\?\UNC\server\share)。
    // 这类路径没有盘符,无法用 Win32_LogicalDisk 按 DeviceID 查询。
    if path_str.starts_with("\\\\") {
        return get_unc_available_space(&path_str).await;
    }

    let drive = path_str.chars().take(2).collect::<String>();
    if !drive.ends_with(':') {
        // 既非盘符也非 UNC,无法探测——返回错误而非谎报充足空间。
        return Err(anyhow::anyhow!(
            "无法解析磁盘标识(非盘符也非 UNC 路径): {}",
            path_str
        ));
    }

    let script = format!(
        "(Get-CimInstance Win32_LogicalDisk -Filter \"DeviceID='{}'\").FreeSpace",
        drive
    );
    let output = tokio::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-Command", &script])
        .output()
        .await?;

    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "检查磁盘剩余空间失败: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .trim()
        .parse::<u64>()
        .map_err(|e| anyhow::anyhow!("无法解析 Windows 磁盘剩余空间: {}", e))
}

/// 探测 UNC 网络共享路径(\\server\share)的可用空间。
///
/// 网络共享无法用 Win32_LogicalDisk 按盘符查询。这里用 PowerShell 的
/// `Get-PSDrive` 配合 UNC 路径(PSDrive 支持 UNC 的 Used/Free)来探测。
/// 探测失败时返回错误(而非谎报无限空间),让调用方走"空间未知"的处理。
#[cfg(windows)]
async fn get_unc_available_space(unc_path: &str) -> Result<u64> {
    // 还原成标准 UNC 形式(canonicalize 给的是 \\?\UNC\server\share,PSDrive 需 \\server\share)
    let normal_unc = if let Some(rest) = unc_path.strip_prefix("\\\\?\\UNC\\") {
        format!("\\\\{}", rest)
    } else {
        unc_path.to_string()
    };

    let script = format!(
        "(Get-PSDrive -Name '{}' -ErrorAction SilentlyContinue).Free",
        normal_unc.replace('\'', "''")
    );
    let output = tokio::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-Command", &script])
        .output()
        .await?;

    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "查询网络共享可用空间失败(可能是只读/无权限): {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        // PSDrive 查不到该 UNC(未映射)——返回错误,让上层按"空间未知"处理
        return Err(anyhow::anyhow!(
            "无法获取网络共享 {} 的可用空间(可能未映射或不可达)",
            normal_unc
        ));
    }
    trimmed
        .parse::<u64>()
        .map_err(|e| anyhow::anyhow!("无法解析网络共享可用空间: {}", e))
}

/// 词法规范化路径(处理 . 和 ..,不要求路径存在)。
/// 用于 output_dir 包含校验:避免 canonicalize 因目录不存在而失败。
fn normalize_path_lexical(path: &std::path::Path) -> std::path::PathBuf {
    use std::path::Component;

    let mut out = Vec::new();
    for comp in path.components() {
        match comp {
            Component::ParentDir => {
                if out.last().map_or(false, |c| {
                    !matches!(c, Component::RootDir | Component::Prefix(_))
                }) {
                    out.pop();
                }
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out.iter().collect()
}

/// 将秒数格式化为人类可读时长，例如 `90 -> "1分30秒"`、`3725 -> "1小时2分5秒"`
fn format_duration(total_secs: i64) -> String {
    if total_secs <= 0 {
        return "0秒".to_string();
    }
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let secs = total_secs % 60;
    let mut parts = Vec::new();
    if hours > 0 {
        parts.push(format!("{}小时", hours));
    }
    if minutes > 0 {
        parts.push(format!("{}分", minutes));
    }
    if secs > 0 || parts.is_empty() {
        parts.push(format!("{}秒", secs));
    }
    parts.join("")
}

/// 任务终态通知发送（落库 + WebSocket 推送）。
///
/// 作为独立 async 函数而非闭包，避免 async 闭包 move 捕获导致的后续借用冲突。
/// 调用方按需传入 ctx / event_sender 的 clone。
#[allow(clippy::too_many_arguments)]
async fn notify_terminal_task(
    ctx: ServiceContext,
    event_sender: Option<EventSender>,
    config_key: Option<&str>,
    category: &str,
    level: &str,
    title: String,
    message: String,
    details: Option<String>,
    task_id: String,
) {
    let svc = NotificationService::new(ctx, event_sender);
    if let Err(e) = svc
        .notify(
            config_key,
            NotifyRequest {
                category: category.to_string(),
                level: level.to_string(),
                title,
                message,
                details,
                task_id: Some(task_id),
            },
        )
        .await
    {
        warn!("发送任务通知失败: {}", e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::Config,
        core::{database, process::ProcessManager},
    };
    use std::{
        path::PathBuf,
        sync::Arc,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_db_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        std::env::temp_dir().join(format!("iptv-recorder-{name}-{nanos}.db"))
    }

    #[test]
    fn selects_recorder_engine_by_stream_type() {
        assert_eq!(
            select_recording_engine("https://example.com/live/stream.m3u8"),
            RecordingEngine::NM3u8dlRe
        );
        assert_eq!(
            select_recording_engine("https://example.com/live/manifest.mpd"),
            RecordingEngine::NM3u8dlRe
        );
        assert_eq!(
            select_recording_engine("http://192.168.0.211:4022/udp/239.77.0.147:5146"),
            RecordingEngine::Ffmpeg
        );
    }

    #[tokio::test]
    async fn load_runtime_settings_prefers_system_config_values() {
        let db_path = temp_db_path("recording-settings");
        let db = database::init(db_path.to_str().expect("utf8 path"), 1)
            .await
            .expect("db init");

        sqlx::query("UPDATE system_config SET value = ? WHERE key = 'storage.recordings_path'")
            .bind("./custom-recordings")
            .execute(&db)
            .await
            .expect("update recordings path");
        sqlx::query("UPDATE system_config SET value = ? WHERE key = 'recording.n_m3u8dl_re_path'")
            .bind("/usr/local/bin/custom-recorder")
            .execute(&db)
            .await
            .expect("update recorder path");
        sqlx::query(
            "UPDATE system_config SET value = ? WHERE key = 'recording.default_duration_minutes'",
        )
        .bind("90")
        .execute(&db)
        .await
        .expect("update duration");
        sqlx::query("UPDATE system_config SET value = ? WHERE key = 'recording.thread_count'")
            .bind("8")
            .execute(&db)
            .await
            .expect("update thread count");

        let service = RecordingService::new(
            Arc::new(ProcessManager::new(
                PathBuf::from("recorder"),
                PathBuf::from("tmp"),
            )),
            ServiceContext::new(db, Config::default()),
            None,
        );

        let settings = service
            .load_runtime_settings()
            .await
            .expect("runtime settings");
        assert_eq!(
            settings.recordings_dir,
            PathBuf::from("./custom-recordings")
        );
        assert_eq!(
            settings.recorder_executable,
            PathBuf::from("/usr/local/bin/custom-recorder")
        );
        assert_eq!(settings.default_duration_seconds, 5400);
        assert_eq!(settings.default_thread_count, 8);

        let _ = tokio::fs::remove_file(db_path).await;
    }

    async fn test_service(name: &str) -> (RecordingService, PathBuf) {
        let db_path = temp_db_path(name);
        let db = database::init(db_path.to_str().expect("utf8 path"), 1)
            .await
            .expect("db init");

        sqlx::query(
            r#"
            INSERT INTO channels (id, name, url, group_name, created_at, updated_at)
            VALUES (?, ?, ?, ?, datetime('now'), datetime('now'))
            "#,
        )
        .bind("channel-1")
        .bind("测试频道")
        .bind("https://example.com/live.m3u8")
        .bind("Test")
        .execute(&db)
        .await
        .expect("insert channel");

        let service = RecordingService::new(
            Arc::new(ProcessManager::new(
                PathBuf::from("recorder"),
                PathBuf::from("tmp"),
            )),
            ServiceContext::new(db, Config::default()),
            None,
        );

        (service, db_path)
    }

    async fn insert_schedule(db: &sqlx::Pool<sqlx::Sqlite>, id: &str, channel_id: &str) {
        sqlx::query(
            r#"
            INSERT INTO schedules (
                id, name, channel_id, cron_expression, duration_seconds, output_template,
                output_dir, priority, enabled, max_retry, notify_on_complete,
                video_quality, audio_quality, max_speed, thread_count,
                transcode_mode, transcode_preset, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'), datetime('now'))
            "#,
        )
        .bind(id)
        .bind(format!("Schedule {id}"))
        .bind(channel_id)
        .bind("0 0 * * *")
        .bind(60)
        .bind("{channel_name}_{date}_{time}.mp4")
        .bind(Option::<String>::None)
        .bind(5)
        .bind(1)
        .bind(3)
        .bind(0)
        .bind("best")
        .bind("best")
        .bind(Option::<String>::None)
        .bind(4)
        .bind("off")
        .bind("medium")
        .execute(db)
        .await
        .expect("insert schedule");
    }

    #[tokio::test]
    async fn cancel_marks_running_task_cancelled_and_completion_writeback_stays_blocked() {
        let (service, db_path) = test_service("recording-cancel").await;

        sqlx::query(
            r#"
            INSERT INTO tasks (id, channel_id, status, created_at, updated_at)
            VALUES (?, ?, 'running', datetime('now'), datetime('now'))
            "#,
        )
        .bind("task-1")
        .bind("channel-1")
        .execute(&service.ctx.db)
        .await
        .expect("insert task");

        service.cancel("task-1").await.expect("cancel task");

        let task = service.get_task("task-1").await.expect("load task");
        assert_eq!(task.status, "cancelled");

        let rows = sqlx::query(
            r#"
            UPDATE tasks
            SET status = 'completed', updated_at = datetime('now')
            WHERE id = ? AND status = 'running'
            "#,
        )
        .bind("task-1")
        .execute(&service.ctx.db)
        .await
        .expect("guarded completion update")
        .rows_affected();
        assert_eq!(rows, 0);

        let task = service.get_task("task-1").await.expect("reload task");
        assert_eq!(task.status, "cancelled");

        let _ = tokio::fs::remove_file(db_path).await;
    }

    #[tokio::test]
    async fn start_manual_with_invalid_absolute_recorder_path_does_not_create_running_task() {
        let (service, db_path) = test_service("recording-invalid-recorder").await;

        sqlx::query("UPDATE system_config SET value = ? WHERE key = 'recording.n_m3u8dl_re_path'")
            .bind("/definitely/missing/N_m3u8DL-RE")
            .execute(&service.ctx.db)
            .await
            .expect("update recorder path");

        let err = service
            .start_manual(ManualRecordRequest {
                channel_id: "channel-1".to_string(),
                schedule_id: None,
                duration_seconds: Some(60),
                output_name: Some("invalid-recorder".to_string()),
                output_dir: None,
                output_template: None,
                video_quality: "best".to_string(),
                audio_quality: "best".to_string(),
                max_speed: None,
                thread_count: Some(1),
                transcode_mode: Some("off".to_string()),
                transcode_preset: Some("medium".to_string()),
            })
            .await
            .expect_err("invalid recorder path should fail");

        assert!(err.to_string().contains("录制工具未找到"));

        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM tasks WHERE status = 'running'")
                .fetch_one(&service.ctx.db)
                .await
                .expect("count running tasks");
        assert_eq!(count, 0);

        let _ = tokio::fs::remove_file(db_path).await;
    }

    #[tokio::test]
    async fn build_output_path_sanitizes_custom_name() {
        let (service, db_path) = test_service("recording-output-name").await;
        let channel = service
            .get_channel("channel-1")
            .await
            .expect("load channel");

        let path = service
            .build_output_path(
                &channel,
                "task-1",
                &Some("../unsafe/subdir/../../video.ts".to_string()),
                &None,
                None,
                None,
                Path::new("./data/recordings"),
            )
            .await
            .expect("build output path");

        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some("video.ts")
        );

        let _ = tokio::fs::remove_file(db_path).await;
    }

    #[tokio::test]
    async fn build_output_path_adds_default_ts_extension() {
        let (service, db_path) = test_service("recording-output-extension").await;
        let channel = service
            .get_channel("channel-1")
            .await
            .expect("load channel");

        let path = service
            .build_output_path(
                &channel,
                "task-1",
                &None,
                &None,
                None,
                None,
                Path::new("./data/recordings"),
            )
            .await
            .expect("build output path");

        assert_eq!(path.extension().and_then(|ext| ext.to_str()), Some("ts"));

        let _ = tokio::fs::remove_file(db_path).await;
    }

    #[tokio::test]
    async fn find_actual_output_file_does_not_pick_unrelated_latest_file() {
        let temp_dir =
            std::env::temp_dir().join(format!("iptv-recorder-find-output-{}", Uuid::new_v4()));
        tokio::fs::create_dir_all(&temp_dir)
            .await
            .expect("create temp dir");

        let expected = temp_dir.join("expected_name");
        let unrelated = temp_dir.join("someone_else.ts");
        tokio::fs::write(&unrelated, b"video")
            .await
            .expect("write unrelated file");

        let found = find_actual_output_file(&expected).await;
        assert!(found.is_none());

        let _ = tokio::fs::remove_file(&unrelated).await;
        let _ = tokio::fs::remove_dir(&temp_dir).await;
    }

    #[tokio::test]
    async fn start_manual_persists_schedule_id() {
        let (service, db_path) = test_service("recording-schedule-id").await;
        insert_schedule(&service.ctx.db, "schedule-42", "channel-1").await;
        let recorder_path = std::env::current_exe().expect("current exe path");

        sqlx::query("UPDATE system_config SET value = ? WHERE key = 'recording.n_m3u8dl_re_path'")
            .bind(recorder_path.to_string_lossy().to_string())
            .execute(&service.ctx.db)
            .await
            .expect("update recorder path");
        sqlx::query("UPDATE system_config SET value = ? WHERE key = 'storage.min_free_space_gb'")
            .bind("0")
            .execute(&service.ctx.db)
            .await
            .expect("disable disk guard");

        let task = service
            .start_manual(ManualRecordRequest {
                channel_id: "channel-1".to_string(),
                schedule_id: Some("schedule-42".to_string()),
                duration_seconds: Some(1),
                output_name: Some("scheduled-task".to_string()),
                output_dir: None,
                output_template: None,
                video_quality: "best".to_string(),
                audio_quality: "best".to_string(),
                max_speed: None,
                thread_count: Some(1),
                transcode_mode: Some("off".to_string()),
                transcode_preset: Some("medium".to_string()),
            })
            .await
            .expect("start manual");

        assert_eq!(task.schedule_id.as_deref(), Some("schedule-42"));

        service.cancel(&task.id).await.expect("cancel task");

        let _ = tokio::fs::remove_file(db_path).await;
    }

    #[tokio::test]
    async fn ensure_recording_capacity_rejects_same_channel_and_schedule() {
        let (service, db_path) = test_service("recording-capacity-channel").await;
        insert_schedule(&service.ctx.db, "schedule-dup", "channel-1").await;

        sqlx::query(
            r#"
            INSERT INTO tasks (id, schedule_id, channel_id, status, created_at, updated_at)
            VALUES (?, ?, ?, 'running', datetime('now'), datetime('now'))
            "#,
        )
        .bind("task-running")
        .bind("schedule-dup")
        .bind("channel-1")
        .execute(&service.ctx.db)
        .await
        .expect("insert running task");

        let req = ManualRecordRequest {
            channel_id: "channel-1".to_string(),
            schedule_id: Some("schedule-dup".to_string()),
            duration_seconds: Some(60),
            output_name: None,
            output_dir: None,
            output_template: None,
            video_quality: "best".to_string(),
            audio_quality: "best".to_string(),
            max_speed: None,
            thread_count: Some(1),
            transcode_mode: Some("off".to_string()),
            transcode_preset: Some("medium".to_string()),
        };

        let err = service
            .ensure_recording_capacity(
                &req,
                &RuntimeRecordingSettings {
                    recorder_executable: PathBuf::from("recorder"),
                    ffmpeg_executable: PathBuf::from("ffmpeg"),
                    recordings_dir: std::env::temp_dir(),
                    default_duration_seconds: 60,
                    default_thread_count: 1,
                    max_concurrent: 10,
                    min_free_space_bytes: 0,
                },
            )
            .await
            .expect_err("same channel should be rejected");
        assert!(err.to_string().contains("该频道当前已有正在运行的录制任务"));

        let _ = tokio::fs::remove_file(db_path).await;
    }

    #[tokio::test]
    async fn ensure_recording_capacity_rejects_global_concurrency_limit() {
        let (service, db_path) = test_service("recording-capacity-limit").await;

        sqlx::query(
            r#"
            INSERT INTO channels (id, name, url, group_name, created_at, updated_at)
            VALUES (?, ?, ?, ?, datetime('now'), datetime('now'))
            "#,
        )
        .bind("channel-2")
        .bind("测试频道2")
        .bind("https://example.com/live2.m3u8")
        .bind("Test")
        .execute(&service.ctx.db)
        .await
        .expect("insert channel 2");

        sqlx::query(
            r#"
            INSERT INTO tasks (id, channel_id, status, created_at, updated_at)
            VALUES (?, ?, 'running', datetime('now'), datetime('now'))
            "#,
        )
        .bind("task-running")
        .bind("channel-1")
        .execute(&service.ctx.db)
        .await
        .expect("insert running task");

        let err = service
            .ensure_recording_capacity(
                &ManualRecordRequest {
                    channel_id: "channel-2".to_string(),
                    schedule_id: None,
                    duration_seconds: Some(60),
                    output_name: None,
                    output_dir: None,
                    output_template: None,
                    video_quality: "best".to_string(),
                    audio_quality: "best".to_string(),
                    max_speed: None,
                    thread_count: Some(1),
                    transcode_mode: Some("off".to_string()),
                    transcode_preset: Some("medium".to_string()),
                },
                &RuntimeRecordingSettings {
                    recorder_executable: PathBuf::from("recorder"),
                    ffmpeg_executable: PathBuf::from("ffmpeg"),
                    recordings_dir: std::env::temp_dir(),
                    default_duration_seconds: 60,
                    default_thread_count: 1,
                    max_concurrent: 1,
                    min_free_space_bytes: 0,
                },
            )
            .await
            .expect_err("global concurrency limit should be enforced");
        assert!(err.to_string().contains("已达到上限"));

        let _ = tokio::fs::remove_file(db_path).await;
    }

    /// migration 0006 兜底：同一频道同时只能有一条 running 记录。
    ///
    /// 即便进程内 ADMISSION_LOCK 失效（或未来多实例），DB 部分唯一索引
    /// 仍会拒绝第二条 running 插入。此测试验证索引确实生效。
    #[tokio::test]
    async fn unique_index_rejects_duplicate_running_task_for_same_channel() {
        let (service, db_path) = test_service("recording-unique-channel").await;

        // 第一条 running 应成功
        sqlx::query(
            r#"
            INSERT INTO tasks (id, channel_id, status, created_at, updated_at)
            VALUES ('task-a', 'channel-1', 'running', datetime('now'), datetime('now'))
            "#,
        )
        .execute(&service.ctx.db)
        .await
        .expect("first running task should succeed");

        // 同频道第二条 running 应被 UNIQUE 索引拒绝
        let err = sqlx::query(
            r#"
            INSERT INTO tasks (id, channel_id, status, created_at, updated_at)
            VALUES ('task-b', 'channel-1', 'running', datetime('now'), datetime('now'))
            "#,
        )
        .execute(&service.ctx.db)
        .await
        .expect_err("duplicate running task should be rejected");

        assert!(
            is_unique_constraint_violation(&err),
            "expected UNIQUE constraint violation, got: {:?}",
            err
        );

        let _ = tokio::fs::remove_file(db_path).await;
    }

    /// 不同频道的 running 记录互不冲突，可并发录制。
    #[tokio::test]
    async fn unique_index_allows_concurrent_running_across_different_channels() {
        let (service, db_path) = test_service("recording-unique-distinct").await;

        // 插入第二个频道
        sqlx::query(
            r#"
            INSERT INTO channels (id, name, url, group_name, created_at, updated_at)
            VALUES ('channel-2', '测试频道2', 'https://example.com/live2.m3u8', 'Test',
                    datetime('now'), datetime('now'))
            "#,
        )
        .execute(&service.ctx.db)
        .await
        .expect("insert channel 2");

        // 两个频道各一条 running，均应成功
        for (task_id, channel_id) in [("task-a", "channel-1"), ("task-b", "channel-2")] {
            sqlx::query(
                r#"
                INSERT INTO tasks (id, channel_id, status, created_at, updated_at)
                VALUES (?, ?, 'running', datetime('now'), datetime('now'))
                "#,
            )
            .bind(task_id)
            .bind(channel_id)
            .execute(&service.ctx.db)
            .await
            .expect("distinct channel running should succeed");
        }

        let _ = tokio::fs::remove_file(db_path).await;
    }

    /// 任务进入终态（cancelled/completed/failed）后，部分唯一索引自动释放，
    /// 该频道可立即开始下一次录制。与 cancel() 的「WHERE status='running'
    /// 原子切到 cancelled」语义一致。
    #[tokio::test]
    async fn unique_index_releases_after_task_leaves_running_state() {
        let (service, db_path) = test_service("recording-unique-release").await;

        // 第一条 running
        sqlx::query(
            r#"
            INSERT INTO tasks (id, channel_id, status, created_at, updated_at)
            VALUES ('task-a', 'channel-1', 'running', datetime('now'), datetime('now'))
            "#,
        )
        .execute(&service.ctx.db)
        .await
        .expect("first running");

        // 切到 cancelled（模拟 cancel() 的原子状态切换）
        sqlx::query("UPDATE tasks SET status = 'cancelled' WHERE id = 'task-a'")
            .execute(&service.ctx.db)
            .await
            .expect("cancel");

        // 同频道应能再次插入 running —— 索引已释放
        sqlx::query(
            r#"
            INSERT INTO tasks (id, channel_id, status, created_at, updated_at)
            VALUES ('task-b', 'channel-1', 'running', datetime('now'), datetime('now'))
            "#,
        )
        .execute(&service.ctx.db)
        .await
        .expect("should be able to start a new running task after cancel");

        let _ = tokio::fs::remove_file(db_path).await;
    }

    /// 同一定时任务（schedule_id）同时只能有一条 running。
    #[tokio::test]
    async fn unique_index_rejects_duplicate_running_task_for_same_schedule() {
        let (service, db_path) = test_service("recording-unique-schedule").await;
        insert_schedule(&service.ctx.db, "schedule-dup", "channel-1").await;

        sqlx::query(
            r#"
            INSERT INTO tasks (id, schedule_id, channel_id, status, created_at, updated_at)
            VALUES ('task-a', 'schedule-dup', 'channel-1', 'running',
                    datetime('now'), datetime('now'))
            "#,
        )
        .execute(&service.ctx.db)
        .await
        .expect("first scheduled running");

        // 同 schedule 第二条 running 应被 uniq_running_per_schedule 拒绝
        let err = sqlx::query(
            r#"
            INSERT INTO tasks (id, schedule_id, channel_id, status, created_at, updated_at)
            VALUES ('task-b', 'schedule-dup', 'channel-1', 'running',
                    datetime('now'), datetime('now'))
            "#,
        )
        .execute(&service.ctx.db)
        .await
        .expect_err("duplicate scheduled running should be rejected");

        assert!(is_unique_constraint_violation(&err));

        let _ = tokio::fs::remove_file(db_path).await;
    }

    /// 锁串行化核心测试：并发提交 N 个不同频道的 admit_recording，
    /// 验证最终 running 数恰好等于 max_concurrent，绝不被突破。
    ///
    /// 这验证了索引单独防不住的语义——`max_concurrent` 是数量上限，无法用
    /// UNIQUE 索引表达，只能靠 ADMISSION_LOCK 串行化「COUNT 检查 → INSERT」。
    /// 若锁失效，并发下 COUNT 会读到过期值，导致远超 max_concurrent 的插入。
    ///
    /// 注意：ADMISSION_LOCK 是进程级单例。测试内部用 JoinSet 制造真实并发，
    /// 每个任务进入锁后短暂 await（给其他任务争锁的机会），最大化暴露竞态。
    #[tokio::test]
    async fn admission_lock_caps_concurrent_admissions_at_max_concurrent() {
        let (service, db_path) = test_service("recording-admission-concurrency").await;

        // 准备 20 个不同频道（编号从 101 起，避开 test_service 预插的 channel-1）
        const TOTAL: usize = 20;
        const MAX_CONCURRENT: usize = 5;
        const CH_BASE: usize = 101;
        let settings = RuntimeRecordingSettings {
            recorder_executable: PathBuf::from("recorder"),
            ffmpeg_executable: PathBuf::from("ffmpeg"),
            recordings_dir: std::env::temp_dir(),
            default_duration_seconds: 60,
            default_thread_count: 1,
            max_concurrent: MAX_CONCURRENT,
            min_free_space_bytes: 0,
        };

        for i in 0..TOTAL {
            let cid = format!("channel-{}", CH_BASE + i);
            sqlx::query(
                r#"
                INSERT INTO channels (id, name, url, group_name, created_at, updated_at)
                VALUES (?, ?, ?, 'Test', datetime('now'), datetime('now'))
                "#,
            )
            .bind(&cid)
            .bind(format!("频道{cid}"))
            .bind(format!("https://example.com/live{cid}.m3u8"))
            .execute(&service.ctx.db)
            .await
            .expect("insert channel");
        }

        // 并发提交 20 个不同频道的 admit_recording
        let mut join_set = tokio::task::JoinSet::new();
        for i in 0..TOTAL {
            let cid = format!("channel-{}", CH_BASE + i);
            let task_id = format!("task-{cid}");
            let service = service.clone();
            let settings = settings.clone();
            join_set.spawn(async move {
                let req = ManualRecordRequest {
                    channel_id: cid.clone(),
                    schedule_id: None,
                    duration_seconds: Some(60),
                    output_name: None,
                    output_dir: None,
                    output_template: None,
                    video_quality: "best".to_string(),
                    audio_quality: "best".to_string(),
                    max_speed: None,
                    thread_count: Some(1),
                    transcode_mode: Some("off".to_string()),
                    transcode_preset: Some("medium".to_string()),
                };
                let now = chrono::Utc::now().to_rfc3339();
                let output_path = std::env::temp_dir().join(&task_id);
                let res = service
                    .admit_recording(&task_id, &now, &output_path, &req, &settings)
                    .await;
                (task_id, res)
            });
        }

        let mut ok_count = 0;
        let mut err_count = 0;
        let mut over_limit_err_count = 0;
        while let Some(res) = join_set.join_next().await {
            let (_task_id, result) = res.expect("task panicked");
            match result {
                Ok(()) => ok_count += 1,
                Err(e) => {
                    err_count += 1;
                    if e.to_string().contains("已达到上限") {
                        over_limit_err_count += 1;
                    }
                }
            }
        }

        // 核心断言：成功的恰好是 max_concurrent 个，其余都被并发上限拦下
        assert_eq!(
            ok_count, MAX_CONCURRENT,
            "成功准入数应等于 max_concurrent，实际 {ok_count}"
        );
        assert_eq!(
            err_count,
            TOTAL - MAX_CONCURRENT,
            "失败数应等于 TOTAL - max_concurrent，实际 {err_count}"
        );
        assert_eq!(over_limit_err_count, TOTAL - MAX_CONCURRENT);

        // DB 层复核：running 记录数精确等于 max_concurrent，绝未突破
        let (running,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM tasks WHERE status = 'running'")
                .fetch_one(&service.ctx.db)
                .await
                .expect("count running");
        assert_eq!(
            running as usize, MAX_CONCURRENT,
            "DB 中 running 数应等于 max_concurrent，实际 {running}"
        );

        let _ = tokio::fs::remove_file(db_path).await;
    }

    #[test]
    fn sanitize_filename_preserves_unicode_letters() {
        let sanitized = RecordingService::sanitize_filename_part("央视新闻 / 直播");
        assert_eq!(sanitized, "央视新闻___直播");
    }

    /// 辅助：构造一个最小可用的 Channel 用于模板渲染测试。
    fn sample_channel(name: &str, url: &str, group: &str, source_visibility: &str) -> Channel {
        Channel {
            id: "test-id".to_string(),
            name: name.to_string(),
            url: url.to_string(),
            group_name: group.to_string(),
            logo_url: None,
            source_type: String::new(),
            source_url: None,
            status: String::new(),
            last_check_at: None,
            fail_count: 0,
            metadata: serde_json::json!({}),
            source_visibility: source_visibility.to_string(),
            playback_strategy: "auto".to_string(),
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn render_output_template_substitutes_legacy_variables() {
        let ch = sample_channel("CCTV-1", "http://example.com/live.m3u8", "央视", "public");
        // 旧变量不应回归；channel_name 含空格会被清洗为 _
        let out = RecordingService::render_output_template("{channel_name}_{date}_{time}", &ch);
        // 形如 CCTV-1_YYYYMMDD_HHMMSS：channel_name 后紧跟日期、再紧跟时间，至少两个分隔下划线
        assert!(
            out.starts_with("CCTV-1_") && out.matches('_').count() >= 2,
            "channel_name/date/time 应被替换: {out}"
        );
    }

    #[test]
    fn render_output_template_substitutes_source_group_and_url() {
        let ch = sample_channel(
            "东方卫视4K",
            "http://192.168.0.211:4022/udp/239.77.0.5",
            "4K频道",
            "private_server_only",
        );
        let out = RecordingService::render_output_template(
            "{channel_name}_{source}_{group}_{source_url}",
            &ch,
        );
        // source: 私有源 -> private
        assert!(out.contains("_private_"), "source 应为 private: {out}");
        // group: 分组名应被替换
        assert!(out.contains("4K频道"), "group 应被替换: {out}");
        // source_url: URL 中的 :// / 等非法字符应被清洗为 _
        assert!(
            !out.contains("://"),
            "source_url 中的协议分隔符应被清洗: {out}"
        );
        assert!(
            out.contains("192.168.0.211"),
            "source_url 应保留主机地址: {out}"
        );
    }

    #[test]
    fn render_output_template_source_is_public_for_public_channel() {
        let ch = sample_channel("公网台", "http://example.com/live.m3u8", "G", "public");
        let out = RecordingService::render_output_template("{source}", &ch);
        assert_eq!(out, "public");
    }
}
