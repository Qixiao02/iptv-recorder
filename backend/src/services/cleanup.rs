//! 自动清理服务

use anyhow::Result;
use std::{path::PathBuf, sync::Arc, time::Duration};
use tracing::{error, info, warn};

use crate::{
    core::event::{AlertLevel, Event, EventSender, SystemAlertEvent},
};

use super::ServiceContext;

pub struct CleanupService {
    ctx: ServiceContext,
    event_sender: Option<EventSender>,
}

impl CleanupService {
    pub fn new(ctx: ServiceContext, event_sender: Option<EventSender>) -> Self {
        Self { ctx, event_sender }
    }

    pub fn start(self: Arc<Self>) {
        tokio::spawn(async move {
            loop {
                if let Err(e) = self.run_once().await {
                    error!("Cleanup task failed: {}", e);
                    self.send_alert(AlertLevel::Error, "自动清理失败", Some(e.to_string()));
                }
                tokio::time::sleep(Duration::from_secs(3600)).await;
            }
        });
    }

    pub async fn run_once(&self) -> Result<u64> {
        let retention_days = self.retention_days().await?;
        let candidates: Vec<(String, Option<String>)> = sqlx::query_as(
            r#"
            SELECT id, output_path
            FROM tasks
            WHERE status IN ('completed', 'failed', 'cancelled')
              AND COALESCE(ended_at, updated_at, created_at) < datetime('now', printf('-%d day', ?))
            "#,
        )
        .bind(retention_days)
        .fetch_all(&self.ctx.db)
        .await?;

        for (_task_id, output_path) in &candidates {
            if let Some(path) = output_path {
                self.remove_recording_file(path).await;
            }
        }

        let result = sqlx::query(
            r#"
            DELETE FROM tasks
            WHERE status IN ('completed', 'failed', 'cancelled')
              AND COALESCE(ended_at, updated_at, created_at) < datetime('now', printf('-%d day', ?))
            "#,
        )
        .bind(retention_days)
        .execute(&self.ctx.db)
        .await?;

        let deleted = result.rows_affected();
        if deleted > 0 {
            info!("Auto cleanup removed {} expired task records", deleted);
            self.send_alert(
                AlertLevel::Info,
                "自动清理完成",
                Some(format!("已清理 {} 条过期任务记录", deleted)),
            );
        }

        Ok(deleted)
    }

    async fn retention_days(&self) -> Result<u32> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT value FROM system_config WHERE key = 'storage.auto_cleanup_days'",
        )
        .fetch_optional(&self.ctx.db)
        .await?;

        Ok(row
            .and_then(|(value,)| value.parse::<u32>().ok())
            .unwrap_or(self.ctx.config.scheduler.task_retention_days))
    }

    async fn remove_recording_file(&self, output_path: &str) {
        let path = PathBuf::from(output_path);
        if !path.exists() {
            return;
        }

        match tokio::fs::remove_file(&path).await {
            Ok(_) => {
                info!("Removed expired recording file {}", path.display());
            }
            Err(e) => {
                warn!("Failed to remove expired recording file {}: {}", path.display(), e);
                self.send_alert(
                    AlertLevel::Warning,
                    "删除过期录制文件失败",
                    Some(format!("{}: {}", path.display(), e)),
                );
            }
        }
    }

    fn send_alert(&self, level: AlertLevel, message: &str, details: Option<String>) {
        if let Some(sender) = &self.event_sender {
            let _ = sender.send(Event::SystemAlert(SystemAlertEvent {
                level,
                message: message.to_string(),
                details,
            }));
        }
    }
}

impl From<(ServiceContext, Option<EventSender>)> for CleanupService {
    fn from((ctx, event_sender): (ServiceContext, Option<EventSender>)) -> Self {
        Self::new(ctx, event_sender)
    }
}
