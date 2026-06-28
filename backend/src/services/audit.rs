//! 审计日志服务

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::models::{AuditLog, SystemHealth};

use super::{Claims, PaginationParams, ServiceContext};

pub struct AuditService {
    ctx: ServiceContext,
}

#[derive(Debug, FromRow)]
struct CountRow {
    total: i64,
}

/// 分页审计日志响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedAuditLogs {
    pub items: Vec<AuditLog>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub total_pages: i64,
}

impl AuditService {
    pub fn new(ctx: ServiceContext) -> Self {
        Self { ctx }
    }

    pub async fn record(
        &self,
        claims: Option<&Claims>,
        action: &str,
        resource_type: &str,
        resource_id: Option<&str>,
        details: Option<&str>,
    ) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO audit_logs
            (id, user_id, username, role, action, resource_type, resource_id, details, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(claims.map(|c| c.sub.clone()))
        .bind(claims.map(|c| c.username.clone()))
        .bind(claims.map(|c| c.role.clone()))
        .bind(action)
        .bind(resource_type)
        .bind(resource_id)
        .bind(details)
        .bind(now)
        .execute(&self.ctx.db)
        .await?;

        Ok(())
    }

    /// 分页查询审计日志（最新在前），与频道分页约定一致：
    /// page 默认 1，page_size 默认 20，clamp 到 [1, 100]。
    pub async fn list_paginated(&self, params: PaginationParams) -> Result<PaginatedAuditLogs> {
        let page = params.page.unwrap_or(1).max(1);
        let page_size = params.page_size.unwrap_or(20).clamp(1, 100);
        let offset = (page - 1) * page_size;

        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_logs")
            .fetch_one(&self.ctx.db)
            .await?;

        let items = sqlx::query_as::<_, AuditLog>(
            "SELECT * FROM audit_logs ORDER BY created_at DESC LIMIT ? OFFSET ?",
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

        Ok(PaginatedAuditLogs {
            items,
            total,
            page,
            page_size,
            total_pages,
        })
    }

    pub async fn system_health(&self) -> Result<SystemHealth> {
        let users_total = count(&self.ctx, "SELECT COUNT(*) as total FROM users").await?;
        let channels_total = count(&self.ctx, "SELECT COUNT(*) as total FROM channels").await?;
        let schedules_total = count(&self.ctx, "SELECT COUNT(*) as total FROM schedules").await?;
        let enabled_schedules = count(
            &self.ctx,
            "SELECT COUNT(*) as total FROM schedules WHERE enabled = 1",
        )
        .await?;
        let running_tasks = count(
            &self.ctx,
            "SELECT COUNT(*) as total FROM tasks WHERE status = 'running'",
        )
        .await?;
        let failed_tasks_24h = count(
            &self.ctx,
            "SELECT COUNT(*) as total FROM tasks WHERE status = 'failed' AND updated_at >= datetime('now', '-1 day')",
        )
        .await?;
        let last_audit_at: Option<(String,)> =
            sqlx::query_as("SELECT created_at FROM audit_logs ORDER BY created_at DESC LIMIT 1")
                .fetch_optional(&self.ctx.db)
                .await?;

        Ok(SystemHealth {
            users_total,
            channels_total,
            schedules_total,
            enabled_schedules,
            running_tasks,
            failed_tasks_24h,
            last_audit_at: last_audit_at.map(|row| row.0),
        })
    }
}

async fn count(ctx: &ServiceContext, query: &str) -> Result<i64> {
    let row = sqlx::query_as::<_, CountRow>(query)
        .fetch_one(&ctx.db)
        .await?;
    Ok(row.total)
}
