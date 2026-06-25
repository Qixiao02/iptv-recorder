//! 业务服务模块
//!
//! 包含频道管理、录制调度等核心业务逻辑

pub mod audit;
pub mod auth;
pub mod channel;
pub mod cleanup;
pub mod config_service;
pub mod epg;
pub mod m3u_parser;
pub mod post_process;
pub mod recording;
pub mod schedule;
pub mod scheduler;
pub mod transcode;

pub use audit::AuditService;
pub use auth::{AuthService, Claims};
pub use channel::{ChannelService, ChannelTestResult, PaginationParams};
pub use cleanup::CleanupService;
pub use config_service::{ConfigService, ConfigUpdateRequest, SystemConfig};
pub use epg::{EpgService, ImportEpgRequest};
pub use m3u_parser::{M3UParseResult, M3UParser};
pub use post_process::PostProcessor;
pub use recording::RecordingService;
pub use schedule::ScheduleService;
pub use scheduler::{CronTrigger, SchedulerManager, UpcomingTask};
pub use transcode::TranscodeService;

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
