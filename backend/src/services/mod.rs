//! 业务服务模块
//!
//! 包含频道管理、录制调度等核心业务逻辑

pub mod channel;
pub mod schedule;
pub mod recording;
pub mod m3u_parser;
pub mod scheduler;
pub mod config_service;
pub mod transcode;
pub mod post_process;
pub mod auth;

pub use channel::{ChannelService, ChannelTestResult, PaginationParams, PaginatedChannels};
pub use schedule::ScheduleService;
pub use recording::RecordingService;
pub use m3u_parser::{M3UParser, M3UParseResult};
pub use scheduler::{SchedulerManager, CronTrigger, UpcomingTask};
pub use config_service::{ConfigService, SystemConfig, ConfigUpdateRequest};
pub use transcode::{TranscodeService, TranscodeSession};
pub use post_process::{PostProcessor, TranscodeMode, get_mode_description, get_preset_description};
pub use auth::{AuthService, Claims};

use crate::config::Config;
use sqlx::{Pool, Sqlite};

/// 服务上下文
#[derive(Clone)]
pub struct ServiceContext {
    pub db: Pool<Sqlite>,
    pub config: Config,
}

impl ServiceContext {
    pub fn new(db: Pool<Sqlite>, config: Config) -> Self {
        Self { db, config }
    }
}
