//! 录制服务

use crate::{
    core::process::{ProcessManager, RecordingConfig},
    core::event::{EventSender, Event, TaskProgressEvent, TaskUpdateEvent, TaskStatus as EventTaskStatus},
    models::{ManualRecordRequest, Task, Channel},
    services::{ServiceContext, PostProcessor},
};
use anyhow::Result;
use chrono::Utc;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// 录制服务
pub struct RecordingService {
    process_manager: Arc<ProcessManager>,
    ctx: ServiceContext,
    post_processor: PostProcessor,
    event_sender: Option<EventSender>,
}

impl RecordingService {
    pub fn new(process_manager: Arc<ProcessManager>, ctx: ServiceContext, event_sender: Option<EventSender>) -> Self {
        let recordings_dir = ctx.config.storage.recordings_dir.clone();
        let post_processor = PostProcessor::new(
            ctx.config.recorder.post_process.clone(),
            recordings_dir,
        );
        Self {
            process_manager,
            ctx,
            post_processor,
            event_sender,
        }
    }

    /// 手动启动录制
    pub async fn start_manual(&self, req: ManualRecordRequest) -> Result<Task> {
        let task_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        // 获取频道信息
        let channel = self.get_channel(&req.channel_id).await?;

        // 构建输出文件路径（支持自定义目录和模板）
        let output_path = self.build_output_path(
            &channel,
            &task_id,
            &req.output_name,
            &req.output_dir,
            req.output_template.as_deref(),
        ).await?;

        // 创建任务记录
        sqlx::query(
            r#"
            INSERT INTO tasks (id, channel_id, status, started_at, output_path, created_at, updated_at)
            VALUES (?, ?, 'running', ?, ?, ?, ?)
            "#,
        )
        .bind(&task_id)
        .bind(&req.channel_id)
        .bind(&now)
        .bind(output_path.to_str().unwrap_or(&task_id))
        .bind(&now)
        .bind(&now)
        .execute(&self.ctx.db)
        .await?;

        // 校验录制工具路径（仅当绝对路径时检查文件是否存在）
        let exe = &self.ctx.config.recorder.executable;
        if std::path::Path::new(exe).is_absolute() && !std::path::Path::new(exe).exists() {
            return Err(anyhow::anyhow!(
                "录制工具未找到: {}，请在设置中配置正确路径或将其加入系统 PATH",
                exe
            ));
        }

        // 构建录制配置
        let config = RecordingConfig {
            url: channel.url.clone(),
            output_path: output_path.clone(),
            duration_seconds: Some(req.duration_seconds as u64),
            headers: vec![],
            user_agent: Some("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36".to_string()),
            proxy: None,
            threads: Some(req.thread_count as usize),
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
        let post_processor = self.post_processor.clone();
        let duration_seconds = req.duration_seconds as i64;
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
                            crate::core::process::ProcessStatus::Completed { exit_code } => {
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
                                    info!("  - {} ({} bytes)", entry.file_name().to_string_lossy(), meta.len());
                                }
                            }
                        }
                    }
                    if let Some(stem) = output_path_clone.file_stem() {
                        info!("期望文件名前缀: {}", stem.to_string_lossy());
                    }

                    // 查找实际的输出文件（N_m3u8DL-RE 可能输出不同扩展名和文件名）
                    let actual_output_path = match find_actual_output_file(&output_path_clone).await {
                        Some(path) => {
                            info!("✅ 找到实际输出文件: {}", path.display());
                            path
                        }
                        None => {
                            warn!("❌ 未找到输出文件，使用预期路径: {}", output_path_clone.display());
                            output_path_clone.clone()
                        }
                    };

                    // 检查文件名是否符合预期，如果不符合则重命名
                    let final_output_path = if actual_output_path != output_path_clone {
                        // 获取实际文件的扩展名
                        let actual_ext = actual_output_path.extension()
                            .and_then(|e| e.to_str())
                            .unwrap_or("ts");

                        // 构建预期的最终路径（使用实际扩展名）
                        let expected_stem = output_path_clone.file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("");
                        let expected_dir = output_path_clone.parent().unwrap_or_else(|| std::path::Path::new("."));

                        if !expected_stem.is_empty() {
                            let new_path = expected_dir.join(format!("{}.{}", expected_stem, actual_ext));

                            // 重命名文件
                            match tokio::fs::rename(&actual_output_path, &new_path).await {
                                Ok(_) => {
                                    info!("文件已重命名: {} -> {}", actual_output_path.display(), new_path.display());
                                    new_path
                                }
                                Err(e) => {
                                    warn!("重命名文件失败: {}, 使用原始路径: {}", e, actual_output_path.display());
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
                        match post_processor.process(&final_output_path, &task_id_clone).await {
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

                    sqlx::query(
                        r#"
                        UPDATE tasks
                        SET status = ?, ended_at = ?, exit_code = ?, file_size = ?,
                            duration_recorded = ?, progress_percent = 100, output_path = ?, updated_at = ?
                        WHERE id = ?
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
                    .await
                    .ok();

                    info!("✅ 录制任务完成: task_id={}, output={}, duration={}s, size={}",
                        task_id_clone, final_path_str, final_elapsed, file_size);

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
                        Some(actual_path) => {
                            tokio::fs::metadata(&actual_path)
                                .await
                                .map(|m| m.len() as i64)
                                .unwrap_or(0)
                        }
                        None => 0,
                    };

                    sqlx::query(
                        r#"
                        UPDATE tasks
                        SET status = ?, ended_at = ?, error_message = ?, file_size = ?,
                            duration_recorded = ?, updated_at = ?
                        WHERE id = ?
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
                    .await
                    .ok();

                    error!("❌ 录制任务失败: task_id={}, error={}", task_id_clone, error);

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
                        Some(actual_path) => {
                            tokio::fs::metadata(&actual_path)
                                .await
                                .map(|m| m.len() as i64)
                                .unwrap_or(0)
                        }
                        None => 0,
                    };

                    sqlx::query(
                        r#"
                        UPDATE tasks
                        SET status = ?, ended_at = ?, file_size = ?, duration_recorded = ?, updated_at = ?
                        WHERE id = ?
                        "#,
                    )
                    .bind("cancelled")
                    .bind(&now)
                    .bind(file_size)
                    .bind(final_elapsed)
                    .bind(&now)
                    .bind(&task_id_clone)
                    .execute(&db)
                    .await
                    .ok();

                    info!("🚫 录制任务已取消: task_id={}, duration={}s", task_id_clone, final_elapsed);

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
                    warn!("⚠️ 录制任务异常结束: task_id={}, status={:?}", task_id_clone, status);
                }
            }
        });

        self.get_task(&task_id).await
    }

    /// 获取任务
    pub async fn get_task(&self, id: &str) -> Result<Task> {
        let task = sqlx::query_as::<_, Task>(
            "SELECT * FROM tasks WHERE id = ?"
        )
        .bind(id)
        .fetch_one(&self.ctx.db)
        .await?;

        Ok(task)
    }

    /// 获取所有任务
    pub async fn list_tasks(&self) -> Result<Vec<Task>> {
        let tasks = sqlx::query_as::<_, Task>(
            "SELECT * FROM tasks ORDER BY created_at DESC"
        )
        .fetch_all(&self.ctx.db)
        .await?;

        Ok(tasks)
    }

    /// 获取运行中的任务
    #[allow(dead_code)]
    pub async fn list_running(&self) -> Result<Vec<Task>> {
        let tasks = sqlx::query_as::<_, Task>(
            "SELECT * FROM tasks WHERE status = 'running'"
        )
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

        // 尝试停止进程（可能不存在，比如僵尸任务）
        match self.process_manager.stop_by_task_id(id).await {
            Ok(_) => {
                info!("进程已停止: task_id={}", id);
            }
            Err(e) => {
                warn!("停止进程失败（可能已结束）: task_id={}, error={}", id, e);
                // 继续执行，仍然更新数据库状态
            }
        }

        // 更新数据库状态为 cancelled
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
        let channel = sqlx::query_as::<_, Channel>(
            "SELECT * FROM channels WHERE id = ?"
        )
        .bind(id)
        .fetch_one(&self.ctx.db)
        .await?;

        Ok(channel)
    }

    /// 构建输出文件路径
    async fn build_output_path(
        &self,
        channel: &Channel,
        _task_id: &str,
        custom_name: &Option<String>,
        custom_dir: &Option<String>,
        output_template: Option<&str>,
    ) -> Result<PathBuf> {
        // 确定输出目录：优先使用自定义目录，否则使用系统默认
        let output_dir: String = if let Some(dir) = custom_dir {
            if !dir.is_empty() {
                info!("使用自定义输出目录: {}", dir);
                dir.clone()
            } else {
                self.ctx.config.storage.recordings_dir.to_string_lossy().to_string()
            }
        } else {
            self.ctx.config.storage.recordings_dir.to_string_lossy().to_string()
        };

        // 确保输出目录存在
        tokio::fs::create_dir_all(&output_dir).await?;

        // 生成文件名（不带后缀，N_m3u8DL-RE 会自动添加）
        // 优先级：output_template > custom_name > 默认模板
        let filename = if let Some(template) = output_template {
            if !template.is_empty() {
                // 使用模板替换变量
                let now = Utc::now();
                let channel_name = channel.name.replace(|c: char| !c.is_alphanumeric() && c != '_' && c != '-', "_");

                let filename = template
                    .replace("{channel_name}", &channel_name)
                    .replace("{date}", &now.format("%Y%m%d").to_string())
                    .replace("{time}", &now.format("%H%M%S").to_string())
                    .replace("{datetime}", &now.format("%Y%m%d_%H%M%S").to_string());

                // 去掉可能的后缀
                let filename = filename.trim();
                let filename = filename.strip_suffix(".mp4")
                    .or_else(|| filename.strip_suffix(".ts"))
                    .or_else(|| filename.strip_suffix(".mkv"))
                    .unwrap_or(filename);
                info!("使用模板生成文件名: {} -> {}", template, filename);
                filename.to_string()
            } else {
                // 模板为空，使用默认
                Self::generate_default_filename(channel)
            }
        } else if let Some(name) = custom_name {
            // 如果指定了 output_name，直接使用（去掉可能的后缀）
            let name = name.trim();
            let name = name.strip_suffix(".mp4")
                .or_else(|| name.strip_suffix(".ts"))
                .or_else(|| name.strip_suffix(".mkv"))
                .unwrap_or(name);
            info!("使用自定义文件名: {}", name);
            name.to_string()
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
        let channel_name = channel.name.replace(|c: char| !c.is_alphanumeric() && c != '_' && c != '-', "_");
        format!("{}_{}", channel_name, now.format("%Y%m%d_%H%M%S"))
    }
}

/// 查找实际的输出文件（N_m3u8DL-RE 可能输出不同扩展名和文件名）
/// 首先尝试按前缀匹配，如果找不到则返回目录中最新的视频文件
async fn find_actual_output_file(expected_path: &PathBuf) -> Option<PathBuf> {
    // 首先检查期望的路径是否存在
    if tokio::fs::metadata(expected_path).await.is_ok() {
        debug!("找到期望的输出文件: {}", expected_path.display());
        return Some(expected_path.clone());
    }

    // 获取目录和文件名前缀
    let parent = expected_path.parent()?;
    let stem = expected_path.file_stem()?.to_str()?;

    debug!("查找输出文件: 目录={}, 文件名前缀={}", parent.display(), stem);

    // 读取目录中的文件
    let mut entries = match tokio::fs::read_dir(parent).await {
        Ok(e) => e,
        Err(e) => {
            warn!("读取目录失败: {}", e);
            return None;
        }
    };

    // 收集所有匹配的文件，按修改时间排序
    let mut matching_files: Vec<(PathBuf, std::time::SystemTime, u64, bool)> = Vec::new();
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
            let is_video = path.extension()
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
                        // 标记是否匹配前缀
                        let matches_prefix = file_name.starts_with(stem);
                        matching_files.push((path, modified, size, matches_prefix));
                    }
                }
            }
        }
    }

    // 优先返回匹配前缀的文件，按修改时间排序
    let mut prefix_matches: Vec<_> = matching_files.iter()
        .filter(|(_, _, _, matches)| *matches)
        .collect();
    prefix_matches.sort_by(|a, b| b.1.cmp(&a.1));

    if let Some((path, _, size, _)) = prefix_matches.first() {
        info!("找到前缀匹配的输出文件: {} (大小: {} bytes)", path.display(), size);
        return Some((*path).clone());
    }

    // 如果没有前缀匹配的，返回最新的视频文件
    matching_files.sort_by(|a, b| b.1.cmp(&a.1));

    if let Some((path, _, size, _)) = matching_files.first() {
        info!("未找到前缀匹配，使用最新的视频文件: {} (大小: {} bytes, 共 {} 个视频文件)",
            path.display(), size, matching_files.len());
        return Some(path.clone());
    }

    warn!("未找到输出文件: 期望={}", expected_path.display());
    None
}
