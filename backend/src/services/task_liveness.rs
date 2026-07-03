//! 录制任务僵尸巡检服务
//!
//! 仿 `HeartbeatService` / `CleanupService` 的常驻后台任务，与录制主流程解耦，
//! 独立 `loop + sleep` 运行。
//!
//! 职责：检测"运行中但长时间无进度"的僵尸录制任务并自动清理为失败。
//!
//! ## 检测原理
//! 正常录制有一个后台监控任务每 3 秒更新 `tasks.updated_at`。当该监控任务因
//! 任何原因（后端重启后残留、监控协程 panic、进程异常）停止工作时，`updated_at`
//! 不再前进。本服务用 `updated_at` 停滞时间 > 阈值 作为"监控丢失"信号——
//! 误判风险极低，因为只有监控任务彻底死亡才会触发（正常录制即便短暂卡顿，
//! 监控任务仍在跑，updated_at 仍前进）。
//!
//! ## 与启动恢复的关系
//! `RecordingService::reconcile_orphaned_tasks()` 负责启动时刻的一次性清理（后端
//! 重启残留）。本服务负责运行期的持续巡检，覆盖监控协程运行中崩溃等场景。

use anyhow::Result;
use std::{sync::Arc, time::Duration};
use tracing::{error, info, warn};

use crate::core::event::{
    Event, EventSender, TaskStatus as EventTaskStatus, TaskUpdateEvent,
};

use super::{
    notification::{category as notif_cat, level as notif_lvl, NotificationService, NotifyRequest},
    ServiceContext,
};

/// 巡检间隔：60 秒
const INSPECT_INTERVAL_SECS: u64 = 60;
/// 僵尸阈值配置键（存于 system_config，可在设置页调整）
const STALE_TIMEOUT_KEY: &str = "task_stale_timeout_secs";
/// 阈值兜底默认值（秒）
const DEFAULT_STALE_TIMEOUT_SECS: i64 = 90;
/// 阈值下限：低于此值视为无效，回退默认。避免误配成 0/负数导致正常录制被误杀。
const MIN_STALE_TIMEOUT_SECS: i64 = 30;

pub struct TaskLivenessService {
    ctx: ServiceContext,
    event_sender: Option<EventSender>,
}

impl TaskLivenessService {
    pub fn new(ctx: ServiceContext, event_sender: Option<EventSender>) -> Self {
        Self {
            ctx,
            event_sender,
        }
    }

    /// 启动常驻后台巡检循环
    pub fn start(self: Arc<Self>) {
        tokio::spawn(async move {
            info!(
                "🧟‍♂️ Task liveness inspector started (every {}s)",
                INSPECT_INTERVAL_SECS
            );
            loop {
                // 启动后先等一个周期再首次巡检，给刚启动的录制监控任务一点时间
                // 写入首个 updated_at，避免误判刚启动的健康任务。
                tokio::time::sleep(Duration::from_secs(INSPECT_INTERVAL_SECS)).await;
                if let Err(e) = self.run_once().await {
                    error!("Task liveness inspection failed: {}", e);
                }
            }
        });
    }

    async fn run_once(&self) -> Result<()> {
        let stale_secs = self.read_stale_timeout().await;
        // 命中僵尸条件的任务（id, channel_id）
        // 用 unixepoch 比较，避免 RFC3339 字符串解析开销。
        // updated_at 由 recording.rs 写入，格式是 RFC3339（含时区），SQLite 的
        // unixepoch() 能正确解析带时区的 ISO8601 字符串。
        let zombies: Vec<(String, String)> = sqlx::query_as(
            r#"
            SELECT id, channel_id FROM tasks
            WHERE status = 'running'
              AND unixepoch(updated_at) < unixepoch('now') - ?
            "#,
        )
        .bind(stale_secs)
        .fetch_all(&self.ctx.db)
        .await?;

        if zombies.is_empty() {
            return Ok(());
        }

        warn!(
            "发现 {} 个僵尸录制任务（updated_at 停滞超过 {}s），开始清理",
            zombies.len(),
            stale_secs
        );

        let now = chrono::Utc::now().to_rfc3339();
        let reason = format!(
            "录制长时间无进度（{} 秒未更新），判定为僵死，已自动标记为失败",
            stale_secs
        );

        for (task_id, channel_id) in &zombies {
            // 用 WHERE id=? AND status='running' 守卫，防止与 cancel() / 监控任务的
            // 正常终态写回产生竞态。若该任务刚好正常结束/被取消，rows_affected=0，跳过。
            let result = sqlx::query(
                r#"
                UPDATE tasks
                SET status = 'failed', ended_at = ?, error_message = ?, updated_at = ?
                WHERE id = ? AND status = 'running'
                "#,
            )
            .bind(&now)
            .bind(&reason)
            .bind(&now)
            .bind(task_id)
            .execute(&self.ctx.db)
            .await?;

            if result.rows_affected() == 0 {
                // 任务已在这期间进入其它终态，不重复处理
                continue;
            }

            info!("僵尸任务已清理: task_id={}", task_id);

            // 推 WebSocket 事件，前端任务列表实时变红
            if let Some(ref sender) = self.event_sender {
                let _ = sender.send(Event::TaskUpdate(TaskUpdateEvent {
                    task_id: task_id.clone(),
                    status: EventTaskStatus::Failed,
                    error_message: Some(reason.clone()),
                }));
            }

            // 频道名（通知文案用）
            let channel_name =
                sqlx::query_scalar::<_, String>("SELECT name FROM channels WHERE id = ?")
                    .bind(channel_id)
                    .fetch_optional(&self.ctx.db)
                    .await
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| "未知频道".to_string());

            // 通知：录制僵死（受 on_failure 开关控制，与正常失败一致）
            let svc = NotificationService::new(self.ctx.clone(), self.event_sender.clone());
            if let Err(e) = svc
                .notify(
                    Some("notification.on_failure"),
                    NotifyRequest {
                        category: notif_cat::RECORDING_FAILED.to_string(),
                        level: notif_lvl::WARNING.to_string(),
                        title: format!("录制异常中断: {}", channel_name),
                        message: format!(
                            "频道「{}」的录制任务长时间无进度，已自动判定为失败。可能原因：录制进程崩溃、网络中断或后端异常",
                            channel_name
                        ),
                        details: Some(
                            serde_json::json!({
                                "channel": channel_name,
                                "reason": "stale_no_progress",
                                "stale_seconds": stale_secs,
                            })
                            .to_string(),
                        ),
                        task_id: Some(task_id.clone()),
                    },
                )
                .await
            {
                warn!("发送僵尸任务通知失败: {}", e);
            }
        }

        Ok(())
    }

    /// 读取僵尸阈值（秒），带兜底与下限保护。
    async fn read_stale_timeout(&self) -> i64 {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT value FROM system_config WHERE key = ?")
                .bind(STALE_TIMEOUT_KEY)
                .fetch_optional(&self.ctx.db)
                .await
                .ok()
                .flatten();

        let parsed = row
            .and_then(|(v,)| v.trim().parse::<i64>().ok())
            .filter(|&v| v >= MIN_STALE_TIMEOUT_SECS);

        parsed.unwrap_or(DEFAULT_STALE_TIMEOUT_SECS)
    }
}
