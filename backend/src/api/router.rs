//! 路由定义

use axum::{
    middleware,
    routing::{get, post},
    Extension, Router,
};
use sqlx::{Pool, Sqlite};
use std::sync::Arc;
use tower_http::{cors::CorsLayer, services::ServeDir, trace::TraceLayer};

use crate::api::handlers::{
    batch_delete_channels,
    cancel_task,
    change_password,
    channel_stream,
    clear_completed_tasks,
    create_channel,
    create_schedule,
    delete_channel,
    delete_notification,
    delete_schedule,
    delete_task,
    get_channel,
    // 配置相关
    get_config,
    get_current_user,
    get_hls_file,
    get_schedule,
    get_system_health,
    get_task,
    // 调度器相关
    get_upcoming,
    import_epg_source,
    import_m3u_content,
    import_m3u_url,
    index_handler,
    list_audit_logs,
    // 频道相关
    list_channels,
    list_epg_programs,
    // EPG 相关
    list_epg_sources,
    list_groups,
    list_notifications,
    // 计划相关
    list_schedules,
    list_server_directories,
    // 任务相关
    list_tasks,
    // 认证相关
    login,
    mark_all_notifications_read,
    mark_notification_read,
    reload_scheduler,
    run_cleanup,
    start_manual_record,
    // 转码
    start_transcode,
    stop_transcode,
    // 流代理
    stream_proxy,
    test_channel,
    toggle_schedule,
    unread_notification_count,
    update_channel,
    update_config,
    update_profile,
    update_schedule,
    // WebSocket
    ws_handler,
};
use crate::config::Config;
use crate::core::event::EventBus;

use crate::api::auth_middleware::{admin_middleware, auth_middleware, operator_middleware};
use crate::core::ProcessManager;
use crate::services::{AuthService, SchedulerManager, TranscodeService};

pub async fn create_router(
    db: Pool<Sqlite>,
    scheduler: Arc<SchedulerManager>,
    process_manager: Arc<ProcessManager>,
    config: Config,
    transcode_service: Arc<TranscodeService>,
    event_bus: Arc<EventBus>,
) -> anyhow::Result<Router> {
    // 初始化默认管理员账号
    let auth_service = AuthService::new(db.clone());
    auth_service.init_default_admin().await?;

    // 公开路由（不需要认证）
    let public_routes = Router::new()
        // 首页
        .route("/", get(index_handler))
        // 登录
        .route("/api/auth/login", post(login))
        // 流代理（播放器通过 token 参数鉴权）
        .route("/api/proxy/stream", get(stream_proxy))
        .route("/api/channels/{id}/stream", get(channel_stream))
        // HLS 文件（直播流不需要认证）
        .route(
            "/api/transcode/hls/{session_id}/{filename}",
            get(get_hls_file),
        )
        // WebSocket 通过 query token 参数验证
        .route("/ws", get(ws_handler));

    // 需要认证的只读路由
    let authenticated_routes = Router::new()
        // ===== 认证 API =====
        .route("/api/auth/me", get(get_current_user))
        .route("/api/auth/password", post(change_password))
        .route("/api/auth/profile", post(update_profile))
        // ===== 频道 API =====
        .route("/api/channels", get(list_channels))
        .route("/api/channels/{id}", get(get_channel))
        .route("/api/channels/{id}/test", post(test_channel))
        .route("/api/channels/groups", get(list_groups))
        // ===== 计划 API =====
        .route("/api/schedules", get(list_schedules))
        .route("/api/schedules/{id}", get(get_schedule))
        // ===== 任务 API =====
        .route("/api/tasks", get(list_tasks))
        .route("/api/tasks/{id}", get(get_task))
        // ===== 调度器 API =====
        .route("/api/scheduler/upcoming", get(get_upcoming))
        // ===== 配置 API =====
        .route("/api/config", get(get_config))
        // ===== EPG API =====
        .route("/api/epg/sources", get(list_epg_sources))
        .route("/api/epg/programs", get(list_epg_programs))
        // ===== 通知 API（读） =====
        .route("/api/notifications", get(list_notifications))
        .route(
            "/api/notifications/unread-count",
            get(unread_notification_count),
        )
        // 添加认证中间件
        .layer(middleware::from_fn(auth_middleware));

    // 需要 operator/admin 的路由
    let operator_routes = Router::new()
        .route("/api/channels", post(create_channel))
        .route("/api/channels/batch-delete", post(batch_delete_channels))
        .route(
            "/api/channels/{id}",
            axum::routing::put(update_channel).delete(delete_channel),
        )
        .route("/api/channels/import/url", post(import_m3u_url))
        .route("/api/channels/import/content", post(import_m3u_content))
        .route("/api/schedules", post(create_schedule))
        .route(
            "/api/schedules/{id}",
            axum::routing::put(update_schedule).delete(delete_schedule),
        )
        .route("/api/schedules/{id}/toggle", post(toggle_schedule))
        .route("/api/tasks/clear", post(clear_completed_tasks))
        .route("/api/tasks/{id}", axum::routing::delete(delete_task))
        .route("/api/tasks/{id}/cancel", post(cancel_task))
        .route("/api/tasks/manual", post(start_manual_record))
        .route("/api/scheduler/reload", post(reload_scheduler))
        // ===== 转码 API（资源消耗型,提到 operator） =====
        .route("/api/transcode/start", post(start_transcode))
        .route("/api/transcode/{session_id}", post(stop_transcode))
        .route("/api/system/cleanup/run", post(run_cleanup))
        .route("/api/epg/sources", post(import_epg_source))
        // ===== 通知 API（写） =====
        .route(
            "/api/notifications/read-all",
            post(mark_all_notifications_read),
        )
        .route("/api/notifications/{id}/read", post(mark_notification_read))
        .route(
            "/api/notifications/{id}",
            axum::routing::delete(delete_notification),
        )
        // 添加认证中间件
        .layer(middleware::from_fn(operator_middleware))
        .layer(middleware::from_fn(auth_middleware));

    let admin_routes = Router::new()
        // 修改配置可重定向可执行文件(命令执行面),目录枚举泄露文件系统结构,均为 admin 专属
        .route("/api/config", post(update_config))
        .route("/api/system/directories", get(list_server_directories))
        .route("/api/system/health", get(get_system_health))
        .route("/api/audit/logs", get(list_audit_logs))
        .layer(middleware::from_fn(admin_middleware))
        .layer(middleware::from_fn(auth_middleware));

    // 合并路由
    let app = Router::new()
        .merge(public_routes)
        .merge(authenticated_routes)
        .merge(operator_routes)
        .merge(admin_routes)
        // 静态文件服务
        .nest_service("/static", ServeDir::new("static"))
        // 中间件
        .layer(Extension(transcode_service))
        .layer(Extension(event_bus))
        // DB pool 同时作为 Extension 提供,供认证中间件校验 token_version
        .layer(Extension(db.clone()))
        .layer(CorsLayer::permissive())
        // TraceLayer:自定义 span,屏蔽请求 URI 里的 token 参数(防止 JWT 进访问日志)
        .layer(
            TraceLayer::new_for_http().make_span_with(|request: &axum::extract::Request| {
                let uri = request.uri().to_string();
                let redacted = if let Some((path, query)) = uri.split_once('?') {
                    let cleaned: Vec<&str> = query
                        .split('&')
                        .map(|kv| {
                            if let Some((k, _)) = kv.split_once('=') {
                                if k.eq_ignore_ascii_case("token") {
                                    return "token=***";
                                }
                            }
                            kv
                        })
                        .collect();
                    format!("{}?{}", path, cleaned.join("&"))
                } else {
                    uri
                };
                tracing::info_span!("http_request", method = %request.method(), uri = %redacted)
            }),
        )
        .with_state((db, scheduler, process_manager, config));

    Ok(app)
}
