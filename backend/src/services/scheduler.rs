//! Cron 调度器模块
//!
//! 基于 tokio-cron-scheduler 实现定时任务调度

#![allow(dead_code)]

use anyhow::Result;
use chrono::Utc;
use cron::Schedule;
use sqlx::{Pool, Sqlite};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{info, error};

use crate::config::Config;
use crate::core::ProcessManager;
use crate::models::ManualRecordRequest;
use crate::services::{ScheduleService, ServiceContext, RecordingService};

/// 调度器管理器
pub struct SchedulerManager {
    /// tokio-cron-scheduler 实例
    scheduler: Arc<RwLock<JobScheduler>>,
    /// 数据库连接池
    db: Pool<Sqlite>,
    /// 配置
    config: Config,
    /// 进程管理器
    process_manager: Arc<ProcessManager>,
    /// schedule_id -> job uuid 映射
    job_uuids: Arc<RwLock<HashMap<String, uuid::Uuid>>>,
}

impl SchedulerManager {
    /// 创建新的调度器管理器
    pub async fn new(db: Pool<Sqlite>, config: Config, process_manager: Arc<ProcessManager>) -> Result<Self> {
        let scheduler = JobScheduler::new().await?;

        Ok(Self {
            scheduler: Arc::new(RwLock::new(scheduler)),
            db,
            config,
            process_manager,
            job_uuids: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// 启动调度器
    pub async fn start(&self) -> Result<()> {
        info!("📅 Cron Scheduler starting...");

        // 加载所有启用的计划
        let ctx = ServiceContext::new(self.db.clone(), self.config.clone());
        let schedule_service = ScheduleService::new(ctx);

        match schedule_service.list_enabled().await {
            Ok(schedules) => {
                let count = schedules.len();
                for schedule in &schedules {
                    if let Err(e) = self.add_schedule(schedule).await {
                        error!("Failed to add schedule {}: {}", schedule.name, e);
                    }
                }
                info!("Loaded {} schedules", count);
            }
            Err(e) => {
                error!("Failed to load schedules: {}", e);
            }
        }

        // 启动调度器
        {
            let scheduler = self.scheduler.read().await;
            scheduler.start().await?;
        }

        info!("📅 Cron Scheduler started successfully");
        Ok(())
    }

    /// 添加计划到调度器
    pub async fn add_schedule(&self, schedule: &crate::models::Schedule) -> Result<()> {
        let schedule_id = schedule.id.clone();

        // 如果已有旧 job，先移除
        {
            let uuids = self.job_uuids.read().await;
            if let Some(&old_uuid) = uuids.get(&schedule_id) {
                drop(uuids);
                let scheduler = self.scheduler.read().await;
                let _ = scheduler.remove(&old_uuid).await;
                let mut uuids = self.job_uuids.write().await;
                uuids.remove(&schedule_id);
            }
        }

        // 解析 Cron 表达式
        let cron_expr = if schedule.cron_expression.contains(' ') {
            schedule.cron_expression.clone()
        } else {
            // 如果是简化格式（如 "daily 19:00"），转换为标准 Cron
            self.parse_simple_format(&schedule.cron_expression)?
        };

        // 转换为 6 字段格式（添加秒字段）
        let cron_expr_6field = {
            let parts: Vec<&str> = cron_expr.split_whitespace().collect();
            if parts.len() == 5 {
                format!("0 {}", cron_expr)
            } else if parts.len() == 6 {
                cron_expr
            } else {
                return Err(anyhow::anyhow!("无效的 Cron 表达式格式: {}", cron_expr));
            }
        };

        // 验证 Cron 表达式
        let _schedule_check = Schedule::from_str(&cron_expr_6field)
            .map_err(|e| anyhow::anyhow!("无效的 Cron 表达式 '{}': {}", cron_expr_6field, e))?;

        // 预先捕获所有需要进入闭包的数据
        let db_for_job = self.db.clone();
        let config_for_job = self.config.clone();
        let pm_for_job = self.process_manager.clone();
        let schedule_id_for_job = schedule_id.clone();
        let channel_id_for_job = schedule.channel_id.clone();
        let duration_for_job = schedule.duration_seconds;
        let output_dir_for_job = schedule.output_dir.clone();
        let output_template_for_job = schedule.output_template.clone();
        let video_quality_for_job = schedule.video_quality.clone();
        let audio_quality_for_job = schedule.audio_quality.clone();
        let max_speed_for_job = schedule.max_speed.clone();
        let thread_count_for_job = schedule.thread_count;
        let schedule_name_for_job = schedule.name.clone();

        // 创建定时任务
        let job = Job::new_async(&cron_expr_6field, move |_uuid, _l| {
            let db = db_for_job.clone();
            let config = config_for_job.clone();
            let pm = pm_for_job.clone();
            let schedule_id = schedule_id_for_job.clone();
            let channel_id = channel_id_for_job.clone();
            let duration_seconds = duration_for_job;
            let output_dir = output_dir_for_job.clone();
            let output_template = output_template_for_job.clone();
            let video_quality = video_quality_for_job.clone();
            let audio_quality = audio_quality_for_job.clone();
            let max_speed = max_speed_for_job.clone();
            let thread_count = thread_count_for_job;
            let schedule_name = schedule_name_for_job.clone();

            Box::pin(async move {
                info!("🕐 触发定时任务: schedule_id={}, channel_id={}", schedule_id, channel_id);

                let ctx = ServiceContext::new(db, config);
                let service = RecordingService::new(pm, ctx, None);

                let req = ManualRecordRequest {
                    channel_id,
                    duration_seconds,
                    output_name: Some(schedule_name.clone()),
                    output_dir,
                    output_template: Some(output_template),
                    video_quality,
                    audio_quality,
                    max_speed,
                    thread_count,
                };

                match service.start_manual(req).await {
                    Ok(task) => {
                        info!("✅ 定时录制任务已创建: schedule_id={}, task_id={}", schedule_id, task.id);
                    }
                    Err(e) => {
                        error!("❌ 定时录制任务创建失败: schedule_id={}, error={}", schedule_id, e);
                    }
                }
            })
        })
        .map_err(|e| anyhow::anyhow!("创建任务失败: {}", e))?;

        // 添加到调度器并保存 UUID
        let job_uuid = {
            let scheduler = self.scheduler.read().await;
            scheduler.add(job).await?
        };

        {
            let mut uuids = self.job_uuids.write().await;
            uuids.insert(schedule_id.clone(), job_uuid);
        }

        info!("✅ 已添加计划: {} ({})", schedule.name, cron_expr_6field);
        Ok(())
    }

    /// 移除计划
    pub async fn remove_schedule(&self, schedule_id: &str) -> Result<()> {
        let uuid = {
            let uuids = self.job_uuids.read().await;
            uuids.get(schedule_id).copied()
        };

        if let Some(job_uuid) = uuid {
            {
                let scheduler = self.scheduler.read().await;
                let _ = scheduler.remove(&job_uuid).await;
            }
            let mut uuids = self.job_uuids.write().await;
            uuids.remove(schedule_id);
            info!("✅ 已移除计划: {}", schedule_id);
        }

        Ok(())
    }

    /// 重新加载所有计划
    pub async fn reload(&self) -> Result<()> {
        info!("🔄 重新加载调度器...");

        // 停止并重新创建调度器
        {
            let mut scheduler = self.scheduler.write().await;
            scheduler.shutdown().await?;
            *scheduler = JobScheduler::new().await?;
        }

        // 清空 UUID 映射
        {
            let mut uuids = self.job_uuids.write().await;
            uuids.clear();
        }

        // 重新启动
        self.start().await?;

        info!("🔄 调度器重新加载完成");
        Ok(())
    }

    /// 解析简化格式
    ///
    /// 支持的格式：
    /// - "daily 19:00" -> "0 19 * * *"
    /// - "weekly 19:00" -> "0 19 * * 1"
    /// - "hourly" -> "0 * * * *"
    fn parse_simple_format(&self, input: &str) -> Result<String> {
        let parts: Vec<&str> = input.split_whitespace().collect();

        match parts.first() {
            Some(&"hourly") => Ok("0 * * * *".to_string()),
            Some(&"daily") => {
                if parts.len() >= 2 {
                    let time = parts[1]; // "19:00"
                    let time_parts: Vec<&str> = time.split(':').collect();
                    if time_parts.len() == 2 {
                        Ok(format!("{} {} * * *", time_parts[1], time_parts[0]))
                    } else {
                        Ok("0 0 * * *".to_string())
                    }
                } else {
                    Ok("0 0 * * *".to_string())
                }
            }
            Some(&"weekly") => {
                if parts.len() >= 2 {
                    let time = parts[1]; // "19:00"
                    let time_parts: Vec<&str> = time.split(':').collect();
                    if time_parts.len() == 2 {
                        Ok(format!("{} {} * * 1", time_parts[1], time_parts[0]))
                    } else {
                        Ok("0 0 * * 1".to_string())
                    }
                } else {
                    Ok("0 0 * * 1".to_string())
                }
            }
            _ => Ok(input.to_string()),
        }
    }

    /// 获取下次执行时间
    pub fn get_next_run_time(&self, cron_expr: &str) -> Result<chrono::DateTime<chrono::Utc>> {
        // 转换为 6 字段格式（添加秒字段）
        let cron_expr_6field = {
            let parts: Vec<&str> = cron_expr.split_whitespace().collect();
            if parts.len() == 5 {
                format!("0 {}", cron_expr)
            } else if parts.len() == 6 {
                cron_expr.to_string()
            } else {
                return Err(anyhow::anyhow!("无效的 Cron 表达式格式: {}", cron_expr));
            }
        };

        let schedule = Schedule::from_str(&cron_expr_6field)
            .map_err(|e| anyhow::anyhow!("无效的 Cron 表达式 '{}': {}", cron_expr_6field, e))?;

        let timezone_str = &self.config.scheduler.timezone;
        let timezone = chrono_tz::Tz::from_str(timezone_str)
            .unwrap_or(chrono_tz::UTC);

        let _now = Utc::now().with_timezone(&timezone);

        let next_time = schedule
            .upcoming(timezone)
            .next()
            .ok_or_else(|| anyhow::anyhow!("无法计算下次执行时间"))?;

        // 转换为 UTC
        Ok(next_time.with_timezone(&chrono::Utc))
    }
}

/// Cron 任务触发器
pub struct CronTrigger {
    scheduler: Arc<SchedulerManager>,
}

impl CronTrigger {
    pub fn new(scheduler: Arc<SchedulerManager>) -> Self {
        Self { scheduler }
    }

    /// 手动触发所有计划（用于测试）
    pub async fn trigger_all(&self) -> Result<Vec<String>> {
        let ctx = ServiceContext::new(
            self.scheduler.db.clone(),
            self.scheduler.config.clone(),
        );
        let schedule_service = ScheduleService::new(ctx);

        let schedules = schedule_service.list_enabled().await?;
        let mut triggered = Vec::new();

        for schedule in schedules {
            match self.scheduler.add_schedule(&schedule).await {
                Ok(_) => {
                    triggered.push(schedule.name);
                }
                Err(e) => {
                    error!("Failed to trigger {}: {}", schedule.name, e);
                }
            }
        }

        Ok(triggered)
    }

    /// 获取下次执行时间列表
    pub async fn get_upcoming(&self) -> Result<Vec<UpcomingTask>> {
        let ctx = ServiceContext::new(
            self.scheduler.db.clone(),
            self.scheduler.config.clone(),
        );
        let schedule_service = ScheduleService::new(ctx);

        let schedules = schedule_service.list_enabled().await?;
        let mut upcoming = Vec::new();

        for schedule in schedules {
            match self.scheduler.get_next_run_time(&schedule.cron_expression) {
                Ok(next_time) => {
                    upcoming.push(UpcomingTask {
                        schedule_id: schedule.id.clone(),
                        schedule_name: schedule.name.clone(),
                        channel_id: schedule.channel_id.clone(),
                        next_run: next_time.to_rfc3339(),
                        duration_seconds: schedule.duration_seconds,
                    });
                }
                Err(e) => {
                    error!("Failed to calculate next run for {}: {}", schedule.name, e);
                }
            }
        }

        // 按时间排序
        upcoming.sort_by(|a, b| a.next_run.cmp(&b.next_run));

        Ok(upcoming)
    }
}

/// 即将执行的任务
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UpcomingTask {
    pub schedule_id: String,
    pub schedule_name: String,
    pub channel_id: String,
    pub next_run: String,
    pub duration_seconds: i64,
}
