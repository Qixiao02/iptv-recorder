//! 核心基础设施模块
//!
//! 包含数据库、事件总线、进程管理等基础设施组件。

pub mod database;
pub mod event;
pub mod process;

pub use process::ProcessManager;
