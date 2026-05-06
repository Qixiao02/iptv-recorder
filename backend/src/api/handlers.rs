//! HTTP 请求处理器

use axum::{
    extract::{Path, Query, State, Extension},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Json, Response},
    body::Body,
};
use serde::Deserialize;
use sqlx::{Pool, Sqlite};
use std::sync::Arc;
use tracing::error as tracing_error;
use reqwest;

use crate::config::Config;
use crate::core::ProcessManager;
use crate::core::event::EventBus;
use crate::models::{
    Channel, CreateChannelRequest, CreateScheduleRequest, ManualRecordRequest,
    Schedule, Task, ErrorResponse, ImportM3UResponse, AuditLog, SystemHealth, EpgSource, EpgProgram,
};
use crate::services::{
    ChannelService, ScheduleService, RecordingService, ServiceContext,
    M3UParser, CronTrigger, UpcomingTask, SchedulerManager,
    ConfigService, ConfigUpdateRequest, ChannelTestResult, PaginationParams,
    AuthService, ImportChannelResult, AuditService, CleanupService, EpgService, ImportEpgRequest, Claims,
};

/// 应用状态
pub type AppState = (
    Pool<Sqlite>,
    Arc<SchedulerManager>,
    Arc<ProcessManager>,
    Config,
);

async fn record_audit(
    db: Pool<Sqlite>,
    config: Config,
    claims: Option<&Claims>,
    action: &str,
    resource_type: &str,
    resource_id: Option<&str>,
    details: Option<String>,
) {
    let service = AuditService::new(ServiceContext::new(db, config));
    if let Err(e) = service
        .record(claims, action, resource_type, resource_id, details.as_deref())
        .await
    {
        tracing_error!("Failed to record audit log: {}", e);
    }
}

/// 首页处理器
pub async fn index_handler() -> &'static str {
    r#"
    __  __            ____      _ _____           _
    |  \/  |          / __ \    | |  __ \         | |
    | \  / | ___ _ __| |  | | __| | |__) |_ _  ___| | _____
    | |\/| |/ _ \ '__| |  | |/ _` |  ___/ _` |/ __| |/ / __|
    | |  | |  __/ |  | |__| | (_| | |   | (_| | (__|   <\ \ \
    |_|  |_|\___|_|   \____/ \__,_|_|    \__,_|\___|_|\_\___/

    IPTV M3U Management & Recording System

    API Endpoints:
    - GET    /api/channels              - List all channels
    - POST   /api/channels              - Create channel
    - POST   /api/channels/import/url   - Import from URL
    - GET    /api/schedules             - List all schedules
    - POST   /api/schedules             - Create schedule
    - GET    /api/tasks                 - List all tasks
    - POST   /api/tasks/manual          - Start manual recording
    - GET    /api/scheduler/upcoming    - Get upcoming tasks
    - POST   /api/scheduler/reload      - Reload scheduler
    - WS     /ws                        - WebSocket connection
    "#
}

// ===== 频道处理器 =====

pub async fn list_channels(
    State((db, _scheduler, _process_manager, config)): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<crate::services::channel::PaginatedChannels>, (StatusCode, Json<ErrorResponse>)> {
    let ctx = ServiceContext::new(db, config);
    let service = ChannelService::new(ctx);

    match service.list_paginated(params).await {
        Ok(result) => Ok(Json(result)),
        Err(e) => Err(internal_error(e)),
    }
}

pub async fn create_channel(
    Extension(claims): Extension<Claims>,
    State((db, _scheduler, _process_manager, config)): State<AppState>,
    Json(req): Json<CreateChannelRequest>,
) -> Result<Json<Channel>, (StatusCode, Json<ErrorResponse>)> {
    let ctx = ServiceContext::new(db.clone(), config.clone());
    let service = ChannelService::new(ctx);

    match service.create(req).await {
        Ok(channel) => {
            record_audit(
                db,
                config,
                Some(&claims),
                "channel.create",
                "channel",
                Some(&channel.id),
                Some(format!("name={}", channel.name)),
            )
            .await;
            Ok(Json(channel))
        }
        Err(e) => Err(internal_error(e)),
    }
}

pub async fn get_channel(
    State((db, _scheduler, _process_manager, config)): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Channel>, (StatusCode, Json<ErrorResponse>)> {
    let ctx = ServiceContext::new(db, config);
    let service = ChannelService::new(ctx);

    match service.get_by_id(&id).await {
        Ok(channel) => Ok(Json(channel)),
        Err(e) => Err(not_found_error(e)),
    }
}

pub async fn update_channel(
    Extension(claims): Extension<Claims>,
    State((db, _scheduler, _process_manager, config)): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<CreateChannelRequest>,
) -> Result<Json<Channel>, (StatusCode, Json<ErrorResponse>)> {
    let ctx = ServiceContext::new(db.clone(), config.clone());
    let service = ChannelService::new(ctx);

    match service.update(&id, req).await {
        Ok(channel) => {
            record_audit(db, config, Some(&claims), "channel.update", "channel", Some(&id), None).await;
            Ok(Json(channel))
        }
        Err(e) => Err(internal_error(e)),
    }
}

pub async fn delete_channel(
    Extension(claims): Extension<Claims>,
    State((db, _scheduler, _process_manager, config)): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let ctx = ServiceContext::new(db.clone(), config.clone());
    let service = ChannelService::new(ctx);

    match service.delete(&id).await {
        Ok(_) => {
            record_audit(db, config, Some(&claims), "channel.delete", "channel", Some(&id), None).await;
            Ok(StatusCode::NO_CONTENT)
        }
        Err(e) => Err(internal_error(e)),
    }
}

pub async fn list_groups(
    State((db, _scheduler, _process_manager, config)): State<AppState>,
) -> Result<Json<Vec<String>>, (StatusCode, Json<ErrorResponse>)> {
    let ctx = ServiceContext::new(db, config);
    let service = ChannelService::new(ctx);

    match service.list_groups().await {
        Ok(groups) => Ok(Json(groups)),
        Err(e) => Err(internal_error(e)),
    }
}

/// 测试频道连接
pub async fn test_channel(
    State((db, _scheduler, _process_manager, config)): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ChannelTestResult>, (StatusCode, Json<ErrorResponse>)> {
    let ctx = ServiceContext::new(db, config);
    let service = ChannelService::new(ctx);

    match service.test_channel(&id).await {
        Ok(result) => Ok(Json(result)),
        Err(e) => Err(internal_error(e)),
    }
}

/// 导入 M3U 请求
#[derive(Debug, Deserialize)]
pub struct ImportM3URequest {
    /// M3U 文件 URL
    pub url: Option<String>,
    /// M3U 文件内容（直接提供）
    pub content: Option<String>,
    /// 是否覆盖现有频道
    #[serde(default)]
    pub overwrite: bool,
}

/// 从 URL 导入 M3U
pub async fn import_m3u_url(
    Extension(claims): Extension<Claims>,
    State((db, _scheduler, _process_manager, config)): State<AppState>,
    Json(req): Json<ImportM3URequest>,
) -> Result<Json<ImportM3UResponse>, (StatusCode, Json<ErrorResponse>)> {
    let url = req.url.as_ref().ok_or_else(|| {
        (StatusCode::BAD_REQUEST, Json(ErrorResponse {
            error: "missing_url".to_string(),
            details: Some("必须提供 URL 或 content".to_string()),
        }))
    })?;

    // 解析 M3U
    let parse_result = M3UParser::from_url(url).await.map_err(|e| {
        internal_error(anyhow::anyhow!("解析 M3U 失败: {}", e))
    })?;

    // 导入频道
    let response = import_channels(db.clone(), config.clone(), parse_result, req.overwrite).await?;
    record_audit(
        db,
        config,
        Some(&claims),
        "channel.import.url",
        "channel",
        None,
        Some(format!("overwrite={}", req.overwrite)),
    )
    .await;
    Ok(response)
}

/// 从内容导入 M3U
pub async fn import_m3u_content(
    Extension(claims): Extension<Claims>,
    State((db, _scheduler, _process_manager, config)): State<AppState>,
    Json(req): Json<ImportM3URequest>,
) -> Result<Json<ImportM3UResponse>, (StatusCode, Json<ErrorResponse>)> {
    let content = req.content.as_ref().ok_or_else(|| {
        (StatusCode::BAD_REQUEST, Json(ErrorResponse {
            error: "missing_content".to_string(),
            details: Some("必须提供 content 或 URL".to_string()),
        }))
    })?;

    // 解析 M3U
    let parse_result = M3UParser::parse(content).map_err(|e| {
        internal_error(anyhow::anyhow!("解析 M3U 失败: {}", e))
    })?;

    // 导入频道
    let response = import_channels(db.clone(), config.clone(), parse_result, req.overwrite).await?;
    record_audit(
        db,
        config,
        Some(&claims),
        "channel.import.content",
        "channel",
        None,
        Some(format!("overwrite={}", req.overwrite)),
    )
    .await;
    Ok(response)
}

/// 导入频道到数据库
async fn import_channels(
    db: Pool<Sqlite>,
    config: Config,
    parse_result: crate::services::M3UParseResult,
    overwrite: bool,
) -> Result<Json<ImportM3UResponse>, (StatusCode, Json<ErrorResponse>)> {
    let ctx = ServiceContext::new(db, config);
    let service = ChannelService::new(ctx);

    let mut imported = 0;
    let mut skipped = 0;
    let mut failed = 0;
    let mut errors = parse_result.errors.clone();

    for channel in parse_result.channels {
        let create_req = CreateChannelRequest {
            name: channel.name.clone(),
            url: channel.url.clone(),
            group_name: channel.group.clone(),
            logo_url: channel.logo.clone(),
        };

        match service.import_channel(create_req, overwrite).await {
            Ok(ImportChannelResult::Created) | Ok(ImportChannelResult::Updated) => imported += 1,
            Ok(ImportChannelResult::Skipped) => {
                skipped += 1;
                errors.push(format!("频道 {} 已存在，已跳过", channel.name));
            }
            Err(e) => {
                failed += 1;
                errors.push(format!("频道 {} 导入失败: {}", channel.name, e));
            }
        }
    }

    Ok(Json(ImportM3UResponse {
        imported,
        skipped,
        failed,
        errors,
    }))
}

// ===== 计划处理器 =====

pub async fn list_schedules(
    State((db, _scheduler, _process_manager, config)): State<AppState>,
) -> Result<Json<Vec<Schedule>>, (StatusCode, Json<ErrorResponse>)> {
    let ctx = ServiceContext::new(db, config);
    let service = ScheduleService::new(ctx);

    match service.list().await {
        Ok(schedules) => Ok(Json(schedules)),
        Err(e) => Err(internal_error(e)),
    }
}

pub async fn create_schedule(
    Extension(claims): Extension<Claims>,
    State((db, scheduler, _process_manager, config)): State<AppState>,
    Json(req): Json<CreateScheduleRequest>,
) -> Result<Json<Schedule>, (StatusCode, Json<ErrorResponse>)> {
    let ctx = ServiceContext::new(db.clone(), config.clone());
    let service = ScheduleService::new(ctx);

    let schedule = service.create(req.clone()).await.map_err(internal_error)?;

    // 添加到调度器
    if let Err(e) = scheduler.sync_schedule(&schedule).await {
        tracing_error!("Failed to sync schedule to scheduler: {}", e);
    }

    record_audit(
        db,
        config,
        Some(&claims),
        "schedule.create",
        "schedule",
        Some(&schedule.id),
        Some(format!("channel_id={}", schedule.channel_id)),
    )
    .await;

    Ok(Json(schedule))
}

pub async fn get_schedule(
    State((db, _scheduler, _process_manager, config)): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Schedule>, (StatusCode, Json<ErrorResponse>)> {
    let ctx = ServiceContext::new(db, config);
    let service = ScheduleService::new(ctx);

    match service.get_by_id(&id).await {
        Ok(schedule) => Ok(Json(schedule)),
        Err(e) => Err(not_found_error(e)),
    }
}

pub async fn update_schedule(
    Extension(claims): Extension<Claims>,
    State((db, scheduler, _process_manager, config)): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<CreateScheduleRequest>,
) -> Result<Json<Schedule>, (StatusCode, Json<ErrorResponse>)> {
    let ctx = ServiceContext::new(db.clone(), config.clone());
    let service = ScheduleService::new(ctx);

    let schedule = service.update(&id, req.clone()).await.map_err(internal_error)?;

    // 同步到调度器，确保禁用状态会移除旧 job
    if let Err(e) = scheduler.sync_schedule(&schedule).await {
        tracing_error!("Failed to sync updated schedule in scheduler: {}", e);
    }

    record_audit(db, config, Some(&claims), "schedule.update", "schedule", Some(&id), None).await;

    Ok(Json(schedule))
}

pub async fn delete_schedule(
    Extension(claims): Extension<Claims>,
    State((db, scheduler, _process_manager, config)): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let ctx = ServiceContext::new(db.clone(), config.clone());
    let service = ScheduleService::new(ctx);

    match service.delete(&id).await {
        Ok(_) => {
            if let Err(e) = scheduler.remove_schedule(&id).await {
                tracing_error!("Failed to remove schedule from scheduler: {}", e);
            }
            record_audit(db, config, Some(&claims), "schedule.delete", "schedule", Some(&id), None).await;
            Ok(StatusCode::NO_CONTENT)
        }
        Err(e) => Err(internal_error(e)),
    }
}

pub async fn toggle_schedule(
    Extension(claims): Extension<Claims>,
    State((db, scheduler, _process_manager, config)): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Schedule>, (StatusCode, Json<ErrorResponse>)> {
    let ctx = ServiceContext::new(db.clone(), config.clone());
    let service = ScheduleService::new(ctx);

    service.toggle_enabled(&id).await.map_err(internal_error)?;

    // 重新获取更新后的计划
    match service.get_by_id(&id).await {
        Ok(schedule) => {
            // 同步到调度器，确保开关状态和内存 job 一致
            if let Err(e) = scheduler.sync_schedule(&schedule).await {
                tracing_error!("Failed to sync toggled schedule to scheduler: {}", e);
            }
            record_audit(
                db,
                config,
                Some(&claims),
                "schedule.toggle",
                "schedule",
                Some(&id),
                Some(format!("enabled={}", schedule.enabled)),
            )
            .await;

            Ok(Json(schedule))
        }
        Err(e) => Err(internal_error(e)),
    }
}

// ===== 任务处理器 =====

pub async fn list_tasks(
    State((db, _scheduler, _process_manager, config)): State<AppState>,
) -> Result<Json<Vec<Task>>, (StatusCode, Json<ErrorResponse>)> {
    let ctx = ServiceContext::new(db, config);
    let service = RecordingService::new(_process_manager, ctx, None);

    match service.list_tasks().await {
        Ok(tasks) => Ok(Json(tasks)),
        Err(e) => Err(internal_error(e)),
    }
}

pub async fn get_task(
    State((db, _scheduler, _process_manager, config)): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Task>, (StatusCode, Json<ErrorResponse>)> {
    let ctx = ServiceContext::new(db, config);
    let service = RecordingService::new(_process_manager, ctx, None);

    match service.get_task(&id).await {
        Ok(task) => Ok(Json(task)),
        Err(e) => Err(not_found_error(e)),
    }
}

pub async fn cancel_task(
    Extension(claims): Extension<Claims>,
    State((db, _scheduler, process_manager, config)): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let ctx = ServiceContext::new(db.clone(), config.clone());
    let service = RecordingService::new(process_manager, ctx, None);

    match service.cancel(&id).await {
        Ok(_) => {
            record_audit(db, config, Some(&claims), "task.cancel", "task", Some(&id), None).await;
            Ok(StatusCode::NO_CONTENT)
        }
        Err(e) => Err(internal_error(e)),
    }
}

/// 清除已完成的任务记录
pub async fn clear_completed_tasks(
    Extension(claims): Extension<Claims>,
    State((db, _scheduler, _process_manager, config)): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let ctx = ServiceContext::new(db.clone(), config.clone());
    let process_manager = Arc::new(crate::core::process::ProcessManager::new(
        config.recorder.executable.clone(),
        config.storage.temp_dir.clone(),
    ));
    let service = RecordingService::new(process_manager, ctx, None);

    match service.clear_completed_tasks().await {
        Ok(count) => {
            record_audit(
                db,
                config,
                Some(&claims),
                "task.clear_completed",
                "task",
                None,
                Some(format!("deleted={count}")),
            )
            .await;
            Ok(Json(serde_json::json!({ "deleted": count })))
        }
        Err(e) => Err(internal_error(e)),
    }
}

/// 删除单条任务记录
pub async fn delete_task(
    Extension(claims): Extension<Claims>,
    State((db, _scheduler, _process_manager, config)): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let ctx = ServiceContext::new(db.clone(), config.clone());
    let process_manager = Arc::new(crate::core::process::ProcessManager::new(
        config.recorder.executable.clone(),
        config.storage.temp_dir.clone(),
    ));
    let service = RecordingService::new(process_manager, ctx, None);

    match service.delete_task(&id).await {
        Ok(_) => {
            record_audit(db, config, Some(&claims), "task.delete", "task", Some(&id), None).await;
            Ok(StatusCode::NO_CONTENT)
        }
        Err(e) => Err(internal_error(e)),
    }
}

pub async fn start_manual_record(
    Extension(claims): Extension<Claims>,
    State((db, _scheduler, process_manager, config)): State<AppState>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Json(req): Json<ManualRecordRequest>,
) -> Result<Json<Task>, (StatusCode, Json<ErrorResponse>)> {
    let ctx = ServiceContext::new(db.clone(), config.clone());
    let service = RecordingService::new(process_manager, ctx, Some(event_bus.sender()));

    match service.start_manual(req).await {
        Ok(task) => {
            record_audit(
                db,
                config,
                Some(&claims),
                "task.manual_start",
                "task",
                Some(&task.id),
                Some(format!("channel_id={}", task.channel_id)),
            )
            .await;
            Ok(Json(task))
        }
        Err(e) => Err(internal_error(e)),
    }
}

// ===== 调度器处理器 =====

/// 获取即将执行的任务
pub async fn get_upcoming(
    State((_db, scheduler, _process_manager, _config)): State<AppState>,
) -> Result<Json<Vec<UpcomingTask>>, (StatusCode, Json<ErrorResponse>)> {
    let trigger = CronTrigger::new(scheduler);

    match trigger.get_upcoming().await {
        Ok(upcoming) => Ok(Json(upcoming)),
        Err(e) => Err(internal_error(e)),
    }
}

/// 重新加载调度器
pub async fn reload_scheduler(
    Extension(claims): Extension<Claims>,
    State((db, scheduler, _process_manager, config)): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    match scheduler.reload().await {
        Ok(_) => {
            record_audit(db, config, Some(&claims), "scheduler.reload", "scheduler", None, None).await;
            Ok(Json(serde_json::json!({
                "status": "ok",
                "message": "调度器已重新加载"
            })))
        }
        Err(e) => Err(internal_error(e)),
    }
}

// ===== 配置处理器 =====

/// 获取系统配置
pub async fn get_config(
    State((db, _scheduler, _process_manager, config)): State<AppState>,
) -> Result<Json<crate::services::SystemConfig>, (StatusCode, Json<ErrorResponse>)> {
    let ctx = ServiceContext::new(db, config);
    let service = ConfigService::new(ctx);

    match service.get_config().await {
        Ok(config) => Ok(Json(config)),
        Err(e) => Err(internal_error(e)),
    }
}

/// 更新系统配置
pub async fn update_config(
    Extension(claims): Extension<Claims>,
    State((db, _scheduler, _process_manager, config)): State<AppState>,
    Json(req): Json<ConfigUpdateRequest>,
) -> Result<Json<crate::services::SystemConfig>, (StatusCode, Json<ErrorResponse>)> {
    let ctx = ServiceContext::new(db.clone(), config.clone());
    let service = ConfigService::new(ctx);

    match service.update_config(req).await {
        Ok(config_response) => {
            record_audit(db, config, Some(&claims), "config.update", "config", None, None).await;
            Ok(Json(config_response))
        }
        Err(e) => Err(internal_error(e)),
    }
}

pub async fn get_system_health(
    State((db, _scheduler, _process_manager, config)): State<AppState>,
) -> Result<Json<SystemHealth>, (StatusCode, Json<ErrorResponse>)> {
    let service = AuditService::new(ServiceContext::new(db, config));
    service.system_health().await.map(Json).map_err(internal_error)
}

pub async fn list_audit_logs(
    State((db, _scheduler, _process_manager, config)): State<AppState>,
) -> Result<Json<Vec<AuditLog>>, (StatusCode, Json<ErrorResponse>)> {
    let service = AuditService::new(ServiceContext::new(db, config));
    service.list_recent(200).await.map(Json).map_err(internal_error)
}

pub async fn run_cleanup(
    Extension(claims): Extension<Claims>,
    State((db, _scheduler, _process_manager, config)): State<AppState>,
    Extension(event_bus): Extension<Arc<EventBus>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let cleanup = CleanupService::new(
        ServiceContext::new(db.clone(), config.clone()),
        Some(event_bus.sender()),
    );
    let deleted = cleanup.run_once().await.map_err(internal_error)?;
    record_audit(
        db,
        config,
        Some(&claims),
        "cleanup.run",
        "cleanup",
        None,
        Some(format!("deleted={deleted}")),
    )
    .await;

    Ok(Json(serde_json::json!({
        "deleted": deleted,
        "message": format!("已清理 {} 条过期任务记录", deleted)
    })))
}

// ===== EPG 处理器 =====

#[derive(Debug, Deserialize)]
pub struct EpgProgramQuery {
    pub channel_ref: String,
    #[serde(default = "default_epg_limit")]
    pub limit: i64,
}

fn default_epg_limit() -> i64 {
    50
}

pub async fn list_epg_sources(
    State((db, _scheduler, _process_manager, config)): State<AppState>,
) -> Result<Json<Vec<EpgSource>>, (StatusCode, Json<ErrorResponse>)> {
    let service = EpgService::new(ServiceContext::new(db, config));
    service.list_sources().await.map(Json).map_err(internal_error)
}

pub async fn import_epg_source(
    Extension(claims): Extension<Claims>,
    State((db, _scheduler, _process_manager, config)): State<AppState>,
    Json(req): Json<ImportEpgRequest>,
) -> Result<Json<EpgSource>, (StatusCode, Json<ErrorResponse>)> {
    let service = EpgService::new(ServiceContext::new(db.clone(), config.clone()));
    let source = service.import_source(req).await.map_err(internal_error)?;
    record_audit(
        db,
        config,
        Some(&claims),
        "epg.import",
        "epg_source",
        Some(&source.id),
        Some(format!("name={}", source.name)),
    )
    .await;
    Ok(Json(source))
}

pub async fn list_epg_programs(
    State((db, _scheduler, _process_manager, config)): State<AppState>,
    Query(query): Query<EpgProgramQuery>,
) -> Result<Json<Vec<EpgProgram>>, (StatusCode, Json<ErrorResponse>)> {
    let service = EpgService::new(ServiceContext::new(db, config));
    service
        .list_programs(&query.channel_ref, query.limit)
        .await
        .map(Json)
        .map_err(internal_error)
}

// ===== WebSocket 处理器 =====

/// WebSocket 查询参数
#[derive(Debug, Deserialize)]
pub struct WsQuery {
    pub token: Option<String>,
}

pub async fn ws_handler(
    ws: axum::extract::WebSocketUpgrade,
    State((db, _scheduler, _process_manager, _config)): State<AppState>,
    Query(params): Query<WsQuery>,
    Extension(event_bus): Extension<Arc<EventBus>>,
) -> impl IntoResponse {
    // 验证 token
    let token = match &params.token {
        Some(t) => t,
        None => return StatusCode::UNAUTHORIZED.into_response(),
    };
    if AuthService::verify_token(token).is_err() {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    ws.on_upgrade(|socket| crate::api::websocket::handle_socket(socket, db, event_bus))
}

// ===== 辅助函数 =====

fn internal_error(err: anyhow::Error) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: "internal_error".to_string(),
            details: Some(err.to_string()),
        }),
    )
}

fn not_found_error(err: anyhow::Error) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            error: "not_found".to_string(),
            details: Some(err.to_string()),
        }),
    )
}

// ===== 流代理处理器 =====

/// 流代理请求参数
#[derive(Debug, Deserialize)]
pub struct StreamProxyQuery {
    /// 要代理的流 URL
    pub url: String,
    /// 查询参数 token，供播放器等无法自定义 Header 的场景使用
    pub token: Option<String>,
}

/// 流代理处理器 - 用于绕过 CORS 限制
pub async fn stream_proxy(
    headers: HeaderMap,
    Query(query): Query<StreamProxyQuery>,
) -> Result<Response<Body>, (StatusCode, Json<ErrorResponse>)> {
    authorize_stream_proxy(&headers, query.token.as_deref())?;
    validate_proxy_url(&query.url).await?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| internal_error(anyhow::anyhow!("创建 HTTP 客户端失败: {}", e)))?;

    let response = client
        .get(&query.url)
        .send()
        .await
        .map_err(|e| internal_error(anyhow::anyhow!("请求流失败: {}", e)))?;

    let status = response.status();
    let headers = response.headers().clone();

    let body_bytes = response
        .bytes()
        .await
        .map_err(|e| internal_error(anyhow::anyhow!("读取流数据失败: {}", e)))?;

    // 构建响应，复制必要的头信息
    let mut response_builder = Response::builder().status(status);

    // 复制 Content-Type 和其他重要头
    for (name, value) in headers.iter() {
        if name == "content-type" || name == "content-length" {
            response_builder = response_builder.header(name, value);
        }
    }

    // 添加 CORS 头
    response_builder = response_builder
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(header::ACCESS_CONTROL_ALLOW_METHODS, "GET, OPTIONS")
        .header(header::ACCESS_CONTROL_ALLOW_HEADERS, "*");

    response_builder
        .body(Body::from(body_bytes))
        .map_err(|e| internal_error(anyhow::anyhow!("构建响应失败: {}", e)))
}

fn authorize_stream_proxy(
    headers: &HeaderMap,
    query_token: Option<&str>,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    let bearer_token = headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));

    let token = bearer_token.or(query_token).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "unauthorized".to_string(),
                details: Some("缺少认证 Token".to_string()),
            }),
        )
    })?;

    AuthService::verify_token(token).map_err(|e| {
        (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "unauthorized".to_string(),
                details: Some(e.to_string()),
            }),
        )
    })?;

    Ok(())
}

async fn validate_proxy_url(url: &str) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    let parsed = url::Url::parse(url).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid_url".to_string(),
                details: Some(format!("无效的 URL: {}", e)),
            }),
        )
    })?;

    match parsed.scheme() {
        "http" | "https" => {}
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "invalid_url".to_string(),
                    details: Some("仅允许代理 HTTP/HTTPS 地址".to_string()),
                }),
            ));
        }
    }

    let host = parsed.host_str().ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid_url".to_string(),
                details: Some("URL 缺少主机名".to_string()),
            }),
        )
    })?;

    if is_disallowed_hostname(host) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "forbidden_target".to_string(),
                details: Some("不允许代理本地或内网地址".to_string()),
            }),
        ));
    }

    let port = parsed.port_or_known_default().unwrap_or(80);
    let addresses = tokio::net::lookup_host((host, port)).await.map_err(|e| {
        internal_error(anyhow::anyhow!("解析代理地址失败: {}", e))
    })?;

    for address in addresses {
        if is_private_ip(address.ip()) {
            return Err((
                StatusCode::FORBIDDEN,
                Json(ErrorResponse {
                    error: "forbidden_target".to_string(),
                    details: Some("不允许代理本地或内网地址".to_string()),
                }),
            ));
        }
    }

    Ok(())
}

fn is_disallowed_hostname(host: &str) -> bool {
    let normalized = host.trim().trim_end_matches('.').to_ascii_lowercase();
    normalized == "localhost"
        || normalized.ends_with(".localhost")
        || normalized.ends_with(".local")
        || normalized.ends_with(".internal")
}

fn is_private_ip(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.is_unspecified()
        }
        std::net::IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_unique_local()
                || v6.is_unicast_link_local()
                || v6.segments()[0] == 0x2001 && v6.segments()[1] == 0x0db8
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    #[tokio::test]
    async fn validate_proxy_url_rejects_localhost_and_non_http_schemes() {
        let localhost = validate_proxy_url("http://localhost/stream.m3u8").await;
        assert!(localhost.is_err());

        let ftp = validate_proxy_url("ftp://example.com/stream.ts").await;
        assert!(ftp.is_err());
    }

    #[test]
    fn private_ip_detection_blocks_internal_ranges() {
        assert!(is_private_ip(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))));
        assert!(is_private_ip(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 8))));
        assert!(is_private_ip(IpAddr::V6(Ipv6Addr::LOCALHOST)));
        assert!(!is_private_ip(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
    }
}

// ===== 转码处理器 =====

use crate::services::TranscodeService;
use tokio::fs;

/// 转码请求
#[derive(Debug, Deserialize)]
pub struct StartTranscodeRequest {
    /// 频道 ID
    pub channel_id: String,
    /// 流 URL
    pub url: String,
}

/// 转码响应
#[derive(Debug, serde::Serialize)]
pub struct TranscodeResponse {
    /// 会话 ID
    pub session_id: String,
    /// HLS 播放列表 URL
    pub playlist_url: String,
}

/// 启动转码
pub async fn start_transcode(
    State((_db, _scheduler, _process_manager, _config)): State<AppState>,
    Extension(transcode_service): Extension<Arc<TranscodeService>>,
    Json(req): Json<StartTranscodeRequest>,
) -> Result<Json<TranscodeResponse>, (StatusCode, Json<ErrorResponse>)> {
    let session = transcode_service
        .start_transcode(&req.channel_id, &req.url)
        .await
        .map_err(|e| internal_error(anyhow::anyhow!("启动转码失败: {}", e)))?;

    Ok(Json(TranscodeResponse {
        session_id: session.id.clone(),
        playlist_url: format!("/api/transcode/hls/{}/stream.m3u8", session.id),
    }))
}

/// 停止转码
pub async fn stop_transcode(
    State((_db, _scheduler, _process_manager, _config)): State<AppState>,
    Extension(transcode_service): Extension<Arc<TranscodeService>>,
    Path(session_id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    transcode_service
        .stop_transcode(&session_id)
        .await
        .map_err(|e| internal_error(anyhow::anyhow!("停止转码失败: {}", e)))?;

    Ok(StatusCode::NO_CONTENT)
}

/// 获取 HLS 文件 - 直接从文件系统读取，不需要会话跟踪
pub async fn get_hls_file(
    State((_db, _scheduler, _process_manager, config)): State<AppState>,
    Path((session_id, filename)): Path<(String, String)>,
) -> Result<Response<Body>, (StatusCode, Json<ErrorResponse>)> {
    tracing::info!("get_hls_file called: session_id={}, filename={}", session_id, filename);

    // 安全检查：防止路径遍历攻击
    if session_id.contains('.') || session_id.contains('/') || session_id.contains('\\') {
        tracing::warn!("Invalid session_id rejected: {}", session_id);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid_session".to_string(),
                details: Some("无效的会话 ID".to_string()),
            }),
        ));
    }

    if filename.contains('/') || filename.contains('\\') {
        tracing::warn!("Invalid filename rejected: {}", filename);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid_filename".to_string(),
                details: Some("无效的文件名".to_string()),
            }),
        ));
    }

    // 构建 HLS 目录路径
    let hls_dir = config.storage.temp_dir.join("hls");
    let file_path = hls_dir.join(&session_id).join(&filename);

    tracing::info!("Looking for HLS file at: {:?}", file_path);

    if !file_path.exists() {
        tracing::warn!("HLS file not found: {:?}", file_path);
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "not_found".to_string(),
                details: Some("HLS 文件未找到".to_string()),
            }),
        ));
    }

    let content = fs::read(&file_path)
        .await
        .map_err(|e| internal_error(anyhow::anyhow!("读取文件失败: {}", e)))?;

    tracing::info!("Successfully read HLS file, size: {} bytes", content.len());

    // 根据文件扩展名确定 Content-Type
    let content_type = if filename.ends_with(".m3u8") {
        "application/vnd.apple.mpegurl"
    } else if filename.ends_with(".ts") {
        "video/mp2t"
    } else {
        "application/octet-stream"
    };

    tracing::info!("Returning HLS file with content-type: {}", content_type);

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from(content))
        .map_err(|e| internal_error(anyhow::anyhow!("构建响应失败: {}", e)))
}

// ===== 认证处理器 =====

use crate::models::{LoginRequest, LoginResponse, ChangePasswordRequest, UserInfo};
use axum::Json as AxumJson;

/// 用户登录
pub async fn login(
    State((db, _scheduler, _process_manager, _config)): State<AppState>,
    AxumJson(req): AxumJson<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, Json<ErrorResponse>)> {
    let auth_service = AuthService::new(db);

    match auth_service.login(req).await {
        Ok(response) => Ok(Json(response)),
        Err(e) => Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "unauthorized".to_string(),
                details: Some(e.to_string()),
            }),
        )),
    }
}

/// 获取当前用户信息
pub async fn get_current_user(
    Extension(claims): Extension<Claims>,
    State((db, _scheduler, _process_manager, _config)): State<AppState>,
) -> Result<Json<UserInfo>, (StatusCode, Json<ErrorResponse>)> {
    let auth_service = AuthService::new(db);

    match auth_service.get_current_user(&claims.sub).await {
        Ok(user) => Ok(Json(user.into())),
        Err(e) => Err(internal_error(e)),
    }
}

/// 修改密码
pub async fn change_password(
    Extension(claims): Extension<Claims>,
    State((db, _scheduler, _process_manager, _config)): State<AppState>,
    AxumJson(req): AxumJson<ChangePasswordRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let auth_service = AuthService::new(db);

    match auth_service.change_password(&claims.sub, req).await {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "password_change_failed".to_string(),
                details: Some(e.to_string()),
            }),
        )),
    }
}

/// 更新用户昵称
#[derive(Debug, Deserialize)]
pub struct UpdateProfileRequest {
    pub nickname: Option<String>,
}

pub async fn update_profile(
    Extension(claims): Extension<Claims>,
    State((db, _scheduler, _process_manager, _config)): State<AppState>,
    AxumJson(req): AxumJson<UpdateProfileRequest>,
) -> Result<Json<UserInfo>, (StatusCode, Json<ErrorResponse>)> {
    let auth_service = AuthService::new(db);

    match auth_service.update_profile(&claims.sub, req.nickname.as_deref()).await {
        Ok(user) => Ok(Json(user.into())),
        Err(e) => Err(internal_error(e)),
    }
}
