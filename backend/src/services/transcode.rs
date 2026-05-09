//! 流转码服务 - 将 UDP 流转码为 HLS 供浏览器播放

use anyhow::{Context, Result};
use std::collections::{HashMap, VecDeque};
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

#[derive(Debug, Clone, Copy)]
enum TranscodeProfile {
    StableFmp4,
    CompatibleMpegTs,
}

impl TranscodeProfile {
    fn name(self) -> &'static str {
        match self {
            Self::StableFmp4 => "stable-fmp4",
            Self::CompatibleMpegTs => "compatible-mpegts",
        }
    }

    fn startup_timeout(self) -> Duration {
        match self {
            // 对组播/网关源多给一些时间等待第一个可解码关键帧。
            Self::StableFmp4 => Duration::from_secs(18),
            Self::CompatibleMpegTs => Duration::from_secs(24),
        }
    }

    fn segment_pattern(self) -> &'static str {
        match self {
            Self::StableFmp4 => "segment_%03d.m4s",
            Self::CompatibleMpegTs => "segment_%03d.ts",
        }
    }
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
                    info!(
                        "Transcode session already exists for channel {}",
                        channel_id
                    );
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
        recreate_dir(&hls_dir).context("Failed to create HLS output directory")?;

        let playlist_path = hls_dir.join("stream.m3u8");

        info!(
            "Starting transcode for channel {}: {} -> {}",
            channel_id,
            source_url,
            hls_dir.display()
        );

        let mut selected_process = None;
        let mut startup_failure = None;
        let profiles = [
            TranscodeProfile::StableFmp4,
            TranscodeProfile::CompatibleMpegTs,
        ];

        for profile in profiles {
            info!(
                "Trying transcode profile {} for channel {}",
                profile.name(),
                channel_id
            );

            recreate_dir(&hls_dir).context("Failed to reset HLS output directory")?;
            let stderr_tail = Arc::new(std::sync::Mutex::new(VecDeque::with_capacity(60)));
            let mut process =
                spawn_ffmpeg(profile, source_url, &hls_dir, &playlist_path, &session_id)?;
            wire_ffmpeg_logs(&mut process, &session_id, profile, stderr_tail.clone());

            match wait_for_playlist_ready(
                &mut process,
                &playlist_path,
                profile,
                stderr_tail.clone(),
            )
            .await
            {
                Ok(()) => {
                    info!(
                        "Transcode profile {} is ready for channel {}",
                        profile.name(),
                        channel_id
                    );
                    selected_process = Some(process);
                    break;
                }
                Err(err) => {
                    warn!(
                        "Transcode profile {} failed for channel {}: {}",
                        profile.name(),
                        channel_id,
                        err
                    );
                    startup_failure = Some(err.to_string());
                    terminate_process(&mut process).await;
                }
            }
        }

        let Some(process) = selected_process else {
            if let Err(err) = std::fs::remove_dir_all(&hls_dir) {
                warn!("Failed to cleanup failed HLS directory: {}", err);
            }
            return Err(anyhow::anyhow!(
                "{}",
                startup_failure
                    .unwrap_or_else(|| "转码启动失败，未生成可播放的 HLS 输出".to_string())
            ));
        };

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
            info!(
                "Storing transcode session {} for channel {}",
                session_id, channel_id
            );
            sessions.insert(
                session_id.clone(),
                ActiveTranscode {
                    session: session.clone(),
                    process,
                },
            );
            info!("Total active sessions: {}", sessions.len());
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
                info!(
                    "Stopping transcode session {} for channel {}",
                    session_id, channel_id
                );

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
        info!(
            "Looking for HLS file: session={}, filename={}",
            session_id, filename
        );
        let sessions = self.sessions.read().await;
        info!(
            "Current sessions count: {}, keys: {:?}",
            sessions.len(),
            sessions.keys().collect::<Vec<_>>()
        );
        let result = sessions.get(session_id).map(|a| {
            let path = a.session.hls_dir.join(filename);
            info!("Found session, returning path: {:?}", path);
            path
        });
        if result.is_none() {
            warn!(
                "HLS file not found for session {}, available sessions: {:?}",
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

fn recreate_dir(path: &PathBuf) -> Result<()> {
    if path.exists() {
        std::fs::remove_dir_all(path).with_context(|| format!("Failed to cleanup {:?}", path))?;
    }
    std::fs::create_dir_all(path).with_context(|| format!("Failed to create {:?}", path))?;
    Ok(())
}

fn spawn_ffmpeg(
    profile: TranscodeProfile,
    source_url: &str,
    hls_dir: &PathBuf,
    playlist_path: &PathBuf,
    session_id: &str,
) -> Result<Child> {
    let segment_pattern = hls_dir.join(profile.segment_pattern());
    let segment_pattern = segment_pattern
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("路径含非 UTF-8 字符"))?;
    let playlist_path = playlist_path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("路径含非 UTF-8 字符"))?;

    let mut command = Command::new("ffmpeg");
    command.args([
        "-hide_banner",
        "-loglevel",
        "warning",
        "-fflags",
        "+genpts+discardcorrupt+igndts",
        "-err_detect",
        "ignore_err",
        "-analyzeduration",
        "15M",
        "-probesize",
        "15M",
        "-reconnect",
        "1",
        "-reconnect_streamed",
        "1",
        "-reconnect_delay_max",
        "2",
        "-i",
        source_url,
        "-map",
        "0:v:0",
        "-map",
        "0:a:0?",
        "-sn",
        "-dn",
        "-max_muxing_queue_size",
        "4096",
    ]);

    match profile {
        TranscodeProfile::StableFmp4 => {
            command.args([
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-tune",
                "fastdecode",
                "-x264-params",
                "repeat-headers=1:scenecut=0",
                "-g",
                "100",
                "-keyint_min",
                "100",
                "-sc_threshold",
                "0",
                "-force_key_frames",
                "expr:gte(t,n_forced*4)",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
                "-b:a",
                "128k",
                "-af",
                "aresample=async=1:first_pts=0",
                "-f",
                "hls",
                "-hls_time",
                "4",
                "-hls_list_size",
                "12",
                "-hls_flags",
                "delete_segments+independent_segments+temp_file",
                "-hls_segment_type",
                "fmp4",
                "-hls_fmp4_init_filename",
                "init.mp4",
                "-hls_segment_filename",
                segment_pattern,
                playlist_path,
            ]);
        }
        TranscodeProfile::CompatibleMpegTs => {
            command.args([
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-tune",
                "zerolatency",
                "-x264-params",
                "repeat-headers=1:scenecut=0",
                "-g",
                "125",
                "-keyint_min",
                "125",
                "-sc_threshold",
                "0",
                "-force_key_frames",
                "expr:gte(t,n_forced*5)",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
                "-b:a",
                "128k",
                "-af",
                "aresample=async=1:first_pts=0",
                "-f",
                "hls",
                "-hls_time",
                "5",
                "-hls_list_size",
                "10",
                "-hls_flags",
                "delete_segments+independent_segments+append_list+temp_file",
                "-hls_segment_type",
                "mpegts",
                "-hls_segment_filename",
                segment_pattern,
                playlist_path,
            ]);
        }
    }

    info!(
        "Spawning FFmpeg for session {} with profile {}",
        session_id,
        profile.name()
    );

    command
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("Failed to start FFmpeg process")
}

fn wire_ffmpeg_logs(
    process: &mut Child,
    session_id: &str,
    profile: TranscodeProfile,
    stderr_tail: Arc<std::sync::Mutex<VecDeque<String>>>,
) {
    if let Some(stderr) = process.stderr.take() {
        let session_id = session_id.to_string();
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                debug!("[FFmpeg:{}:{}] {}", session_id, profile.name(), line);
                if let Ok(mut tail) = stderr_tail.lock() {
                    if tail.len() >= 60 {
                        tail.pop_front();
                    }
                    tail.push_back(line);
                }
            }
        });
    }
}

async fn wait_for_playlist_ready(
    process: &mut Child,
    playlist_path: &PathBuf,
    profile: TranscodeProfile,
    stderr_tail: Arc<std::sync::Mutex<VecDeque<String>>>,
) -> Result<()> {
    let started_at = Instant::now();
    let deadline = started_at + profile.startup_timeout();

    loop {
        if playlist_is_ready(playlist_path) {
            info!(
                "HLS playlist ready with profile {}: {:?}",
                profile.name(),
                playlist_path
            );
            return Ok(());
        }

        if let Some(status) = process.try_wait()? {
            let tail = format_log_tail(&stderr_tail);
            return Err(anyhow::anyhow!(
                "FFmpeg 进程提前退出（profile={}, status={}）。{}",
                profile.name(),
                status,
                tail
            ));
        }

        if Instant::now() >= deadline {
            let tail = format_log_tail(&stderr_tail);
            return Err(anyhow::anyhow!(
                "等待 HLS 播放列表超时（profile={}, timeout={}s）。{}",
                profile.name(),
                profile.startup_timeout().as_secs(),
                tail
            ));
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

fn playlist_is_ready(path: &PathBuf) -> bool {
    let Ok(content) = std::fs::read_to_string(path) else {
        return false;
    };

    if !content.contains("#EXTM3U") {
        return false;
    }

    content.contains("#EXTINF:")
        || content.contains("segment_")
        || (content.contains("#EXT-X-MAP") && content.contains("init.mp4"))
}

fn format_log_tail(stderr_tail: &Arc<std::sync::Mutex<VecDeque<String>>>) -> String {
    let Ok(tail) = stderr_tail.lock() else {
        return "未能读取 FFmpeg 日志".to_string();
    };

    if tail.is_empty() {
        return "FFmpeg 未输出可用错误日志".to_string();
    }

    format!(
        "FFmpeg 最近日志：{}",
        tail.iter()
            .rev()
            .take(6)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join(" | ")
    )
}

async fn terminate_process(process: &mut Child) {
    if let Err(err) = process.kill().await {
        warn!("Failed to terminate FFmpeg process: {}", err);
    }
}

#[cfg(test)]
mod tests {
    use super::{format_log_tail, playlist_is_ready};
    use std::collections::VecDeque;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Arc;
    use uuid::Uuid;

    fn temp_playlist_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("iptv-recorder-{}-{}", name, Uuid::new_v4()))
    }

    #[test]
    fn playlist_ready_requires_manifest_and_segment() {
        let path = temp_playlist_path("playlist-ready");
        fs::write(&path, "#EXTM3U\n#EXT-X-VERSION:7\n#EXTINF:4.0,\nsegment_000.ts\n")
            .expect("write playlist");

        assert!(playlist_is_ready(&path));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn playlist_ready_rejects_empty_manifest() {
        let path = temp_playlist_path("playlist-empty");
        fs::write(&path, "#EXTM3U\n#EXT-X-VERSION:7\n").expect("write playlist");

        assert!(!playlist_is_ready(&path));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn format_log_tail_keeps_recent_messages() {
        let mut deque = VecDeque::new();
        deque.push_back("first".to_string());
        deque.push_back("second".to_string());
        deque.push_back("third".to_string());

        let tail = Arc::new(std::sync::Mutex::new(deque));
        let formatted = format_log_tail(&tail);

        assert!(formatted.contains("first"));
        assert!(formatted.contains("third"));
    }
}
