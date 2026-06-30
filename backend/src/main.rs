//! IPTV Recorder - M3U管理与定时录制系统
//!
//! # 架构概览
//! ```text
//! ┌─────────────────────────────────────────────────────┐
//! │              Web Interface (Axum)                   │
//! │  ┌──────────┐  ┌──────────┐  ┌──────────────────┐  │
//! │  │   HTTP   │  │ WebSocket│  │  Static Files    │  │
//! │  └──────────┘  └──────────┘  └──────────────────┘  │
//! ├─────────────────────────────────────────────────────┤
//! │                   Business Logic                    │
//! │  ┌──────────┐  ┌──────────┐  ┌──────────────────┐  │
//! │  │ Channel  │  │ Schedule │  │  Recording       │  │
//! │  │ Manager  │  │  Engine  │  │    Manager       │  │
//! │  └──────────┘  └──────────┘  └──────────────────┘  │
//! ├─────────────────────────────────────────────────────┤
//! │                   Infrastructure                    │
//! │  ┌──────────┐  ┌──────────┐  ┌──────────────────┐  │
//! │  │Database  │  │ File     │  │  Event Bus       │  │
//! │  │(SQLite)  │  │ Storage  │  │  (broadcast)     │  │
//! │  └──────────┘  └──────────┘  └──────────────────┘  │
//! └─────────────────────────────────────────────────────┘
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]

use anyhow::Result;
use std::sync::Arc;
use tracing::{info, Level};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use crate::core::event::EventBus;

mod api;
mod config;
mod core;
mod models;
mod services;

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .with_writer(std::io::stdout)
                .with_ansi(true)
                .with_level(true)
                .with_target(true),
        )
        .with(EnvFilter::from_default_env().add_directive(Level::INFO.into()))
        .init();

    info!("🚀 IPTV Recorder starting...");

    // 加载配置
    let loaded = config::load()?;
    info!("📝 Configuration loaded from: {:?}", loaded.config_path);
    let config = loaded.config.clone();

    // 校验安全关键配置，避免使用弱默认值启动
    services::AuthService::validate_runtime_config()?;

    // 初始化数据库
    let db_path = config.database.path.to_string_lossy().to_string();
    let db = core::database::init(&db_path, config.database.pool_size).await?;
    info!(
        "🗄️  Database initialized: {}",
        config.database.path.display()
    );

    // 初始化 ProcessManager
    let recorder_path = config.recorder.executable.clone();
    let temp_dir = config.storage.temp_dir.clone();
    let process_manager = Arc::new(core::ProcessManager::new(recorder_path, temp_dir));
    info!("🎬 Process Manager initialized");

    // 创建事件总线
    let event_bus = Arc::new(EventBus::default());
    info!("📡 Event Bus initialized");

    let service_ctx = services::ServiceContext::new(db.clone(), config.clone());

    // 启动自动清理任务
    Arc::new(services::CleanupService::new(
        service_ctx.clone(),
        Some(event_bus.sender()),
    ))
    .start();
    info!("🧹 Cleanup Service initialized");

    // 启动后台巡检服务（磁盘空间定时巡检，独立窗口，与录制主流程解耦）
    Arc::new(services::HeartbeatService::new(
        service_ctx.clone(),
        Some(event_bus.sender()),
    ))
    .start();
    info!("💓 Heartbeat Inspection Service initialized");

    // 启动恢复：把上次进程残留的 running 任务批量标记为 failed。
    // 必须在调度器启动之前执行——否则僵尸 running 行会因 migration 0006 的
    // 部分唯一索引阻止对应频道/计划触发新录制。用 process_manager 构造一个
    // 临时 RecordingService 仅用于调用 reconcile（它不需要实际管理进程）。
    let recovery_svc = services::RecordingService::new(
        process_manager.clone(),
        service_ctx.clone(),
        Some(event_bus.sender()),
    );
    if let Err(e) = recovery_svc.reconcile_orphaned_tasks().await {
        tracing::warn!("启动恢复（清理僵尸 running 任务）失败: {}", e);
    }

    // 启动录制任务僵尸巡检服务：运行期持续检测"长时间无进度"的 running 任务
    Arc::new(services::TaskLivenessService::new(
        service_ctx.clone(),
        Some(event_bus.sender()),
    ))
    .start();
    info!("🧟‍♂️ Task Liveness Inspector initialized");

    // 启动 Cron 调度器
    let scheduler = Arc::new(
        services::SchedulerManager::new(db.clone(), config.clone(), process_manager.clone())
            .await?,
    );
    scheduler.start().await?;
    info!("📅 Cron Scheduler started");

    // 初始化转码服务
    let hls_dir = config.storage.preview_hls_dir();
    info!("🧠 Preview HLS temp dir: {}", hls_dir.display());
    let transcode_service = Arc::new(services::TranscodeService::new(hls_dir));
    transcode_service.clone().start_cleanup_task();
    info!("🎬 Transcode Service initialized");

    // 启动 Web 服务
    let app = api::router::create_router(
        db.clone(),
        scheduler.clone(),
        process_manager.clone(),
        config.clone(),
        transcode_service.clone(),
        event_bus.clone(),
    )
    .await?;
    let addr = format!("{}:{}", config.server.host, config.server.port);
    info!("🌐 Web server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    // into_make_service_with_connect_info 让 handler 能通过 ConnectInfo<SocketAddr> 获取客户端 IP(登录限流用)
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;

    Ok(())
}
