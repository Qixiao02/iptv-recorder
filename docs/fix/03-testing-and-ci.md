# 测试与 CI 修复

> 优先级：**P0 + P1 + P2**
> 预计工时：4-6 天
> 推荐执行人：`qa-engineer` 牵头，dev 配合
> 配套审计章节：`deep-analysis-2026-06-02.md` §2.3, §5 P1-1, P1-14, P2-1, P2-2

## 范围与背景

本项目测试覆盖率极低（< 5%），无 CI。本文件建立**自动化质量门**。共 5 个子任务：起 GitHub Actions、补 3 个核心集成测试、写组件测试、E2E、覆盖率工具。

## 子任务清单

### 子任务 7.1：起 GitHub Actions CI 骨架（**P0**）

**审计引用**：§2.3, §5 P1-1

**目标**：每次 PR 触发自动构建 + 测试。先跑通骨架，不要求覆盖完美。

**修复方案**：
```yaml
# .github/workflows/ci.yml
name: CI

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main, develop]

jobs:
  backend:
    name: Backend (Rust)
    runs-on: ubuntu-latest
    services:
      # 没有外部服务，SQLite 文件型
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
      
      - name: Cache cargo
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            backend/target
          key: backend-${{ hashFiles('backend/Cargo.lock') }}
      
      - name: Check formatting
        working-directory: backend
        run: cargo fmt --check
      
      - name: Clippy
        working-directory: backend
        run: cargo clippy --all-targets -- -D warnings
      
      - name: Test
        working-directory: backend
        env:
          IPTV_JWT_SECRET: ci-test-secret-with-at-least-32-chars
        run: cargo test --all
  
  frontend:
    name: Frontend (Node)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - uses: pnpm/action-setup@v3
        with:
          version: 9
      
      - uses: actions/setup-node@v4
        with:
          node-version: 20
          cache: pnpm
          cache-dependency-path: frontend/pnpm-lock.yaml
      
      - name: Install
        working-directory: frontend
        run: pnpm install --frozen-lockfile
      
      - name: Type check
        working-directory: frontend
        run: pnpm tsc --noEmit
      
      - name: Lint
        working-directory: frontend
        run: pnpm lint
      
      - name: Test
        working-directory: frontend
        run: pnpm test --run
      
      - name: Build
        working-directory: frontend
        run: pnpm build
```

**验收**：
- [ ] PR 创建时自动触发 workflow
- [ ] backend: fmt + clippy + test 全绿
- [ ] frontend: tsc + lint + test + build 全绿
- [ ] Cache 命中：第二次跑 < 2 分钟

**风险**：低。先用宽松规则（test 跑通即可），覆盖率后续加。

---

### 子任务 7.2：补 3 个核心集成测试（**P0**）

**审计引用**：§2.3, §5 P1-14

**目标**：在 CI 通过的最低基础上，加 3 个**真实业务流**测试，覆盖最关键的生产风险点。

#### 测试 1：scheduler 触发 → recording 启动 → WS 事件（避免 1.4 子任务的 bug 复现）

```rust
// backend/tests/integration_scheduler_recording.rs
#[tokio::test]
async fn scheduler_trigger_emits_ws_events() {
    // 1. 启动 mini iptv-recorder（用 test app builder）
    let ctx = TestContext::new().await;
    let event_bus = ctx.event_bus.clone();
    let mut rx = event_bus.subscribe();
    
    // 2. 创建一个 1 秒后触发的 schedule（mock clock 或用极短 duration）
    ctx.create_schedule("test-1", "channel-1", "*/1 * * * * *").await;  // 每秒
    
    // 3. 等待触发
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // 4. 验证：至少收到一个 TaskUpdate(Running) + TaskUpdate(Completed/Failed)
    let mut got_running = false;
    let mut got_terminal = false;
    for _ in 0..10 {
        match rx.recv().await {
            Ok(Event::TaskUpdate(e)) if e.status == TaskStatus::Running => got_running = true,
            Ok(Event::TaskUpdate(e)) if matches!(e.status, TaskStatus::Completed | TaskStatus::Failed) => got_terminal = true,
            _ => {}
        }
        if got_running && got_terminal { break; }
    }
    assert!(got_running, "scheduler 没 emit TaskUpdate(Running)");
    assert!(got_terminal, "scheduler 没 emit TaskUpdate(Completed/Failed)");
}
```

#### 测试 2：recording 取消的原子终态（验证 §3.1 R1 的修复）

```rust
// backend/tests/integration_recording_cancel.rs
#[tokio::test]
async fn cancel_blocks_completion_writeback() {
    // 这是 services/recording.rs:1216 已有测试的扩展
    // 验证：cancel 后，即便监控任务 race 上发 complete 写回，也被 WHERE status='running' 守卫拒绝
}
```

#### 测试 3：process 进程清理（验证 §3.1 R2 的修复）

```rust
// backend/tests/integration_process_cleanup.rs
#[tokio::test]
async fn recording_process_dies_with_parent() {
    // 1. 启动 1 个真实 N_m3u8DL-RE 进程（用 mock binary：sleep 60）
    let ctx = TestContext::new().await;
    let pid = ctx.spawn_long_running_mock_recorder().await;
    
    // 2. 验证进程在
    assert!(process_exists(pid));
    
    // 3. drop ProcessManager
    drop(ctx.process_manager);
    tokio::time::sleep(Duration::from_millis(200)).await;
    
    // 4. 验证 mock recorder 进程也被 kill
    assert!(!process_exists(pid), "kill_on_drop 没生效，孤儿进程残留");
}
```

**验收**：
- [ ] 3 个集成测试都通过
- [ ] CI workflow 包含 `cargo test --all`（包括 integration）
- [ ] 总测试运行时间 < 5 分钟

**风险**：中。需要先建一个 `tests/common/` 共享测试 helper（建临时 DB、起服务、生成 token 等）。第一次建会花 1-2 天。

---

### 子任务 7.3：覆盖率工具（**P1**）

**目标**：让 CI 显示覆盖率数字，PR 阻塞低覆盖的代码。

**后端**：
```yaml
# .github/workflows/ci.yml 中 backend 步骤加
- name: Install tarpaulin
  run: cargo install cargo-tarpaulin --locked

- name: Coverage
  working-directory: backend
  env:
    IPTV_JWT_SECRET: ci-test-secret-with-at-least-32-chars
  run: cargo tarpaulin --timeout 120 --out Xml --output-dir coverage

- name: Upload coverage
  uses: codecov/codecov-action@v4
  with:
    files: backend/coverage/cobertura.xml
    flags: backend
```

**前端**：
```yaml
- name: Coverage
  working-directory: frontend
  run: pnpm test --run --coverage

- name: Upload coverage
  uses: codecov/codecov-action@v4
  with:
    files: frontend/coverage/cobertura-coverage.xml
    flags: frontend
```

**验收**：
- [ ] Codecov badge 在 README
- [ ] PR 时 Codecov bot 评论覆盖率 diff
- [ ] 后端 ~10%（当前水平），前端 ~5%

**风险**：低。**不强制覆盖率门**——只是展示趋势，避免推高维护成本。

---

### 子任务 7.4：组件测试（**P2**）

**审计引用**：§5 P2-1, §2.3

**目标**：补 5 个 page 的 `@testing-library/react` 组件测试，至少覆盖"渲染 + 主要交互"。

**实施**（按 ROI 排序）：
1. **Login**（最简单）：表单提交、loading、错误提示
2. **Settings**（最复杂 → 拆完后）：4 个核心 mutation
3. **Dashboard**：stat 卡片、upcoming timeline
4. **Channels**：列表渲染、一键测试交互
5. **Tasks**：状态过滤、WS 状态徽章

**验收**：
- [ ] 5 个 page 都有至少 1 个测试
- [ ] 跑通 + 覆盖率 > 30%

**风险**：低-中。需要一些 mocking 实践。

---

### 子任务 7.5：E2E 测试（**P2**）

**审计引用**：§5 P2-2

**目标**：用 Playwright 跑真实浏览器，验证完整用户流程。

**第一个 E2E（最小可交付）**：登录 → 导入 M3U → 创建计划 → 看到 task
```ts
// e2e/login-to-task.spec.ts
import { test, expect } from '@playwright/test';

test('登录到看到第一个 task', async ({ page }) => {
  // 启动后端（用 Playwright global setup）
  // ...
  
  // 登录
  await page.goto('/');
  await page.fill('input[name="username"]', 'admin');
  await page.fill('input[name="password"]', process.env.TEST_ADMIN_PASSWORD!);
  await page.click('button[type="submit"]');
  
  // 看到 Dashboard
  await expect(page.locator('h1')).toContainText('Dashboard');
  
  // 导入 M3U（用测试 fixture URL）
  // ...
  
  // 建计划
  // ...
  
  // 等手动录制出现在 Tasks 页
  // ...
});
```

**集成到 CI**：
```yaml
e2e:
  name: E2E (Playwright)
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - uses: actions/setup-node@v4
      with:
        node-version: 20
    - run: cd backend && cargo build --release
    - run: cd frontend && pnpm install && pnpm build
    - name: Start backend
      run: |
        cd backend
        IPTV__SERVER__PORT=3456 IPTV_JWT_SECRET=e2e-secret-with-at-least-32-chars \
          IPTV_INITIAL_ADMIN_PASSWORD=e2e-test-pass-123 \
          ./target/release/iptv-recorder &
    - uses: microsoft/playwright-github-action@v1
    - run: cd e2e && pnpm install && pnpm exec playwright test
```

**验收**：
- [ ] 1 个完整流程 E2E 在 CI 通过
- [ ] 时间 < 5 分钟

**风险**：中。E2E 容易 flake。建议先有 1 个稳定的再扩。

---

## 测试基础设施

### 共享 `tests/common/` helper

```rust
// backend/tests/common/mod.rs
use sqlx::{Pool, Sqlite};
use std::path::PathBuf;
use uuid::Uuid;

pub struct TestContext {
    pub db: Pool<Sqlite>,
    pub process_manager: Arc<ProcessManager>,
    pub event_bus: Arc<EventBus>,
    pub config: Config,
    pub db_path: PathBuf,
}

impl TestContext {
    pub async fn new() -> Self {
        let db_path = std::env::temp_dir().join(format!("iptv-test-{}.db", Uuid::new_v4()));
        let db = crate::core::database::init(db_path.to_str().unwrap(), 1).await.unwrap();
        let config = Config::default();
        let process_manager = Arc::new(ProcessManager::new(...));
        let event_bus = Arc::new(EventBus::default());
        // ...
        Self { db, process_manager, event_bus, config, db_path }
    }
    
    pub async fn spawn_long_running_mock_recorder(&self) -> u32 {
        // 写一个 /tmp/sleep-forever.sh + chmod，spawn，返回 PID
        // ...
    }
    
    pub async fn create_schedule(&self, name: &str, channel_id: &str, cron: &str) { ... }
}
```

---

## 测试要求（每子任务）

| 子任务 | 测试 |
| --- | --- |
| 7.1 | 跑通 + PR 阻塞不通过 |
| 7.2 | 3 个集成测试 + 1 个 kill_on_drop 测试 |
| 7.3 | Codecov badge 出现 |
| 7.4 | 5 个组件测试 |
| 7.5 | 1 个 E2E 跑通 |

## 提交策略

- 7.1 先上（与代码修复解耦）
- 7.2 在 P0 修复（01-backend-01 + 01-backend-02）后做
- 7.3 / 7.4 / 7.5 持续

## 风险汇总

| 子任务 | 风险 | 缓解 |
| --- | --- | --- |
| 7.1 | CI 启动慢 | 用 cache + 并发 job |
| 7.2 | 集成测试不稳定 | 共享 helper 写好 + 不依赖时序 |
| 7.3 | Codecov 配置错 | 一次跑通后定型 |
| 7.4 | 组件测试 mock 复杂 | 先 Login 这种简单组件 |
| 7.5 | E2E 跑太久 | 1 个完整流程就够 |

---

*执行入口：7.1 → 7.2 → 7.3 → 7.4 → 7.5。*
