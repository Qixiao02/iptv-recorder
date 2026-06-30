# IPTV Recorder 深度优化分析报告

> **分析日期**:2026-06-30
> **分析范围**:后端 (Rust/Axum) + 前端 (React 19/Vite) + UI/UX 设计 + 性能/构建/部署
> **分析方法**:四路并行只读代码审计,覆盖 50+ 文件,所有发现均带 `file:line` 引用,无臆造问题
> **关联文档**:`deep-analysis-2026-06-02.md`(上一次分析)

---

## 目录

- [TL;DR — 优先级总览](#tldr--优先级总览)
- [🔴 P0 — 立即修复(正确性 / 阻断性)](#-p0--立即修复正确性--阻断性)
- [🟠 P1 — 高价值优化(性能 / 可用性)](#-p1--高价值优化性能--可用性)
- [🟡 P2 — 设计一致性 & UX 提升](#-p2--设计一致性--ux-提升)
- [💡 高价值新功能清单](#-高价值新功能清单)
- [推进顺序建议](#推进顺序建议)
- [附录:已做好的部分(应保留)](#附录已做好的部分应保留)

---

## TL;DR — 优先级总览

| 级别 | 数量 | 关键项 |
|------|------|--------|
| 🔴 Critical | 4 | SQLite WAL 缺失、移动导航失效、压缩未启用、EPG 非原子导入 |
| 🟠 High | ~15 | tasks 热查询无索引/无分页、阻塞 IO、401 硬跳、模态无 a11y、运行时镜像臃肿、无 CI |
| 🟡 Medium/Low | ~30 | 死代码、硬编码 hex、死按钮、i18n defaultValue、字体阻塞、token 残缺 |
| 💡 新功能 | 10 | 媒体库、Dashboard 图表、EPG 一键预约、任务重录、日历视图、频道健康监控等 |

**最高优先级**:SQLite PRAGMA(WAL + busy_timeout + foreign_keys + synchronous)— 3 行代码修复潜伏的正确性炸弹(`ON DELETE CASCADE` 当前完全不生效)和并发写锁问题。

---

## 🔴 P0 — 立即修复(正确性 / 阻断性)

这几项是**正在发生的正确性 bug 或严重缺陷**,投入极小但回报极大。

### P0-1. SQLite 未启用 WAL + foreign_keys 被关闭 ⚠️ Critical

**位置**:`backend/src/core/database.rs:25-34`

**现状**:连接池只设了 `create_if_missing(true)`,缺少三项关键 PRAGMA:

```rust
// 当前代码
SqliteConnectOptions::new()
    .filename(...)
    .create_if_missing(true);
// 就这样,没了
```

| 缺失项 | 后果 |
|--------|------|
| `journal_mode=WAL` | 默认 DELETE 模式,每次写全表锁 + fsync;并发录制(3s 心跳 + cron + 清理)必触发 `database is locked` |
| `busy_timeout` | 默认 0ms,锁冲突立即报错而非等待 |
| `synchronous` | 默认 FULL,每次事务都 fsync(WAL 下 NORMAL 即足够安全) |
| `foreign_keys=ON` | **`ON DELETE CASCADE` 完全没生效**(SQLite 默认 OFF)— 删频道会留下孤立的 tasks/schedules/recordings |

**影响**:
- 并发录制心跳(每 3s)+ cron 触发 + 清理任务 → 写锁冲突,间歇性失败
- 删除频道后,关联的 tasks/schedules 不级联删除,产生脏数据
- 所有写操作都付全量 fsync 成本

**修复**:
```rust
SqliteConnectOptions::new()
    .filename(&absolute_path)
    .create_if_missing(true)
    .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
    .busy_timeout(std::time::Duration::from_secs(5))
    .pragma("synchronous", "normal")   // WAL 下安全
    .foreign_keys(true);                // 强制 schema 里声明的 ON DELETE CASCADE
```

**验证**:写并发测试,N 个并行 INSERT 不再 `SQLITE_BUSY`;删除有外键关联的频道,验证子表行被级联删除。

**独立印证**:后端审计 + 性能审计两路 agent 均标为 Critical/High。

---

### P0-2. 移动端侧边栏导航完全失效 ⚠️ Critical(阻断整类设备)

**位置**:`frontend/src/components/Layout/index.tsx:155-157`,CSS 在 `Layout.css:602-637`

**现状**:
```tsx
<button className="header-btn mobile-menu">
  <Menu size={20} />
</button>
// 没有 onClick
```

CSS 定义了 `.sidebar.mobile-open` 类(`Layout.css:608-610`)和 ≤768px 下 `translateX(-100%)` 隐藏侧边栏,但:
- 汉堡按钮**没有 onClick 处理器**
- `mobileOpen` 状态**不存在**
- `mobile-open` 类**永远不会被加上**

**结果**:≤768px(所有手机)下侧边栏永久隐藏,**用户无法导航到任何页面**,应用完全不可用。

**修复**:
```tsx
const [mobileNavOpen, setMobileNavOpen] = useState(false);
// ...
<button className="header-btn mobile-menu" onClick={() => setMobileNavOpen(true)}>
  <Menu size={20} />
</button>
<aside className={`sidebar ${mobileNavOpen ? 'mobile-open' : ''}`}>
  {/* nav items 加 onClick={() => setMobileNavOpen(false)} */}
</aside>
{mobileNavOpen && <div className="sidebar-backdrop" onClick={() => setMobileNavOpen(false)} />}
```

---

### P0-3. brotli/gzip 压缩已编译但未启用(P0,3 行代码 ~70% 带宽节省)

**位置**:`backend/Cargo.toml:16`(已引 `compression-br` feature),`backend/src/api/router.rs`(未挂 layer)

**现状**:`compression-br` feature 已编译进二进制,但 `router.rs` 中间件栈**没有 `CompressionLayer`**。所有 JSON 列表(channels/tasks/EPG)、`index.html` 全部裸传。

**影响**:JSON 列表用 brotli/gzip 可压缩 70-80%。内网/弱网下体感明显。

**修复**:
```rust
// router.rs,在 TraceLayer 之前
.route(...)
.layer(tower_http::compression::CompressionLayer::new())
```
建议同时在 `Cargo.toml` 启用 `compression-gzip` feature。

---

### P0-4. EPG 导入非原子 + N+1(Medium,可致数据损坏)

**位置**:`backend/src/services/epg.rs:89-108`

**现状**:每个 programme 一条独立 INSERT,无事务:
```rust
for programme in parsed.programmes {
    sqlx::query("INSERT INTO programmes ...").execute(&self.ctx.db).await?;
}
```

**影响**:
- 大型 XMLTV(上万条 programme)每个 INSERT 一次 fsync(叠加 P0-1 的 DELETE journal 更糟)
- **中途失败无法回滚**,留下半截脏数据
- 导入过程长时间持锁,阻塞其他写

**修复**:参照同项目 `backend/src/services/channel.rs:241-358`(`import_channels_batch`)的批处理事务模式:
```rust
let mut tx = pool.begin().await?;
// 分块批量 INSERT ... VALUES (...),(...),...
tx.commit().await?;
```
顺带修复 P0-1 后,单事务只需一次 fsync。

---

## 🟠 P1 — 高价值优化(性能 / 可用性)

### 后端(P1-BE)

#### P1-BE-1. tasks 热查询无索引 + GET /api/tasks 无分页

**位置**:`backend/src/services/recording.rs:631-637`,`migrations/0001_initial_schema.sql:84-90`

**现状**:`SELECT * FROM tasks ORDER BY created_at DESC` 无分页,且 `created_at` **无索引**(已有 `status/channel/started_at` 索引)。

**影响**:这是**最高频的重查询** — 每次 WS 任务状态变化、每次 Tasks 页挂载都全表扫描 + 全量序列化所有历史任务。

**修复**:
1. 新 migration:`CREATE INDEX IF NOT EXISTS idx_tasks_created ON tasks(created_at DESC);`
2. `GET /api/tasks` 加 limit/offset 分页(默认 100)
3. 前端配合加"加载更多"

#### P1-BE-2. 阻塞 `std::fs` 卡 Tokio worker

**位置**:
- `backend/src/services/transcode.rs:935, 962, 1008` — 在 `sleep(500ms)` 轮询循环里用 `std::fs::read_to_string` / `read_dir`(检测 HLS 就绪)
- `backend/src/api/handlers.rs:824, 839` — `std::fs::metadata` / `read_dir`(目录列举)
- `backend/src/services/config_service.rs:312, 322, 324, 331`

**影响**:同步文件 IO 在 Tokio worker 线程上执行,磁盘压力下会卡住整个 async runtime,延迟 WS 推送和 HTTP 响应。transcode 轮询循环是高频元凶。

**修复**:统一换 `tokio::fs::*`(项目其他地方如 `recording.rs`、`database.rs` 已在用)。真正同步的少数点用 `tokio::task::spawn_blocking` 包裹。

#### P1-BE-3. proxy_stream_response 把整条流读进内存

**位置**:`backend/src/api/handlers.rs:1257`

**现状**:`.bytes().await` 把上游媒体流完整读进内存再转发。

**影响**:长媒体流是内存耗尽向量;且延迟高于真正的流式转发。

**修复**:`Body::from_stream(resp.bytes_stream())` 增量转发,内存占用 O(buffer)。

#### P1-BE-4. 录制失败无重试

**位置**:`backend/src/core/process.rs`(spawn 路径),`backend/src/services/recording.rs`(`admit_recording` ~line 1235)

**现状**:任务被 admit 后,子进程(N_m3u8DL-RE / FFmpeg)spawn 一次,早退即判失败,无自动重试。

**影响**:CDN 503、DNS 抖动、token 轮换等瞬时错误直接判失败,而 schedule 是周期性的。

**修复**:加 `max_retries` + 指数退避 + jitter;区分 4xx(不重试)/ 网络·5xx(重试)。每 schedule 可配。

#### P1-BE-5. 登录限流是进程内内存

**位置**:`backend/src/api/rate_limit.rs`(82 行,`LoginRateLimiter`,5 次/15 分钟)

**影响**:多实例部署下每个实例独立计数(攻击者跨实例轮换获 N× 预算),重启清零。与 JWT 层无共享状态。

**修复**:要么文档明确限定单实例;要么挪到 SQLite/Redis,key 为 `(ip, username)` + TTL。

#### P1-BE-6. test_channel 默认接受无效 TLS 证书

**位置**:`backend/src/services/channel.rs:446`(`.danger_accept_invalid_certs(true)`)

**影响**:自签/MITM 节点被标成"健康",训练运维忽视不安全源。同一 flag 在 proxy 客户端路径也存在。

**修复**:运行时/代理路径严格验证;仅"测试连接"显式操作可 opt-in `verify_tls: bool`。

#### P1-BE-7. 次要索引补充

| 查询 | 文件 | 建议索引 |
|------|------|----------|
| 历史 schedule 查询 | `migrations/0001` | `CREATE INDEX idx_tasks_schedule_id ON tasks(schedule_id);`(0006 的 partial 索引只覆盖 running) |
| audit `failed_tasks_24h` | `audit.rs:117` | `CREATE INDEX idx_tasks_status_updated_at ON tasks(status, updated_at);` |
| `reconcile_orphaned_tasks` 启动 N+1 | `recording.rs:706-744` | 单条 `SELECT id,name FROM channels WHERE id IN (...)` 批量替代 |

---

### 前端(P1-FE)

#### P1-FE-1. 死代码 / 双系统(删了首屏变小、心智负担降低)

| 文件 | 问题 | 处置 |
|------|------|------|
| `frontend/src/stores/channelStore.ts` | **零引用**(全靠 TanStack Query) | 删除 |
| `frontend/src/components/useToast.ts` | 与 `toastStore.ts` 两套 toast,仅 `ToastItem` 类型被 `ConfirmDialog` 引用 | 类型移入 `toastStore.ts` 后删除 |
| `frontend/src/locales/zh-CN.ts` + `en-US.ts` | 与 `i18n/modules/` **完全重复**,且被静态打进首屏 chunk | 删除,首屏只 seed `common` namespace |
| `frontend/src/App.css` | Vite 模板残留(`.react`、`logo-spin`、`@keyframes`),零引用 | 删除 |

#### P1-FE-2. Tasks 页双份实时状态

**位置**:`frontend/src/pages/Tasks/index.tsx:89-180`

**现状**:`App.tsx:97-109` 已经 patch `['tasks']` 缓存,Tasks 页又维护一个 `liveProgress` Map 再 merge(`tasksWithLiveProgress` memo)。每个 progress tick 两次状态写 + 整列重渲染。

**修复**:删 page-local Map,信任缓存(`lib/taskRealtime.ts` 已抽出 patch 逻辑)。

#### P1-FE-3. 401 处理用硬跳 + 不回原页面

**位置**:`frontend/src/api/client.ts:34-41`

**现状**:`window.location.href = '/login'` — 丢内存状态、绕过 router、直接操作 localStorage 跳过 store 清理;Login 提交后永远 `navigate('/')`,**废弃了 `ProtectedRoute` 已实现的 `location.state.from` 机制**。

**修复**:`useAuthStore.getState().logout()` + router navigate + Login 提交 honor `location.state.from`。

#### P1-FE-4. 模态框无 a11y(7 个模态全是)

**位置**:`TaskDetailModal.tsx:87`、`ScheduleModal.tsx:214`、`ChannelModal.tsx:94`、`ImportM3UModal.tsx:136`、`EpgProgramsModal.tsx:30`、`ConfirmDialog.tsx:48`、`MiniPlayer.tsx:175`

**问题**:无 Esc 关闭、无焦点陷阱、关闭不还原焦点。键盘/屏幕阅读器用户被困住。

**修复**:抽一个带 focus-trap 的可复用 `<Modal>`,所有模态改用它。

#### P1-FE-5. 侧边栏导航是 `<div onClick>`

**位置**:`frontend/src/components/Layout/index.tsx:128-136`

**问题**:不可聚焦、不可键盘操作、无 `role`。同样模式见 `Channels/index.tsx:570`(卡片)、Tasks 任务卡、Schedules 卡。

**修复**:导航/选择元素用 `<button>`/`<a>`,或加 `role="button" tabIndex={0}` + Enter/Space 键处理。

#### P1-FE-6. Dashboard 加载全部频道建 channelMap

**位置**:`frontend/src/pages/Dashboard/index.tsx:139-142`

**现状**:`getAllChannels`(398 条全表拉)只为建 `channelMap` 给 ~10 行任务查名字;还附带一条 `['channels','count']`(`page_size:1` 取 total)。

**修复**:后端给 task DTO JOIN 上 `channel_name`;前端这两条请求彻底删掉。

#### P1-FE-7. 无 query-key 工厂

**位置**:query key 字面量散落 6+ 文件(`['tasks']`、`['channels', page, ...]`、`['channels','all']`、`['config']`、`['audit','logs',...]` 等)

**风险**:一处拼写错误静默破坏缓存同步。

**修复**:引入 `queryKeys` 工厂(`queryKeys.tasks.all()`、`queryKeys.channels.list(filters)`)。

#### P1-FE-8. 顶层 ErrorBoundary 恢复不力

**位置**:`frontend/src/components/ErrorBoundary.tsx:13-61`

**问题**:整个 app 一个顶层 boundary;lazy chunk 加载失败时,"Retry" 只清 state 不重新触发 `import()`,留空白;内联硬编码 `#fff/#d9d9d9` 在暗色下违和。

**修复**:每路由独立 boundary(套进 `withSuspense`);retry 对 chunk-load 失败强制 `window.location.reload()`。

---

### 部署 / 构建(P1-OPS)

#### P1-OPS-1. 运行时镜像用 node:20-alpine

**位置**:`Dockerfile:53`

**现状**:跑的是 Rust 二进制 + 静态文件,却用 `node:20-alpine` 作 runtime base(~150MB+,含完整 Node 运行时)。

**修复**:换 `alpine:3` + `apk add ca-certificates curl ffmpeg libgcc openssl sqlite-libs`。**省 100MB+、移除 Node 攻击面**。

#### P1-OPS-2. Rust 构建阶段用 node 基镜像 + 无 cargo-chef

**位置**:`Dockerfile:23, 27-28`

**问题**:
- Rust 构建基于 `node:20-alpine` + apk 装 cargo(版本可能过时)
- 无 `cargo-chef`,任何 `backend/src` 改动都从零全量 release 构建(数分钟)

**修复**:
- builder 换 `FROM rust:1-alpine AS builder`
- 引入 `cargo-chef`(recipe → deps → src 三层,src 改动只重编本 crate)

#### P1-OPS-3. 无 CI/CD

**现状**:`.github/` 不存在;`deny.toml` 已写好却没接;clippy/eslint 靠自觉。

**修复**:加 `.github/workflows/ci.yml` 跑 `cargo fmt --check` + `cargo clippy -- -D warnings` + `cargo test` + `cargo deny check` + `pnpm lint` + `pnpm test` + `pnpm build`。

#### P1-OPS-4. Cargo release profile 未调优

**位置**:`backend/Cargo.toml`(无 `[profile.release]`)

**修复**:
```toml
[profile.release]
lto = "thin"
codegen-units = 1
strip = true
opt-level = 3
```

---

## 🟡 P2 — 设计一致性 & UX 提升

### 设计系统(P2-DS)

#### P2-DS-1. ~106 处硬编码 hex 绕过 token 系统

**位置**:`ConfirmDialog.css:74-91,158-171`、`Modal.css:656-700`、`PlayerModal.css:130-137`、`Settings.css:911,922,928`、`Login.css:10-17` 等

**问题**:toast、transcode-help、player badge、login 背景等用 `#fbbf24/#f87171/#22c55e` 等裸 hex,**不跟主题变**。注意 `Settings.css` 里出现的 `#22c55e` 与 `--color-success`(`#10B981`)还不一致。

**修复**:全换 `var(--color-*)`;如需 `rgba(...,0.15)` 弱色背景,补 `--success-weak/--error-weak` 等 tint token。**顺带补完 light mode**。

#### P2-DS-2. 4 个 CSS 变量被引用却从未定义

| 变量 | 引用位置 | 静默回退 |
|------|----------|----------|
| `--bg-secondary` | `Layout.css:337`、`Channels.css:464`、`Settings.css:964` | 浅色 fallback,暗色下违和 |
| `--bg-hover` | `Layout.css:356,365,374` | 同上 |
| `--border-color` | `Layout.css:356,365,374` | 同上 |
| `--font-mono` | `Modal.css:316,838` | mono 字体在 html/.font-mono 上,未作变量 |

**修复**:在两个主题块补定义(如 `--bg-secondary: var(--bg-panel)`),然后删 fallback。

#### P2-DS-3. 通用选择器 transition 是性能隐患

**位置**:`frontend/src/index.css:686-688`

```css
*, *::before, *::after {
  transition: background-color 0.2s ease, border-color 0.2s ease,
              color 0.2s ease, box-shadow 0.2s ease;
}
```

**问题**:每个元素都加过渡,主题切换/颜色变化时几百节点卡顿;作者已不得不为动画类开特例(`index.css:690-695`)。

**修复**:只给 `.card/.btn/.input` 等具体类加过渡。

#### P2-DS-4. render-blocking Google Fonts @import

**位置**:`frontend/src/index.css:1`

**问题**:`@import url('...fonts.googleapis.com...')` 阻塞渲染,离线/NAT 部署下字体挂起致白屏/闪烁。

**修复**:自托管 woff2(放 `/static`),用 `<link rel="preload">`。

#### P2-DS-5. Tailwind v4 装了却几乎没用

**位置**:`package.json:47`、`index.css:3`(`@import "tailwindcss"`),无 `tailwind.config.*`

**问题**:整个 app 是手写 BEM CSS + CSS 变量,Tailwind 几乎零使用 — 要么死重要么未完成迁移。

**修复**:二选一 — 真用 utility 或删依赖 + `@import`。

#### P2-DS-6. `.card` 三处定义冲突 + 死文件

- `App.css:36`(`.card { padding: 2em; }`)— Vite 脚手架死代码
- `index.css:269` — 全局 `.card`(border + radius + hover)
- `Dashboard.css:96` — 页面局部 `.card`(overflow:hidden,无 hover)— 按特异性覆盖,静默移除全局 hover 行为

**修复**:删 `App.css`;Dashboard 改名 `.dashboard-card` 或用 modifier。

#### P2-DS-7. 无 spacing/typography scale token

颜色/圆角已 token 化,但 padding(`12px 14px`/`16px 20px`/`32px 28px`...)、字号(11/12/13/14/15/16/18/24/28px)全 ad-hoc。

**修复**:补 `--space-1..--space-8` 和 `--text-xs..--text-2xl`。这是长期一致性的最大杠杆。

#### P2-DS-8. 组件定义重复

- `.btn-sm` 在 `index.css:415`、`Channels.css:142`、`Schedules.css` 多处定义 — 漂移风险
- `.toggle` 在 `index.css:640`(44×24)和 `Settings.css:295`(48×26)定义了不同尺寸
- `.btn-primary` 无 `:active` 按压样式

**修复**:统一到 `index.css`。

#### P2-DS-9. `.page-loading` class 被 8 处引用却无 CSS 规则

**位置**:`Dashboard:185`、`Channels:315`、`Tasks:205`、`Settings:349`、`Schedules:154`、`Login:44`、`Layout:108`、`App.tsx:35`

**问题**:grep 全仓无 `.page-loading` 规则,路由切换时显示**纯黑左对齐无样式文字** "Loading..."。

**修复**:在 `index.css` 加居中 spinner + 弱化文字的 `.page-loading` 样式。

---

### 交互 UX(P2-UX)

#### P2-UX-1. 大量"看起来能点却没反应"的按钮(削弱信任)

| 位置 | 按钮 | 问题 |
|------|------|------|
| `Dashboard/index.tsx:329` | "View All" | 无 onClick |
| `Channels/index.tsx:431` | "Batch Record" | 无 onClick |
| `Dashboard/index.tsx:121-123` | TaskRow `MoreHorizontal` | 无 onClick |
| `TaskDetailModal.tsx:127-135` | "Open folder" | 只 toast 显示路径(浏览器开不了服务端文件夹) |
| `Dashboard/index.tsx:208-229` | stat 卡片 `trend` | 误用为无关信息(Storage 卡显示失败数),`trendUp` 恒为 true 总显示绿色箭头 |

**修复**:要么接上(见新功能 #4 重录),要么删;误用的 `trend` 改名 `footnote` 去掉箭头。

#### P2-UX-2. i18n 大量硬编码中文 defaultValue

**位置**:`channels.ts:35-61`、`ImportM3UModal.tsx:82-257`、`ScheduleModal.tsx:21-22,260`、`MiniPlayer.tsx:204,317`、`ErrorBoundary.tsx:26-28`、`client.ts:43`

**问题**:`t('key', { defaultValue: '中文硬编码...' })` — namespace 没加载时英文用户会闪一下中文。

**修复**:移除所有 defaultValue,确保两套 locale bundle 都有 key,靠 `fallbackLng`。

#### P2-UX-3. Toasts 缺类型图标 + 无堆叠上限 + 无进度条

**位置**:`ConfirmDialog.tsx:84-93`(Toast)、`toastStore.ts:32-35`

**问题**:只有彩色左边框,无成功/错误/信息图标区分;固定 3s 无进度条;无堆叠上限(错误爆发会堆积)。

**修复**:加前导图标(CheckCircle/AlertCircle/Info/AlertTriangle)、堆叠上限 ~3 折叠、细进度条。

#### P2-UX-4. 无 `prefers-reduced-motion` 支持

**位置**:唯一一处 `@media (prefers-reduced-motion)` 在死掉的 `App.css` 里。

**问题**:`pulse-recording`、`shimmer`、`float`、`glow-pulse`、`stagger-item`、`animate-spin` 对所有人跑。

**修复**:加 `@media (prefers-reduced-motion: reduce)` 块禁用这些动画。

#### P2-UX-5. 频道 logo 无 lazy loading / 无代理缓存

**位置**:`Channels/index.tsx:486, 577`

**问题**:`<img src={channel.logo_url}>` 无 `loading="lazy"`、无 `decoding="async"`、无 onError 回退、无 referrer policy。page_size 最大 100 时,100 个外部图片请求立即触发,屏外也加载;还会向第三方泄露 referrer。

**修复**:加 `loading="lazy" decoding="async"`,onError 回退 `<Tv/>` 图标;考虑走 `/api/proxy` 代理 + 缓存头。对照 `Markdown.tsx:65` 已正确处理。

#### P2-UX-6. 频道 URL JS 截断

**位置**:`Channels/index.tsx:495` — `<code>{channel.url.slice(0,40)}...</code>`

**问题**:<40 字符也加 `...`;长 URL 丢信息;无法复制完整值。

**修复**:CSS `text-overflow: ellipsis` + `title` 属性。

#### P2-UX-7. Channels 表无粘性表头 / 无斑马纹;表格三套实现

**位置**:
- `Channels.css:156-189` — 真 `<table>`,无 sticky header,长列表滚动丢表头
- `Dashboard/index.tsx:258` — CSS-grid "假表",不同列语法
- `Settings.css:680-705` — **唯一**有 sticky header + scroll wrap 的(最佳实现,但局部)

**修复**:把 Settings audit-table 模式提升为全局可复用 `.data-table`,所有列表共用;长列表加斑马纹。

#### P2-UX-8. 移动端表格退化差

**位置**:`Channels.css:417-435`

**问题**:≤768px 只堆叠工具栏,表本身仍横向滚动,无 Dashboard 那种 card-collapse(`Dashboard.css:449-466`)。

**修复**:给 Channels 表加移动端 card-view 变换,或 ≤768 默认 card 视图。

#### P2-UX-9. Schedules 无搜索/筛选/排序 + cron UX 易错

**位置**:`ScheduleModal.tsx:353-371`(主输入是自由文本 cron,只 6 预设 chips + 帮助表);`Schedules/index.tsx:26-64`(`CronDescription` 人话解释**只在列表不在 modal**)

**问题**:编辑时看不到所输 cron 的人话含义,要保存后才知道;复杂表达式落到原文;Schedules 列表不能搜索/筛选。

**修复**:modal 内实时预览"下次:今天 19:00(2h14m 后)";加 cron 可视化构建器(周几 chips + 时间选择器);列表加搜索。

#### P2-UX-10. ChannelModal 无客户端表单校验

**位置**:`ChannelModal.tsx:76-84`

**问题**:只校验 `name/url` 非空禁用提交,无 URL 格式校验。坏 URL 发到后端才报通用 toast。

**修复**:客户端校验 URL 格式 + 行内错误。

#### P2-UX-11. icon-only 按钮缺 aria-label

**位置**:`Channels/index.tsx:506-548`、`Tasks/index.tsx:386-391`、`Dashboard/index.tsx:117-123`、`ScheduleModal.tsx:218`

**问题**:靠 `title`(屏幕阅读器不朗读)。注意 `MiniPlayer.tsx` 已正确加 aria-label,可作范本。

**修复**:所有 icon-only 按钮加 `aria-label`。

#### P2-UX-12. 错误反馈仅靠瞬态 toast

**位置**:所有 mutation `onError`

**问题**:错误只显示 3s toast 然后消失;`refetchOnWindowFocus:false` 意味着失败的页查询重聚焦也不重试。用户可能完全错过失败。

**修复**:加持久错误状态/错误日志;关键操作给可重试 toast。

#### P2-UX-13. 重复消息包装

**位置**:`client.ts:43-45`

**问题**:拦截器 reject `new Error(message)`,而 message 已来自 `error.response.data.details`;下游 `toast.error(t('common:toast.operationFailed', { message }))` 再包一层"操作失败:...",产生嵌套冗长消息;错误类型全丢成 plain Error。

**修复**:保留类型化错误信息,让 UI 决定包装。

---

## 💡 高价值新功能清单

按影响力排序。每项标注"可复用现有"以降低实现成本。

| # | 功能 | 价值 | 复用现有 |
|---|------|------|----------|
| 1 | **录制媒体库 + 浏览器内回放** — 当前录下来的只能看元数据,不能播放/下载/缩略图(ffmpeg 抽帧) | 高 | HLS.js 已有 |
| 2 | **Dashboard 真实图表** — 当前零数据可视化。加 30 天录制趋势图 + 存储 gauge + 成功率环形图 + GitHub 风格热力图 | 高 | `getTasks`/`getConfig` 数据已齐 |
| 3 | **EPG 一键预约** — EPG 弹窗能看节目但不能操作。点节目 → 自动建单次 schedule(start_at + duration) | 高 | EPG modal + ScheduleModal 已有 |
| 4 | **任务重试 / 重录** — 失败任务只能"查看错误 + 删除",completed 只能"查看 + 删除"。加 "Record again" 闭环 | 高 | `startManualRecord` API 已有 |
| 5 | **日历视图 + cron 实时预览** — Schedules 是平铺列表,ScheduleModal 改 cron 看不到人话。加周历 + "下次:今天 19:00"预览 | 中高 | `getUpcoming` 已有 |
| 6 | **频道健康监控 + 自动禁用/故障转移** — `fail_count` 字段已有但无 UI。加 uptime% + 失败历史 + N 次连续失败自动切备用 URL | 中高 | `fail_count` 字段已有 |
| 7 | **录制健康 watchdog + 自动重启** — 僵尸任务检测已有,但卡死 ffmpeg(没退出但不出数据)没人管。N 分钟无字节 → 杀掉按重试策略重录 | 中 | 配套重试策略(P1-BE-4) |
| 8 | **存储生命周期 / 保留策略** — 定期清理旧录制 + 任务,按年龄/每频道保留数,挂接 heartbeat 磁盘阈值 | 中 | heartbeat 已有 |
| 9 | **Webhook 事件分发** — 把 notification 服务泛化成 webhook,发 `recording.started/completed/failed`,接 Discord/Telegram/智能家居 | 中 | `redact_url_for_log` 可复用 |
| 10 | **播放器:全屏 + 触摸 + 频道列表抽屉** — MiniPlayer 当前无全屏按钮、拖拽/调整/控制栏 mouse-only(触屏不可用)、无播放列表 | 中 | playerStore 跨路由已就绪 |

### Top 3 功能草图

**#1 Dashboard Analytics Row** — 在现有 4 张 stat 卡下方插 `2fr 1fr` 网格:左 = 30 天"Recordings"面积图(x=天,y=录制分钟,completed/failed 堆叠);右 = 径向"Storage"gauge(`totalStorage` 对 `min_free_space_gb`)。下方加 7 列 GitHub 风格录制活跃热力图。图表库 `recharts`(轻量可主题化)或手撸 SVG,复用 `--gradient-brand`/`--color-error` token。

**#2 Calendar Schedule View** — Schedules 页加视图切换(列表 ↔ 日历),默认周网格(7 天列 × 24 时行)。每个启用 schedule 按 cron + duration 渲染为定位块,按频道着色。点块开 ScheduleModal(编辑);拖拽建新 schedule。ScheduleModal 内把裸 cron 输入换成 **cron 输入 + 实时预览**:`▶ Next: Today 19:00 (in 2h 14m), then daily`。

**#3 Re-record / Retry on Tasks** — 每个非运行任务卡(`Tasks/index.tsx:357-384`)加 `.btn-ghost` "Record again"(icon `RotateCcw`)。点击开 `ConfirmDialog`("重录 *{channel}* 时长 *{duration}*?"),确认调 `startManualRecord({ channel_id, duration_seconds, ... })` — 与 `Schedules/index.tsx:128` 同一调用。乐观插入新任务 + 成功 toast。失败任务标 "Retry" 预填同参数。闭环"我漏录一集"。

---

## 推进顺序建议

每块都是独立、可测试、可提交的功能级 unit,符合 git-workflow skill(dev 分支开发、conventional commits、功能级粒度)。

### Sprint 1 — 正确性(P0,建议先做)
> 小改动、低风险、含潜伏正确性炸弹,可单独发 hotfix 版本

- [ ] P0-1: WAL + busy_timeout + foreign_keys + synchronous
- [ ] P0-2: 移动端侧边栏导航
- [ ] P0-3: CompressionLayer
- [ ] P0-4: EPG 事务化批处理
- [ ] (顺带)P1-BE-7: 补 tasks 索引

### Sprint 2 — 后端性能(P1-BE)
- [ ] P1-BE-1: tasks 分页 + 索引
- [ ] P1-BE-2: `tokio::fs` 替换
- [ ] P1-BE-3: 流式代理
- [ ] P1-BE-4: 录制重试

### Sprint 3 — 前端清理(P1-FE)
- [ ] P1-FE-1: 删死代码(channelStore/useToast/locales/App.css)
- [ ] P1-FE-2: Tasks 双状态去重
- [ ] P1-FE-3: 401 路由化
- [ ] P1-FE-6: Dashboard channelMap 去除
- [ ] P1-FE-7: query-key 工厂

### Sprint 4 — a11y
- [ ] P1-FE-4: 抽 `<Modal>` focus-trap
- [ ] P1-FE-5: 侧边栏可访问化
- [ ] P2-UX-11: aria-label 补全

### Sprint 5 — 设计系统
- [ ] P2-DS-1/2: 补 token、去硬编码 hex
- [ ] P2-DS-3: 去通用 transition
- [ ] P2-DS-4: 自托管字体
- [ ] P2-DS-9: `.page-loading` 样式

### Sprint 6 — 部署/构建
- [ ] P1-OPS-1/2: 镜像瘦身 + cargo-chef
- [ ] P1-OPS-3: CI/CD
- [ ] P1-OPS-4: release profile

### Sprint 7+ — 新功能(按需)
- [ ] 功能 #4 任务重录(最小闭环,先做)
- [ ] 功能 #2 Dashboard 图表
- [ ] 功能 #1 媒体库
- [ ] 功能 #3 EPG 一键预约
- [ ] 其余按优先级

---

## 附录:已做好的部分(应保留)

审计也发现了一批**高于平均水平的实践**,这些应予保留、不破坏:

### 后端
- **分层 SSRF 防护** — 私有 IP 检测
- **JWT secret 最小长度强制 + `token_version` 撤销机制**
- **进程内 admission lock + DB partial unique indexes 双保险**(migration 0006)
- **per-task-id 临时目录隔离**(process manager)
- **HLS session/文件服务的路径穿越防护**
- **僵尸任务活跃检测**(via `updated_at` 过期)
- **进程日志 URL 脱敏**(`redact_url_for_log`,`process.rs:506-535`)
- **admission 并发测试覆盖**(recording.rs:2376)
- **TraceLayer token 脱敏**(router.rs:271)

### 前端
- **`usePlayerCore` 抽取得当** — HLS 错误恢复(media/network 计数器、6 次验证循环、attempt-id 取消避免竞态)、共享 video 节点跨大/小模式保活
- **WebSocket 单例 + 类型化事件 + 优雅重连退避**(`websocket.ts:86-103`)
- **MiniPlayer 拖拽/调整零依赖 hooks**(viewport 边界钳制)、PiP 支持、localStorage 持久化带 try/catch
- **Zustand 选择器防全量重渲染**(MiniPlayer 用 `(s) => s.channel` 粒度选择器)
- **完整暗/亮主题 token 对**(`index.css:5-104`)
- **Settings 页 IA 优秀** — 8 分区粘性侧导航 + role-gated + 粘性 Save/Reset footer + 服务端目录浏览器
- **静态资源缓存策略正确** — `/static/assets/*` immutable 1 年,`index.html` no-cache(router.rs:217-236)
- **语义化 HTML**(nav/header/aside/main)、全局 `:focus-visible` 轮廓、登录输入 autoComplete
- **类型严格** — 零 `any`、零 `@ts-ignore`、`strict` + `noUnusedLocals` + `verbatimModuleSyntax`

---

## 附录:关键文件索引

| 领域 | 文件 |
|------|------|
| 后端 DB | `backend/src/core/database.rs:25-34`、`backend/src/config.rs:237-239`、`backend/migrations/0001_initial_schema.sql`、`backend/migrations/0006_unique_running_tasks.sql` |
| 后端热查询 | `backend/src/services/recording.rs:631-637`、`backend/src/services/channel.rs:116-185`、`backend/src/services/epg.rs:89-108`、`backend/src/services/scheduler.rs:386-414` |
| 后端阻塞 IO | `backend/src/services/transcode.rs:935,962,1008`、`backend/src/api/handlers.rs:70,824,839,1257` |
| 后端安全 | `backend/src/services/channel.rs:446`、`backend/src/api/rate_limit.rs` |
| 静态服务/缓存 | `backend/src/api/router.rs:217-240,271`、`backend/src/api/handlers.rs:69-93` |
| 前端构建 | `frontend/vite.config.ts`、`frontend/package.json`、`frontend/src/App.tsx` |
| 前端热路径 | `frontend/src/pages/Dashboard/index.tsx`、`frontend/src/pages/Channels/index.tsx`、`frontend/src/pages/Tasks/index.tsx`、`frontend/src/api/websocket.ts`、`frontend/src/api/client.ts:34` |
| 前端死代码 | `frontend/src/stores/channelStore.ts`、`frontend/src/components/useToast.ts`、`frontend/src/locales/`、`frontend/src/App.css` |
| 前端样式 | `frontend/src/index.css:1,686`、`frontend/src/components/Layout/Layout.css:337,602`、`frontend/src/components/ConfirmDialog.css` |
| Docker | `Dockerfile:23,53`、`.dockerignore`、`backend/Cargo.toml` |

---

*本文档由 2026-06-30 四路并行代码审计综合生成。所有发现均基于实际源码,带 `file:line` 引用。后续实施按 git-workflow skill 在 dev 分支逐功能提交。*
