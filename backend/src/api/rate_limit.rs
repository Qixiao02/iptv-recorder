//! 登录限流:基于 IP 的失败次数计数,防止暴力破解。
//!
//! 内存实现(单实例),不持久化。每个 IP 连续失败 5 次后锁定 15 分钟,
//! 期间所有登录请求直接拒绝。成功登录或锁定过期后重置。

use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// 最大失败次数,超过后锁定
const MAX_FAILURES: u32 = 5;
/// 锁定时长
const LOCK_DURATION: Duration = Duration::from_secs(15 * 60);

#[derive(Debug, Clone)]
struct FailState {
    count: u32,
    locked_until: Option<Instant>,
}

/// 登录限流器(线程安全)
#[derive(Debug, Default)]
pub struct LoginRateLimiter {
    state: RwLock<HashMap<String, FailState>>,
}

impl LoginRateLimiter {
    pub fn new() -> Self {
        Self::default()
    }

    /// 检查该 IP 是否被锁定。返回 Ok 表示可继续登录,Err 表示已被锁定。
    pub async fn check(&self, ip: &str) -> Result<(), String> {
        let state = self.state.read().await;
        if let Some(fail) = state.get(ip) {
            if let Some(until) = fail.locked_until {
                if until > Instant::now() {
                    let secs = (until - Instant::now()).as_secs();
                    return Err(format!(
                        "登录失败次数过多,已锁定 {} 秒,请稍后再试",
                        secs
                    ));
                }
            }
        }
        Ok(())
    }

    /// 记录一次失败。若达到阈值则锁定。
    pub async fn record_failure(&self, ip: &str) {
        let mut state = self.state.write().await;
        let fail = state.entry(ip.to_string()).or_insert(FailState {
            count: 0,
            locked_until: None,
        });
        fail.count += 1;
        if fail.count >= MAX_FAILURES {
            fail.locked_until = Some(Instant::now() + LOCK_DURATION);
        }
    }

    /// 记录成功登录,清除该 IP 的失败计数。
    pub async fn record_success(&self, ip: &str) {
        let mut state = self.state.write().await;
        state.remove(ip);
    }

    /// 清理过期的锁定记录(可选,防内存泄漏)
    #[allow(dead_code)]
    pub async fn cleanup(&self) {
        let mut state = self.state.write().await;
        let now = Instant::now();
        state.retain(|_, fail| {
            if let Some(until) = fail.locked_until {
                // 锁定过期的清除;未锁定的保留(可能在累积失败)
                until > now || fail.count < MAX_FAILURES
            } else {
                true
            }
        });
    }
}
