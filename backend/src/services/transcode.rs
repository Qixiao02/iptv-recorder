//! 流转码服务 - 将 UDP 流转码为 HLS 供浏览器播放

use anyhow::{Context, Result};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use url::Url;

/// 异步文件系统入口。tokio::fs 把读写交给阻塞线程池,避免在 async 轮询循环里
/// (wait_for_playlist)用同步 std::fs 卡住 Tokio worker——磁盘压力下会延迟
/// 其他异步任务(WebSocket 推送、HTTP 响应)。测试模块用各自的 std::fs 建临时文件。
use tokio::fs as tokio_fs;

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
    /// 最近一次被 HLS 客户端访问的时间（用于空闲回收）。
    /// 与 `session.started_at` 区分开：后者是会话启动时刻，
    /// 而 `last_accessed_at` 在每次 get_hls_file 时被刷新，
    /// 只有真正"无人观看"的会话才会被回收。
    pub last_accessed_at: Instant,
}

#[derive(Debug, Clone, Copy)]
enum TranscodeProfile {
    FastRemux,
    StableFmp4,
    CompatibleMpegTs,
}

impl TranscodeProfile {
    fn name(self) -> &'static str {
        match self {
            Self::FastRemux => "fast-remux",
            Self::StableFmp4 => "stable-fmp4",
            Self::CompatibleMpegTs => "compatible-mpegts",
        }
    }

    fn startup_timeout(self) -> Duration {
        match self {
            // 组播/网关源的 FastRemux：等第一个 IDR 关键帧 + 切出第一个分片。
            // 之前 8s 太短——IPTV GOP 常 2~6s，加上 ffmpeg 探测缓冲(probesize 4M),
            // 8s 内多半切不出 3 个分片(需 ~18-24s),导致几乎必然超时降级到 30s 全编码。
            // 放宽到 15s 配合 min_ready_segments=1,首帧一到就起播,多数情况不再降级。
            Self::FastRemux => Duration::from_secs(15),
            // 对组播/网关源多给一些时间等待第一个可解码关键帧。
            Self::StableFmp4 => Duration::from_secs(30),
            Self::CompatibleMpegTs => Duration::from_secs(40),
        }
    }

    fn segment_pattern(self) -> &'static str {
        match self {
            Self::FastRemux => "segment_%03d.ts",
            Self::StableFmp4 => "segment_%03d.m4s",
            Self::CompatibleMpegTs => "segment_%03d.ts",
        }
    }

    fn segment_extension(self) -> &'static str {
        match self {
            Self::FastRemux => ".ts",
            Self::StableFmp4 => ".m4s",
            Self::CompatibleMpegTs => ".ts",
        }
    }

    fn min_ready_segments(self) -> usize {
        match self {
            // FastRemux：首帧一到就起播(1 个分片足够)。
            // 之前要求 3 个分片(~18-24s)配合 8s 超时几乎必然失败,每次都降级到
            // 30s 的 StableFmp4 全编码。改为 1 个分片,hls.js 会自动追后续分片缓冲。
            // 单个分片 6s(-hls_time 6)已足够首屏播放,不会 buffer 不足。
            Self::FastRemux => 1,
            Self::StableFmp4 => 2,
            Self::CompatibleMpegTs => 2,
        }
    }
}

struct PlaylistReadyState {
    ready: bool,
    segment_count: usize,
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
            // 仅在"空闲"(没有 HLS 请求)超过该时长时回收，避免边看边被杀。
            session_timeout_secs: 600, // 10 分钟无访问才回收
            max_sessions_per_user: 2,
        }
    }

    pub fn start_cleanup_task(self: Arc<Self>) {
        tokio::spawn(async move {
            // 回收检查间隔比空闲阈值短，保证能及时清理已关闭的预览。
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
        ffmpeg_path: &Path,
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

        // 解析出 FFmpeg 实际要用的 URL。
        // 关键修复：很多 IPTV 源域名同时有 AAAA(IPv6) 和 A(IPv4) 记录，
        // 而部署环境(尤其是 Docker 容器)常常 IPv6 出站不通。FFmpeg 走 libc
        // getaddrinfo 会优先尝试 IPv6 → 连接超时/失败 → 拿到 0 字节分片 →
        // 播放器卡死(manifestLoadError)。这里在 Rust 侧显式解析 IPv4，
        // 把 hostname 替换成 IP 再交给 FFmpeg，绕过 IPv6 与 DNS 抖动。
        // IPv6-only 的源不在 http(s) 范围内，不受影响。
        let effective_url = resolve_ipv4_url(source_url).await;

        preflight_http_stream(&effective_url).await?;

        let mut selected_process = None;
        let mut startup_failure = None;
        // 预览优先保证”最快起播”:先用 FastRemux(纯 copy,不编码,秒起),
        // 绝大多数 IPTV 源 remux 后浏览器即可播放。仅在 remux 失败时才降级到编码方案。
        // (之前把 StableFmp4 排第一,导致每次预览都要等全编码超时,非常慢)
        let profiles = [
            TranscodeProfile::FastRemux,
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
            let mut process = spawn_ffmpeg(
                ffmpeg_path,
                profile,
                &effective_url,
                &hls_dir,
                &playlist_path,
                &session_id,
            )?;
            wire_ffmpeg_logs(&mut process, &session_id, profile, stderr_tail.clone());

            match wait_for_playlist_ready(
                &mut process,
                &playlist_path,
                &hls_dir,
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
                    if is_source_unavailable_error(&err.to_string()) {
                        startup_failure = Some(humanize_transcode_startup_error(&err.to_string()));
                        terminate_process(&mut process).await;
                        break;
                    }

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
                    last_accessed_at: Instant::now(),
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
        // 播放热路径，每个分片都会进来，日志降到 DEBUG 避免阻塞播放器。
        let sessions = self.sessions.read().await;
        let result = sessions.get(session_id).map(|a| a.session.hls_dir.join(filename));
        if result.is_none() {
            warn!(
                "HLS file not found for session {}, available sessions: {:?}",
                session_id,
                sessions.keys().collect::<Vec<_>>()
            );
        }
        result
    }

    /// 标记会话最近被访问（HLS 客户端拉取分片/播放列表时调用）。
    /// 用于把空闲回收的计时器向后推延，避免边看边被 cleanup 杀掉。
    /// 即使会话已被 cleanup 移除（例如刚超时），这里也安全地返回 false。
    pub async fn touch_session(&self, session_id: &str) -> bool {
        let mut sessions = self.sessions.write().await;
        if let Some(active) = sessions.get_mut(session_id) {
            active.last_accessed_at = Instant::now();
            true
        } else {
            false
        }
    }

    /// 清理超时的会话
    #[allow(dead_code)]
    pub async fn cleanup_expired(&self) {
        let mut sessions = self.sessions.write().await;
        let now = Instant::now();

        // 关键：基于"空闲时间"(last_accessed_at) 回收，而不是会话总寿命。
        // 之前用 started_at + 300s 硬上限，会导致用户还在看的时候 FFmpeg
        // 被强杀、HLS 目录被删，前端在 ~5 分钟时收到 manifestLoadError。
        let expired: Vec<String> = sessions
            .iter()
            .filter(|(_, active)| {
                now.duration_since(active.last_accessed_at).as_secs() > self.session_timeout_secs
            })
            .map(|(id, _)| id.clone())
            .collect();

        for session_id in expired {
            if let Some(mut active) = sessions.remove(&session_id) {
                info!(
                    "Cleaning up idle transcode session {} (idle {}s)",
                    session_id,
                    now.duration_since(active.last_accessed_at).as_secs()
                );

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
    ffmpeg_path: &Path,
    profile: TranscodeProfile,
    source_url: &str,
    hls_dir: &PathBuf,
    playlist_path: &PathBuf,
    session_id: &str,
) -> Result<Child> {
    if ffmpeg_path.is_absolute() && !ffmpeg_path.is_file() {
        return Err(anyhow::anyhow!(
            "FFmpeg executable not found: {}",
            ffmpeg_path.display()
        ));
    }

    let segment_pattern = hls_dir.join(profile.segment_pattern());
    let segment_pattern = segment_pattern
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("路径含非 UTF-8 字符"))?;
    let playlist_path = playlist_path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("路径含非 UTF-8 字符"))?;

    let mut command = Command::new(ffmpeg_path);
    // 关键性能点：直播转码下，统计输出（frame=... speed=...x）每秒会刷出数十~上百行，
    // 它们全部进入 stderr 的同步读取 + tracing，会拖慢分片写盘 → 播放器卡顿。
    // 因此日志级别降到 warning（只保留真正的错误），并移除 stats_period 触发器。
    command.args([
        "-hide_banner",
        "-loglevel",
        "warning",
        "-fflags",
        "+genpts+discardcorrupt+igndts",
        "-err_detect",
        "ignore_err",
    ]);

    // HTTP/HTTPS 源(包括 UDP-over-HTTP 网关,如 http://host:port/udp/239.x)必须开重连：
    // 这类网关常会周期性重置 TCP 连接(实测 ~40-50s 一次),没有 reconnect 的话
    // FFmpeg 在连接断开时会持续吐 0 字节分片进 HLS,播放器卡死。
    // reconnect 是 FFmpeg 的【输入】选项,必须放在 -i 之前,且只对 http(s) 生效,
    // 对纯 udp:// rtsp:// 多播源无效,所以这里按协议判断,避免噪音。
    let is_http_source = source_url.starts_with("http://") || source_url.starts_with("https://");
    if is_http_source {
        command.args([
            "-reconnect",
            "1",
            "-reconnect_streamed",
            "1",
            // 网关重置 TCP 连接时(最常见的中断原因)自动重连。
            "-reconnect_on_network_error",
            "1",
            "-reconnect_delay_max",
            "5",
        ]);
    }

    // 输入探测与缓冲。
    match profile {
        TranscodeProfile::FastRemux => {
            command.args([
                "-thread_queue_size",
                // 1080i/p TS 的码率可达 8Mbps+，原 512 易丢包花屏。
                // 拉到 2048 给内核 socket 足够缓冲。
                "2048",
                "-analyzeduration",
                "4M",
                "-probesize",
                "4M",
            ]);
        }
        TranscodeProfile::StableFmp4 | TranscodeProfile::CompatibleMpegTs => {
            command.args([
                "-thread_queue_size",
                "2048",
                "-analyzeduration",
                "10M",
                "-probesize",
                "10M",
            ]);
        }
    }

    command.args([
        "-i",
        source_url,
        "-sn",
        "-dn",
        "-max_muxing_queue_size",
        "4096",
    ]);

    match profile {
        TranscodeProfile::FastRemux => {
            command.args([
                "-c:v",
                "copy",
                "-c:a",
                "copy",
                "-muxpreload",
                "0",
                "-muxdelay",
                "0",
                "-f",
                "hls",
                // copy 模式只能在关键帧切分片。IPTV GOP 常为 2~4s，
                // 原 hls_time=2 会迫使 FFmpeg 在非 IDR 帧切，结果分片时长在 2/4/6s 间漂移，
                // 触发 hls.js 的 maxBufferHole 与时间戳跳变 → 卡顿。
                // 改成 6s：与典型 GOP 对齐，分片时长稳定。
                "-hls_time",
                "6",
                // 原 list_size=8 在直播下窗口只有 ~16s，播放器追得很紧，
                // 任何后端抖动都会立刻显现。放宽到 15（~90s 窗口）。
                "-hls_list_size",
                "15",
                "-hls_flags",
                "delete_segments+append_list+temp_file",
                "-hls_segment_type",
                "mpegts",
                "-hls_segment_filename",
                segment_pattern,
                playlist_path,
            ]);
        }
        TranscodeProfile::StableFmp4 => {
            command.args([
                "-map",
                "0:v:0?",
                "-map",
                "0:a:0?",
                "-vf",
                "yadif=0:-1:0",
                "-r",
                "25",
                "-c:v",
                "libx264",
                "-preset",
                "superfast",
                "-tune",
                "zerolatency",
                "-profile:v",
                "main",
                "-x264-params",
                "repeat-headers=1:scenecut=0",
                "-b:v",
                "2500k",
                "-maxrate",
                "3000k",
                "-bufsize",
                "6000k",
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
                "96k",
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
                "-map",
                "0:v:0?",
                "-map",
                "0:a:0?",
                "-vf",
                "yadif=0:-1:0",
                "-r",
                "25",
                "-c:v",
                "libx264",
                "-preset",
                "superfast",
                "-tune",
                "zerolatency",
                "-profile:v",
                "main",
                "-x264-params",
                "repeat-headers=1:scenecut=0",
                "-b:v",
                "2500k",
                "-maxrate",
                "3000k",
                "-bufsize",
                "6000k",
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
                "96k",
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
        "Spawning FFmpeg for session {} with profile {} using {}",
        session_id,
        profile.name(),
        ffmpeg_path.display()
    );

    command
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .with_context(|| format!("Failed to start FFmpeg process: {}", ffmpeg_path.display()))
}

async fn preflight_http_stream(source_url: &str) -> Result<()> {
    let Ok(parsed) = Url::parse(source_url) else {
        return Ok(());
    };

    if !matches!(parsed.scheme(), "http" | "https") {
        return Ok(());
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .user_agent("Mozilla/5.0 IPTV-Recorder/1.0")
        .build()
        .context("创建源站预检客户端失败")?;

    let response = client
        .get(source_url)
        .header(reqwest::header::RANGE, "bytes=0-4095")
        .send()
        .await
        .with_context(|| format!("源站连接失败：{source_url}"))?;

    if response.status().is_server_error() {
        return Err(anyhow::anyhow!(
            "源站暂不可用：HTTP {}。当前地址能连上服务器，但服务器拒绝返回直播流，请稍后重试或更换可用频道源。",
            response.status()
        ));
    }

    if response.status().is_client_error() {
        return Err(anyhow::anyhow!(
            "源地址不可播放：HTTP {}。请检查频道 URL 是否正确或是否需要鉴权。",
            response.status()
        ));
    }

    Ok(())
}

/// 把 http(s) 源 URL 里的 hostname 解析为 IPv4 并替换，
/// 返回 FFmpeg 应当使用的有效 URL。非 http(s) 或解析失败时原样返回。
///
/// 背景：部署环境(尤其是 Docker)常常 IPv6 出站不通，而很多 IPTV 源域名
/// 的 AAAA 记录会被 getaddrinfo 优先返回。FFmpeg 拿到 IPv6 地址后连接超时，
/// 转码出 0 字节分片，前端表现为"录制中无法播放"或播放卡死。
/// 显式锁定 IPv4 可彻底规避，对 IPv6-only 源(罕见)无影响。
async fn resolve_ipv4_url(source_url: &str) -> String {
    let Ok(parsed) = Url::parse(source_url) else {
        return source_url.to_string();
    };
    if !matches!(parsed.scheme(), "http" | "https") {
        return source_url.to_string();
    }

    let Some(host) = parsed.host_str() else {
        return source_url.to_string();
    };

    // 已经是 IP 字面量就不必再解析(无论是 v4 还是 v6)。
    if host.parse::<std::net::IpAddr>().is_ok() {
        return source_url.to_string();
    }

    // 限定 3 秒，避免 DNS 慢拖累起播。
    let resolved = match tokio::time::timeout(
        Duration::from_secs(3),
        tokio::net::lookup_host(format!("{}:0", host)),
    )
    .await
    {
        Ok(Ok(addrs)) => addrs,
        Ok(Err(e)) => {
            debug!("IPv4 解析失败({host}): {e},沿用原 URL");
            return source_url.to_string();
        }
        Err(_) => {
            debug!("IPv4 解析超时({host}),沿用原 URL");
            return source_url.to_string();
        }
    };

    // 优先取第一个 IPv4 地址。
    let Some(ipv4) = resolved
        .map(|sa| sa.ip())
        .find(|ip| ip.is_ipv4())
    else {
        // 这个域名只有 AAAA 记录，没有 A 记录 —— 保持原样让 FFmpeg 走 IPv6。
        return source_url.to_string();
    };

    // 重建 URL：hostname → IPv4，保留 scheme/port/path/query。
    let mut rebuilt = parsed.clone();
    if rebuilt.set_host(Some(&ipv4.to_string())).is_err() {
        return source_url.to_string();
    }
    debug!("源 URL 已锁定 IPv4: {source_url} -> {rebuilt}");
    rebuilt.to_string()
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

fn is_source_unavailable_error(message: &str) -> bool {
    // Broad HTTP 5xx detection from recording tool stderr:
    //   FFmpeg:   "HTTP error 502 Bad Gateway", "HTTP error 503 Service Unavailable"
    //   N_m3u8DL-RE: "Server returned 5XX Server Error reply"
    message.contains("HTTP error 5")
        || message.contains("Server returned 5")
        || message.contains("5XX")
}

fn humanize_transcode_startup_error(message: &str) -> String {
    if is_source_unavailable_error(message) {
        "源站不可用：服务器返回 5XX 错误，当前频道源拒绝返回直播流。请稍后重试，或换一个可用的频道源。".to_string()
    } else {
        message.to_string()
    }
}

async fn wait_for_playlist_ready(
    process: &mut Child,
    playlist_path: &PathBuf,
    hls_dir: &PathBuf,
    profile: TranscodeProfile,
    stderr_tail: Arc<std::sync::Mutex<VecDeque<String>>>,
) -> Result<()> {
    let started_at = Instant::now();
    let deadline = started_at + profile.startup_timeout();

    loop {
        let ready_state = playlist_ready_state(playlist_path, hls_dir, profile).await;
        if ready_state.ready {
            info!(
                "HLS playlist ready with profile {}: {:?}, segments={}",
                profile.name(),
                playlist_path,
                ready_state.segment_count
            );
            return Ok(());
        }

        if let Some(status) = process.try_wait()? {
            let tail = format_log_tail(&stderr_tail);
            let diagnostics = collect_hls_diagnostics(hls_dir, playlist_path, profile).await;
            return Err(anyhow::anyhow!(
                "FFmpeg 进程提前退出（profile={}, status={}）。{}{}",
                profile.name(),
                status,
                tail,
                diagnostics
            ));
        }

        if Instant::now() >= deadline {
            let tail = format_log_tail(&stderr_tail);
            let diagnostics = collect_hls_diagnostics(hls_dir, playlist_path, profile).await;
            return Err(anyhow::anyhow!(
                "等待 HLS 播放列表超时（profile={}, timeout={}s）。{}{}",
                profile.name(),
                profile.startup_timeout().as_secs(),
                tail,
                diagnostics
            ));
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

#[cfg(test)]
async fn playlist_is_ready(path: &PathBuf) -> bool {
    playlist_ready_state(path, &PathBuf::new(), TranscodeProfile::FastRemux)
        .await
        .ready
}

async fn playlist_ready_state(
    playlist_path: &PathBuf,
    hls_dir: &PathBuf,
    profile: TranscodeProfile,
) -> PlaylistReadyState {
    let Ok(content) = tokio_fs::read_to_string(playlist_path).await else {
        return PlaylistReadyState {
            ready: false,
            segment_count: 0,
        };
    };

    if !content.contains("#EXTM3U") {
        return PlaylistReadyState {
            ready: false,
            segment_count: 0,
        };
    }

    let playlist_segments = content.matches("#EXTINF:").count();
    let file_segments = count_hls_segments(hls_dir, profile).await;
    let segment_count = playlist_segments.max(file_segments);
    let has_segment_reference = content.contains("segment_")
        || (content.contains("#EXT-X-MAP") && content.contains("init.mp4"));

    PlaylistReadyState {
        ready: has_segment_reference && segment_count >= profile.min_ready_segments(),
        segment_count,
    }
}

async fn count_hls_segments(hls_dir: &PathBuf, profile: TranscodeProfile) -> usize {
    // tokio::fs::read_dir 返回异步迭代器,用 next_entry().await 逐项读取,
    // 不阻塞 Tokio worker(对照原先 std::fs::read_dir 同步迭代)。
    let Ok(mut entries) = tokio_fs::read_dir(hls_dir).await else {
        return 0;
    };
    let mut count = 0;
    while let Ok(Some(entry)) = entries.next_entry().await {
        if entry
            .file_name()
            .to_string_lossy()
            .ends_with(profile.segment_extension())
        {
            count += 1;
        }
    }
    count
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

async fn collect_hls_diagnostics(
    hls_dir: &PathBuf,
    playlist_path: &PathBuf,
    profile: TranscodeProfile,
) -> String {
    let mut parts = Vec::new();

    let playlist_exists = tokio_fs::try_exists(playlist_path).await.unwrap_or(false);
    parts.push(format!(" playlist_exists={playlist_exists}"));

    if let Ok(content) = tokio_fs::read_to_string(playlist_path).await {
        let preview = content.lines().take(8).collect::<Vec<_>>().join(" || ");
        if !preview.is_empty() {
            parts.push(format!(" playlist_preview={preview:?}"));
        }
    }

    let init_exists = tokio_fs::try_exists(&hls_dir.join("init.mp4"))
        .await
        .unwrap_or(false);
    if matches!(profile, TranscodeProfile::StableFmp4) {
        parts.push(format!(" init_exists={init_exists}"));
    }

    let segment_count = count_hls_segments(hls_dir, profile).await;
    parts.push(format!(" segment_count={segment_count}"));

    if playlist_exists && segment_count == 0 {
        parts.push(
            " hint=\"已生成播放列表但还没有媒体分片，通常是慢首帧或迟迟未等到关键帧\"".to_string(),
        );
    } else if !playlist_exists {
        parts.push(" hint=\"连播放列表都未生成，通常仍卡在输入探测或视频轨识别阶段\"".to_string());
    }

    parts.concat()
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

    #[tokio::test]
    async fn playlist_ready_requires_manifest_and_segment() {
        let path = temp_playlist_path("playlist-ready");
        fs::write(
            &path,
            "#EXTM3U\n#EXT-X-VERSION:7\n#EXTINF:2.0,\nsegment_000.ts\n#EXTINF:2.0,\nsegment_001.ts\n#EXTINF:2.0,\nsegment_002.ts\n",
        )
        .expect("write playlist");

        assert!(playlist_is_ready(&path).await);

        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn playlist_ready_rejects_empty_manifest() {
        let path = temp_playlist_path("playlist-empty");
        fs::write(&path, "#EXTM3U\n#EXT-X-VERSION:7\n").expect("write playlist");

        assert!(!playlist_is_ready(&path).await);

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
