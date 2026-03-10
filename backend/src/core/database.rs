//! 数据库模块
//!
//! 使用 SQLite 作为持久化存储，通过 sqlx 进行类型安全的查询。

use sqlx::{sqlite::{SqliteConnectOptions, SqlitePoolOptions}, Pool, Sqlite};
use anyhow::Result;
use std::path::Path;

/// 数据库连接池类型
pub type Db = Pool<Sqlite>;

/// 初始化数据库
///
/// 创建数据库文件（如果不存在）并运行迁移
pub async fn init(db_path: &str) -> Result<Db> {
    // 解析完整路径
    let path = Path::new(db_path);

    // 获取绝对路径
    let absolute_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };

    // 确保父目录存在
    if let Some(parent) = absolute_path.parent() {
        if !parent.as_os_str().is_empty() {
            tokio::fs::create_dir_all(parent).await?;
            tracing::info!("Created data directory: {}", parent.display());
        }
    }

    // 配置 SQLite 连接选项
    let options = SqliteConnectOptions::new()
        .filename(&absolute_path)
        .create_if_missing(true);

    tracing::info!("Connecting to database: {}", absolute_path.display());

    // 创建连接池
    let pool = SqlitePoolOptions::new()
        .max_connections(10)
        .connect_with(options)
        .await?;

    // 运行迁移
    run_migrations(&pool).await?;

    Ok(pool)
}

/// 运行数据库迁移
async fn run_migrations(pool: &Db) -> Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS channels (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            url TEXT NOT NULL,
            group_name TEXT DEFAULT 'Uncategorized',
            logo_url TEXT,
            source_type TEXT DEFAULT 'remote_url',
            source_url TEXT,
            status TEXT DEFAULT 'unknown',
            last_check_at TEXT,
            fail_count INTEGER DEFAULT 0,
            metadata TEXT DEFAULT '{}',
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS schedules (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            channel_id TEXT NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
            cron_expression TEXT NOT NULL,
            duration_seconds INTEGER NOT NULL DEFAULT 3600,
            output_template TEXT DEFAULT '{channel_name}_{date}_{time}.mp4',
            priority INTEGER DEFAULT 5,
            enabled INTEGER DEFAULT 1,
            max_retry INTEGER DEFAULT 3,
            notify_on_complete INTEGER DEFAULT 0,
            video_quality TEXT DEFAULT 'best',
            audio_quality TEXT DEFAULT 'best',
            max_speed TEXT,
            thread_count INTEGER DEFAULT 20,
            transcode_mode TEXT DEFAULT 'off',
            transcode_preset TEXT DEFAULT 'medium',
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS tasks (
            id TEXT PRIMARY KEY,
            schedule_id TEXT REFERENCES schedules(id) ON DELETE SET NULL,
            channel_id TEXT NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
            status TEXT DEFAULT 'pending',
            started_at TEXT,
            ended_at TEXT,
            exit_code INTEGER,
            error_message TEXT,
            output_path TEXT,
            file_size INTEGER DEFAULT 0,
            duration_recorded INTEGER DEFAULT 0,
            progress_percent INTEGER DEFAULT 0,
            current_speed TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS recordings (
            id TEXT PRIMARY KEY,
            task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
            pid INTEGER,
            started_at TEXT NOT NULL,
            temp_path TEXT,
            log_path TEXT,
            last_progress_at TEXT,
            is_healthy INTEGER DEFAULT 1
        );

        CREATE INDEX IF NOT EXISTS idx_channels_status ON channels(status);
        CREATE INDEX IF NOT EXISTS idx_schedules_channel ON schedules(channel_id);
        CREATE INDEX IF NOT EXISTS idx_schedules_enabled ON schedules(enabled);
        CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
        CREATE INDEX IF NOT EXISTS idx_tasks_channel ON tasks(channel_id);
        CREATE INDEX IF NOT EXISTS idx_tasks_started ON tasks(started_at);

        -- 系统配置表
        CREATE TABLE IF NOT EXISTS system_config (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        -- 用户表
        CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY,
            username TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL,
            nickname TEXT,
            role TEXT DEFAULT 'user',
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE INDEX IF NOT EXISTS idx_users_username ON users(username);

        -- 插入默认配置值
        INSERT OR IGNORE INTO system_config (key, value) VALUES
            ('storage.recordings_path', './data/recordings'),
            ('storage.auto_cleanup_days', '30'),
            ('storage.min_free_space_gb', '10'),
            ('recording.default_duration_minutes', '60'),
            ('recording.n_m3u8dl_re_path', 'N_m3u8DL-RE'),
            ('recording.max_retry', '3'),
            ('recording.thread_count', '4'),
            ('notification.on_complete', 'true'),
            ('notification.on_failure', 'true'),
            ('notification.disk_warning', 'true');
        "#
    )
    .execute(pool)
    .await?;

    tracing::info!("Database migrations completed");

    // 添加 output_dir 字段到 schedules 表（如果不存在）
    let add_output_dir = sqlx::query(
        "ALTER TABLE schedules ADD COLUMN output_dir TEXT"
    )
    .execute(pool)
    .await;

    match add_output_dir {
        Ok(_) => tracing::info!("Added output_dir column to schedules table"),
        Err(e) => {
            // 如果字段已存在，会报错，可以忽略
            let err_str = e.to_string();
            if err_str.contains("duplicate column") || err_str.contains("already exists") {
                tracing::debug!("output_dir column already exists in schedules table");
            } else {
                tracing::warn!("Failed to add output_dir column: {}", e);
            }
        }
    }

    Ok(())
}
