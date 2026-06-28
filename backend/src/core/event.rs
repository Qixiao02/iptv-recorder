//! 事件总线模块
//!
//! 基于 Tokio broadcast channel 的内存事件总线

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::fmt;
use tokio::sync::broadcast;

/// 事件类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    /// 任务状态更新
    TaskUpdate(TaskUpdateEvent),

    /// 任务进度更新
    TaskProgress(TaskProgressEvent),

    /// 频道状态变更
    ChannelStatus(ChannelStatusEvent),

    /// 系统告警
    SystemAlert(SystemAlertEvent),

    /// 应用内通知（持久化通知中心，区别于一次性系统告警）
    Notification(NotificationEvent),
}

/// 任务状态更新事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskUpdateEvent {
    pub task_id: String,
    pub status: TaskStatus,
    pub error_message: Option<String>,
}

/// 任务进度事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskProgressEvent {
    pub task_id: String,
    pub percent: u8,
    pub downloaded_bytes: u64,
    pub speed: String,
    pub eta_seconds: Option<u64>,
}

/// 频道状态事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelStatusEvent {
    pub channel_id: String,
    pub status: ChannelStatus,
}

/// 系统告警事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemAlertEvent {
    pub level: AlertLevel,
    pub message: String,
    pub details: Option<String>,
}

/// 应用内通知事件（携带已落库的通知完整数据）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationEvent {
    pub id: String,
    pub category: String,
    pub level: String,
    pub title: String,
    pub message: String,
    pub details: Option<String>,
    pub task_id: Option<String>,
    pub created_at: String,
}

/// 任务状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Running => write!(f, "running"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// 频道状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelStatus {
    Unknown,
    Online,
    Offline,
}

impl fmt::Display for ChannelStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown => write!(f, "unknown"),
            Self::Online => write!(f, "online"),
            Self::Offline => write!(f, "offline"),
        }
    }
}

/// 告警级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertLevel {
    Info,
    Warning,
    Error,
    Critical,
}

impl fmt::Display for AlertLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Info => write!(f, "info"),
            Self::Warning => write!(f, "warning"),
            Self::Error => write!(f, "error"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// 事件总线发送器
pub type EventSender = broadcast::Sender<Event>;

/// 事件总线接收器
pub type EventReceiver = broadcast::Receiver<Event>;

/// 事件总线
#[derive(Debug, Clone)]
pub struct EventBus {
    sender: EventSender,
}

impl EventBus {
    /// 创建新的事件总线
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// 获取发送器
    pub fn sender(&self) -> EventSender {
        self.sender.clone()
    }

    /// 订阅事件
    pub fn subscribe(&self) -> EventReceiver {
        self.sender.subscribe()
    }

    /// 发布事件
    pub fn publish(&self, event: Event) -> anyhow::Result<()> {
        self.sender.send(event)?;
        Ok(())
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(1000)
    }
}
