//! 数据库模块
//!
//! 使用 SQLite 作为持久化存储，通过 sqlx 进行类型安全的查询。

use anyhow::Result;
use sqlx::{
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    Pool, Sqlite,
};
use std::path::Path;

/// 数据库连接池类型
pub type Db = Pool<Sqlite>;

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// 初始化数据库
///
/// 创建数据库文件（如果不存在）并运行迁移
pub async fn init(db_path: &str, pool_size: u32) -> Result<Db> {
    let absolute_path = resolve_db_path(db_path)?;
    ensure_parent_dir(&absolute_path).await?;

    let options = SqliteConnectOptions::new()
        .filename(&absolute_path)
        .create_if_missing(true);

    tracing::info!("Connecting to database: {}", absolute_path.display());

    let pool = SqlitePoolOptions::new()
        .max_connections(pool_size.max(1))
        .connect_with(options)
        .await?;

    run_migrations(&pool).await?;

    Ok(pool)
}

fn resolve_db_path(db_path: &str) -> Result<std::path::PathBuf> {
    let path = Path::new(db_path);
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

async fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            tokio::fs::create_dir_all(parent).await?;
            tracing::info!("Created data directory: {}", parent.display());
        }
    }
    Ok(())
}

/// 运行数据库迁移
async fn run_migrations(pool: &Db) -> Result<()> {
    MIGRATOR.run(pool).await?;
    ensure_legacy_compatibility(pool).await?;
    tracing::info!("Database migrations completed");
    Ok(())
}

/// 对旧版本数据库做一次性兼容修复，避免历史运行时 schema 演进丢失字段。
async fn ensure_legacy_compatibility(pool: &Db) -> Result<()> {
    let has_output_dir = column_exists(pool, "schedules", "output_dir").await?;
    if !has_output_dir {
        sqlx::query("ALTER TABLE schedules ADD COLUMN output_dir TEXT")
            .execute(pool)
            .await?;
        tracing::info!("Added missing output_dir column to legacy schedules table");
    }

    let has_source_visibility = column_exists(pool, "channels", "source_visibility").await?;
    if !has_source_visibility {
        sqlx::query(
            "ALTER TABLE channels ADD COLUMN source_visibility TEXT NOT NULL DEFAULT 'public'",
        )
        .execute(pool)
        .await?;
        tracing::info!("Added missing source_visibility column to legacy channels table");
    }

    let has_playback_strategy = column_exists(pool, "channels", "playback_strategy").await?;
    if !has_playback_strategy {
        sqlx::query(
            "ALTER TABLE channels ADD COLUMN playback_strategy TEXT NOT NULL DEFAULT 'auto'",
        )
        .execute(pool)
        .await?;
        tracing::info!("Added missing playback_strategy column to legacy channels table");
    }

    Ok(())
}

async fn column_exists(pool: &Db, table: &str, column: &str) -> Result<bool> {
    let pragma = format!("PRAGMA table_info({table})");
    let columns: Vec<(i64, String, String, i64, Option<String>, i64)> =
        sqlx::query_as(&pragma).fetch_all(pool).await?;

    Ok(columns
        .into_iter()
        .any(|(_, name, _, _, _, _)| name == column))
}
