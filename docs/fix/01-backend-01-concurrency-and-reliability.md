# 后端修复 01：并发与可靠性

> 优先级：**P0 + P1**（部分内容 P2）
> 预计工时：3-5 天
> 推荐执行人：`rust-backend-engineer`
> 配套审计章节：`deep-analysis-2026-06-02.md` §3.1 R1-R3, §3.2 I1-I5, §3.3 S1

## 范围与背景

本文件覆盖后端**核心数据通路和进程生命周期**的并发与可靠性问题。共 6 个子任务：SQLite WAL、`kill_on_drop`、原子 INSERT、event_sender 注入、task_timeout 落地、优雅停机。这 6 个的共同特点是**改完一个就堵一类生产事故**。

## 子任务清单

### 子任务 1.1：SQLite 启用 WAL + busy_timeout（**P0**）

**审计引用**：`backend/src/core/database.rs:25-39`、§3.1 R3

**问题**：`SqliteConnectOptions` 只设了 `filename` + `create_if_missing(true)`，未启用 WAL 模式。SQLite 默认 rollback journal 在并发写时会 `SQLITE_BUSY`。代码里有 3s 一次的进度 UPDATE + scheduler 触发的 INSERT + 用户 HTTP 拉 tasks，写竞争会非常明显。

**修复方案**：
```rust
// backend/src/core/database.rs
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};

pub async fn init(db_path: &str, pool_size: u32) -> Result<Db> {
    let absolute_path = resolve_db_path(db_path)?;
    ensure_parent_dir(&absolute_path).await?;

    let options = SqliteConnectOptions::new()
        .filename(&absolute_path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)        // 新增
        .busy_timeout(Duration::from_secs(5))         // 新增
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal);  // 新增，WAL 下 NORMAL 是合理选择

    let pool = SqlitePoolOptions::new()
        .max_connections(pool_size.max(1))
        .connect_with(options)
        .await?;

    run_migrations(&pool).await?;
    Ok(pool)
}
```

**注意**：
- WAL 模式下 `.db-wal` 和 `.db-shm` 是正常产物，**不要**在备份脚本里只备份 `.db`
- 备份前需要 `PRAGMA wal_checkpoint(TRUNCATE)` 合并 WAL 到主库

**验收**：
- [ ] 启动后 `PRAGMA journal_mode` 返回 `wal`
- [ ] 启动后 `PRAGMA busy_timeout` 返回 `5000`
- [ ] 跑 `pnpm dev` + 启动 5 个并发录制 1h 不出现 `SQLITE_BUSY`
- [ ] `data/` 目录出现 `iptv-recorder.db-wal` 和 `iptv-recorder.db-shm` 文件

**风险**：低。WAL 是 SQLite 推荐的标准做法。

---

### 子任务 1.2：`tokio::process::Command` 设 `kill_on_drop(true)`（**P0**）

**审计引用**：`backend/src/core/process.rs:240-242`、§3.1 R2

**问题**：`RecordingService` 用 `tokio::spawn` 启动监控任务，子进程 N_m3u8DL-RE 与主进程没有绑定——主进程被 OOM kill、panic、或被 `scheduler.reload` 触发整体重启时，子进程会**变成孤儿**继续录制，耗尽磁盘。

**修复方案**：
```rust
// backend/src/core/process.rs:240 附近
let mut child = cmd
    .kill_on_drop(true)   // 新增
    .spawn()
    .map_err(|e| anyhow!("启动录制进程失败: {}", e))?;
```

**进一步建议（不强制）**：在子进程启动时也注册 `prctl::set_death_signal(SIGTERM)` 让 N_m3u8DL-RE 在主进程意外死亡时收到信号——但这要求给 N_m3u8DL-RE 注入代码，超出本任务范围。

**验收**：
- [ ] `cargo build` 通过
- [ ] 手动 kill 主进程（`kill -9 $(pgrep iptv-recorder)`），验证所有 N_m3u8DL-RE 子进程也消失（`pgrep N_m3u8DL-RE` 应为空）
- [ ] 集成测试：故意 panic 一段，验证进程列表为空

**风险**：低。`kill_on_drop` 是 tokio 的标准 API。

---

### 子任务 1.3：`ensure_recording_capacity` 改原子 INSERT（**P0**）

**审计引用**：`backend/src/services/recording.rs:869-892`、§3.1 R1

**问题**：现有逻辑是 "count → check → INSERT"，两个并发 start_manual 都能通过 count 检查，导致实际并发数超过 `max_concurrent` 配置。

**修复方案**（三选一，按工作量排序）：

**方案 A（推荐）**：用 SQL 原子化检查
```rust
async fn ensure_recording_capacity_atomic(
    &self,
    req: &ManualRecordRequest,
    runtime_settings: &RuntimeRecordingSettings,
) -> Result<()> {
    // 用事务隔离：SQLite BEGIN IMMEDIATE 锁 DB，写检查一起做
    let mut tx = self.ctx.db.begin().await?;

    let (running_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM tasks WHERE status = 'running'"
    )
    .fetch_one(&mut *tx)
    .await?;

    if running_count >= runtime_settings.max_concurrent as i64 {
        return Err(anyhow::anyhow!(
            "当前运行中的录制任务已达到上限 ({})",
            runtime_settings.max_concurrent
        ));
    }

    if self.has_running_task_for_channel_tx(&mut tx, &req.channel_id).await? {
        return Err(anyhow::anyhow!("该频道当前已有正在运行的录制任务"));
    }

    // commit 由调用方处理（INSERT task 也在同一事务里）
    Ok(())
}
```

**注意**：把 `INSERT INTO tasks` 也放进同一事务，确保 count + insert 原子。

**方案 B（备选）**：用 `INSERT ... WHERE (SELECT COUNT(*) ...) < ?` 一次完成
```sql
INSERT INTO tasks (id, schedule_id, channel_id, status, started_at, output_path, created_at, updated_at)
SELECT ?, ?, ?, 'running', ?, ?, ?, ?
WHERE (SELECT COUNT(*) FROM tasks WHERE status = 'running') < ?
  AND NOT EXISTS (SELECT 1 FROM tasks WHERE channel_id = ? AND status = 'running')
```

如果 `rows_affected() == 0`，就报"已满"。

**方案 B 风险**：SQLite 写并发下 `WHERE` 子查询的快照是事务级，与 `INSERT` 一起原子化——**实测要验证**。如果出问题退回方案 A。

**验收**：
- [ ] 单元测试：spawn 5 个并发 `start_manual`，`max_concurrent: 2`，最后只 2 个成功，3 个报"已满"
- [ ] 手动：开 5 个 terminal 几乎同时点"立即录制"，验证 2 个成功 3 个失败
- [ ] 已有的"取消/完成后状态写回"测试仍绿

**风险**：中。需要小心事务边界（`begin` 在哪个函数，commit 在哪）；如果方案 B 不稳就用方案 A。

---

### 子任务 1.4：scheduler 触发的录制也要推 WS 事件（**P0**）

**审计引用**：`backend/src/services/scheduler.rs:172`、`backend/src/services/recording.rs:172`、§3.2 I2

**问题**：`RecordingService::new(pm, ctx, None)` 在 scheduler 闭包里被调用，`event_sender` 是 `None`——意味着通过 cron 触发的录制全程**不 emit 任何事件**。前端 Dashboard 看不到定时任务的进度。

**修复方案**：
```rust
// backend/src/main.rs:93-99
// 启动 Cron 调度器
let scheduler = Arc::new(
    services::SchedulerManager::new(
        db.clone(),
        config.clone(),
        process_manager.clone(),
        Some(event_bus.sender()),  // 新增
    ).await?
);
```

```rust
// backend/src/services/scheduler.rs:38-50
impl SchedulerManager {
    pub async fn new(
        db: Pool<Sqlite>,
        config: Config,
        process_manager: Arc<ProcessManager>,
        event_sender: Option<EventSender>,  // 新增参数
    ) -> Result<Self> {
        // ... 构造时保存
        Ok(Self {
            scheduler: Arc::new(RwLock::new(JobScheduler::new().await?)),
            db,
            config,
            process_manager,
            event_sender,  // 新增字段
            job_uuids: Arc::new(RwLock::new(HashMap::new())),
        })
    }
}
```

```rust
// 在 scheduler.rs:172 处，构造 RecordingService 时
let ctx = ServiceContext::new(db, config);
let service = RecordingService::new(pm, ctx, self.event_sender.clone());
// 之前是: RecordingService::new(pm, ctx, None)
```

```rust
// backend/src/services/recording.rs
pub struct RecordingService {
    // ...
    event_sender: Option<EventSender>,  // 已有字段
}

impl RecordingService {
    pub fn new(
        process_manager: Arc<ProcessManager>,
        ctx: ServiceContext,
        event_sender: Option<EventSender>,
    ) -> Self {
        // 已经接受 Option<EventSender>，但 scheduler.rs:172 传了 None
        // 修完 scheduler.rs 后自动解决
    }
}
```

**验收**：
- [ ] 集成测试：建一个 30 秒的 schedule，等 cron 触发后 `EventBus.subscribe()` 能收到 `TaskUpdate(Running)` + `TaskProgress` + `TaskUpdate(Completed)`
- [ ] 手动：前端 Dashboard 实时显示定时录制进度（不用刷新）

**风险**：低。只是传参问题。

---

### 子任务 1.5：`task_timeout_secs` 真正 enforce（**P1**）

**审计引用**：`backend/src/config.rs:101`、`backend/src/core/process.rs:240-260`、§3.2 I5

**问题**：配置项 `task_timeout_secs`（默认 7200）实际上**没被任何代码读**。运维以为有超时保护，实际没有。

**修复方案**：
```rust
// backend/src/core/process.rs
pub async fn start_recording(
    &self,
    config: RecordingConfig,
    timeout_secs: u64,  // 新增参数
) -> Result<RecordingHandle> {
    // ... 现有 spawn 逻辑 ...
    
    // 监控任务里加超时分支
    tokio::spawn(async move {
        let result: Result<Option<i32>, String> = tokio::select! {
            // 现有：等待子进程结束
            exit_status = child.wait() => { ... }
            // 现有：等待 kill 信号
            _ = kill_rx => { ... }
            // 新增：超时
            _ = tokio::time::sleep(Duration::from_secs(timeout_secs)) => {
                warn!("录制任务超时 ({}s)，强制停止: task_id={}", timeout_secs, task_id_clone);
                let _ = child.kill().await;
                Err(format!("录制超时 ({}s)", timeout_secs))
            }
        };
        // ... 后续逻辑
    });
}
```

**调用方**：
```rust
// backend/src/services/recording.rs:117
let handle = match self.process_manager
    .start_recording(config, self.ctx.config.recorder.task_timeout_secs)  // 传 timeout
    .await
{
    // ...
};
```

**验收**：
- [ ] 单元测试：把 `task_timeout_secs` 设成 2 秒，启一个 mock 录制，3 秒后状态是 `failed` 且 error_message 含"超时"
- [ ] 手动：把超时设成 60 秒，启一个会卡住的录制，60 秒后子进程被 kill

**风险**：低。超时只影响"流氓进程"。

---

### 子任务 1.6：优雅停机（**P2 但建议阶段 0 优先做**）

**审计引用**：`backend/src/main.rs:121-122`、§3.1 R2

**问题**：`axum::serve(listener, app).await?` 没监听 SIGTERM，进程被强杀时：
- N_m3u8DL-RE 子进程成为孤儿（子任务 1.2 已修 `kill_on_drop`）
- 进行中的 task 留在 `running` 状态（重启后无人收尾）
- 录到一半的视频文件可能没关闭

**修复方案**：
```rust
// backend/src/main.rs
use tokio::signal;

#[tokio::main]
async fn main() -> Result<()> {
    // ... 现有初始化 ...
    
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app).await
    });
    
    // 等待关闭信号
    tokio::select! {
        res = server_handle => return res?,
        _ = signal::ctrl_c() => {
            info!("🛑 收到 SIGINT，开始优雅停机");
        }
        _ = async {
            #[cfg(unix)]
            {
                let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())?;
                sigterm.recv().await;
            }
            Ok::<_, anyhow::Error>(())
        } => {
            info!("🛑 收到 SIGTERM，开始优雅停机");
        }
    }
    
    // 收尾：取消所有运行中的 task（让子进程退出）
    let ctx = services::ServiceContext::new(db.clone(), config.clone());
    let recording_service = services::RecordingService::new(
        process_manager.clone(),
        ctx,
        Some(event_bus.sender()),
    );
    
    // 标记所有 running task 为 cancelled
    sqlx::query("UPDATE tasks SET status = 'cancelled', ended_at = datetime('now'), updated_at = datetime('now') WHERE status = 'running'")
        .execute(&db)
        .await?;
    
    // 等待子进程被 kill_on_drop 处理（最多 5s）
    tokio::time::sleep(Duration::from_secs(5)).await;
    
    info!("✅ 优雅停机完成");
    Ok(())
}
```

**验收**：
- [ ] 启 1 个录制，`kill <pid>` 主进程，5 秒内子进程消失
- [ ] DB 里之前 `running` 的 task 状态变成 `cancelled`
- [ ] 重启服务后无残留

**风险**：低。SIGTERM 处理是标准做法。

---

## 测试要求

每子任务完成后必须**新增 1 个测试**：

| 子任务 | 测试位置 | 测试内容 |
| --- | --- | --- |
| 1.1 | `core/database.rs` `#[cfg(test)]` | 初始化后 `journal_mode` 是 `wal` |
| 1.2 | `core/process.rs` `#[cfg(test)]` | spawn 后 `Drop` 触发 `kill`（需要 mock child） |
| 1.3 | `services/recording.rs` `#[cfg(test)]` | 5 并发 → 2 成功 3 失败 |
| 1.4 | `tests/events.rs`（新文件）| scheduler 触发的 task 在 event bus 上可见 |
| 1.5 | `services/recording.rs` `#[cfg(test)]` | `task_timeout_secs=2` 时 3s 后 failed |
| 1.6 | 集成测试（手动 + E2E）| `kill <pid>` 后 task 状态变 cancelled |

## 提交策略

- 每个子任务一个 commit，commit message 写明 fix 编号（如 `fix(backend): enable SQLite WAL (1.1)`）
- 关联 `docs/fix/01-backend-01-concurrency-and-reliability.md` 在 PR 描述

## 风险汇总

| 子任务 | 风险 | 缓解 |
| --- | --- | --- |
| 1.1 | WAL 文件不参与备份 | 文档说明 + 备份脚本加 `wal_checkpoint` |
| 1.2 | 误杀其他子进程 | 只对 `process.rs:240` 处的 cmd 设 |
| 1.3 | 事务边界写错 | 优先用方案 A（更明确） |
| 1.4 | event_sender 注入时序 | SchedulerManager::new 已接收 Optional，按需 clone |
| 1.5 | 超时把正常录制打断 | 配置项默认 7200，用户感知不到 |
| 1.6 | SIGTERM 期间新请求进来 | 先停 axum::serve，再做收尾 |

---

*执行入口：从 1.1 开始，按编号往下做。*
