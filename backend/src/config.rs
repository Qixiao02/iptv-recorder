//! 配置管理模块
//!
//! 支持分层配置：默认值 → 配置文件 → 环境变量
//!
//! # 环境变量格式
//! - `IPTV__SERVER__HOST=0.0.0.0`
//! - `IPTV__DATABASE__PATH=/path/to/db`

use figment::{
    providers::{Env, Format, Serialized, Toml, Yaml},
    Figment,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 应用配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    /// 服务器配置
    #[serde(default)]
    pub server: ServerConfig,

    /// 数据库配置
    #[serde(default)]
    pub database: DatabaseConfig,

    /// 存储配置
    #[serde(default)]
    pub storage: StorageConfig,

    /// 录制器配置
    #[serde(default)]
    pub recorder: RecorderConfig,

    /// 调度配置
    #[serde(default)]
    pub scheduler: SchedulerConfig,
}

/// 服务器配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    /// 监听地址
    #[serde(default = "default_host")]
    pub host: String,

    /// 监听端口
    #[serde(default = "default_port")]
    pub port: u16,

    /// Workers 数量
    #[serde(default = "default_workers")]
    pub workers: usize,

    /// CORS 允许的来源列表(逗号分隔或数组)。环境变量 IPTV__SERVER__CORS_ORIGINS
    #[serde(default = "default_cors_origins")]
    pub cors_origins: Vec<String>,
}

/// 数据库配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DatabaseConfig {
    /// SQLite 数据库路径
    #[serde(default = "default_db_path")]
    pub path: PathBuf,

    /// 连接池最大连接数
    #[serde(default = "default_pool_size")]
    pub pool_size: u32,
}

/// 存储配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StorageConfig {
    /// 录制根目录
    #[serde(default = "default_recordings_dir")]
    pub recordings_dir: PathBuf,

    /// 临时文件目录
    #[serde(default = "default_temp_dir")]
    pub temp_dir: PathBuf,

    /// 预览 HLS 临时目录，优先建议使用内存文件系统
    #[serde(default)]
    pub preview_temp_dir: Option<PathBuf>,

    /// 最小剩余空间（MB）
    #[serde(default = "default_min_space")]
    pub min_free_space_mb: u64,
}

/// 录制器配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RecorderConfig {
    /// N_m3u8DL-RE 可执行文件路径
    #[serde(default = "default_recorder_path")]
    pub executable: PathBuf,

    /// 全局最大并发录制数
    #[serde(default = "default_concurrent")]
    pub max_concurrent: usize,

    /// 单任务超时（秒）
    #[serde(default = "default_timeout")]
    pub task_timeout_secs: u64,

    /// 后处理配置
    #[serde(default)]
    pub post_process: PostProcessConfig,
}

/// 后处理配置（录制完成后自动转码）
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct PostProcessConfig {
    /// 转码模式: off, realtime, post
    #[serde(default)]
    pub mode: String,

    /// FFmpeg 可执行文件路径
    #[serde(default)]
    pub ffmpeg_path: String,

    /// 转码预设: high, medium, low, custom
    #[serde(default = "default_preset")]
    pub preset: String,

    /// 视频码率 (如: 2M, 1500K)
    #[serde(default)]
    pub video_bitrate: String,

    /// 音频码率 (如: 192K, 128K)
    #[serde(default = "default_audio_bitrate")]
    pub audio_bitrate: String,

    /// CRF 质量 (0-51)
    #[serde(default = "default_crf")]
    pub crf: u8,

    /// 编码速度预设
    #[serde(default = "default_encode_preset")]
    pub encode_preset: String,

    /// 自定义 FFmpeg 参数
    #[serde(default)]
    pub custom_args: String,

    /// 转码完成后删除原始文件
    #[serde(default = "default_delete_original")]
    pub delete_original: bool,

    /// 输出格式: mp4, mkv, ts
    #[serde(default = "default_output_format")]
    pub output_format: String,
}

fn default_preset() -> String {
    "medium".to_string()
}

fn default_audio_bitrate() -> String {
    "128k".to_string()
}

fn default_crf() -> u8 {
    23
}

fn default_encode_preset() -> String {
    "medium".to_string()
}

fn default_delete_original() -> bool {
    true
}

fn default_output_format() -> String {
    "mp4".to_string()
}

impl PostProcessConfig {
    /// 是否启用转码
    pub fn is_enabled(&self) -> bool {
        self.mode != "off"
    }

    /// 是否实时转码
    #[allow(dead_code)]
    pub fn is_realtime(&self) -> bool {
        self.mode == "realtime"
    }

    /// 是否后期转码
    pub fn is_post(&self) -> bool {
        self.mode == "post"
    }
}

/// 调度器配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SchedulerConfig {
    /// 默认时区
    #[serde(default = "default_timezone")]
    pub timezone: String,

    /// 任务保留天数
    #[serde(default = "default_retention")]
    pub task_retention_days: u32,
}

// ===== 默认值函数 =====

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    3000
}

fn default_workers() -> usize {
    4
}

fn default_cors_origins() -> Vec<String> {
    vec![
        "http://localhost:5173".to_string(),
        "http://127.0.0.1:5173".to_string(),
        "http://localhost:3033".to_string(),
        "http://127.0.0.1:3033".to_string(),
    ]
}

fn default_db_path() -> PathBuf {
    PathBuf::from("data/iptv-recorder.db")
}

fn default_pool_size() -> u32 {
    10
}

fn default_recordings_dir() -> PathBuf {
    PathBuf::from("data/recordings")
}

fn default_temp_dir() -> PathBuf {
    PathBuf::from("data/.tmp")
}

fn default_min_space() -> u64 {
    1024 // 1GB
}

fn default_recorder_path() -> PathBuf {
    PathBuf::from("N_m3u8DL-RE")
}

fn default_concurrent() -> usize {
    5
}

fn default_timeout() -> u64 {
    7200 // 2 hours
}

fn default_timezone() -> String {
    "Asia/Shanghai".to_string()
}

fn default_retention() -> u32 {
    30
}

// ===== 默认实现 =====

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            workers: default_workers(),
            cors_origins: default_cors_origins(),
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            path: default_db_path(),
            pool_size: default_pool_size(),
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            recordings_dir: default_recordings_dir(),
            temp_dir: default_temp_dir(),
            preview_temp_dir: None,
            min_free_space_mb: default_min_space(),
        }
    }
}

impl StorageConfig {
    /// 预览 HLS 优先使用内存文件系统，避免频繁切台时伤盘。
    pub fn preview_hls_dir(&self) -> PathBuf {
        if let Some(path) = &self.preview_temp_dir {
            return path.clone();
        }

        let shm_dir = PathBuf::from("/dev/shm");
        if shm_dir.exists() {
            return shm_dir.join("iptv-recorder-hls");
        }

        self.temp_dir.join("hls")
    }
}

impl Default for RecorderConfig {
    fn default() -> Self {
        Self {
            executable: default_recorder_path(),
            max_concurrent: default_concurrent(),
            task_timeout_secs: default_timeout(),
            post_process: Default::default(),
        }
    }
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            timezone: default_timezone(),
            task_retention_days: default_retention(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: Default::default(),
            database: Default::default(),
            storage: Default::default(),
            recorder: Default::default(),
            scheduler: Default::default(),
        }
    }
}

/// 配置加载结果
#[derive(Debug)]
pub struct LoadedConfig {
    pub config: Config,
    pub config_path: Option<PathBuf>,
}

/// 加载配置
///
/// 优先级：环境变量 > 配置文件 > 默认值
pub fn load() -> anyhow::Result<LoadedConfig> {
    let config_path = find_config_file()?;

    let figment = if let Some(ref path) = config_path {
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        match extension {
            "toml" => Figment::from(Toml::file(path)).merge(Env::prefixed("IPTV__").split("__")),
            "yaml" | "yml" => {
                Figment::from(Yaml::file(path)).merge(Env::prefixed("IPTV__").split("__"))
            }
            _ => anyhow::bail!("不支持的配置文件格式: {}", extension),
        }
    } else {
        Figment::from(Serialized::defaults(Config::default()))
            .merge(Env::prefixed("IPTV__").split("__"))
    };

    let config: Config = figment.extract()?;

    Ok(LoadedConfig {
        config,
        config_path,
    })
}

/// 查找配置文件
///
/// 查找顺序：
/// 1. 当前目录: config.toml, config.yaml, config.yml
/// 2. config 子目录: config/default.toml
/// 3. 用户配置目录: ~/.iptv-recorder/config.*
/// 4. /etc/iptv-recorder/config.*
fn find_config_file() -> anyhow::Result<Option<PathBuf>> {
    let candidates = [
        "./config.toml",
        "./config.yaml",
        "./config.yml",
        "./config/default.toml",
        "./config/default.yaml",
        "~/.iptv-recorder/config.toml",
        "~/.iptv-recorder/config.yaml",
        "/etc/iptv-recorder/config.toml",
        "/etc/iptv-recorder/config.yaml",
    ];

    for path in candidates {
        let expanded = shellexpand::tilde(path);
        let path = PathBuf::from(expanded.as_ref());
        if path.exists() {
            return Ok(Some(path));
        }
    }

    Ok(None)
}
