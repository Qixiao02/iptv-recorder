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

        let output_template = req.output_template.unwrap_or_else(|| {
            "{channel_name}_{date}_{time}.mp4".to_string()
        });

        let output_dir = req.output_dir.and_then(|s| if s.is_empty() { None } else { Some(s) });
        let priority = req.priority.unwrap_or(5);
        let video_quality = if req.video_quality.is_empty() { "best".to_string() } else { req.video_quality.clone() };
        let audio_quality = if req.audio_quality.is_empty() { "best".to_string() } else { req.audio_quality.clone() };
        let max_speed = req.max_speed.clone().and_then(|s| if s.is_empty() { None } else { Some(s) });
        let thread_count = if req.thread_count == 0 { 20 } else { req.thread_count };
        let transcode_mode = if req.transcode_mode.is_empty() { "off".to_string() } else { req.transcode_mode.clone() };
        let transcode_preset = if req.transcode_preset.is_empty() { "medium".to_string() } else { req.transcode_preset.clone() };

        sqlx::query(
            r#"
            INSERT INTO schedules
            (id, name, channel_id, cron_expression, duration_seconds, output_template, output_dir, priority,
             video_quality, audio_quality, max_speed, thread_count, transcode_mode, transcode_preset, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(&req.name)
        .bind(&req.channel_id)
        .bind(&req.cron_expression)
        .bind(req.duration_seconds)
        .bind(&output_template)
        .bind(&output_dir)
        .bind(priority)
        .bind(&video_quality)
        .bind(&audio_quality)
        .bind(&max_speed)
        .bind(thread_count)
        .bind(&transcode_mode)
        .bind(&transcode_preset)
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

        let output_dir = req.output_dir.clone().and_then(|s| if s.is_empty() { None } else { Some(s) });
        let video_quality = if req.video_quality.is_empty() { "best".to_string() } else { req.video_quality.clone() };
        let audio_quality = if req.audio_quality.is_empty() { "best".to_string() } else { req.audio_quality.clone() };
        let max_speed = req.max_speed.clone().and_then(|s| if s.is_empty() { None } else { Some(s) });
        let thread_count = if req.thread_count == 0 { 20 } else { req.thread_count };
        let transcode_mode = if req.transcode_mode.is_empty() { "off".to_string() } else { req.transcode_mode.clone() };
        let transcode_preset = if req.transcode_preset.is_empty() { "medium".to_string() } else { req.transcode_preset.clone() };

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
        .bind(&req.output_template)
        .bind(&output_dir)
        .bind(&req.priority)
        .bind(&video_quality)
        .bind(&audio_quality)
        .bind(&max_speed)
        .bind(thread_count)
        .bind(&transcode_mode)
        .bind(&transcode_preset)
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
