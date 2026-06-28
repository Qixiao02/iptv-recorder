//! 频道管理服务

use crate::{
    models::{Channel, CreateChannelRequest},
    services::{url_safety::assert_safe_url, ServiceContext},
};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::{QueryBuilder, Row, Sqlite};
use std::net::IpAddr;
use url::Url;
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

#[derive(Debug, Default)]
pub struct ImportBatchResult {
    pub imported: usize,
    pub skipped: usize,
    pub failed: usize,
    pub errors: Vec<String>,
}

impl ChannelService {
    pub fn new(ctx: ServiceContext) -> Self {
        Self { ctx }
    }

    /// 创建频道
    pub async fn create(&self, req: CreateChannelRequest) -> Result<Channel> {
        let req = normalize_channel_request(req);
        // SSRF 校验:私有服务器频道(内网源)跳过严格校验,其它做完整校验(含 DNS 解析)
        assert_channel_url_safe(&req).await?;
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO channels (id, name, url, group_name, logo_url, source_visibility, playback_strategy, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(&req.name)
        .bind(&req.url)
        .bind(&req.group_name)
        .bind(&req.logo_url)
        .bind(&req.source_visibility)
        .bind(&req.playback_strategy)
        .bind(&now)
        .bind(&now)
        .execute(&self.ctx.db)
        .await?;

        self.get_by_id(&id).await
    }

    /// 根据 ID 获取频道
    pub async fn get_by_id(&self, id: &str) -> Result<Channel> {
        let row = sqlx::query_as::<_, Channel>("SELECT * FROM channels WHERE id = ?")
            .bind(id)
            .fetch_one(&self.ctx.db)
            .await?;

        Ok(row)
    }

    /// 获取所有频道
    #[allow(dead_code)]
    pub async fn list(&self) -> Result<Vec<Channel>> {
        let channels =
            sqlx::query_as::<_, Channel>("SELECT * FROM channels ORDER BY group_name, name")
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
        let total: i64 = count_query.fetch_one(&self.ctx.db).await?.get("count");

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
            "SELECT * FROM channels WHERE group_name = ? ORDER BY name",
        )
        .bind(group)
        .fetch_all(&self.ctx.db)
        .await?;

        Ok(channels)
    }

    /// 获取所有分组
    pub async fn list_groups(&self) -> Result<Vec<String>> {
        let rows = sqlx::query("SELECT DISTINCT group_name FROM channels ORDER BY group_name")
            .fetch_all(&self.ctx.db)
            .await?;

        let groups = rows
            .iter()
            .filter_map(|r| r.try_get("group_name").ok())
            .collect();

        Ok(groups)
    }

    /// 更新频道
    pub async fn update(&self, id: &str, req: CreateChannelRequest) -> Result<Channel> {
        let req = normalize_channel_request(req);
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            UPDATE channels
            SET name = ?, url = ?, group_name = ?, logo_url = ?, source_visibility = ?, playback_strategy = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&req.name)
        .bind(&req.url)
        .bind(&req.group_name)
        .bind(&req.logo_url)
        .bind(&req.source_visibility)
        .bind(&req.playback_strategy)
        .bind(&now)
        .bind(id)
        .execute(&self.ctx.db)
        .await?;

        self.get_by_id(id).await
    }

    /// 批量导入频道，避免逐条查重和逐条提交导致导入慢。
    pub async fn import_channels_batch(
        &self,
        requests: Vec<CreateChannelRequest>,
        overwrite: bool,
    ) -> Result<ImportBatchResult> {
        if requests.is_empty() {
            return Ok(ImportBatchResult::default());
        }

        let requests: Vec<CreateChannelRequest> = requests
            .into_iter()
            .map(normalize_channel_request)
            .collect();
        let existing_by_url = self.find_existing_ids_by_url(&requests).await?;
        let mut seen_urls = std::collections::HashSet::new();
        let now = chrono::Utc::now().to_rfc3339();
        let mut tx = self.ctx.db.begin().await?;
        let mut result = ImportBatchResult::default();

        for req in requests {
            if !seen_urls.insert(req.url.clone()) {
                result.skipped += 1;
                result.errors.push(format!(
                    "频道 {} 与本次导入中的其他频道使用了相同流地址，已跳过",
                    req.name
                ));
                continue;
            }

            // 批量导入用轻量 SSRF 校验(同步,不做 DNS 解析,避免批量时阻塞过久)
            if let Err(e) = assert_channel_url_safe_light(&req) {
                result.failed += 1;
                result.errors.push(format!("频道 {} 地址不安全,已跳过: {}", req.name, e));
                continue;
            }

            if let Some(existing_id) = existing_by_url.get(&req.url) {
                if overwrite {
                    let updated = sqlx::query(
                        r#"
                        UPDATE channels
                        SET name = ?, url = ?, group_name = ?, logo_url = ?, source_visibility = ?, playback_strategy = ?, updated_at = ?
                        WHERE id = ?
                        "#,
                    )
                    .bind(&req.name)
                    .bind(&req.url)
                    .bind(&req.group_name)
                    .bind(&req.logo_url)
                    .bind(&req.source_visibility)
                    .bind(&req.playback_strategy)
                    .bind(&now)
                    .bind(existing_id)
                    .execute(&mut *tx)
                    .await;

                    match updated {
                        Ok(_) => result.imported += 1,
                        Err(e) => {
                            result.failed += 1;
                            if is_unique_constraint_error(&e) {
                                result.errors.push(format!("频道 URL 已存在：{}", req.name));
                            } else {
                                result.errors.push(format!(
                                    "频道 {} 保存失败，请检查频道名称和流地址后重试（{}）",
                                    req.name,
                                    humanize_database_error(&e)
                                ));
                            }
                        }
                    }
                } else {
                    result.skipped += 1;
                    result
                        .errors
                        .push(format!("频道 {} 已存在，已跳过", req.name));
                }
            } else {
                let id = Uuid::new_v4().to_string();
                let inserted = sqlx::query(
                    r#"
                    INSERT INTO channels (id, name, url, group_name, logo_url, source_visibility, playback_strategy, created_at, updated_at)
                    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                    "#,
                )
                .bind(&id)
                .bind(&req.name)
                .bind(&req.url)
                .bind(&req.group_name)
                .bind(&req.logo_url)
                .bind(&req.source_visibility)
                .bind(&req.playback_strategy)
                .bind(&now)
                .bind(&now)
                .execute(&mut *tx)
                .await;

                match inserted {
                    Ok(_) => result.imported += 1,
                    Err(e) => {
                        result.failed += 1;
                        if is_unique_constraint_error(&e) {
                            result.errors.push(format!("频道 URL 已存在：{}", req.name));
                        } else {
                            result.errors.push(format!(
                                "频道 {} 保存失败，请检查频道名称和流地址后重试（{}）",
                                req.name,
                                humanize_database_error(&e)
                            ));
                        }
                    }
                }
            }
        }

        tx.commit().await?;
        Ok(result)
    }

    async fn find_existing_ids_by_url(
        &self,
        requests: &[CreateChannelRequest],
    ) -> Result<std::collections::HashMap<String, String>> {
        let mut urls: Vec<&str> = requests.iter().map(|req| req.url.as_str()).collect();
        urls.sort_unstable();
        urls.dedup();

        let mut existing = std::collections::HashMap::new();
        for chunk in urls.chunks(500) {
            let mut query: QueryBuilder<Sqlite> =
                QueryBuilder::new("SELECT id, url FROM channels WHERE url IN (");
            let mut separated = query.separated(", ");
            for url in chunk {
                separated.push_bind(*url);
            }
            separated.push_unseparated(")");

            let rows = query.build().fetch_all(&self.ctx.db).await?;
            for row in rows {
                existing.insert(
                    row.try_get::<String, _>("url")?,
                    row.try_get::<String, _>("id")?,
                );
            }
        }

        Ok(existing)
    }

    /// 删除频道
    pub async fn delete(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM channels WHERE id = ?")
            .bind(id)
            .execute(&self.ctx.db)
            .await?;

        Ok(())
    }

    /// 批量删除频道，避免前端逐个请求导致大量网络往返。
    pub async fn delete_many(&self, ids: &[String]) -> Result<u64> {
        if ids.is_empty() {
            return Ok(0);
        }

        let mut tx = self.ctx.db.begin().await?;
        let placeholders = std::iter::repeat_n("?", ids.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!("DELETE FROM channels WHERE id IN ({placeholders})");
        let mut query = sqlx::query(&sql);

        for id in ids {
            query = query.bind(id);
        }

        let deleted = query.execute(&mut *tx).await?.rows_affected();
        tx.commit().await?;

        Ok(deleted)
    }

    /// 更新频道状态
    #[allow(dead_code)]
    pub async fn update_status(&self, id: &str, status: &str) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query("UPDATE channels SET status = ?, last_check_at = ? WHERE id = ?")
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

        // Some IPTV gateways return 200 to HEAD but 503 when the stream is
        // actually opened. Probe with a tiny GET so the status matches playback.
        let result = client
            .get(&url)
            .header(reqwest::header::RANGE, "bytes=0-4095")
            .send()
            .await;

        let response_time_ms = Some(start.elapsed().as_millis() as u64);

        match result {
            Ok(resp) => {
                let http_status = resp.status();
                let status = if http_status.is_success() {
                    "online"
                } else {
                    "offline"
                };
                let error = if status == "online" {
                    None
                } else {
                    Some(format!("HTTP {}", http_status))
                };

                // 更新数据库状态
                let _ = self.update_status(id, status).await;

                Ok(ChannelTestResult {
                    channel_id: id.to_string(),
                    status: status.to_string(),
                    response_time_ms,
                    error,
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

/// 频道 URL 安全校验(完整版,含 DNS 解析):用于单条 create。
/// 私有服务器频道(内网源)跳过严格校验,只校验 scheme;其它做完整 SSRF 校验。
async fn assert_channel_url_safe(req: &CreateChannelRequest) -> Result<()> {
    if req.source_visibility == "private_server_only" {
        crate::services::url_safety::assert_safe_url_scheme_only(&req.url)
    } else {
        assert_safe_url(&req.url).await
    }
}

/// 频道 URL 安全校验(轻量版,同步):用于批量导入,不做 DNS 解析避免阻塞。
fn assert_channel_url_safe_light(req: &CreateChannelRequest) -> Result<()> {
    use crate::services::url_safety::{assert_safe_url_scheme_only, is_disallowed_hostname};
    use url::Url;

    assert_safe_url_scheme_only(&req.url)?;
    // 私有服务器频道允许内网主机名
    if req.source_visibility != "private_server_only" {
        if let Ok(parsed) = Url::parse(&req.url) {
            if let Some(host) = parsed.host_str() {
                if is_disallowed_hostname(host) {
                    anyhow::bail!("不允许使用本地或内网地址: {}", host);
                }
            }
        }
    }
    Ok(())
}

fn normalize_channel_request(mut req: CreateChannelRequest) -> CreateChannelRequest {
    if req.group_name.trim().is_empty() {
        req.group_name = "Uncategorized".to_string();
    }

    req.source_visibility = match req.source_visibility.trim() {
        "private_server_only" => "private_server_only".to_string(),
        _ => "public".to_string(),
    };
    if is_private_stream_url(&req.url) {
        req.source_visibility = "private_server_only".to_string();
    }

    req.playback_strategy = match req.playback_strategy.trim() {
        "hls_only" => "hls_only".to_string(),
        "proxy_only" => "proxy_only".to_string(),
        "record_only" => "record_only".to_string(),
        _ => "auto".to_string(),
    };

    req
}

fn is_unique_constraint_error(error: &sqlx::Error) -> bool {
    let message = error.to_string();
    message.contains("UNIQUE constraint failed: channels.url")
        || message.contains("UNIQUE constraint failed")
}

fn humanize_database_error(error: &sqlx::Error) -> String {
    let message = error.to_string();
    if message.contains("UNIQUE constraint failed: channels.url") {
        "流地址已存在".to_string()
    } else if message.contains("UNIQUE constraint failed") {
        "存在重复数据".to_string()
    } else {
        "数据库保存失败".to_string()
    }
}

fn is_private_stream_url(raw_url: &str) -> bool {
    let Ok(parsed) = Url::parse(raw_url.trim()) else {
        return false;
    };

    let Some(host) = parsed.host_str() else {
        return false;
    };

    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }

    let normalized_host = host.trim_matches(['[', ']']);
    let Ok(ip) = normalized_host.parse::<IpAddr>() else {
        return false;
    };

    match ip {
        IpAddr::V4(ip) => {
            ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_unspecified()
                || ip.octets()[0] == 100 && (64..=127).contains(&ip.octets()[1])
        }
        IpAddr::V6(ip) => {
            ip.is_loopback()
                || ip.is_unspecified()
                || ((ip.segments()[0] & 0xfe00) == 0xfc00)
                || ((ip.segments()[0] & 0xffc0) == 0xfe80)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::Config, core::database};
    use std::{
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

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
    async fn import_channels_batch_skips_existing_when_overwrite_disabled() {
        let (service, db_path) = test_service("channel-skip").await;
        let original = CreateChannelRequest {
            name: "CCTV-1".to_string(),
            url: "http://example.com/live.m3u8".to_string(),
            group_name: "央视".to_string(),
            logo_url: None,
            source_visibility: "public".to_string(),
            playback_strategy: "auto".to_string(),
        };
        service.create(original).await.expect("create channel");

        let duplicate = CreateChannelRequest {
            name: "CCTV-1 HD".to_string(),
            url: "http://example.com/live.m3u8".to_string(),
            group_name: "高清".to_string(),
            logo_url: Some("http://example.com/logo.png".to_string()),
            source_visibility: "public".to_string(),
            playback_strategy: "auto".to_string(),
        };

        let result = service
            .import_channels_batch(vec![duplicate], false)
            .await
            .expect("import channels");
        assert_eq!(result.imported, 0);
        assert_eq!(result.skipped, 1);
        assert_eq!(result.failed, 0);

        let stored = channel_by_url(&service, "http://example.com/live.m3u8")
            .await
            .expect("query by url");
        assert_eq!(stored.name, "CCTV-1");
        assert_eq!(stored.group_name, "央视");

        let _ = tokio::fs::remove_file(db_path).await;
    }

    #[tokio::test]
    async fn import_channels_batch_updates_existing_when_overwrite_enabled() {
        let (service, db_path) = test_service("channel-overwrite").await;
        service
            .create(CreateChannelRequest {
                name: "CCTV-1".to_string(),
                url: "http://example.com/live.m3u8".to_string(),
                group_name: "央视".to_string(),
                logo_url: None,
                source_visibility: "public".to_string(),
                playback_strategy: "auto".to_string(),
            })
            .await
            .expect("create channel");

        let result = service
            .import_channels_batch(
                vec![CreateChannelRequest {
                    name: "CCTV-1 HD".to_string(),
                    url: "http://example.com/live.m3u8".to_string(),
                    group_name: "高清".to_string(),
                    logo_url: Some("http://example.com/logo.png".to_string()),
                    source_visibility: "public".to_string(),
                    playback_strategy: "auto".to_string(),
                }],
                true,
            )
            .await
            .expect("import overwrite");

        assert_eq!(result.imported, 1);
        assert_eq!(result.skipped, 0);
        assert_eq!(result.failed, 0);

        let stored = channel_by_url(&service, "http://example.com/live.m3u8")
            .await
            .expect("query by url");
        assert_eq!(stored.name, "CCTV-1 HD");
        assert_eq!(stored.group_name, "高清");
        assert_eq!(
            stored.logo_url.as_deref(),
            Some("http://example.com/logo.png")
        );

        let _ = tokio::fs::remove_file(db_path).await;
    }

    #[tokio::test]
    async fn import_channels_batch_skips_duplicate_urls_in_same_batch() {
        let (service, db_path) = test_service("channel-duplicate-batch").await;
        let result = service
            .import_channels_batch(
                vec![
                    CreateChannelRequest {
                        name: "深圳卫视高清".to_string(),
                        url: "http://example.com/same.m3u8".to_string(),
                        group_name: "卫视".to_string(),
                        logo_url: None,
                        source_visibility: "public".to_string(),
                        playback_strategy: "auto".to_string(),
                    },
                    CreateChannelRequest {
                        name: "深圳卫视超清".to_string(),
                        url: "http://example.com/same.m3u8".to_string(),
                        group_name: "卫视".to_string(),
                        logo_url: None,
                        source_visibility: "public".to_string(),
                        playback_strategy: "auto".to_string(),
                    },
                ],
                false,
            )
            .await
            .expect("import duplicate batch");

        assert_eq!(result.imported, 1);
        assert_eq!(result.skipped, 1);
        assert_eq!(result.failed, 0);
        assert!(result.errors[0].contains("相同流地址"));

        let _ = tokio::fs::remove_file(db_path).await;
    }

    async fn channel_by_url(service: &ChannelService, url: &str) -> Result<Channel> {
        Ok(
            sqlx::query_as::<_, Channel>("SELECT * FROM channels WHERE url = ? LIMIT 1")
                .bind(url)
                .fetch_one(&service.ctx.db)
                .await?,
        )
    }

    #[test]
    fn detects_private_stream_urls() {
        for url in [
            "http://192.168.1.10/live.m3u8",
            "http://10.0.0.5/live.m3u8",
            "http://172.16.0.5/live.m3u8",
            "http://172.31.255.255/live.m3u8",
            "http://127.0.0.1/live.m3u8",
            "http://localhost/live.m3u8",
            "http://169.254.1.1/live.m3u8",
            "http://100.64.0.1/live.m3u8",
            "http://[fc00::1]/live.m3u8",
            "http://[fe80::1]/live.m3u8",
        ] {
            assert!(is_private_stream_url(url), "{url}");
        }

        for url in [
            "http://8.8.8.8/live.m3u8",
            "https://example.com/live.m3u8",
            "http://172.32.0.1/live.m3u8",
            "http://100.128.0.1/live.m3u8",
        ] {
            assert!(!is_private_stream_url(url), "{url}");
        }
    }

    #[tokio::test]
    async fn create_marks_private_ip_as_private_server_only() {
        let (service, db_path) = test_service("channel-private-url").await;
        let stored = service
            .create(CreateChannelRequest {
                name: "LAN Channel".to_string(),
                url: "http://192.168.31.10/live.m3u8".to_string(),
                group_name: "LAN".to_string(),
                logo_url: None,
                source_visibility: "public".to_string(),
                playback_strategy: "auto".to_string(),
            })
            .await
            .expect("create channel");

        assert_eq!(stored.source_visibility, "private_server_only");

        let _ = tokio::fs::remove_file(db_path).await;
    }
}
