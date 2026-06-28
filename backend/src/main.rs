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
    axum::serve(listener, app).await?;

    Ok(())
}
