//! 后台巡检服务（独立窗口）
//!
//! 仿 `CleanupService` 的常驻后台任务，承载周期性巡检逻辑。
//! 与录制主流程解耦：不挤进 `recording.rs` 的输出，而是独立 `loop + sleep`。
//!
//! 当前职责：磁盘空间定时巡检 —— 每 10 分钟探测一次录制目录剩余空间，
//! 低于阈值时通过 `NotificationService` 落库并推送告警，带去重（同级别
//! 至少间隔 1 小时不再重复发送）。

use anyhow::Result;
use std::{
    path::Path,
    sync::atomic::{AtomicI64, Ordering},
    sync::Arc,
    time::Duration,
};
use tracing::{error, info, warn};

use crate::core::event::EventSender;

use super::notification::{category, level, NotificationService, NotifyRequest};
use super::ServiceContext;

/// 巡检间隔：10 分钟
const INSPECT_INTERVAL_SECS: u64 = 60 * 10;
/// 同级别磁盘告警去重间隔：1 小时
const DEDUP_INTERVAL_SECS: i64 = 60 * 60;

pub struct HeartbeatService {
    ctx: ServiceContext,
    event_sender: Option<EventSender>,
    /// 上次发送磁盘告警的 unix 时间戳（秒）；0 表示尚未发送过
    last_disk_alert_at: AtomicI64,
}

impl HeartbeatService {
    pub fn new(ctx: ServiceContext, event_sender: Option<EventSender>) -> Self {
        Self {
            ctx,
            event_sender,
            last_disk_alert_at: AtomicI64::new(0),
        }
    }

    /// 启动常驻后台巡检循环
    pub fn start(self: Arc<Self>) {
        tokio::spawn(async move {
            info!(
                "💓 Heartbeat inspection service started (every {}s)",
                INSPECT_INTERVAL_SECS
            );
            loop {
                if let Err(e) = self.run_once().await {
                    error!("Heartbeat inspection failed: {}", e);
                }
                tokio::time::sleep(Duration::from_secs(INSPECT_INTERVAL_SECS)).await;
            }
        });
    }

    async fn run_once(&self) -> Result<()> {
        if let Err(e) = self.inspect_disk_space().await {
            warn!("Disk space inspection skipped: {}", e);
        }
        Ok(())
    }

    /// 磁盘空间巡检
    async fn inspect_disk_space(&self) -> Result<()> {
        let disk_warning_enabled = self
            .get_system_value::<bool>("notification.disk_warning", true)
            .await?;
        if !disk_warning_enabled {
            return Ok(());
        }

        // 阈值：DB 的 min_free_space_gb（GB），回退 config 的 min_free_space_mb
        let min_free_gb = self
            .get_system_value::<u64>("storage.min_free_space_gb", 0)
            .await?;
        let min_free_bytes = if min_free_gb > 0 {
            min_free_gb.saturating_mul(1024 * 1024 * 1024)
        } else {
            (self.ctx.config.storage.min_free_space_mb) * 1024 * 1024
        };
        if min_free_bytes == 0 {
            // 未配置阈值，跳过
            return Ok(());
        }

        let recordings_dir = self.recordings_dir().await?;
        // df / CIM 探测需要目录存在
        tokio::fs::create_dir_all(&recordings_dir).await.ok();

        let available = get_available_space(&recordings_dir).await?;
        if available >= min_free_bytes {
            // 空间充足：清除去重状态，下次告警可立即发送
            self.last_disk_alert_at.store(0, Ordering::Relaxed);
            return Ok(());
        }

        // 去重：同级别告警 1 小时内不重复
        let now = chrono::Utc::now().timestamp();
        let last = self.last_disk_alert_at.load(Ordering::Relaxed);
        if last > 0 && now - last < DEDUP_INTERVAL_SECS {
            return Ok(());
        }
        self.last_disk_alert_at.store(now, Ordering::Relaxed);

        let avail_gb = available as f64 / 1024_f64 / 1024_f64 / 1024_f64;
        let title = "磁盘空间不足".to_string();
        let message = format!(
            "录制目录剩余空间仅 {:.2} GB，低于设定的 {:.2} GB 阈值，请及时清理或扩容",
            avail_gb,
            min_free_bytes as f64 / 1024_f64 / 1024_f64 / 1024_f64
        );
        let details = serde_json::json!({
            "available_bytes": available,
            "threshold_bytes": min_free_bytes,
            "available_gb": (avail_gb * 100.0).round() / 100.0,
            "path": recordings_dir.to_string_lossy(),
        })
        .to_string();

        let svc = NotificationService::new(self.ctx.clone(), self.event_sender.clone());
        if let Err(e) = svc
            .notify(
                None,
                NotifyRequest {
                    category: category::DISK_WARNING.to_string(),
                    level: level::WARNING.to_string(),
                    title,
                    message,
                    details: Some(details),
                    task_id: None,
                },
            )
            .await
        {
            warn!("Failed to send disk warning notification: {}", e);
        }

        Ok(())
    }

    /// 读取录制目录（DB 优先，回退 config）
    async fn recordings_dir(&self) -> Result<std::path::PathBuf> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT value FROM system_config WHERE key = 'storage.recordings_path'")
                .fetch_optional(&self.ctx.db)
                .await?;
        if let Some((value,)) = row {
            if !value.trim().is_empty() {
                return Ok(std::path::PathBuf::from(value));
            }
        }
        Ok(self.ctx.config.storage.recordings_dir.clone())
    }

    async fn get_system_value<T: std::str::FromStr>(&self, key: &str, default: T) -> Result<T> {
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
}

/// 探测指定路径所在分区的可用空间（字节）。
/// 内联实现，避免依赖 recording 模块的私有函数。
async fn get_available_space(path: &Path) -> Result<u64> {
    #[cfg(windows)]
    {
        get_available_space_windows(path).await
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
    // 父目录可能尚不存在，逐级回退到一个存在的祖先
    let mut probe = path.to_path_buf();
    while !probe.exists() {
        if !probe.pop() {
            break;
        }
    }
    let probe_str = probe.to_string_lossy().replace('\'', "''");

    let script = format!(
        "(Get-CimInstance Win32_LogicalDisk -Filter \"DeviceID='$(Split-Path -Qualifier '{}')'\").FreeSpace",
        probe_str
    );

    let output = tokio::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .await?;

    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "PowerShell 查询可用空间失败: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let out = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if out.is_empty() {
        return Err(anyhow::anyhow!("PowerShell 返回空结果"));
    }
    let avail: u64 = out
        .parse()
        .map_err(|e| anyhow::anyhow!("解析可用空间失败: {}: '{}'", e, out))?;
    Ok(avail)
}
