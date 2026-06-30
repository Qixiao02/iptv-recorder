//! HTTP 请求处理器

use axum::{
    body::Body,
    extract::{ConnectInfo, Extension, Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Json, Response},
};
use reqwest;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Sqlite};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{error as tracing_error, warn};

use crate::config::Config;
use crate::core::event::EventBus;
use crate::core::ProcessManager;
use crate::models::{
    Channel, CreateChannelRequest, CreateScheduleRequest, EpgProgram, EpgSource, ErrorResponse,
    ImportM3UResponse, ManualRecordRequest, PaginatedTasks, Schedule, SystemHealth, Task,
    TaskListParams,
};
use crate::services::{
    AuditService, AuthService, ChannelService, ChannelTestResult, Claims, CleanupService,
    ConfigService, ConfigUpdateRequest, CronTrigger, EpgService, ImportEpgRequest, M3UParser,
    NotificationPaginationParams, NotificationService, PaginatedAuditLogs, PaginatedNotifications,
    PaginationParams, RecordingService, ScheduleService, SchedulerManager, ServiceContext,
    UnreadCount, UpcomingTask,
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
        .record(
            claims,
            action,
            resource_type,
            resource_id,
            details.as_deref(),
        )
        .await
    {
        tracing_error!("Failed to record audit log: {}", e);
    }
}

/// SPA 首页处理器
///
/// 读取前端构建产物 `static/index.html` 返回。
/// 作为路由树的 `fallback` 使用:除 `/api/*`、`/static/*` 等具名路由外,
/// 任意前端路由(如 `/channels`、`/tasks`)刷新时都回退到此,
/// 由 react-router 在客户端接管路由,实现 SPA history 模式。
pub async fn spa_index_handler() -> Response {
    match tokio::fs::read("static/index.html").await {
        Ok(body) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            // index.html 不强缓存:每次回源验证(304),
            // 保证发版后浏览器立即拿到引用新 hash chunk 的新版 index.html。
            .header(header::CACHE_CONTROL, "no-cache")
            .body(Body::from(body))
            .unwrap(),
        Err(e) => {
            // static 目录缺失通常意味着:前端未构建,或镜像构建阶段未 COPY dist。
            warn!("SPA 入口缺失 static/index.html: {}", e);
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
                .body(Body::from(
                    "Frontend build not found. \
                     Run `pnpm build` in frontend/ and ensure the Dockerfile \
                     copies `dist` into `static/`.",
                ))
                .unwrap()
        }
    }
}

// ===== 频道处理器 =====

#[derive(Debug, Deserialize)]
pub struct BatchDeleteChannelsRequest {
    pub ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct BatchDeleteChannelsResponse {
    pub deleted: u64,
}

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

/// 获取全部频道(不分页)
///
/// 供前端下拉选择器(如新建计划选频道、Dashboard 统计、任务页)使用。
/// 与分页接口 `/api/channels` 区分:分页接口的 page_size 上限为 100(防止
/// 单次查询过大),当下拉需要展示全部频道(可能数百个)时,分页接口会截断,
/// 导致部分频道搜不到。此接口直接返回全部频道,无截断。
pub async fn list_all_channels(
    State((db, _scheduler, _process_manager, config)): State<AppState>,
) -> Result<Json<Vec<Channel>>, (StatusCode, Json<ErrorResponse>)> {
    let ctx = ServiceContext::new(db, config);
    let service = ChannelService::new(ctx);
    service.list().await.map(Json).map_err(internal_error)
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
            record_audit(
                db,
                config,
                Some(&claims),
                "channel.update",
                "channel",
                Some(&id),
                None,
            )
            .await;
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
            record_audit(
                db,
                config,
                Some(&claims),
                "channel.delete",
                "channel",
                Some(&id),
                None,
            )
            .await;
            Ok(StatusCode::NO_CONTENT)
        }
        Err(e) => Err(internal_error(e)),
    }
}

pub async fn batch_delete_channels(
    Extension(claims): Extension<Claims>,
    State((db, _scheduler, _process_manager, config)): State<AppState>,
    Json(req): Json<BatchDeleteChannelsRequest>,
) -> Result<Json<BatchDeleteChannelsResponse>, (StatusCode, Json<ErrorResponse>)> {
    let ids: Vec<String> = req
        .ids
        .into_iter()
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect();

    if ids.is_empty() {
        return Ok(Json(BatchDeleteChannelsResponse { deleted: 0 }));
    }

    let ctx = ServiceContext::new(db.clone(), config.clone());
    let service = ChannelService::new(ctx);

    match service.delete_many(&ids).await {
        Ok(deleted) => {
            record_audit(
                db,
                config,
                Some(&claims),
                "channel.batch_delete",
                "channel",
                None,
                Some(format!("requested={}, deleted={}", ids.len(), deleted)),
            )
            .await;
            Ok(Json(BatchDeleteChannelsResponse { deleted }))
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
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "missing_url".to_string(),
                details: Some("必须提供 URL 或 content".to_string()),
            }),
        )
    })?;

    // 解析 M3U
    let parse_result = M3UParser::from_url(url)
        .await
        .map_err(|e| internal_error(anyhow::anyhow!("解析 M3U 失败: {}", e)))?;

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
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "missing_content".to_string(),
                details: Some("必须提供 content 或 URL".to_string()),
            }),
        )
    })?;

    // 解析 M3U
    let parse_result = M3UParser::parse(content)
        .map_err(|e| internal_error(anyhow::anyhow!("解析 M3U 失败: {}", e)))?;

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

    let mut errors = parse_result.errors.clone();
    let requests = parse_result
        .channels
        .into_iter()
        .map(|channel| CreateChannelRequest {
            name: channel.name,
            url: channel.url,
            group_name: channel.group,
            logo_url: channel.logo,
            source_visibility: "public".to_string(),
            playback_strategy: "auto".to_string(),
        })
        .collect();

    let result = service
        .import_channels_batch(requests, overwrite)
        .await
        .map_err(internal_error)?;
    errors.extend(result.errors);

    Ok(Json(ImportM3UResponse {
        imported: result.imported,
        skipped: result.skipped,
        failed: result.failed,
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

    let schedule = service
        .update(&id, req.clone())
        .await
        .map_err(internal_error)?;

    // 同步到调度器，确保禁用状态会移除旧 job
    if let Err(e) = scheduler.sync_schedule(&schedule).await {
        tracing_error!("Failed to sync updated schedule in scheduler: {}", e);
    }

    record_audit(
        db,
        config,
        Some(&claims),
        "schedule.update",
        "schedule",
        Some(&id),
        None,
    )
    .await;

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
            record_audit(
                db,
                config,
                Some(&claims),
                "schedule.delete",
                "schedule",
                Some(&id),
                None,
            )
            .await;
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
    Query(params): Query<TaskListParams>,
) -> Result<Json<PaginatedTasks>, (StatusCode, Json<ErrorResponse>)> {
    let ctx = ServiceContext::new(db, config);
    let service = RecordingService::new(_process_manager, ctx, None);

    match service.list_tasks_paginated(params).await {
        Ok(result) => Ok(Json(result)),
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
            record_audit(
                db,
                config,
                Some(&claims),
                "task.cancel",
                "task",
                Some(&id),
                None,
            )
            .await;
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
            record_audit(
                db,
                config,
                Some(&claims),
                "task.delete",
                "task",
                Some(&id),
                None,
            )
            .await;
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
    let channel_id = req.channel_id.clone();
    let schedule_id = req.schedule_id.clone();

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
        Err(e) => {
            warn!(
                "手动/计划立即执行录制失败: channel_id={}, schedule_id={:?}, error={}",
                channel_id, schedule_id, e
            );
            Err(internal_error(e))
        }
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
            record_audit(
                db,
                config,
                Some(&claims),
                "scheduler.reload",
                "scheduler",
                None,
                None,
            )
            .await;
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
            record_audit(
                db,
                config,
                Some(&claims),
                "config.update",
                "config",
                None,
                None,
            )
            .await;
            Ok(Json(config_response))
        }
        Err(e) => Err(internal_error(e)),
    }
}

#[derive(Debug, Deserialize)]
pub struct DirectoryListQuery {
    path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DirectoryEntry {
    name: String,
    path: String,
}

#[derive(Debug, Serialize)]
pub struct DirectoryListResponse {
    current_path: String,
    parent_path: Option<String>,
    entries: Vec<DirectoryEntry>,
}

pub async fn list_server_directories(
    Query(query): Query<DirectoryListQuery>,
) -> Result<Json<DirectoryListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let target = resolve_directory_query(query.path.as_deref()).map_err(bad_request_error)?;

    if cfg!(windows) && query.path.as_deref().unwrap_or("").trim().is_empty() {
        return Ok(Json(DirectoryListResponse {
            current_path: String::new(),
            parent_path: None,
            entries: list_windows_drives(),
        }));
    }

    let metadata = tokio::fs::metadata(&target).await.map_err(|e| {
        internal_error(anyhow::anyhow!(
            "无法读取服务器目录 {}: {}",
            target.display(),
            e
        ))
    })?;
    if !metadata.is_dir() {
        return Err(bad_request_error(anyhow::anyhow!(
            "路径不是服务器目录: {}",
            target.display()
        )));
    }

    let mut entries = Vec::new();
    // tokio::fs::read_dir 返回异步迭代器,用 next_entry().await 逐项读取,
    // 不阻塞 Tokio worker(对照原先 std::fs::read_dir 同步迭代)。
    let mut read_dir = tokio::fs::read_dir(&target).await.map_err(|e| {
        internal_error(anyhow::anyhow!(
            "无法列出服务器目录 {}: {}",
            target.display(),
            e
        ))
    })?;

    while let Ok(Some(entry)) = read_dir.next_entry().await {
        let file_type = entry.file_type().await;
        let is_dir = match file_type {
            Ok(ft) => ft.is_dir(),
            // 无法判定类型时回退:取路径再 stat(慢路径,极少触发)
            Err(_) => entry.path().is_dir(),
        };
        if is_dir {
            let path = entry.path();
            entries.push(DirectoryEntry {
                name: entry.file_name().to_string_lossy().to_string(),
                path: path.to_string_lossy().to_string(),
            });
        }
    }

    entries.sort_by_key(|entry| entry.name.to_ascii_lowercase());
    let current_path = target.to_string_lossy().to_string();
    let parent_path = target
        .parent()
        .map(|path| path.to_string_lossy().to_string());

    Ok(Json(DirectoryListResponse {
        current_path,
        parent_path,
        entries,
    }))
}

fn resolve_directory_query(path: Option<&str>) -> Result<PathBuf, anyhow::Error> {
    let Some(path) = path.map(str::trim).filter(|path| !path.is_empty()) else {
        return std::env::current_dir().map_err(Into::into);
    };

    let candidate = PathBuf::from(path);
    let resolved = if candidate.is_absolute() {
        candidate
    } else {
        std::env::current_dir()?.join(candidate)
    };

    Ok(resolved)
}

fn list_windows_drives() -> Vec<DirectoryEntry> {
    #[cfg(windows)]
    {
        let mut entries: Vec<DirectoryEntry> = ('A'..='Z')
            .filter_map(|letter| {
                let path = format!("{}:\\", letter);
                if std::path::Path::new(&path).is_dir() {
                    Some(DirectoryEntry {
                        name: path.clone(),
                        path,
                    })
                } else {
                    None
                }
            })
            .collect();

        // 追加已映射的网络共享(UNC 路径),让用户能在根视图直接看到并进入网络位置。
        // 通过 PowerShell Get-SmbMapping 列出当前用户的 SMB 映射;失败则静默跳过(不影响盘符列表)。
        if let Ok(output) = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Get-SmbMapping | Select-Object -ExpandProperty RemotePath | Sort-Object",
            ])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    let share = line.trim();
                    if !share.is_empty() && std::path::Path::new(share).is_dir() {
                        entries.push(DirectoryEntry {
                            name: format!("🌐 {}", share),
                            path: share.to_string(),
                        });
                    }
                }
            }
        }

        entries
    }

    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

pub async fn get_system_health(
    State((db, _scheduler, _process_manager, config)): State<AppState>,
) -> Result<Json<SystemHealth>, (StatusCode, Json<ErrorResponse>)> {
    let service = AuditService::new(ServiceContext::new(db, config));
    service
        .system_health()
        .await
        .map(Json)
        .map_err(internal_error)
}

pub async fn list_audit_logs(
    State((db, _scheduler, _process_manager, config)): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedAuditLogs>, (StatusCode, Json<ErrorResponse>)> {
    let service = AuditService::new(ServiceContext::new(db, config));
    service
        .list_paginated(params)
        .await
        .map(Json)
        .map_err(internal_error)
}

/// 分页查询应用内通知（最新在前）
pub async fn list_notifications(
    State((db, _scheduler, _process_manager, config)): State<AppState>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Query(params): Query<NotificationPaginationParams>,
) -> Result<Json<PaginatedNotifications>, (StatusCode, Json<ErrorResponse>)> {
    let service =
        NotificationService::new(ServiceContext::new(db, config), Some(event_bus.sender()));
    service
        .list_paginated(params)
        .await
        .map(Json)
        .map_err(internal_error)
}

/// 未读通知数量
pub async fn unread_notification_count(
    State((db, _scheduler, _process_manager, config)): State<AppState>,
    Extension(event_bus): Extension<Arc<EventBus>>,
) -> Result<Json<UnreadCount>, (StatusCode, Json<ErrorResponse>)> {
    let service =
        NotificationService::new(ServiceContext::new(db, config), Some(event_bus.sender()));
    service
        .unread_count()
        .await
        .map(Json)
        .map_err(internal_error)
}

/// 标记单条通知已读
pub async fn mark_notification_read(
    State((db, _scheduler, _process_manager, config)): State<AppState>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let service =
        NotificationService::new(ServiceContext::new(db, config), Some(event_bus.sender()));
    let updated = service.mark_read(&id).await.map_err(internal_error)?;
    Ok(Json(serde_json::json!({ "updated": updated })))
}

/// 全部通知标记已读
pub async fn mark_all_notifications_read(
    State((db, _scheduler, _process_manager, config)): State<AppState>,
    Extension(event_bus): Extension<Arc<EventBus>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let service =
        NotificationService::new(ServiceContext::new(db, config), Some(event_bus.sender()));
    let updated = service.mark_all_read().await.map_err(internal_error)?;
    Ok(Json(serde_json::json!({ "updated": updated })))
}

/// 删除单条通知
pub async fn delete_notification(
    State((db, _scheduler, _process_manager, config)): State<AppState>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let service =
        NotificationService::new(ServiceContext::new(db, config), Some(event_bus.sender()));
    let deleted = service.delete(&id).await.map_err(internal_error)?;
    Ok(Json(serde_json::json!({ "deleted": deleted })))
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
    service
        .list_sources()
        .await
        .map(Json)
        .map_err(internal_error)
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
    State((db, _scheduler, _process_manager, config)): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<WsQuery>,
    Extension(event_bus): Extension<Arc<EventBus>>,
) -> impl IntoResponse {
    // Origin 校验:只允许 CORS allowlist 内的源(防 CSWSH 跨站 WebSocket 劫持)
    if let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
        if !config.server.cors_origins.iter().any(|o| o == origin) {
            return StatusCode::FORBIDDEN.into_response();
        }
    }

    // 验证 token(含 token_version 吊销检查)
    let token = match &params.token {
        Some(t) => t,
        None => return StatusCode::UNAUTHORIZED.into_response(),
    };
    let auth_service = AuthService::new(db.clone());
    let claims = match auth_service.verify_token_with_db(token).await {
        Ok(c) => c,
        Err(_) => return StatusCode::UNAUTHORIZED.into_response(),
    };

    // 传 db + claims 进 handle_socket,用于连接后周期性复查 token_version
    ws.on_upgrade(move |socket| {
        crate::api::websocket::handle_socket(socket, db, event_bus, claims)
    })
}

// ===== 辅助函数 =====

/// 内部错误:不向客户端泄露原始错误(DB 错误/文件路径/SQL 片段等),
/// 只返回通用文案;原始错误记入服务端日志便于排查。
fn internal_error(err: anyhow::Error) -> (StatusCode, Json<ErrorResponse>) {
    tracing::error!("Internal error: {:?}", err);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: "internal_error".to_string(),
            details: Some("服务器内部错误,请稍后重试或联系管理员".to_string()),
        }),
    )
}

/// 资源不存在:同样不回传可能含内部信息的原始错误,记日志后返回通用文案。
fn not_found_error(err: anyhow::Error) -> (StatusCode, Json<ErrorResponse>) {
    tracing::warn!("Resource not found: {:?}", err);
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            error: "not_found".to_string(),
            details: Some("请求的资源不存在".to_string()),
        }),
    )
}

/// 请求参数错误:保留原始错误(用户输入错误,需告知具体原因以便修正)
fn bad_request_error(err: anyhow::Error) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: "bad_request".to_string(),
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
    State((db, _scheduler, _process_manager, _config)): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<StreamProxyQuery>,
) -> Result<Response<Body>, (StatusCode, Json<ErrorResponse>)> {
    let _claims = authorize_stream_proxy(&db, &headers, query.token.as_deref()).await?;
    validate_proxy_url(&query.url).await?;

    proxy_stream_response(&query.url).await
}

pub async fn channel_stream(
    State((db, _scheduler, _process_manager, config)): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<WsQuery>,
) -> Result<Response<Body>, (StatusCode, Json<ErrorResponse>)> {
    let _claims = authorize_stream_proxy(&db, &headers, query.token.as_deref()).await?;

    let ctx = ServiceContext::new(db, config);
    let service = ChannelService::new(ctx);
    let channel = service.get_by_id(&id).await.map_err(not_found_error)?;

    if channel.playback_strategy == "record_only" {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "playback_disabled".to_string(),
                details: Some("该频道仅允许录制，不允许在线预览".to_string()),
            }),
        ));
    }

    // 安全校验:私有服务器场景(内网流)只校验 scheme,避免 file:// 等;
    // 其它场景用严格 SSRF 校验(拦截内网/localhost/私有 IP)。
    use crate::services::url_safety::assert_safe_url_scheme_only;
    if channel.source_visibility == "private_server_only" {
        assert_safe_url_scheme_only(&channel.url).map_err(bad_request_error)?;
    } else {
        validate_proxy_url(&channel.url).await?;
    }

    proxy_stream_response(&channel.url).await
}

async fn proxy_stream_response(
    url: &str,
) -> Result<Response<Body>, (StatusCode, Json<ErrorResponse>)> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| internal_error(anyhow::anyhow!("创建 HTTP 客户端失败: {}", e)))?;

    let response = client
        .get(url)
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

async fn authorize_stream_proxy(
    db: &Pool<Sqlite>,
    headers: &HeaderMap,
    query_token: Option<&str>,
) -> Result<Claims, (StatusCode, Json<ErrorResponse>)> {
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

    let auth_service = AuthService::new(db.clone());
    auth_service.verify_token_with_db(token).await.map_err(|e| {
        (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "unauthorized".to_string(),
                details: Some(e.to_string()),
            }),
        )
    })
}

/// 校验流代理 URL 是否安全(SSRF 防护):委托给 services::url_safety,
/// 把 anyhow 错误转换为 HTTP 响应(保持原有错误码语义)。
async fn validate_proxy_url(url: &str) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    use crate::services::url_safety::assert_safe_url;

    assert_safe_url(url).await.map_err(|e| {
        let msg = e.to_string();
        let status = if msg.contains("无效的 URL") || msg.contains("缺少主机名") || msg.contains("不支持") {
            StatusCode::BAD_REQUEST
        } else {
            StatusCode::FORBIDDEN
        };
        let code = if status == StatusCode::BAD_REQUEST {
            "invalid_url"
        } else {
            "forbidden_target"
        };
        (
            status,
            Json(ErrorResponse {
                error: code.to_string(),
                details: Some(msg),
            }),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::url_safety::is_private_ip;
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
}

/// 转码响应
#[derive(Debug, serde::Serialize)]
pub struct TranscodeResponse {
    /// 会话 ID
    pub session_id: String,
    /// HLS 播放列表 URL
    pub playlist_url: String,
    /// 同频道是否正在录制
    pub recording_active: bool,
}

/// 启动转码
pub async fn start_transcode(
    Extension(claims): Extension<Claims>,
    State((db, _scheduler, _process_manager, config)): State<AppState>,
    Extension(transcode_service): Extension<Arc<TranscodeService>>,
    Json(req): Json<StartTranscodeRequest>,
) -> Result<Json<TranscodeResponse>, (StatusCode, Json<ErrorResponse>)> {
    let ctx = ServiceContext::new(db.clone(), config.clone());
    let channel_service = ChannelService::new(ctx);
    let channel = channel_service
        .get_by_id(&req.channel_id)
        .await
        .map_err(not_found_error)?;

    if channel.playback_strategy == "record_only" {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "playback_disabled".to_string(),
                details: Some("该频道仅允许录制，不提供在线预览".to_string()),
            }),
        ));
    }

    let default_ffmpeg_path = if config.recorder.post_process.ffmpeg_path.is_empty() {
        "ffmpeg".to_string()
    } else {
        config.recorder.post_process.ffmpeg_path.clone()
    };
    let ffmpeg_path: String = sqlx::query_scalar("SELECT value FROM system_config WHERE key = ?")
        .bind("recorder.post_process.ffmpeg_path")
        .fetch_optional(&db)
        .await
        .map_err(|e| internal_error(anyhow::anyhow!("读取 FFmpeg 配置失败: {}", e)))?
        .filter(|value: &String| !value.trim().is_empty())
        .unwrap_or(default_ffmpeg_path);
    let ffmpeg_path = resolve_ffmpeg_executable(&ffmpeg_path);
    let recording_active = is_channel_recording(&db, &channel.id)
        .await
        .map_err(|e| internal_error(anyhow::anyhow!("读取录制状态失败: {}", e)))?;

    let session = transcode_service
        .start_transcode(
            &req.channel_id,
            &channel.url,
            &claims.sub,
            &claims.username,
            &ffmpeg_path,
        )
        .await
        .map_err(|e| internal_error(anyhow::anyhow!("启动转码失败: {}", e)))?;

    record_audit(
        db,
        config,
        Some(&claims),
        "playback.session_start",
        "channel",
        Some(&channel.id),
        Some(format!(
            "session_id={}, visibility={}, strategy={}, recording_active={}",
            session.id, channel.source_visibility, channel.playback_strategy, recording_active
        )),
    )
    .await;

    Ok(Json(TranscodeResponse {
        session_id: session.id.clone(),
        playlist_url: format!("/api/transcode/hls/{}/stream.m3u8", session.id),
        recording_active,
    }))
}

async fn is_channel_recording(db: &Pool<Sqlite>, channel_id: &str) -> Result<bool, sqlx::Error> {
    let existing: Option<(String,)> =
        sqlx::query_as("SELECT id FROM tasks WHERE channel_id = ? AND status = 'running' LIMIT 1")
            .bind(channel_id)
            .fetch_optional(db)
            .await?;

    Ok(existing.is_some())
}

fn resolve_ffmpeg_executable(configured_path: &str) -> PathBuf {
    let configured_path = configured_path.trim();
    if configured_path.is_empty() {
        return PathBuf::from("ffmpeg");
    }

    let path = PathBuf::from(configured_path);
    if path.is_absolute() && !path.is_file() && command_exists("ffmpeg") {
        warn!(
            "Configured FFmpeg path does not exist in this runtime: {}. Falling back to ffmpeg from PATH.",
            path.display()
        );
        return PathBuf::from("ffmpeg");
    }

    path
}

fn command_exists(command: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(command).is_file()))
        .unwrap_or(false)
}

/// 停止转码
pub async fn stop_transcode(
    Extension(claims): Extension<Claims>,
    State((db, _scheduler, _process_manager, config)): State<AppState>,
    Extension(transcode_service): Extension<Arc<TranscodeService>>,
    Path(session_id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let session = transcode_service
        .get_session(&session_id)
        .await
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "session_not_found".to_string(),
                    details: Some("转码会话不存在".to_string()),
                }),
            )
        })?;

    if claims.sub != session.owner_user_id && !claims.can_manage_content() {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "forbidden".to_string(),
                details: Some("不能停止其他用户的预览会话".to_string()),
            }),
        ));
    }

    transcode_service
        .stop_transcode(&session_id)
        .await
        .map_err(|e| internal_error(anyhow::anyhow!("停止转码失败: {}", e)))?;

    record_audit(
        db,
        config,
        Some(&claims),
        "playback.session_stop",
        "channel",
        Some(&session.channel_id),
        Some(format!(
            "session_id={}, owner={}",
            session.id, session.owner_username
        )),
    )
    .await;

    Ok(StatusCode::NO_CONTENT)
}

/// 获取 HLS 文件 - 直接从文件系统读取，不需要会话跟踪
pub async fn get_hls_file(
    State((_db, _scheduler, _process_manager, config)): State<AppState>,
    Extension(transcode_service): Extension<Arc<TranscodeService>>,
    Path((session_id, filename)): Path<(String, String)>,
) -> Result<Response<Body>, (StatusCode, Json<ErrorResponse>)> {
    // 客户端正在拉取分片/播放列表 → 刷新空闲计时器，
    // 防止 cleanup 在用户还在观看时把会话回收掉（曾经的 5 分钟 manifestLoadError）。
    transcode_service.touch_session(&session_id).await;

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
    let hls_dir = config.storage.preview_hls_dir();
    let file_path = hls_dir.join(&session_id).join(&filename);

    // 关键性能点：这里是播放热路径，每个分片都会被请求多次（m3u8 + 每个 .ts）。
    // 之前每个请求都打 4 条 INFO 日志并格式化 PathBuf，会同步阻塞响应 → 播放卡顿。
    // 现在只在 DEBUG 级别输出，生产环境默认 INFO 看不到，零开销。
    tracing::debug!("get_hls_file: {} / {}", session_id, filename);

    if !file_path.exists() {
        // 404 仍然需要 warn，方便排查"为什么播放器请求的分片没了"
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

    // 关键容错：当上游(UDP-over-HTTP 网关)周期性重置连接时，FFmpeg 在重连的几秒内
    // 仍会按 hls_time 切出"空分片"或仅含头的废分片(0~4KB)。这些坏分片一旦被 hls.js
    // 下载并尝试解码，会导致缓冲空洞/卡死。
    // 对策：检测到异常小的 .ts 分片时返回 404，让 hls.js 走 fragLoadingMaxRetry 重试，
    // 在重试窗口内 ffmpeg 通常已完成重连并产出后续正常分片，播放器就能跳过中断段继续。
    // 阈值 10KB：正常 1080p TS 分片(@6s)约 6MB，10KB 以下的 .ts 必然是无有效数据的废片。
    if filename.ends_with(".ts") && content.len() < 10_000 {
        tracing::debug!(
            "丢弃空分片(上游中断产物): {} {} bytes",
            filename,
            content.len()
        );
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "not_found".to_string(),
                details: Some("HLS 文件未找到".to_string()),
            }),
        ));
    }

    // 根据文件扩展名确定 Content-Type
    let content_type = if filename.ends_with(".m3u8") {
        "application/vnd.apple.mpegurl"
    } else if filename.ends_with(".ts") {
        "video/mp2t"
    } else {
        "application/octet-stream"
    };

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        // 播放列表必须不缓存（直播 m3u8 持续更新），分片可短缓存避免重复请求。
        .header(
            header::CACHE_CONTROL,
            if filename.ends_with(".m3u8") {
                "no-cache"
            } else {
                "public, max-age=60"
            },
        )
        .body(Body::from(content))
        .map_err(|e| internal_error(anyhow::anyhow!("构建响应失败: {}", e)))
}

// ===== 认证处理器 =====

use crate::models::{ChangePasswordRequest, LoginRequest, LoginResponse, UserInfo};
use axum::Json as AxumJson;

/// 用户登录
pub async fn login(
    State((db, _scheduler, _process_manager, _config)): State<AppState>,
    Extension(login_limiter): Extension<std::sync::Arc<crate::api::rate_limit::LoginRateLimiter>>,
    ConnectInfo(addr): ConnectInfo<std::net::SocketAddr>,
    AxumJson(req): AxumJson<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, Json<ErrorResponse>)> {
    let client_ip = addr.ip().to_string();

    // 限流检查:被锁定则直接拒绝
    if let Err(msg) = login_limiter.check(&client_ip).await {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(ErrorResponse {
                error: "rate_limited".to_string(),
                details: Some(msg),
            }),
        ));
    }

    let auth_service = AuthService::new(db);

    match auth_service.login(req).await {
        Ok(response) => {
            login_limiter.record_success(&client_ip).await;
            Ok(Json(response))
        }
        Err(e) => {
            login_limiter.record_failure(&client_ip).await;
            Err((
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: "unauthorized".to_string(),
                    details: Some(e.to_string()),
                }),
            ))
        }
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

    match auth_service
        .update_profile(&claims.sub, req.nickname.as_deref())
        .await
    {
        Ok(user) => Ok(Json(user.into())),
        Err(e) => Err(internal_error(e)),
    }
}
