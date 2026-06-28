//! 系统配置服务
//!
//! 从数据库读取和更新系统配置

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::ServiceContext;

/// 系统配置响应结构 (匹配前端类型)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    pub server: ServerConfigResponse,
    pub storage: StorageConfigResponse,
    pub recording: RecordingConfigResponse,
    pub notification: NotificationConfigResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfigResponse {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfigResponse {
    pub recordings_path: String,
    pub auto_cleanup_days: u32,
    pub min_free_space_gb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingConfigResponse {
    pub default_duration_minutes: u32,
    pub n_m3u8dl_re_path: String,
    pub max_retry: u32,
    pub thread_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfigResponse {
    pub on_complete: bool,
    pub on_failure: bool,
    pub disk_warning: bool,
}

/// 配置更新请求 (部分更新)
#[derive(Debug, Clone, Deserialize)]
pub struct ConfigUpdateRequest {
    pub storage: Option<StorageConfigUpdate>,
    pub recording: Option<RecordingConfigUpdate>,
    pub notification: Option<NotificationConfigUpdate>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StorageConfigUpdate {
    pub recordings_path: Option<String>,
    pub auto_cleanup_days: Option<u32>,
    pub min_free_space_gb: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RecordingConfigUpdate {
    pub default_duration_minutes: Option<u32>,
    /// 录制工具路径不再开放修改（由后端/Docker 镜像内部集成）。
    /// 字段保留是为了向前兼容旧前端请求的反序列化，但 update_config 会忽略它。
    #[allow(dead_code)]
    pub n_m3u8dl_re_path: Option<String>,
    pub max_retry: Option<u32>,
    pub thread_count: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NotificationConfigUpdate {
    pub on_complete: Option<bool>,
    pub on_failure: Option<bool>,
    pub disk_warning: Option<bool>,
}

/// 系统配置服务
pub struct ConfigService {
    ctx: ServiceContext,
}

impl ConfigService {
    pub fn new(ctx: ServiceContext) -> Self {
        Self { ctx }
    }

    /// 获取完整系统配置
    pub async fn get_config(&self) -> Result<SystemConfig> {
        // 服务器配置从启动配置读取
        let server = ServerConfigResponse {
            host: self.ctx.config.server.host.clone(),
            port: self.ctx.config.server.port,
        };

        // 从数据库读取其他配置
        let storage = StorageConfigResponse {
            recordings_path: self
                .get_value_string("storage.recordings_path", "./data/recordings")
                .await?,
            auto_cleanup_days: self.get_value("storage.auto_cleanup_days", 30).await?,
            min_free_space_gb: self.get_value("storage.min_free_space_gb", 10).await?,
        };

        let recording = RecordingConfigResponse {
            default_duration_minutes: self
                .get_value("recording.default_duration_minutes", 60)
                .await?,
            n_m3u8dl_re_path: self
                .get_value_string("recording.n_m3u8dl_re_path", "N_m3u8DL-RE")
                .await?,
            max_retry: self.get_value("recording.max_retry", 3).await?,
            thread_count: self.get_value("recording.thread_count", 4).await?,
        };

        let notification = NotificationConfigResponse {
            on_complete: self.get_value("notification.on_complete", true).await?,
            on_failure: self.get_value("notification.on_failure", true).await?,
            disk_warning: self.get_value("notification.disk_warning", true).await?,
        };

        Ok(SystemConfig {
            server,
            storage,
            recording,
            notification,
        })
    }

    /// 更新系统配置
    pub async fn update_config(&self, req: ConfigUpdateRequest) -> Result<SystemConfig> {
        // 参数范围校验
        if let Some(ref storage) = req.storage {
            if let Some(days) = storage.auto_cleanup_days {
                if days > 365 {
                    return Err(anyhow::anyhow!("auto_cleanup_days 不能超过 365 天"));
                }
            }
            if let Some(space) = storage.min_free_space_gb {
                if space < 1 {
                    return Err(anyhow::anyhow!("min_free_space_gb 至少为 1 GB"));
                }
            }
        }
        if let Some(ref recording) = req.recording {
            if let Some(minutes) = recording.default_duration_minutes {
                if !(1..=1440).contains(&minutes) {
                    return Err(anyhow::anyhow!(
                        "default_duration_minutes 必须在 1-1440 之间"
                    ));
                }
            }
            if let Some(retry) = recording.max_retry {
                if retry > 10 {
                    return Err(anyhow::anyhow!("max_retry 不能超过 10"));
                }
            }
            if let Some(threads) = recording.thread_count {
                if !(1..=32).contains(&threads) {
                    return Err(anyhow::anyhow!("thread_count 必须在 1-32 之间"));
                }
            }
        }

        // 更新存储配置
        if let Some(storage) = req.storage {
            if let Some(v) = storage.recordings_path {
                // 保存前校验录制路径:必须可创建且可写,避免配错导致录制启动才失败
                validate_recordings_path(&v)?;
                self.set_value("storage.recordings_path", &v).await?;
            }
            if let Some(v) = storage.auto_cleanup_days {
                self.set_value("storage.auto_cleanup_days", &v.to_string())
                    .await?;
            }
            if let Some(v) = storage.min_free_space_gb {
                self.set_value("storage.min_free_space_gb", &v.to_string())
                    .await?;
            }
        }

        // 更新录制配置
        if let Some(recording) = req.recording {
            if let Some(v) = recording.default_duration_minutes {
                self.set_value("recording.default_duration_minutes", &v.to_string())
                    .await?;
            }
            // n_m3u8dl_re_path 不再开放给用户修改——录制工具由后端/Docker 镜像
            // 内部集成，用户改错会导致录制全废。保持后端默认值，忽略前端传入。
            if let Some(v) = recording.max_retry {
                self.set_value("recording.max_retry", &v.to_string())
                    .await?;
            }
            if let Some(v) = recording.thread_count {
                self.set_value("recording.thread_count", &v.to_string())
                    .await?;
            }
        }

        // 更新通知配置
        if let Some(notification) = req.notification {
            if let Some(v) = notification.on_complete {
                self.set_value("notification.on_complete", &v.to_string())
                    .await?;
            }
            if let Some(v) = notification.on_failure {
                self.set_value("notification.on_failure", &v.to_string())
                    .await?;
            }
            if let Some(v) = notification.disk_warning {
                self.set_value("notification.disk_warning", &v.to_string())
                    .await?;
            }
        }

        // 返回更新后的配置
        self.get_config().await
    }

    /// 从数据库获取配置值
    async fn get_value<T>(&self, key: &str, default: T) -> Result<T>
    where
        T: std::str::FromStr,
    {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT value FROM system_config WHERE key = ?")
                .bind(key)
                .fetch_optional(&self.ctx.db)
                .await?;

        if let Some((value,)) = row {
            value.parse().or(Ok(default))
        } else {
            Ok(default)
        }
    }

    /// 从数据库获取字符串配置值
    async fn get_value_string(&self, key: &str, default: &str) -> Result<String> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT value FROM system_config WHERE key = ?")
                .bind(key)
                .fetch_optional(&self.ctx.db)
                .await?;

        if let Some((value,)) = row {
            Ok(value)
        } else {
            Ok(default.to_string())
        }
    }

    /// 设置配置值到数据库
    async fn set_value(&self, key: &str, value: &str) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO system_config (key, value, updated_at)
            VALUES (?, ?, datetime('now'))
            ON CONFLICT(key) DO UPDATE SET
                value = excluded.value,
                updated_at = datetime('now')
            "#,
        )
        .bind(key)
        .bind(value)
        .execute(&self.ctx.db)
        .await?;

        Ok(())
    }
}

/// 校验录制保存路径是否可访问、可写。
///
/// 支持本地路径和网络路径(Windows UNC `\\server\share`、Linux 挂载点 `/mnt/nas`)。
/// 校验步骤:
/// 1. 非空检查(空路径直接拒绝)
/// 2. 尝试创建目录(不存在则 create_dir_all,已存在则幂等)
/// 3. 写入并删除一个临时文件,确认该路径确实可写(避免只读目录/权限不足)
///
/// 校验在保存配置时进行,而非等到录制启动才暴露问题。
fn validate_recordings_path(path: &str) -> Result<()> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        anyhow::bail!("录制保存路径不能为空");
    }

    let dir = std::path::Path::new(trimmed);

    // 1. 确保目录存在(不存在则创建)
    std::fs::create_dir_all(dir).map_err(|e| {
        anyhow::anyhow!(
            "录制保存路径无法访问或创建: {} (路径: {})。如果是网络路径,请确认已挂载/映射且凭据有效",
            e,
            trimmed
        )
    })?;

    // 2. 写临时文件验证可写权限
    let probe = dir.join(format!(".iptv-recorder-probe-{}.tmp", uuid::Uuid::new_v4()));
    std::fs::write(&probe, b"probe").map_err(|e| {
        // 清理可能残留的探测文件
        let _ = std::fs::remove_file(&probe);
        anyhow::anyhow!(
            "录制保存路径不可写: {} (路径: {})。请检查目录权限",
            e,
            trimmed
        )
    })?;
    let _ = std::fs::remove_file(&probe);

    Ok(())
}
