//! 录制计划服务

use crate::{
    models::{Schedule, CreateScheduleRequest},
    services::ServiceContext,
};
use anyhow::Result;
use uuid::Uuid;

pub struct ScheduleService {
    ctx: ServiceContext,
}

impl ScheduleService {
    pub fn new(ctx: ServiceContext) -> Self {
        Self { ctx }
    }

    /// 创建录制计划
    pub async fn create(&self, req: CreateScheduleRequest) -> Result<Schedule> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let normalized = NormalizedScheduleRequest::from(req);

        sqlx::query(
            r#"
            INSERT INTO schedules
            (id, name, channel_id, cron_expression, duration_seconds, output_template, output_dir, priority,
             video_quality, audio_quality, max_speed, thread_count, transcode_mode, transcode_preset, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(&normalized.name)
        .bind(&normalized.channel_id)
        .bind(&normalized.cron_expression)
        .bind(normalized.duration_seconds)
        .bind(&normalized.output_template)
        .bind(&normalized.output_dir)
        .bind(normalized.priority)
        .bind(&normalized.video_quality)
        .bind(&normalized.audio_quality)
        .bind(&normalized.max_speed)
        .bind(normalized.thread_count)
        .bind(&normalized.transcode_mode)
        .bind(&normalized.transcode_preset)
        .bind(&now)
        .bind(&now)
        .execute(&self.ctx.db)
        .await?;

        self.get_by_id(&id).await
    }

    /// 根据 ID 获取计划
    pub async fn get_by_id(&self, id: &str) -> Result<Schedule> {
        let schedule = sqlx::query_as::<_, Schedule>(
            "SELECT * FROM schedules WHERE id = ?"
        )
        .bind(id)
        .fetch_one(&self.ctx.db)
        .await?;

        Ok(schedule)
    }

    /// 获取所有计划
    pub async fn list(&self) -> Result<Vec<Schedule>> {
        let schedules = sqlx::query_as::<_, Schedule>(
            "SELECT * FROM schedules ORDER BY created_at DESC"
        )
        .fetch_all(&self.ctx.db)
        .await?;

        Ok(schedules)
    }

    /// 获取启用的计划
    #[allow(dead_code)]
    pub async fn list_enabled(&self) -> Result<Vec<Schedule>> {
        let schedules = sqlx::query_as::<_, Schedule>(
            "SELECT * FROM schedules WHERE enabled = 1 ORDER BY priority DESC"
        )
        .fetch_all(&self.ctx.db)
        .await?;

        Ok(schedules)
    }

    /// 更新计划
    pub async fn update(&self, id: &str, req: CreateScheduleRequest) -> Result<Schedule> {
        let now = chrono::Utc::now().to_rfc3339();
        let normalized = NormalizedScheduleRequest::from(req.clone());

        sqlx::query(
            r#"
            UPDATE schedules
            SET name = ?, channel_id = ?, cron_expression = ?,
                duration_seconds = ?, output_template = ?, output_dir = ?, priority = ?,
                video_quality = ?, audio_quality = ?, max_speed = ?, thread_count = ?,
                transcode_mode = ?, transcode_preset = ?,
                updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&req.name)
        .bind(&req.channel_id)
        .bind(&req.cron_expression)
        .bind(req.duration_seconds)
        .bind(&normalized.output_template)
        .bind(&normalized.output_dir)
        .bind(normalized.priority)
        .bind(&normalized.video_quality)
        .bind(&normalized.audio_quality)
        .bind(&normalized.max_speed)
        .bind(normalized.thread_count)
        .bind(&normalized.transcode_mode)
        .bind(&normalized.transcode_preset)
        .bind(&now)
        .bind(id)
        .execute(&self.ctx.db)
        .await?;

        self.get_by_id(id).await
    }

    /// 删除计划
    pub async fn delete(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM schedules WHERE id = ?")
            .bind(id)
            .execute(&self.ctx.db)
            .await?;

        Ok(())
    }

    /// 切换启用状态
    pub async fn toggle_enabled(&self, id: &str) -> Result<bool> {
        let schedule = self.get_by_id(id).await?;
        let new_enabled = !schedule.enabled;

        sqlx::query("UPDATE schedules SET enabled = ? WHERE id = ?")
            .bind(new_enabled as i32)
            .bind(id)
            .execute(&self.ctx.db)
            .await?;

        Ok(new_enabled)
    }
}

struct NormalizedScheduleRequest {
    name: String,
    channel_id: String,
    cron_expression: String,
    duration_seconds: i64,
    output_template: String,
    output_dir: Option<String>,
    priority: i32,
    video_quality: String,
    audio_quality: String,
    max_speed: Option<String>,
    thread_count: i32,
    transcode_mode: String,
    transcode_preset: String,
}

impl From<CreateScheduleRequest> for NormalizedScheduleRequest {
    fn from(req: CreateScheduleRequest) -> Self {
        Self {
            name: req.name,
            channel_id: req.channel_id,
            cron_expression: req.cron_expression,
            duration_seconds: req.duration_seconds,
            output_template: req
                .output_template
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| "{channel_name}_{date}_{time}.mp4".to_string()),
            output_dir: req.output_dir.and_then(|s| if s.is_empty() { None } else { Some(s) }),
            priority: req.priority.unwrap_or(5),
            video_quality: if req.video_quality.is_empty() { "best".to_string() } else { req.video_quality },
            audio_quality: if req.audio_quality.is_empty() { "best".to_string() } else { req.audio_quality },
            max_speed: req.max_speed.and_then(|s| if s.is_empty() { None } else { Some(s) }),
            thread_count: if req.thread_count == 0 { 20 } else { req.thread_count },
            transcode_mode: if req.transcode_mode.is_empty() { "off".to_string() } else { req.transcode_mode },
            transcode_preset: if req.transcode_preset.is_empty() { "medium".to_string() } else { req.transcode_preset },
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

    async fn test_service(name: &str) -> (ScheduleService, PathBuf) {
        let db_path = temp_db_path(name);
        let db = database::init(db_path.to_str().expect("utf8 path"), 1)
            .await
            .expect("db init");

        sqlx::query(
            r#"
            INSERT INTO channels (id, name, url, group_name, created_at, updated_at)
            VALUES (?, ?, ?, ?, datetime('now'), datetime('now'))
            "#,
        )
        .bind("channel-1")
        .bind("Test Channel")
        .bind("https://example.com/stream.m3u8")
        .bind("Test")
        .execute(&db)
        .await
        .expect("insert channel");

        (ScheduleService::new(ServiceContext::new(db, Config::default())), db_path)
    }

    #[tokio::test]
    async fn create_and_update_normalize_optional_fields() {
        let (service, db_path) = test_service("schedule-normalization").await;

        let created = service.create(CreateScheduleRequest {
            name: "Morning".to_string(),
            channel_id: "channel-1".to_string(),
            cron_expression: "0 8 * * *".to_string(),
            duration_seconds: 1800,
            output_template: Some(String::new()),
            output_dir: Some(String::new()),
            priority: None,
            video_quality: String::new(),
            audio_quality: String::new(),
            max_speed: Some(String::new()),
            thread_count: 0,
            transcode_mode: String::new(),
            transcode_preset: String::new(),
        }).await.expect("create schedule");

        assert_eq!(created.output_template, "{channel_name}_{date}_{time}.mp4");
        assert_eq!(created.output_dir, None);
        assert_eq!(created.priority, 5);
        assert_eq!(created.video_quality, "best");
        assert_eq!(created.audio_quality, "best");
        assert_eq!(created.max_speed, None);
        assert_eq!(created.thread_count, 20);
        assert_eq!(created.transcode_mode, "off");
        assert_eq!(created.transcode_preset, "medium");

        let updated = service.update(&created.id, CreateScheduleRequest {
            name: "Morning Updated".to_string(),
            channel_id: "channel-1".to_string(),
            cron_expression: "0 9 * * *".to_string(),
            duration_seconds: 2400,
            output_template: Some(String::new()),
            output_dir: Some(String::new()),
            priority: None,
            video_quality: String::new(),
            audio_quality: String::new(),
            max_speed: Some(String::new()),
            thread_count: 0,
            transcode_mode: String::new(),
            transcode_preset: String::new(),
        }).await.expect("update schedule");

        assert_eq!(updated.name, "Morning Updated");
        assert_eq!(updated.output_template, "{channel_name}_{date}_{time}.mp4");
        assert_eq!(updated.output_dir, None);
        assert_eq!(updated.priority, 5);
        assert_eq!(updated.video_quality, "best");
        assert_eq!(updated.audio_quality, "best");
        assert_eq!(updated.max_speed, None);
        assert_eq!(updated.thread_count, 20);
        assert_eq!(updated.transcode_mode, "off");
        assert_eq!(updated.transcode_preset, "medium");

        let _ = tokio::fs::remove_file(db_path).await;
    }
}
