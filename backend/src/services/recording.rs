//! 录制服务

use crate::{
    core::event::{
        Event, EventSender, TaskProgressEvent, TaskStatus as EventTaskStatus, TaskUpdateEvent,
    },
    core::process::{ProcessManager, RecordingConfig},
    models::{Channel, ManualRecordRequest, Task},
    services::{PostProcessor, ServiceContext},
};
use anyhow::Result;
use chrono::Utc;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// 录制服务
pub struct RecordingService {
    process_manager: Arc<ProcessManager>,
    ctx: ServiceContext,
    event_sender: Option<EventSender>,
}

struct RuntimeRecordingSettings {
    recorder_executable: PathBuf,
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
        self.validate_recorder_executable(&runtime_settings.recorder_executable)?;
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
        self.validate_schedule_id(req.schedule_id.as_deref()).await?;
        self.ensure_recording_capacity(&req, &runtime_settings).await?;

        // 构建输出文件路径（支持自定义目录和模板）
        let output_path = self
            .build_output_path(
                &channel,
                &task_id,
                &req.output_name,
                &req.output_dir,
                req.output_template.as_deref(),
                &runtime_settings.recordings_dir,
            )
            .await?;

        // 创建任务记录
        sqlx::query(
            r#"
            INSERT INTO tasks (id, schedule_id, channel_id, status, started_at, output_path, created_at, updated_at)
            VALUES (?, ?, ?, 'running', ?, ?, ?, ?)
            "#,
        )
        .bind(&task_id)
        .bind(&req.schedule_id)
        .bind(&req.channel_id)
        .bind(&now)
        .bind(output_path.to_str().unwrap_or(&task_id))
        .bind(&now)
        .bind(&now)
        .execute(&self.ctx.db)
        .await?;

        // 构建录制配置
        let config = RecordingConfig {
            recorder_executable: Some(runtime_settings.recorder_executable.clone()),
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
        default_recordings_dir: &std::path::Path,
    ) -> Result<PathBuf> {
        // 确定输出目录：优先使用自定义目录，否则使用系统默认
        let output_dir: String = if let Some(dir) = custom_dir {
            if !dir.is_empty() {
                info!("使用自定义输出目录: {}", dir);
                dir.clone()
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
                // 使用模板替换变量
                let now = Utc::now();
                let channel_name = Self::sanitize_filename_part(&channel.name);

                let filename = template
                    .replace("{channel_name}", &channel_name)
                    .replace("{date}", &now.format("%Y%m%d").to_string())
                    .replace("{time}", &now.format("%H%M%S").to_string())
                    .replace("{datetime}", &now.format("%Y%m%d_%H%M%S").to_string());

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

        info!("最终输出路径: {}/{}", output_dir, filename);
        Ok(PathBuf::from(&output_dir).join(filename))
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

    async fn load_runtime_settings(&self) -> Result<RuntimeRecordingSettings> {
        let recorder_executable = self
            .get_system_value_string(
                "recording.n_m3u8dl_re_path",
                &self.ctx.config.recorder.executable.to_string_lossy(),
            )
            .await?;
        let recordings_dir = self
            .get_system_value_string(
                "storage.recordings_path",
                &self.ctx.config.storage.recordings_dir.to_string_lossy(),
            )
            .await?;
        let min_free_space_gb = self.get_system_value("storage.min_free_space_gb", 0u64).await?;

        Ok(RuntimeRecordingSettings {
            recorder_executable: PathBuf::from(recorder_executable),
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

    fn validate_recorder_executable(&self, executable: &Path) -> Result<()> {
        if executable.is_absolute() && !executable.exists() {
            return Err(anyhow::anyhow!(
                "录制工具未找到: {}，请在设置中配置正确路径或将其加入系统 PATH",
                executable.display()
            ));
        }

        Ok(())
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

    async fn count_running_tasks(&self) -> Result<i64> {
        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM tasks WHERE status = 'running'")
                .fetch_one(&self.ctx.db)
                .await?;
        Ok(count)
    }

    async fn has_running_task_for_channel(&self, channel_id: &str) -> Result<bool> {
        let existing: Option<(String,)> =
            sqlx::query_as("SELECT id FROM tasks WHERE channel_id = ? AND status = 'running' LIMIT 1")
                .bind(channel_id)
                .fetch_optional(&self.ctx.db)
                .await?;
        Ok(existing.is_some())
    }

    async fn has_running_task_for_schedule(&self, schedule_id: &str) -> Result<bool> {
        let existing: Option<(String,)> =
            sqlx::query_as("SELECT id FROM tasks WHERE schedule_id = ? AND status = 'running' LIMIT 1")
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

        let available = get_available_space(&runtime_settings.recordings_dir).await?;
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

async fn get_available_space(path: &Path) -> Result<u64> {
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

        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tasks WHERE status = 'running'")
            .fetch_one(&service.ctx.db)
            .await
            .expect("count running tasks");
        assert_eq!(count, 0);

        let _ = tokio::fs::remove_file(db_path).await;
    }

    #[tokio::test]
    async fn build_output_path_sanitizes_custom_name() {
        let (service, db_path) = test_service("recording-output-name").await;
        let channel = service.get_channel("channel-1").await.expect("load channel");

        let path = service
            .build_output_path(
                &channel,
                "task-1",
                &Some("../unsafe/subdir/../../video.ts".to_string()),
                &None,
                None,
                Path::new("./data/recordings"),
            )
            .await
            .expect("build output path");

        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some("video")
        );

        let _ = tokio::fs::remove_file(db_path).await;
    }

    #[tokio::test]
    async fn find_actual_output_file_does_not_pick_unrelated_latest_file() {
        let temp_dir = std::env::temp_dir().join(format!(
            "iptv-recorder-find-output-{}",
            Uuid::new_v4()
        ));
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

        sqlx::query("UPDATE system_config SET value = ? WHERE key = 'recording.n_m3u8dl_re_path'")
            .bind("/bin/sh")
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
                output_dir: Some(std::env::temp_dir().to_string_lossy().to_string()),
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

    #[test]
    fn sanitize_filename_preserves_unicode_letters() {
        let sanitized = RecordingService::sanitize_filename_part("央视新闻 / 直播");
        assert_eq!(sanitized, "央视新闻___直播");
    }
}
