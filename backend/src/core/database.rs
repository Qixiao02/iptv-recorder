//! 数据库模块
//!
//! 使用 SQLite 作为持久化存储，通过 sqlx 进行类型安全的查询。

use anyhow::Result;
use sqlx::{
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions, SqliteJournalMode, SqliteSynchronous},
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

    // 关键 PRAGMA:默认 journal 模式为 DELETE(全表锁 + 每事务 fsync),并发录制心跳 +
    // cron + 清理会触发 "database is locked"。WAL 允许并发读 + 单写,synchronous=Normal
    // 在 WAL 下足够安全且大幅降低 fsync 成本;busy_timeout 让锁冲突等待而非立即报错。
    // foreign_keys=ON 让 schema 声明的 ON DELETE CASCADE 真正生效(SQLite 默认关闭)。
    let options = SqliteConnectOptions::new()
        .filename(&absolute_path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(std::time::Duration::from_secs(5))
        .foreign_keys(true);

    tracing::info!("Connecting to database: {}", absolute_path.display());

    let pool = SqlitePoolOptions::new()
        .max_connections(pool_size.max(1))
        .connect_with(options)
        .await?;

    run_migrations(&pool).await?;

    // 诊断:确认连接级 PRAGMA(foreign_keys/synchronous)真的在应用连接上生效。
    // 这两个 PRAGMA 是连接级、非持久(SQLite 默认 foreign_keys=OFF),CLI 独立连接
    // 查到的是默认值,无法代表应用连接,故启动时在应用连接上查一次并记录。
    let fk: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
        .fetch_one(&pool)
        .await
        .unwrap_or(-1);
    let sync: i64 = sqlx::query_scalar("PRAGMA synchronous")
        .fetch_one(&pool)
        .await
        .unwrap_or(-1);
    tracing::info!(
        "DB PRAGMA check: foreign_keys={} (期望 1), synchronous={} (期望 1=NORMAL)",
        fk,
        sync
    );

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

    let fixed_private_sources = sqlx::query(
        r#"
        UPDATE channels
        SET source_visibility = 'private_server_only'
        WHERE source_visibility = 'public'
          AND (
            lower(url) LIKE 'http://localhost/%'
            OR lower(url) LIKE 'https://localhost/%'
            OR url LIKE 'http://10.%'
            OR url LIKE 'https://10.%'
            OR url LIKE 'http://192.168.%'
            OR url LIKE 'https://192.168.%'
            OR url LIKE 'http://127.%'
            OR url LIKE 'https://127.%'
            OR url LIKE 'http://169.254.%'
            OR url LIKE 'https://169.254.%'
            OR url LIKE 'http://100.6[4-9].%'
            OR url LIKE 'http://100.[7-9][0-9].%'
            OR url LIKE 'http://100.1[0-1][0-9].%'
            OR url LIKE 'http://100.12[0-7].%'
            OR url LIKE 'https://100.6[4-9].%'
            OR url LIKE 'https://100.[7-9][0-9].%'
            OR url LIKE 'https://100.1[0-1][0-9].%'
            OR url LIKE 'https://100.12[0-7].%'
            OR url LIKE 'http://172.1[6-9].%'
            OR url LIKE 'http://172.2[0-9].%'
            OR url LIKE 'http://172.3[0-1].%'
            OR url LIKE 'https://172.1[6-9].%'
            OR url LIKE 'https://172.2[0-9].%'
            OR url LIKE 'https://172.3[0-1].%'
            OR lower(url) LIKE 'http://[fc%'
            OR lower(url) LIKE 'https://[fc%'
            OR lower(url) LIKE 'http://[fd%'
            OR lower(url) LIKE 'https://[fd%'
            OR lower(url) LIKE 'http://[fe80:%'
            OR lower(url) LIKE 'https://[fe80:%'
            OR lower(url) LIKE 'http://[::1]%'
            OR lower(url) LIKE 'https://[::1]%'
          )
        "#,
    )
    .execute(pool)
    .await?
    .rows_affected();

    if fixed_private_sources > 0 {
        tracing::info!(
            "Marked {} existing private-address channels as private_server_only",
            fixed_private_sources
        );
    }

    let normalized_output_templates = sqlx::query(
        r#"
        UPDATE schedules
        SET output_template = substr(output_template, 1, length(output_template) - 4)
        WHERE output_template LIKE '%.mp4'
        "#,
    )
    .execute(pool)
    .await?
    .rows_affected();

    if normalized_output_templates > 0 {
        tracing::info!(
            "Normalized {} legacy schedule output templates by removing .mp4 suffix",
            normalized_output_templates
        );
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
