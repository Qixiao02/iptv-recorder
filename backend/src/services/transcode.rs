//! 流转码服务 - 将 UDP 流转码为 HLS 供浏览器播放

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// 转码会话信息
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct TranscodeSession {
    pub id: String,
    pub channel_id: String,
    pub owner_user_id: String,
    pub owner_username: String,
    pub source_url: String,
    pub hls_dir: PathBuf,
    pub playlist_path: PathBuf,
    pub started_at: Instant,
}

/// 活动的转码会话
pub struct ActiveTranscode {
    pub session: TranscodeSession,
    pub process: Child,
}

/// 转码服务
#[allow(dead_code)]
pub struct TranscodeService {
    /// 活动的转码会话
    sessions: Arc<RwLock<HashMap<String, ActiveTranscode>>>,
    /// HLS 输出目录
    hls_base_dir: PathBuf,
    /// 会话超时时间（秒）
    session_timeout_secs: u64,
    /// 单用户最大预览会话数
    max_sessions_per_user: usize,
}

impl TranscodeService {
    /// 创建新的转码服务
    pub fn new(hls_base_dir: PathBuf) -> Self {
        // 确保目录存在
        if !hls_base_dir.exists() {
            if let Err(e) = std::fs::create_dir_all(&hls_base_dir) {
                warn!("Failed to create HLS directory: {}", e);
            }
        }

        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            hls_base_dir,
            session_timeout_secs: 300, // 5 分钟超时
            max_sessions_per_user: 2,
        }
    }

    pub fn start_cleanup_task(self: Arc<Self>) {
        tokio::spawn(async move {
            let interval = Duration::from_secs(60);
            loop {
                tokio::time::sleep(interval).await;
                self.cleanup_expired().await;
            }
        });
    }

    /// 启动转码
    pub async fn start_transcode(
        &self,
        channel_id: &str,
        source_url: &str,
        owner_user_id: &str,
        owner_username: &str,
    ) -> Result<TranscodeSession> {
        // 同一用户重复打开同一频道时复用现有会话，避免反复拉起 FFmpeg。
        {
            let sessions = self.sessions.read().await;
            for (_, active) in sessions.iter() {
                if active.session.channel_id == channel_id
                    && active.session.owner_user_id == owner_user_id
                {
                    info!("Transcode session already exists for channel {}", channel_id);
                    return Ok(active.session.clone());
                }
            }
        }

        {
            let sessions = self.sessions.read().await;
            let active_for_user = sessions
                .values()
                .filter(|active| active.session.owner_user_id == owner_user_id)
                .count();

            if active_for_user >= self.max_sessions_per_user {
                return Err(anyhow::anyhow!(
                    "单个用户最多只能同时预览 {} 个转码会话",
                    self.max_sessions_per_user
                ));
            }
        }

        // 创建会话 ID
        let session_id = uuid::Uuid::new_v4().to_string();

        // 创建 HLS 输出目录
        let hls_dir = self.hls_base_dir.join(&session_id);
        std::fs::create_dir_all(&hls_dir)
            .context("Failed to create HLS output directory")?;

        let playlist_path = hls_dir.join("stream.m3u8");

        info!("Starting transcode for channel {}: {} -> {}", channel_id, source_url, hls_dir.display());

        // 启动 FFmpeg 进程
        // 优化配置说明：
        // - hls_time: 2秒分片，减少等待时间
        // - hls_list_size: 保留 20 个分片（约 40 秒内容）
        // - g: 关键帧间隔设为帧率的2倍，确保每个分片开始有关键帧
        // - sc_threshold: 禁用场景切换检测，确保固定的关键帧间隔
        // - hls_flags: append_list + independent_segments，允许播放器从任意分片开始
        let mut process = Command::new("ffmpeg")
            .args([
                "-i", source_url,
                "-c:v", "libx264",
                "-preset", "ultrafast",
                "-tune", "zerolatency",
                "-g", "50",           // 关键帧间隔（25fps * 2秒）
                "-sc_threshold", "0", // 禁用场景切换强制关键帧
                "-c:a", "aac",
                "-b:a", "128k",
                "-f", "hls",
                "-hls_time", "2",
                "-hls_list_size", "20",
                "-hls_flags", "append_list+independent_segments",
                "-hls_segment_type", "mpegts",
                "-hls_segment_filename",
            ])
            .arg(hls_dir.join("segment_%03d.ts").to_str().ok_or_else(|| anyhow::anyhow!("路径含非 UTF-8 字符"))?)
            .arg(playlist_path.to_str().ok_or_else(|| anyhow::anyhow!("路径含非 UTF-8 字符"))?)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .context("Failed to start FFmpeg process")?;

        // 在后台读取 FFmpeg 输出（用于调试）
        if let Some(stderr) = process.stderr.take() {
            let session_id_clone = session_id.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    debug!("[FFmpeg:{}] {}", session_id_clone, line);
                }
            });
        }

        let session = TranscodeSession {
            id: session_id.clone(),
            channel_id: channel_id.to_string(),
            owner_user_id: owner_user_id.to_string(),
            owner_username: owner_username.to_string(),
            source_url: source_url.to_string(),
            hls_dir,
            playlist_path,
            started_at: Instant::now(),
        };

        // 保存会话
        {
            let mut sessions = self.sessions.write().await;
            info!("Storing transcode session {} for channel {}", session_id, channel_id);
            sessions.insert(session_id.clone(), ActiveTranscode {
                session: session.clone(),
                process,
            });
            info!("Total active sessions: {}", sessions.len());
        }

        // 等待 HLS playlist 生成
        // FFmpeg 需要时间启动并创建第一个分片（特别是 4 秒分片时长）
        let playlist_path = session.playlist_path.clone();
        for i in 0..40 {
            if playlist_path.exists() {
                info!("HLS playlist ready: {:?}", playlist_path);
                break;
            }
            if i == 39 {
                warn!("HLS playlist not generated after 10 seconds");
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }

        Ok(session)
    }

    /// 停止转码
    pub async fn stop_transcode(&self, session_id: &str) -> Result<()> {
        let mut sessions = self.sessions.write().await;

        if let Some(mut active) = sessions.remove(session_id) {
            info!("Stopping transcode session {}", session_id);

            // 终止 FFmpeg 进程
            if let Err(e) = active.process.kill().await {
                warn!("Failed to kill FFmpeg process: {}", e);
            }

            // 清理 HLS 文件
            if let Err(e) = std::fs::remove_dir_all(&active.session.hls_dir) {
                warn!("Failed to cleanup HLS directory: {}", e);
            }
        }

        Ok(())
    }

    /// 停止频道的所有转码
    #[allow(dead_code)]
    pub async fn stop_channel_transcode(&self, channel_id: &str) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        let session_ids: Vec<String> = sessions
            .iter()
            .filter(|(_, active)| active.session.channel_id == channel_id)
            .map(|(id, _)| id.clone())
            .collect();

        for session_id in session_ids {
            if let Some(mut active) = sessions.remove(&session_id) {
                info!("Stopping transcode session {} for channel {}", session_id, channel_id);

                if let Err(e) = active.process.kill().await {
                    warn!("Failed to kill FFmpeg process: {}", e);
                }

                if let Err(e) = std::fs::remove_dir_all(&active.session.hls_dir) {
                    warn!("Failed to cleanup HLS directory: {}", e);
                }
            }
        }

        Ok(())
    }

    /// 获取转码会话
    #[allow(dead_code)]
    pub async fn get_session(&self, session_id: &str) -> Option<TranscodeSession> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).map(|a| a.session.clone())
    }

    /// 获取 HLS 文件路径
    #[allow(dead_code)]
    pub async fn get_hls_file(&self, session_id: &str, filename: &str) -> Option<PathBuf> {
        info!("Looking for HLS file: session={}, filename={}", session_id, filename);
        let sessions = self.sessions.read().await;
        info!("Current sessions count: {}, keys: {:?}", sessions.len(), sessions.keys().collect::<Vec<_>>());
        let result = sessions.get(session_id).map(|a| {
            let path = a.session.hls_dir.join(filename);
            info!("Found session, returning path: {:?}", path);
            path
        });
        if result.is_none() {
            warn!("HLS file not found for session {}, available sessions: {:?}",
                session_id,
                sessions.keys().collect::<Vec<_>>()
            );
        }
        result
    }

    /// 清理超时的会话
    #[allow(dead_code)]
    pub async fn cleanup_expired(&self) {
        let mut sessions = self.sessions.write().await;
        let now = Instant::now();

        let expired: Vec<String> = sessions
            .iter()
            .filter(|(_, active)| {
                now.duration_since(active.session.started_at).as_secs() > self.session_timeout_secs
            })
            .map(|(id, _)| id.clone())
            .collect();

        for session_id in expired {
            if let Some(mut active) = sessions.remove(&session_id) {
                info!("Cleaning up expired transcode session {}", session_id);

                if let Err(e) = active.process.kill().await {
                    warn!("Failed to kill FFmpeg process: {}", e);
                }

                if let Err(e) = std::fs::remove_dir_all(&active.session.hls_dir) {
                    warn!("Failed to cleanup HLS directory: {}", e);
                }
            }
        }
    }

    /// 停止所有转码
    #[allow(dead_code)]
    pub async fn stop_all(&self) {
        let mut sessions = self.sessions.write().await;

        for (session_id, mut active) in sessions.drain() {
            info!("Stopping transcode session {}", session_id);

            if let Err(e) = active.process.kill().await {
                warn!("Failed to kill FFmpeg process: {}", e);
            }

            if let Err(e) = std::fs::remove_dir_all(&active.session.hls_dir) {
                warn!("Failed to cleanup HLS directory: {}", e);
            }
        }
    }
}

impl Drop for TranscodeService {
    fn drop(&mut self) {
        // 尝试同步清理（在 tokio runtime 之外）
        // 这是备用清理，正常情况下应该调用 stop_all()
    }
}
