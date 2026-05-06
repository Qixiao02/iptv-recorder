//! 频道管理服务

use crate::{
    models::{Channel, CreateChannelRequest},
    services::ServiceContext,
};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

/// 频道测试结果
#[derive(Debug, Serialize, Deserialize)]
pub struct ChannelTestResult {
    pub channel_id: String,
    pub status: String,
    pub response_time_ms: Option<u64>,
    pub error: Option<String>,
}

/// 分页频道列表
#[derive(Debug, Serialize, Deserialize)]
pub struct PaginatedChannels {
    pub items: Vec<Channel>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub total_pages: i64,
}

/// 分页查询参数
#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    pub group: Option<String>,
    pub search: Option<String>,
}

pub struct ChannelService {
    ctx: ServiceContext,
}

pub enum ImportChannelResult {
    Created,
    Updated,
    Skipped,
}

impl ChannelService {
    pub fn new(ctx: ServiceContext) -> Self {
        Self { ctx }
    }

    /// 创建频道
    pub async fn create(&self, req: CreateChannelRequest) -> Result<Channel> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        let group_name = if req.group_name.is_empty() {
            "Uncategorized".to_string()
        } else {
            req.group_name
        };

        sqlx::query(
            r#"
            INSERT INTO channels (id, name, url, group_name, logo_url, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(&req.name)
        .bind(&req.url)
        .bind(&group_name)
        .bind(&req.logo_url)
        .bind(&now)
        .bind(&now)
        .execute(&self.ctx.db)
        .await?;

        self.get_by_id(&id).await
    }

    /// 根据 ID 获取频道
    pub async fn get_by_id(&self, id: &str) -> Result<Channel> {
        let row = sqlx::query_as::<_, Channel>(
            "SELECT * FROM channels WHERE id = ?"
        )
        .bind(id)
        .fetch_one(&self.ctx.db)
        .await?;

        Ok(row)
    }

    /// 根据 URL 获取频道
    pub async fn get_by_url(&self, url: &str) -> Result<Option<Channel>> {
        let channel = sqlx::query_as::<_, Channel>(
            "SELECT * FROM channels WHERE url = ? LIMIT 1"
        )
        .bind(url)
        .fetch_optional(&self.ctx.db)
        .await?;

        Ok(channel)
    }

    /// 获取所有频道
    #[allow(dead_code)]
    pub async fn list(&self) -> Result<Vec<Channel>> {
        let channels = sqlx::query_as::<_, Channel>(
            "SELECT * FROM channels ORDER BY group_name, name"
        )
        .fetch_all(&self.ctx.db)
        .await?;

        Ok(channels)
    }

    /// 分页获取频道
    pub async fn list_paginated(&self, params: PaginationParams) -> Result<PaginatedChannels> {
        let page = params.page.unwrap_or(1).max(1);
        let page_size = params.page_size.unwrap_or(20).min(100).max(1);
        let offset = (page - 1) * page_size;

        // 构建查询条件
        let mut where_clauses = Vec::new();
        let mut bind_values: Vec<String> = Vec::new();

        if let Some(group) = &params.group {
            if group != "all" && !group.is_empty() {
                where_clauses.push("group_name = ?");
                bind_values.push(group.clone());
            }
        }

        if let Some(search) = &params.search {
            if !search.is_empty() {
                where_clauses.push("name LIKE ?");
                bind_values.push(format!("%{}%", search));
            }
        }

        let where_sql = if where_clauses.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", where_clauses.join(" AND "))
        };

        // 查询总数
        let count_sql = format!("SELECT COUNT(*) as count FROM channels {}", where_sql);
        let mut count_query = sqlx::query(&count_sql);
        for value in &bind_values {
            count_query = count_query.bind(value);
        }
        let total: i64 = count_query
            .fetch_one(&self.ctx.db)
            .await?
            .get("count");

        // 查询分页数据
        let data_sql = format!(
            "SELECT * FROM channels {} ORDER BY group_name, name LIMIT ? OFFSET ?",
            where_sql
        );
        let mut data_query = sqlx::query_as::<_, Channel>(&data_sql);
        for value in &bind_values {
            data_query = data_query.bind(value);
        }
        let channels = data_query
            .bind(page_size)
            .bind(offset)
            .fetch_all(&self.ctx.db)
            .await?;

        let total_pages = (total + page_size - 1) / page_size;

        Ok(PaginatedChannels {
            items: channels,
            total,
            page,
            page_size,
            total_pages,
        })
    }

    /// 按分组获取频道
    #[allow(dead_code)]
    pub async fn list_by_group(&self, group: &str) -> Result<Vec<Channel>> {
        let channels = sqlx::query_as::<_, Channel>(
            "SELECT * FROM channels WHERE group_name = ? ORDER BY name"
        )
        .bind(group)
        .fetch_all(&self.ctx.db)
        .await?;

        Ok(channels)
    }

    /// 获取所有分组
    pub async fn list_groups(&self) -> Result<Vec<String>> {
        let rows = sqlx::query(
            "SELECT DISTINCT group_name FROM channels ORDER BY group_name"
        )
        .fetch_all(&self.ctx.db)
        .await?;

        let groups = rows.iter()
            .filter_map(|r| r.try_get("group_name").ok())
            .collect();

        Ok(groups)
    }

    /// 更新频道
    pub async fn update(&self, id: &str, req: CreateChannelRequest) -> Result<Channel> {
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            UPDATE channels
            SET name = ?, url = ?, group_name = ?, logo_url = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&req.name)
        .bind(&req.url)
        .bind(&req.group_name)
        .bind(&req.logo_url)
        .bind(&now)
        .bind(id)
        .execute(&self.ctx.db)
        .await?;

        self.get_by_id(id).await
    }

    /// 导入频道，支持覆盖现有频道
    pub async fn import_channel(&self, req: CreateChannelRequest, overwrite: bool) -> Result<ImportChannelResult> {
        if let Some(existing) = self.get_by_url(&req.url).await? {
            if overwrite {
                self.update(&existing.id, req).await?;
                Ok(ImportChannelResult::Updated)
            } else {
                Ok(ImportChannelResult::Skipped)
            }
        } else {
            self.create(req).await?;
            Ok(ImportChannelResult::Created)
        }
    }

    /// 删除频道
    pub async fn delete(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM channels WHERE id = ?")
            .bind(id)
            .execute(&self.ctx.db)
            .await?;

        Ok(())
    }

    /// 更新频道状态
    #[allow(dead_code)]
    pub async fn update_status(&self, id: &str, status: &str) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            "UPDATE channels SET status = ?, last_check_at = ? WHERE id = ?"
        )
        .bind(status)
        .bind(&now)
        .bind(id)
        .execute(&self.ctx.db)
        .await?;

        Ok(())
    }

    /// 测试频道连接
    pub async fn test_channel(&self, id: &str) -> Result<ChannelTestResult> {
        let channel = self.get_by_id(id).await?;
        let url = channel.url.clone();

        let start = std::time::Instant::now();
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .danger_accept_invalid_certs(true)
            .build()?;

        let result = client
            .head(&url)
            .send()
            .await;

        let response_time_ms = Some(start.elapsed().as_millis() as u64);

        match result {
            Ok(resp) => {
                let status = if resp.status().is_success() {
                    "online"
                } else if resp.status().is_client_error() {
                    // 某些服务器不支持 HEAD，尝试 GET
                    let get_result = client
                        .get(&url)
                        .timeout(std::time::Duration::from_secs(10))
                        .send()
                        .await;

                    match get_result {
                        Ok(r) => if r.status().is_success() { "online" } else { "offline" },
                        Err(_) => "offline",
                    }
                } else {
                    "offline"
                };

                // 更新数据库状态
                let _ = self.update_status(id, status).await;

                Ok(ChannelTestResult {
                    channel_id: id.to_string(),
                    status: status.to_string(),
                    response_time_ms,
                    error: None,
                })
            }
            Err(e) => {
                // 更新数据库状态
                let _ = self.update_status(id, "offline").await;

                Ok(ChannelTestResult {
                    channel_id: id.to_string(),
                    status: "offline".to_string(),
                    response_time_ms,
                    error: Some(e.to_string()),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::Config, core::database};
    use std::{path::PathBuf, time::{SystemTime, UNIX_EPOCH}};

    fn temp_db_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        std::env::temp_dir().join(format!("iptv-recorder-{name}-{nanos}.db"))
    }

    async fn test_service(name: &str) -> (ChannelService, PathBuf) {
        let db_path = temp_db_path(name);
        let db = database::init(db_path.to_str().expect("utf8 path"), 1)
            .await
            .expect("db init");
        let service = ChannelService::new(ServiceContext::new(db, Config::default()));
        (service, db_path)
    }

    #[tokio::test]
    async fn import_channel_skips_existing_when_overwrite_disabled() {
        let (service, db_path) = test_service("channel-skip").await;
        let original = CreateChannelRequest {
            name: "CCTV-1".to_string(),
            url: "http://example.com/live.m3u8".to_string(),
            group_name: "央视".to_string(),
            logo_url: None,
        };
        service.create(original).await.expect("create channel");

        let duplicate = CreateChannelRequest {
            name: "CCTV-1 HD".to_string(),
            url: "http://example.com/live.m3u8".to_string(),
            group_name: "高清".to_string(),
            logo_url: Some("http://example.com/logo.png".to_string()),
        };

        let result = service.import_channel(duplicate, false).await.expect("import channel");
        assert!(matches!(result, ImportChannelResult::Skipped));

        let stored = service
            .get_by_url("http://example.com/live.m3u8")
            .await
            .expect("query by url")
            .expect("existing channel");
        assert_eq!(stored.name, "CCTV-1");
        assert_eq!(stored.group_name, "央视");

        let _ = tokio::fs::remove_file(db_path).await;
    }

    #[tokio::test]
    async fn import_channel_updates_existing_when_overwrite_enabled() {
        let (service, db_path) = test_service("channel-overwrite").await;
        service.create(CreateChannelRequest {
            name: "CCTV-1".to_string(),
            url: "http://example.com/live.m3u8".to_string(),
            group_name: "央视".to_string(),
            logo_url: None,
        }).await.expect("create channel");

        let result = service.import_channel(CreateChannelRequest {
            name: "CCTV-1 HD".to_string(),
            url: "http://example.com/live.m3u8".to_string(),
            group_name: "高清".to_string(),
            logo_url: Some("http://example.com/logo.png".to_string()),
        }, true).await.expect("import overwrite");

        assert!(matches!(result, ImportChannelResult::Updated));

        let stored = service
            .get_by_url("http://example.com/live.m3u8")
            .await
            .expect("query by url")
            .expect("existing channel");
        assert_eq!(stored.name, "CCTV-1 HD");
        assert_eq!(stored.group_name, "高清");
        assert_eq!(stored.logo_url.as_deref(), Some("http://example.com/logo.png"));

        let _ = tokio::fs::remove_file(db_path).await;
    }
}
