//! 进程管理模块
//!
//! 管理外部录制进程（N_m3u8DL-RE）的生命周期

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::{oneshot, watch, RwLock};
use tokio::time::{timeout, Duration};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// 录制进程配置
#[derive(Debug, Clone)]
pub struct RecordingConfig {
    /// 录制器可执行文件路径，未指定时使用进程管理器默认值
    pub recorder_executable: Option<PathBuf>,
    pub engine: RecordingEngine,

    /// 频道 URL
    pub url: String,

    /// 输出文件路径
    pub output_path: PathBuf,

    /// 录制时长（秒），None 表示不限制
    pub duration_seconds: Option<u64>,

    /// 自定义 Headers
    pub headers: Vec<(String, String)>,

    /// 用户代理
    pub user_agent: Option<String>,

    /// 代理设置
    pub proxy: Option<String>,

    /// 线程数
    pub threads: Option<usize>,

    /// 视频质量 (best, 1080p, 720p, 480p, 或自定义正则)
    pub video_quality: String,

    /// 音频质量 (best, 或自定义正则)
    pub audio_quality: String,

    /// 下载限速 (如: 10M, 500K)
    pub max_speed: Option<String>,

    /// 任务 ID（用于日志和跟踪）
    pub task_id: String,

    /// 频道名称
    pub channel_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordingEngine {
    NM3u8dlRe,
    Ffmpeg,
}

/// 录制进程句柄
#[derive(Debug)]
pub struct RecordingHandle {
    #[allow(dead_code)]
    pub id: Uuid,
    #[allow(dead_code)]
    pub task_id: String,
    /// 进程状态接收器
    pub status_rx: watch::Receiver<ProcessStatus>,
}

/// 进程状态
#[derive(Debug, Clone, PartialEq)]
pub enum ProcessStatus {
    Starting,
    Running,
    #[allow(dead_code)]
    Stopping,
    Completed {
        exit_code: Option<i32>,
    },
    Failed {
        error: String,
    },
    #[allow(dead_code)]
    Cancelled,
}

/// 进程信息
#[allow(dead_code)]
struct ProcessInfo {
    id: Uuid,
    task_id: String,
    kill_tx: oneshot::Sender<()>,
}

/// 用于停止录制的句柄
#[allow(dead_code)]
pub struct StopHandle {
    pub task_id: String,
    pub kill_tx: Option<oneshot::Sender<()>>,
}

/// 进程管理器
pub struct ProcessManager {
    recorder_path: PathBuf,
    temp_dir: PathBuf,
    processes: Arc<RwLock<HashMap<Uuid, ProcessInfo>>>,
}

impl ProcessManager {
    /// 创建新的进程管理器
    pub fn new(recorder_path: PathBuf, temp_dir: PathBuf) -> Self {
        Self {
            recorder_path,
            temp_dir,
            processes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 启动录制进程
    pub async fn start_recording(&self, config: RecordingConfig) -> Result<RecordingHandle> {
        let id = Uuid::new_v4();
        let task_id = config.task_id.clone();

        info!(
            "🎬 启动录制任务: task_id={}, channel={}",
            task_id, config.channel_name
        );

        // 确保输出目录存在
        if let Some(parent) = config.output_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // 每个录制任务使用独立的临时子目录（按 task_id 隔离），
        // 避免并发录制时 HLS 分片（1.ts/2.ts...）在共享 tmp-dir 中互相覆盖，
        // 这正是“文件名是 A 台、内容却是 B 台”串台问题的根因。
        let task_tmp_dir = self.temp_dir.join(&config.task_id);
        tokio::fs::create_dir_all(&task_tmp_dir).await?;

        // 构建命令参数 (N_m3u8DL-RE 2025 版本)
        let recorder_path = config
            .recorder_executable
            .as_ref()
            .unwrap_or(&self.recorder_path);
        let mut cmd = Command::new(recorder_path);

        if config.engine == RecordingEngine::Ffmpeg {
            cmd.arg("-y")
                // 与转码(播放)路径保持一致的输入稳定性参数,
                // 避免对不稳定的 mpegts/udp 流在长时间录制中断流。
                .args([
                    "-hide_banner",
                    "-fflags",
                    "+genpts+discardcorrupt+igndts",
                    "-err_detect",
                    "ignore_err",
                    "-reconnect",
                    "1",
                    "-reconnect_streamed",
                    "1",
                    "-reconnect_delay_max",
                    "2",
                    // 多 Program 的 mpegts 流(如 udpxy 转发的 IPTV 组播)PMT 表
                    // 经常动态变化、多个 program 顺序漂移。不显式指定时 ffmpeg
                    // 可能锁到错误的 program/频道(表现为"录错台")。
                    // 下面用 -map 显式只取第一个视频流和对应的音频,避免选台漂移。
                    "-thread_queue_size",
                    "512",
                    "-analyzeduration",
                    "4M",
                    "-probesize",
                    "4M",
                ]);
            if let Some(ua) = &config.user_agent {
                cmd.arg("-user_agent").arg(ua);
            }
            for (key, value) in &config.headers {
                cmd.arg("-headers").arg(format!("{}: {}\r\n", key, value));
            }
            if let Some(duration) = config.duration_seconds {
                cmd.arg("-t").arg(duration.to_string());
            }
            cmd.arg("-i").arg(&config.url);
            // 显式映射:第一个视频流 + 它的音频,杜绝多 program 选台漂移
            cmd.args(["-map", "0:v:0", "-map", "0:a?"]);
            cmd.arg("-c").arg("copy");
            cmd.arg("-sn").arg("-dn");
            cmd.arg("-f").arg("mpegts");
            cmd.arg(&config.output_path);

            info!("FFmpeg command:");
            info!("  URL: {}", redact_url_for_log(&config.url));
            info!("  output: {}", config.output_path.display());
            info!("  duration: {:?}s", config.duration_seconds);
        } else {
            // 基本参数：使用按 task_id 隔离的临时目录，防止并发录制分片互相覆盖
            cmd.arg(&config.url).arg("--tmp-dir").arg(&task_tmp_dir);

            // 视频质量选择
            match config.video_quality.as_str() {
                "best" => {
                    cmd.arg("--auto-select");
                }
                "1080p" | "720p" | "480p" | "360p" => {
                    // 按分辨率选择
                    cmd.arg("-sv")
                        .arg(format!("res=\"{}\"", config.video_quality));
                    cmd.arg("-sa").arg("best");
                }
                _ => {
                    // 自定义正则表达式
                    if !config.video_quality.is_empty() {
                        cmd.arg("-sv").arg(&config.video_quality);
                    } else {
                        cmd.arg("--auto-select");
                    }
                }
            }

            // 音频质量选择
            if config.audio_quality != "best" && !config.audio_quality.is_empty() {
                cmd.arg("-sa").arg(&config.audio_quality);
            }

            // 设置输出目录和文件名
            if let Some(parent) = config.output_path.parent() {
                cmd.arg("--save-dir").arg(parent);
                info!("N_m3u8DL-RE save-dir: {}", parent.display());
            }
            if let Some(stem) = config.output_path.file_stem() {
                // 使用 --save-name 指定文件名（不带后缀）
                cmd.arg("--save-name").arg(stem);
                info!("N_m3u8DL-RE save-name: {}", stem.to_string_lossy());
            }

            // 设置时长（直播模式）
            if let Some(duration) = config.duration_seconds {
                // 转换为 HH:mm:ss 格式
                let hours = duration / 3600;
                let minutes = (duration % 3600) / 60;
                let seconds = duration % 60;
                let duration_str = format!("{:02}:{:02}:{:02}", hours, minutes, seconds);
                cmd.arg("--live-record-limit").arg(&duration_str);
            }

            // 设置 User-Agent (使用 -H 参数)
            if let Some(ua) = &config.user_agent {
                cmd.arg("-H").arg(format!("User-Agent: {}", ua));
            }

            // 设置其他 Headers
            for (key, value) in &config.headers {
                cmd.arg("-H").arg(format!("{}: {}", key, value));
            }

            // 设置代理
            if let Some(proxy) = &config.proxy {
                cmd.arg("--custom-proxy").arg(proxy);
            }

            // 设置下载限速
            if let Some(max_speed) = &config.max_speed {
                if !max_speed.is_empty() {
                    cmd.arg("-R").arg(max_speed);
                }
            }

            // 设置线程数
            if let Some(threads) = config.threads {
                cmd.arg("--thread-count").arg(threads.to_string());
            }

            // 设置实时日志（用于解析进度）
            cmd.arg("--log-level").arg("INFO");

            // 完成后删除临时文件
            cmd.arg("--del-after-done");

            // 打印完整命令用于调试
            info!("N_m3u8DL-RE 完整命令:");
            info!("  URL: {}", redact_url_for_log(&config.url));
            info!("  save-dir: {:?}", config.output_path.parent());
            info!("  save-name: {:?}", config.output_path.file_stem());
            info!("  duration: {:?}s", config.duration_seconds);
        }

        debug!("启动命令: {:?}", cmd);

        // 创建终止通道
        let (kill_tx, kill_rx) = oneshot::channel::<()>();
        let (status_tx, status_rx) = watch::channel(ProcessStatus::Starting);

        // 启动进程
        let mut child = cmd.spawn().map_err(|e| {
            anyhow!(
                "failed to start {:?} recorder ({}): {}",
                config.engine,
                recorder_path.display(),
                e
            )
        })?;

        let process_id = id;
        let task_id_clone = task_id.clone();
        let output_path = config.output_path.clone();
        let tmp_dir_to_cleanup = task_tmp_dir.clone();

        // 注册进程
        {
            let mut processes = self.processes.write().await;
            processes.insert(
                id,
                ProcessInfo {
                    id,
                    task_id: task_id.clone(),
                    kill_tx,
                },
            );
        }

        // 更新状态为运行中
        let _ = status_tx.send(ProcessStatus::Running);

        // 启动监控任务
        let processes = self.processes.clone();
        tokio::spawn(async move {
            // 等待进程结束或收到终止信号
            let result: Result<Option<i32>, String> = tokio::select! {
                // 等待进程结束
                exit_status = child.wait() => {
                    match exit_status {
                        Ok(status) => Ok(status.code()),
                        Err(e) => Err(format!("等待进程失败: {}", e)),
                    }
                }
                // 等待终止信号
                _ = kill_rx => {
                    debug!("收到终止信号，正在停止录制任务: {}", task_id_clone);
                    // 发送 SIGTERM
                    let _ = child.kill().await;
                    Ok(None)  // Cancelled, no exit code
                }
            };

            // 从进程列表中移除
            {
                let mut processes = processes.write().await;
                processes.remove(&process_id);
            }

            // 清理该任务专属的临时子目录（N_m3u8DL-RE 的 --del-after-done 只删文件不删目录）。
            // 失败/取消时也清理，避免残留分片占用磁盘。
            if tmp_dir_to_cleanup.exists() {
                if let Err(e) = tokio::fs::remove_dir_all(&tmp_dir_to_cleanup).await {
                    debug!(
                        "清理任务临时目录失败（可忽略）: task_id={}, dir={}, error={}",
                        task_id_clone,
                        tmp_dir_to_cleanup.display(),
                        e
                    );
                }
            }

            // 处理结果
            match result {
                Ok(exit_code) => {
                    // Some(0) = 正常退出, None = 被取消
                    if exit_code == Some(0) || exit_code.is_none() {
                        info!(
                            "✅ 录制完成: task_id={}, output={}",
                            task_id_clone,
                            output_path.display()
                        );
                        let _ = status_tx.send(ProcessStatus::Completed { exit_code });
                    } else {
                        warn!(
                            "⚠️ 录制进程异常退出: task_id={}, code={:?}",
                            task_id_clone, exit_code
                        );
                        let _ = status_tx.send(ProcessStatus::Failed {
                            error: format!("录制进程异常退出，退出码: {:?}", exit_code),
                        });
                    }
                }
                Err(e) => {
                    error!("❌ 录制进程错误: task_id={}, error={}", task_id_clone, e);
                    let _ = status_tx.send(ProcessStatus::Failed { error: e });
                }
            }
        });

        Ok(RecordingHandle {
            id,
            task_id,
            status_rx,
        })
    }

    /// 终止录制进程
    #[allow(dead_code)]
    pub async fn stop_recording(&self, handle: RecordingHandle) -> Result<()> {
        info!("🛑 停止录制任务: task_id={}", handle.task_id);

        // 从进程列表中获取并移除
        let kill_tx = {
            let mut processes = self.processes.write().await;
            processes
                .remove(&handle.id)
                .map(|info| info.kill_tx)
                .ok_or_else(|| anyhow!("录制任务不存在: {}", handle.task_id))?
        };

        // 发送终止信号
        kill_tx.send(()).map_err(|_| anyhow!("发送终止信号失败"))?;

        // 等待最多 5 秒让进程优雅退出
        let _ = timeout(Duration::from_secs(5), async {
            loop {
                if *handle.status_rx.borrow() != ProcessStatus::Running {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await;

        Ok(())
    }

    /// 通过 task_id 终止录制进程
    pub async fn stop_by_task_id(&self, task_id: &str) -> Result<()> {
        info!("🛑 停止录制任务: task_id={}", task_id);

        // 从进程列表中查找并移除
        let process_info = {
            let mut processes = self.processes.write().await;
            processes
                .iter()
                .find(|(_, info)| info.task_id == task_id)
                .map(|(id, _)| *id)
                .and_then(|id| processes.remove(&id))
        };

        if let Some(info) = process_info {
            // 发送终止信号
            let _ = info.kill_tx.send(());
            info!("已发送终止信号: task_id={}", task_id);
            Ok(())
        } else {
            // 任务可能已经完成或不存在
            warn!("录制任务不存在或已完成: task_id={}", task_id);
            Err(anyhow!("录制任务不存在或已完成: {}", task_id))
        }
    }

    /// 检查任务是否正在运行
    #[allow(dead_code)]
    pub async fn is_task_running(&self, task_id: &str) -> bool {
        let processes = self.processes.read().await;
        processes.values().any(|info| info.task_id == task_id)
    }

    /// 检查进程状态
    #[allow(dead_code)]
    pub async fn get_status(&self, handle: &RecordingHandle) -> ProcessStatus {
        handle.status_rx.borrow().clone()
    }

    /// 获取运行中的进程数量
    #[allow(dead_code)]
    pub async fn running_count(&self) -> usize {
        self.processes.read().await.len()
    }

    /// 终止所有运行中的进程
    #[allow(dead_code)]
    pub async fn stop_all(&self) -> Result<()> {
        let processes: Vec<_> = {
            let processes = self.processes.read().await;
            processes
                .values()
                .map(|p| (p.id, p.task_id.clone()))
                .collect()
        };

        for (id, task_id) in processes {
            info!("停止录制任务: {}", task_id);
            let mut processes = self.processes.write().await;
            if let Some(info) = processes.remove(&id) {
                let _ = info.kill_tx.send(());
            }
        }

        Ok(())
    }
}

/// URL 日志脱敏:隐藏 query string 中的敏感参数(token/key/password/sign/auth)。
/// 例:`http://x.com/stream?token=abc&foo=bar` → `http://x.com/stream?token=***&foo=bar`
fn redact_url_for_log(url: &str) -> String {
    if let Some((before_q, query)) = url.split_once('?') {
        let redacted: Vec<String> = query
            .split('&')
            .map(|kv| {
                if let Some((k, _)) = kv.split_once('=') {
                    let key = k.to_ascii_lowercase();
                    if key.contains("token")
                        || key.contains("key")
                        || key.contains("password")
                        || key.contains("passwd")
                        || key.contains("sign")
                        || key.contains("auth")
                        || key.contains("secret")
                        || key.contains("access")
                    {
                        format!("{}=***", k)
                    } else {
                        kv.to_string()
                    }
                } else {
                    kv.to_string()
                }
            })
            .collect();
        format!("{}?{}", before_q, redacted.join("&"))
    } else {
        url.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recording_config() {
        let config = RecordingConfig {
            recorder_executable: None,
            engine: RecordingEngine::NM3u8dlRe,
            url: "https://example.com/stream.m3u8".to_string(),
            output_path: PathBuf::from("/tmp/test.mp4"),
            duration_seconds: Some(60),
            headers: vec![("User-Agent".to_string(), "Test".to_string())],
            user_agent: None,
            proxy: None,
            threads: Some(4),
            video_quality: "best".to_string(),
            audio_quality: "best".to_string(),
            max_speed: None,
            task_id: "test-123".to_string(),
            channel_name: "Test Channel".to_string(),
        };

        assert_eq!(config.duration_seconds, Some(60));
        assert_eq!(config.task_id, "test-123");
    }
}
