//! 数据模型模块
//!
//! 定义系统中使用的所有数据结构

#![allow(dead_code)]

mod user;

pub use user::{ChangePasswordRequest, LoginRequest, LoginResponse, User, UserInfo, UserRole};

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AuditLog {
    pub id: String,
    pub user_id: Option<String>,
    pub username: Option<String>,
    pub role: Option<String>,
    pub action: String,
    pub resource_type: String,
    pub resource_id: Option<String>,
    pub details: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealth {
    pub users_total: i64,
    pub channels_total: i64,
    pub schedules_total: i64,
    pub enabled_schedules: i64,
    pub running_tasks: i64,
    pub failed_tasks_24h: i64,
    pub last_audit_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EpgSource {
    pub id: String,
    pub name: String,
    pub source_url: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EpgProgram {
    pub id: String,
    pub source_id: String,
    pub channel_ref: String,
    pub title: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub start_at: String,
    pub end_at: String,
    pub created_at: String,
}

/// 频道信息
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Channel {
    pub id: String,
    pub name: String,
    pub url: String,
    #[serde(default = "default_group")]
    pub group_name: String,
    pub logo_url: Option<String>,
    #[serde(default)]
    pub source_type: String,
    pub source_url: Option<String>,
    #[serde(default)]
    pub status: String,
    pub last_check_at: Option<String>,
    #[serde(default)]
    pub fail_count: i32,
    #[serde(default = "default_metadata")]
    pub metadata: serde_json::Value,
    #[serde(default = "default_source_visibility")]
    pub source_visibility: String,
    #[serde(default = "default_playback_strategy")]
    pub playback_strategy: String,
    pub created_at: String,
    pub updated_at: String,
}

fn default_group() -> String {
    "Uncategorized".to_string()
}

fn default_metadata() -> serde_json::Value {
    serde_json::json!({})
}

fn default_source_visibility() -> String {
    "public".to_string()
}

fn default_playback_strategy() -> String {
    "auto".to_string()
}

/// 创建频道请求
#[derive(Debug, Deserialize)]
pub struct CreateChannelRequest {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub group_name: String,
    pub logo_url: Option<String>,
    #[serde(default = "default_source_visibility")]
    pub source_visibility: String,
    #[serde(default = "default_playback_strategy")]
    pub playback_strategy: String,
}

/// 录制计划
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Schedule {
    pub id: String,
    pub name: String,
    pub channel_id: String,
    pub cron_expression: String,
    #[serde(default = "default_duration")]
    pub duration_seconds: i64,
    #[serde(default = "default_output_template")]
    pub output_template: String,
    /// 自定义输出目录，为空时使用系统默认
    pub output_dir: Option<String>,
    #[serde(default = "default_priority")]
    pub priority: i32,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_max_retry")]
    pub max_retry: i32,
    #[serde(default)]
    pub notify_on_complete: bool,
    /// 视频质量选择 (best, 1080p, 720p, 480p, 或自定义正则)
    #[serde(default = "default_video_quality")]
    pub video_quality: String,
    /// 音频质量选择 (best, 或自定义正则)
    #[serde(default = "default_audio_quality")]
    pub audio_quality: String,
    /// 下载限速 (如: 10M, 500K)
    pub max_speed: Option<String>,
    /// 下载线程数
    #[serde(default = "default_thread_count")]
    pub thread_count: i32,
    /// 转码模式 (off, realtime, post)
    #[serde(default)]
    pub transcode_mode: String,
    /// 转码预设 (high, medium, low, custom)
    #[serde(default = "default_transcode_preset")]
    pub transcode_preset: String,
    pub created_at: String,
    pub updated_at: String,
}

fn default_video_quality() -> String {
    "best".to_string()
}

fn default_audio_quality() -> String {
    "best".to_string()
}

fn default_thread_count() -> i32 {
    20
}

fn default_transcode_preset() -> String {
    "medium".to_string()
}

fn default_duration() -> i64 {
    3600
}

fn default_output_template() -> String {
    "{channel_name}_{date}_{time}.mp4".to_string()
}

fn default_priority() -> i32 {
    5
}

fn default_max_retry() -> i32 {
    3
}

/// 创建计划请求
#[derive(Debug, Clone, Deserialize)]
pub struct CreateScheduleRequest {
    pub name: String,
    pub channel_id: String,
    pub cron_expression: String,
    #[serde(default = "default_duration")]
    pub duration_seconds: i64,
    pub output_template: Option<String>,
    /// 自定义输出目录，为空时使用系统默认
    pub output_dir: Option<String>,
    pub priority: Option<i32>,
    /// 视频质量选择 (best, 1080p, 720p, 480p)
    #[serde(default = "default_video_quality")]
    pub video_quality: String,
    /// 音频质量选择 (best)
    #[serde(default = "default_audio_quality")]
    pub audio_quality: String,
    /// 下载限速 (如: 10M, 500K)
    pub max_speed: Option<String>,
    /// 下载线程数
    #[serde(default = "default_thread_count")]
    pub thread_count: i32,
    /// 转码模式 (off, realtime, post)
    #[serde(default)]
    pub transcode_mode: String,
    /// 转码预设 (high, medium, low, custom)
    #[serde(default = "default_transcode_preset")]
    pub transcode_preset: String,
}

/// 任务
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Task {
    pub id: String,
    pub schedule_id: Option<String>,
    pub channel_id: String,
    #[serde(default)]
    pub status: String,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub exit_code: Option<i32>,
    pub error_message: Option<String>,
    pub output_path: Option<String>,
    #[serde(default)]
    pub file_size: i64,
    #[serde(default)]
    pub duration_recorded: i64,
    #[serde(default)]
    pub progress_percent: i32,
    pub current_speed: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// 手动录制请求
#[derive(Debug, Deserialize)]
pub struct ManualRecordRequest {
    pub channel_id: String,
    pub schedule_id: Option<String>,
    pub duration_seconds: Option<i64>,
    pub output_name: Option<String>,
    /// 自定义输出目录，为空时使用系统默认
    pub output_dir: Option<String>,
    /// 输出文件名模板（可选）
    pub output_template: Option<String>,
    /// 视频质量选择 (best, 1080p, 720p, 480p)
    #[serde(default = "default_video_quality")]
    pub video_quality: String,
    /// 音频质量选择 (best)
    #[serde(default = "default_audio_quality")]
    pub audio_quality: String,
    /// 下载限速 (如: 10M, 500K)
    pub max_speed: Option<String>,
    /// 下载线程数
    pub thread_count: Option<i32>,
    /// 转码模式 (off, realtime, post)
    pub transcode_mode: Option<String>,
    /// 转码预设 (high, medium, low, custom)
    pub transcode_preset: Option<String>,
}

/// 分页响应
#[derive(Debug, Serialize)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub total: u64,
    pub page: u32,
    pub page_size: u32,
}

/// API 错误响应
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

/// 导入 M3U 响应
#[derive(Debug, Serialize)]
pub struct ImportM3UResponse {
    /// 导入的频道数量
    pub imported: usize,
    /// 跳过的频道数量
    pub skipped: usize,
    /// 失败的频道数量
    pub failed: usize,
    /// 错误信息
    pub errors: Vec<String>,
}

/// WebSocket 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WsMessage {
    /// 任务状态更新
    #[serde(rename = "task.update")]
    TaskUpdate(TaskUpdateData),

    /// 任务进度更新
    #[serde(rename = "task.progress")]
    TaskProgress(TaskProgressData),

    /// 频道状态变更
    #[serde(rename = "channel.status")]
    ChannelStatus(ChannelStatusData),

    /// 系统告警
    #[serde(rename = "system.alert")]
    SystemAlert(SystemAlertData),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskUpdateData {
    pub task_id: String,
    pub status: String,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskProgressData {
    pub task_id: String,
    pub percent: u8,
    pub downloaded_bytes: u64,
    pub speed: String,
    pub eta_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelStatusData {
    pub channel_id: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemAlertData {
    pub level: String,
    pub message: String,
    pub details: Option<String>,
}
