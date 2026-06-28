//! 应用内通知服务
//!
//! 持久化通知到 `notifications` 表，并通过事件总线实时推送给前端。
//! 区别于一次性的 `SystemAlert`（fire-and-forget），这里的通知会落库、
//! 可标记已读、可分页查询历史，是真正意义上的"通知中心"。

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::warn;
use uuid::Uuid;

use crate::core::event::{Event, EventSender, NotificationEvent};
use crate::models::Notification;

use super::ServiceContext;

/// 通知类别常量
pub mod category {
    pub const RECORDING_COMPLETE: &str = "recording_complete";
    pub const RECORDING_FAILED: &str = "recording_failed";
    pub const DISK_WARNING: &str = "disk_warning";
    pub const SYSTEM: &str = "system";
}

/// 通知级别常量
pub mod level {
    pub const INFO: &str = "info";
    pub const WARNING: &str = "warning";
    pub const ERROR: &str = "error";
}

/// 分页查询参数（与前端 PaginationParams 对齐）
#[derive(Debug, Clone, Deserialize)]
pub struct NotificationPaginationParams {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

/// 分页响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedNotifications {
    pub items: Vec<Notification>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub total_pages: i64,
}

/// 未读数响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnreadCount {
    pub count: i64,
}

/// 通知创建请求（内部调用，不直接对外）
#[derive(Debug, Clone)]
pub struct NotifyRequest {
    pub category: String,
    pub level: String,
    pub title: String,
    pub message: String,
    /// 任意 JSON 字符串，存放扩展信息（任务ID/文件大小/磁盘数值等）
    pub details: Option<String>,
    pub task_id: Option<String>,
}

pub struct NotificationService {
    ctx: ServiceContext,
    event_sender: Option<EventSender>,
}

impl NotificationService {
    pub fn new(ctx: ServiceContext, event_sender: Option<EventSender>) -> Self {
        Self { ctx, event_sender }
    }

    /// 发送一条通知：写库 + 实时推送。
    ///
    /// `config_key` 用于按用户开关过滤（`notification.on_complete` 等）；
    /// 传 `None` 表示不受开关控制（如磁盘告警单独由 `disk_warning` 开关在外部判断）。
    pub async fn notify(
        &self,
        config_key: Option<&str>,
        req: NotifyRequest,
    ) -> Result<Notification> {
        // 按开关过滤：若提供了 config_key 且对应开关为关闭，则直接跳过
        if let Some(key) = config_key {
            let enabled = self.get_system_value::<bool>(key, true).await?;
            if !enabled {
                return Ok(Notification {
                    id: String::new(),
                    category: req.category,
                    level: req.level,
                    title: req.title,
                    message: req.message,
                    details: req.details,
                    task_id: req.task_id,
                    is_read: false,
                    created_at: chrono::Utc::now().to_rfc3339(),
                });
            }
        }

        let id = Uuid::new_v4().to_string();
        let created_at = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO notifications (id, category, level, title, message, details, task_id, read, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, 0, ?)
            "#,
        )
        .bind(&id)
        .bind(&req.category)
        .bind(&req.level)
        .bind(&req.title)
        .bind(&req.message)
        .bind(&req.details)
        .bind(&req.task_id)
        .bind(&created_at)
        .execute(&self.ctx.db)
        .await?;

        let notification = Notification {
            id: id.clone(),
            category: req.category,
            level: req.level,
            title: req.title,
            message: req.message,
            details: req.details,
            task_id: req.task_id,
            is_read: false,
            created_at: created_at.clone(),
        };

        // 实时推送给已连接的前端
        if let Some(sender) = &self.event_sender {
            let _ = sender.send(Event::Notification(NotificationEvent {
                id,
                category: notification.category.clone(),
                level: notification.level.clone(),
                title: notification.title.clone(),
                message: notification.message.clone(),
                details: notification.details.clone(),
                task_id: notification.task_id.clone(),
                created_at,
            }));
        }

        Ok(notification)
    }

    /// 分页查询通知（最新在前）
    pub async fn list_paginated(
        &self,
        params: NotificationPaginationParams,
    ) -> Result<PaginatedNotifications> {
        let page = params.page.unwrap_or(1).max(1);
        let page_size = params.page_size.unwrap_or(20).clamp(1, 100);
        let offset = (page - 1) * page_size;

        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM notifications")
            .fetch_one(&self.ctx.db)
            .await?;

        let items = sqlx::query_as::<_, Notification>(
            "SELECT * FROM notifications ORDER BY created_at DESC LIMIT ? OFFSET ?",
        )
        .bind(page_size)
        .bind(offset)
        .fetch_all(&self.ctx.db)
        .await?;

        let total_pages = if page_size == 0 {
            0
        } else {
            (total + page_size - 1) / page_size
        };

        Ok(PaginatedNotifications {
            items,
            total,
            page,
            page_size,
            total_pages,
        })
    }

    /// 未读数量
    pub async fn unread_count(&self) -> Result<UnreadCount> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM notifications WHERE read = 0")
            .fetch_one(&self.ctx.db)
            .await?;
        Ok(UnreadCount { count })
    }

    /// 标记单条已读，返回是否找到并更新
    pub async fn mark_read(&self, id: &str) -> Result<bool> {
        let result = sqlx::query("UPDATE notifications SET read = 1 WHERE id = ? AND read = 0")
            .bind(id)
            .execute(&self.ctx.db)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// 全部标记已读，返回更新条数
    pub async fn mark_all_read(&self) -> Result<u64> {
        let result = sqlx::query("UPDATE notifications SET read = 1 WHERE read = 0")
            .execute(&self.ctx.db)
            .await?;
        Ok(result.rows_affected())
    }

    /// 删除单条
    pub async fn delete(&self, id: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM notifications WHERE id = ?")
            .bind(id)
            .execute(&self.ctx.db)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// 读取 system_config 中的布尔值（与 recording.rs / config_service.rs 同款实现）
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

    /// 兜底：吞掉通知发送错误，避免影响调用方主流程
    #[allow(dead_code)]
    pub async fn notify_or_warn(&self, config_key: Option<&str>, req: NotifyRequest) {
        if let Err(e) = self.notify(config_key, req).await {
            warn!("Failed to persist notification: {}", e);
        }
    }
}
