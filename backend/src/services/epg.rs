//! EPG 节目单服务

use anyhow::{Context, Result};
use quick_xml::de::from_str;
use serde::Deserialize;
use uuid::Uuid;

use crate::models::{EpgProgram, EpgSource};

use super::ServiceContext;

#[derive(Debug, Deserialize)]
struct XmlTv {
    #[serde(default, rename = "programme")]
    programmes: Vec<XmlProgramme>,
}

#[derive(Debug, Deserialize)]
struct XmlProgramme {
    #[serde(rename = "@channel")]
    channel: String,
    #[serde(rename = "@start")]
    start: String,
    #[serde(rename = "@stop")]
    stop: String,
    title: XmlText,
    #[serde(default)]
    desc: Option<XmlText>,
    #[serde(default)]
    category: Option<XmlText>,
}

#[derive(Debug, Deserialize)]
struct XmlText {
    #[serde(rename = "$text")]
    value: String,
}

#[derive(Debug, Deserialize)]
pub struct ImportEpgRequest {
    pub name: String,
    pub url: Option<String>,
    pub content: Option<String>,
}

pub struct EpgService {
    ctx: ServiceContext,
}

impl EpgService {
    pub fn new(ctx: ServiceContext) -> Self {
        Self { ctx }
    }

    pub async fn import_source(&self, req: ImportEpgRequest) -> Result<EpgSource> {
        let content = if let Some(content) = req.content {
            content
        } else if let Some(url) = &req.url {
            reqwest::get(url)
                .await
                .context("下载 EPG 源失败")?
                .text()
                .await
                .context("读取 EPG 源失败")?
        } else {
            anyhow::bail!("必须提供 EPG URL 或 content");
        };

        let parsed: XmlTv = from_str(&content).context("解析 XMLTV 失败")?;
        let source_id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO epg_sources (id, name, source_url, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(&source_id)
        .bind(&req.name)
        .bind(req.url.as_deref())
        .bind(&now)
        .bind(&now)
        .execute(&self.ctx.db)
        .await?;

        for programme in parsed.programmes {
            sqlx::query(
                r#"
                INSERT INTO epg_programs
                (id, source_id, channel_ref, title, description, category, start_at, end_at, created_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(Uuid::new_v4().to_string())
            .bind(&source_id)
            .bind(&programme.channel)
            .bind(&programme.title.value)
            .bind(programme.desc.map(|d| d.value))
            .bind(programme.category.map(|c| c.value))
            .bind(normalize_xmltv_datetime(&programme.start))
            .bind(normalize_xmltv_datetime(&programme.stop))
            .bind(&now)
            .execute(&self.ctx.db)
            .await?;
        }

        self.get_source(&source_id).await
    }

    pub async fn list_sources(&self) -> Result<Vec<EpgSource>> {
        let sources = sqlx::query_as::<_, EpgSource>(
            "SELECT * FROM epg_sources ORDER BY created_at DESC",
        )
        .fetch_all(&self.ctx.db)
        .await?;
        Ok(sources)
    }

    pub async fn get_source(&self, id: &str) -> Result<EpgSource> {
        let source = sqlx::query_as::<_, EpgSource>("SELECT * FROM epg_sources WHERE id = ?")
            .bind(id)
            .fetch_one(&self.ctx.db)
            .await?;
        Ok(source)
    }

    pub async fn list_programs(&self, channel_ref: &str, limit: i64) -> Result<Vec<EpgProgram>> {
        let programs = sqlx::query_as::<_, EpgProgram>(
            r#"
            SELECT * FROM epg_programs
            WHERE channel_ref = ?
            ORDER BY start_at ASC
            LIMIT ?
            "#,
        )
        .bind(channel_ref)
        .bind(limit.max(1))
        .fetch_all(&self.ctx.db)
        .await?;
        Ok(programs)
    }
}

fn normalize_xmltv_datetime(value: &str) -> String {
    if let Ok(dt) = chrono::DateTime::parse_from_str(value, "%Y%m%d%H%M%S %z") {
        return dt.with_timezone(&chrono::Utc).to_rfc3339();
    }
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(value, "%Y%m%d%H%M%S") {
        return chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(dt, chrono::Utc).to_rfc3339();
    }
    value.to_string()
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

    #[tokio::test]
    async fn imports_xmltv_programmes() {
        let db_path = temp_db_path("epg");
        let db = database::init(db_path.to_str().expect("utf8 path"), 1)
            .await
            .expect("db init");
        let service = EpgService::new(ServiceContext::new(db, Config::default()));

        let source = service
            .import_source(ImportEpgRequest {
                name: "test-epg".to_string(),
                url: None,
                content: Some(
                    r#"<tv>
                        <programme start="20260506080000 +0000" stop="20260506090000 +0000" channel="cctv-news">
                          <title>Morning News</title>
                          <desc>Daily roundup</desc>
                          <category>News</category>
                        </programme>
                    </tv>"#
                        .to_string(),
                ),
            })
            .await
            .expect("import epg");

        let programs = service
            .list_programs("cctv-news", 10)
            .await
            .expect("list programs");

        assert_eq!(source.name, "test-epg");
        assert_eq!(programs.len(), 1);
        assert_eq!(programs[0].title, "Morning News");

        let _ = tokio::fs::remove_file(db_path).await;
    }
}
