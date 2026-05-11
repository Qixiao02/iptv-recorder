//! 审计日志服务

use anyhow::Result;
use sqlx::FromRow;
use uuid::Uuid;

use crate::models::{AuditLog, SystemHealth};

use super::{Claims, ServiceContext};

pub struct AuditService {
    ctx: ServiceContext,
}

#[derive(Debug, FromRow)]
struct CountRow {
    total: i64,
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

    pub async fn list_recent(&self, limit: i64) -> Result<Vec<AuditLog>> {
        let logs = sqlx::query_as::<_, AuditLog>(
            "SELECT * FROM audit_logs ORDER BY created_at DESC LIMIT ?",
        )
        .bind(limit.max(1))
        .fetch_all(&self.ctx.db)
        .await?;

        Ok(logs)
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
