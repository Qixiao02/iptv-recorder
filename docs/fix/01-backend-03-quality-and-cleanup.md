# 后端修复 03：质量与清理

> 优先级：**P1 + P2**（部分 P0）
> 预计工时：5-7 天
> 推荐执行人：`rust-backend-engineer`
> 配套审计章节：`deep-analysis-2026-06-02.md` §3.2 I6, I7, I8, I9, I10, §3.3 S1, §4 H9, H10, P2-9, P2-10, P2-11

## 范围与背景

本文件覆盖后端的**质量债务和长期能力**。共 6 个子任务：跨平台磁盘空间、转码可配置、Channel 模型死字段、cron 简写补全、录制备恢复、可观测性。这些不是 P0 必做，但**都做完后系统才达到"成熟"水平**。

## 子任务清单

### 子任务 3.1：用 `sysinfo` 替代 `df` 解析（**P1**）

**审计引用**：`backend/src/services/recording.rs:1043-1068`、§3.2 I6

**问题**：
```rust
async fn get_available_space(path: &Path) -> Result<u64> {
    let output = tokio::process::Command::new("df")
        .arg("-B1")
        .arg(path)
        .output().await?;
    // 按 Linux `df -B1` 输出格式解析
    let available = line
        .split_whitespace()
        .nth(3)  // ← 硬编码第 4 列
        .ok_or_else(|| anyhow::anyhow!("无法解析 df 可用空间字段"))?
        .parse::<u64>()?;
    Ok(available)
}
```
macOS 的 `df -B1` 输出列名是 `Filesystem 1024-blocks Used Available Capacity iused ifree %iused Mounted`——会解析失败或拿到错误字段。

**修复方案**：用 `sysinfo` crate（已经在依赖生态里常见，跨平台）：
```toml
# backend/Cargo.toml
[dependencies]
sysinfo = "0.32"
```

```rust
// backend/src/services/recording.rs
use sysinfo::Disks;

async fn get_available_space(path: &Path) -> Result<u64> {
    // 把 path 挂到最近的 mount point，sysinfo 是按 mount 列表的
    let disks = Disks::new_with_refreshed_list();
    
    let canonical = tokio::fs::canonicalize(path).await
        .unwrap_or_else(|_| path.to_path_buf());
    
    for disk in &disks {
        if canonical.starts_with(disk.mount_point()) {
            return Ok(disk.available_space());
        }
    }
    
    Err(anyhow::anyhow!("找不到路径对应的磁盘: {}", path.display()))
}
```

**验收**：
- [ ] `cargo build --target x86_64-apple-darwin` 通过（如果装得上的话；不行就在 CI 注释里跑 Linux 模拟）
- [ ] Linux 上 `get_available_space("./data")` 返回与 `df -B1 ./data | tail -1 | awk '{print $4}'` 接近的值
- [ ] 单元测试：mock 一个 `Disks` 列表返回固定值（用 `mockall` 或 trait 抽象）

**风险**：低。`sysinfo` 是 tokio 生态标准 crate。

---

### 子任务 3.2：转码可配置（**P2**）

**审计引用**：`backend/src/services/transcode.rs:78-87`、§3.2 I12

**问题**：
```rust
pub struct TranscodeService {
    // ...
    session_timeout_secs: u64,  // 写死 300
    max_sessions_per_user: usize,  // 写死 2
}

impl TranscodeService {
    pub fn new(hls_base_dir: PathBuf) -> Self {
        Self {
            // ...
            session_timeout_secs: 300,
            max_sessions_per_user: 2,
        }
    }
}
```

**修复方案**：加 `TranscodeConfig` + 从 `Config.transcode` 读
```rust
// backend/src/config.rs 加
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TranscodeConfig {
    #[serde(default = "default_session_timeout")]
    pub session_timeout_secs: u64,
    #[serde(default = "default_max_sessions_per_user")]
    pub max_sessions_per_user: usize,
}

fn default_session_timeout() -> u64 { 300 }
fn default_max_sessions_per_user() -> usize { 2 }

impl Default for TranscodeConfig { /* ... */ }
```

```rust
// backend/src/services/transcode.rs
pub struct TranscodeService {
    // ...
    session_timeout_secs: u64,
    max_sessions_per_user: usize,
}

impl TranscodeService {
    pub fn new(hls_base_dir: PathBuf, config: TranscodeConfig) -> Self {
        Self {
            // ...
            session_timeout_secs: config.session_timeout_secs,
            max_sessions_per_user: config.max_sessions_per_user,
        }
    }
}
```

```toml
# backend/config/default.toml
[transcode]
session_timeout_secs = 300
max_sessions_per_user = 2
```

**验收**：
- [ ] `IPTV__TRANSCODE__MAX_SESSIONS_PER_USER=5` 启动后 `TranscodeService` 内的字段是 5
- [ ] 单元测试：从 config 读

**风险**：低。纯参数化。

---

### 子任务 3.3：Channel 模型死字段清理或实现 health check（**P2**）

**审计引用**：`backend/src/models/mod.rs:60-85`、§3.2 I7

**问题**：Channel 有 `source_type`、`source_url`、`status`、`last_check_at`、`fail_count` 5 个字段，但代码不消费。schema 看着丰富，实际是死字段。

**修复方案**（二选一）：

**方案 A（推荐）：实现 health check**
- 加一个 `services/health_check.rs`，每 N 分钟测一次所有 enabled channel 的连通性
- 更新 `status` / `last_check_at` / `fail_count`
- 超阈值（如 fail_count >= 5）发 `Event::SystemAlert`
- 在 `Dashboard` 的频道卡片显示状态

**方案 B：删字段**
- 写 migration `0005_drop_channel_health_fields.sql`：DROP COLUMN
- 从 Channel struct 删字段
- 从前端 types/index.ts 删字段

**推荐 A**，因为 health check 是**真实有价值的功能**而不是技术债削减。Channel 健康度对用户是有用的。

**如果选 A**：
```rust
// backend/src/services/health_check.rs
pub struct HealthCheckService {
    db: Pool<Sqlite>,
    interval_secs: u64,
    http_client: reqwest::Client,
}

impl HealthCheckService {
    pub async fn run_once(&self) -> Result<()> {
        let channels: Vec<Channel> = sqlx::query_as("SELECT * FROM channels")
            .fetch_all(&self.db).await?;
        for channel in channels {
            // HEAD 请求测试可达性
            match self.http_client.head(&channel.url).timeout(Duration::from_secs(10)).send().await {
                Ok(resp) if resp.status().is_success() => {
                    self.mark_online(&channel.id).await?;
                }
                Ok(_) | Err(_) => {
                    self.mark_offline(&channel.id).await?;
                }
            }
        }
        Ok(())
    }
}
```

在 `main.rs` 加后台任务：
```rust
let health_service = Arc::new(services::HealthCheckService::new(db.clone(), 300));
health_service.clone().start_periodic();  // 5 分钟一次
```

**验收**：
- [ ] 配置 `IPTV__HEALTH_CHECK__INTERVAL_SECS=60`，60 秒后 DB 里 `last_check_at` 更新
- [ ] 故意把一个 channel 的 url 改成无效，等下次检查后 `fail_count` + 1
- [ ] `fail_count >= 5` 时 `event_bus` 收到 `SystemAlert(Warning)`
- [ ] 前端 Dashboard 显示 channel 状态

**风险**：中。需要小心 rate limit（HTTPS 探测可能触发对方 WAF）和 HTTP client 配置。

---

### 子任务 3.4：cron 简写补全 7 种（**P0**）

**审计引用**：`backend/src/services/scheduler.rs:283-316`、§3.2 I8

**问题**：前端 `Schedules/index.tsx:25-72` 暴露 7 种简写（每分钟、每天、工作日、周末、每月 X 日、X 小时、X 分钟），但后端 `parse_simple_format` 只识别 3 种（hourly / daily / weekly）。用户在前端选了"每月 X 日"保存 → 后端解析错误 → cron 不触发。

**修复方案**：扩展 `parse_simple_format` 支持全部 7 种。
```rust
fn parse_simple_format(&self, input: &str) -> Result<String> {
    let parts: Vec<&str> = input.split_whitespace().collect();
    
    match parts.first().copied() {
        Some("hourly") => Ok("0 * * * *".to_string()),
        Some("daily") => Self::parse_with_time(parts.get(1), "0 {} * * *", "0 0 * * *"),
        Some("weekly") => Self::parse_with_time(parts.get(1), "0 {} * * 1", "0 0 * * 1"),
        Some("weekdays") => Ok("0 0 * * 1-5".to_string()),
        Some("weekends") => Ok("0 0 * * 6,0".to_string()),
        Some("monthly") => {
            // "monthly 15" -> "0 0 15 * *"
            let day = parts.get(1).and_then(|d| d.parse::<u32>().ok()).unwrap_or(1);
            Ok(format!("0 0 {} * *", day))
        }
        Some("every_n_hours") => {
            // "every_n_hours 2" -> "0 */2 * * *"
            let n = parts.get(1).and_then(|h| h.parse::<u32>().ok()).unwrap_or(1);
            Ok(format!("0 */{} * * *", n))
        }
        Some("every_n_minutes") => {
            // "every_n_minutes 15" -> "*/15 * * * *"
            let n = parts.get(1).and_then(|m| m.parse::<u32>().ok()).unwrap_or(5);
            Ok(format!("*/{} * * * *", n))
        }
        _ => Ok(input.to_string()),  // 未知 → 假设是标准 5/6 字段 cron
    }
}

fn parse_with_time(time: Option<&&str>, fmt: &str, default: &str) -> Result<String> {
    if let Some(t) = time {
        let parts: Vec<&str> = t.split(':').collect();
        if parts.len() == 2 {
            return Ok(fmt.replace("{}", &format!("{} {}", parts[1], parts[0])));
        }
    }
    Ok(default.to_string())
}
```

**前端同步**：把 `ScheduleModal.tsx` 的简写命名与后端对齐（避免再改一次）。

**验收**：
- [ ] 7 种简写全部返回标准 5 字段 cron
- [ ] 7 个单元测试（每种一个）
- [ ] 前端保存"每月 15 日"后，看 DB 里 cron_expression 是 `0 0 15 * *`
- [ ] 集成测试：建"每月 1 日"的 schedule，等 cron 触发（用 mock 时间）

**风险**：低。但**前端 ScheduleModal 的简写列表**需要同步更新——先 grep 一遍确认命名一致再改。

---

### 子任务 3.5：录制备恢复（**P2**）

**审计引用**：跨 §3 §4 H1、§5 P2-9

**问题**：服务崩溃后重启，正在录制的 task 留在 `running` 状态——但实际子进程已死。重启后这些 task 永远 `running`，占着 `max_concurrent` 配额。

**修复方案**：启动时扫描所有 `status='running'` 的 task，做一次"回收"：
```rust
// backend/src/main.rs
async fn recover_orphaned_tasks(db: &Pool<Sqlite>) -> Result<u64> {
    // 把所有 running 且不是真正运行中的 task 标记为 failed
    let now = Utc::now().to_rfc3339();
    let result = sqlx::query(
        r#"
        UPDATE tasks
        SET status = 'failed',
            error_message = '服务异常退出，task 未正常完成',
            ended_at = ?,
            updated_at = ?
        WHERE status = 'running'
          AND id NOT IN (
            -- 排除真正还在运行的（如果有外部标记）
            SELECT id FROM active_processes
          )
        "#,
    )
    .bind(&now)
    .bind(&now)
    .execute(db)
    .await?;
    
    let count = result.rows_affected();
    if count > 0 {
        tracing::warn!("回收 {} 个孤儿 task", count);
    }
    Ok(count)
}
```

**进阶方案**：把"真正运行中"通过 ProcessManager 自己的 in-memory map 校验——重启后 in-memory map 是空的，所以所有 running task 都变 orphaned。最简单：重启时清空 running → failed。

```rust
// main.rs
let recovered = sqlx::query(
    "UPDATE tasks SET status = 'interrupted', ended_at = ?, updated_at = ? WHERE status = 'running'"
)
    .bind(&now).bind(&now).execute(&db).await?;
tracing::info!("启动时回收 {} 个未完成任务", recovered.rows_affected());
```

**前端配合**：增加 `interrupted` 状态显示，提示用户"录制未完成但文件可能不完整"。

**验收**：
- [ ] 启动 1 个录制 → `kill -9` 主进程 → 重启 → DB 里这个 task 状态是 `interrupted` 而不是 `running`
- [ ] 用户在前端 Tasks 页看到 `interrupted` 状态有视觉提示

**风险**：低。启动时一次 SQL 而已。

---

### 子任务 3.6：可观测性（Prometheus / OpenTelemetry）（**P2**）

**审计引用**：§5 P2-10

**问题**：项目有 `tracing` 但没 metrics 端点。生产部署没法监控"每秒 HTTP 请求数"、"录制成功率"、"WS 连接数"等关键指标。

**修复方案**（按工时分）：

#### 3.6a：加 `metrics` crate + `/metrics` 端点
```toml
# backend/Cargo.toml
[dependencies]
metrics = "0.23"
metrics-exporter-prometheus = "0.15"
```

```rust
// backend/src/observability/metrics.rs
use metrics_exporter_prometheus::PrometheusBuilder;

pub fn init() {
    PrometheusBuilder::new()
        .install()
        .expect("Failed to install Prometheus recorder");
}
```

```rust
// 在 main.rs
observability::metrics::init();

// 在 handlers.rs 关键路径
metrics::counter!("http_requests_total", "path" => path, "method" => method).increment(1);
metrics::histogram!("http_request_duration_seconds", "path" => path).record(duration);
```

```rust
// 在 router.rs 加 metrics 端点（admin only）
.route("/metrics", get(metrics_handler))
```

**关键指标**（至少要有）：
- `http_requests_total{method, path, status}`
- `http_request_duration_seconds{method, path}`
- `recording_tasks_total{result}` (success/fail/cancel)
- `recording_duration_seconds{channel_id}` (histogram)
- `ws_connections_active` (gauge)
- `db_query_duration_seconds{query_name}` (histogram)

**验收**：
- [ ] `curl http://localhost:3000/metrics` 返回 Prometheus 格式
- [ ] 跑 1 次录制后 metrics 里有 `recording_tasks_total{result="success"} 1`
- [ ] prometheus.yml scrape 配置示例进 `docs/operations-runbook.md`

**风险**：中。Prometheus 中间件可能影响性能（默认每 60s scrape 一次，但有 200+ 标签的 metric 会爆）。先从 5 个核心指标开始，不一上来全打。

---

## 测试要求

| 子任务 | 测试 |
| --- | --- |
| 3.1 | `get_available_space` 跨平台（用 trait 抽象后 mock）|
| 3.2 | config → TranscodeService 字段映射 |
| 3.3 | health check 一次完整周期（mock reqwest）|
| 3.4 | 7 种简写 + 7 个单元测试 |
| 3.5 | 启动时回收（DB 操作后断言状态）|
| 3.6 | `/metrics` 端点返回 200 + 含 `recording_tasks_total` |

## 提交策略

- 子任务 3.4 优先级最高（前端用户已受影响）
- 其余按 ROI 排序
- 每个子任务独立 commit

## 风险汇总

| 子任务 | 风险 | 缓解 |
| --- | --- | --- |
| 3.1 | sysinfo 跨平台 API 差异 | trait 抽象 + mock 测试 |
| 3.2 | 配置文件升级 | migration 文档 |
| 3.3 | health check 触发 WAF | 限制并发 + 标记 `User-Agent: iptv-recorder/0.1.0` |
| 3.4 | 前端简写列表与后端不一致 | 先 grep 同步再改 |
| 3.5 | 误标记正常 task | 只在启动时跑一次 |
| 3.6 | 性能开销 | 控制标签基数 + 采样 |

---

*执行入口：3.4 → 3.1 → 3.5 → 3.2 → 3.3 → 3.6。*
