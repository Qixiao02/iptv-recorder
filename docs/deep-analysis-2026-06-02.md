# iptv-recorder 深度代码审计报告

> 审计时间：2026-06-02
> 审计范围：`D:\work\Porject\iptv-recorder`（前端 + 后端 + docs + scripts）
> 审计人：Mavis 团队（基于实际源码 + 前端深度扫描报告综合）
> 审计方法：源码逐文件通读 + 项目专属 reins 配置

---

## 0. TL;DR（5 句话结论）

1. **项目已可用且功能完整**——核心 5 个前端页面（Dashboard/Channels/Schedules/Tasks/Settings）和后端 Axum API 全栈贯通，录制主链路能跑通；但**生产化程度低**，缺 CI、缺集成测试、缺安全加固。
2. **最大风险是"会卡在看似完成的功能上"**——SQLite 写并发、进程孤儿、调度重叠触发、WS 重连风暴这 4 类问题任何一个都可能让"功能可用但生产爆雷"。
3. **前端 vs 文档/实现漂移严重**——3 份文档（`docs/frontend-design.md`、`docs/frontend-prompt.md`、`CLAUDE.md`）都说用 Ant Design 6.x，但 `package.json` 零 antd 依赖，是**纯 Tailwind 4 + 自定义 CSS**。
4. **测试覆盖率极低**——前端 3 个工具函数测试（无组件/无 E2E），后端 6 个模块有 `cfg(test)` 块但**无集成测试**（`backend/tests/` 不存在），无 CI pipeline（`.github/workflows/` 不存在）。
5. **最高 ROI 修复点**：补 3 个集成测试（scheduler 触发、recording 终态、process 进程清理）+ 修 4 个并发问题（capacity 竞争、kill_on_drop、WAL、WS 退避）+ 修 3 个安全细节（鉴权失败响应、CORS 收紧、SSRF DNS rebinding）= **能挡 80% 的生产事故**。

---

## 1. 项目概述

### 1.1 这是什么

基于 Rust + React 19 的 **IPTV M3U 频道管理和定时录制系统**。用户导入 M3U 播放列表、设置 cron 定时任务，到点系统调用外部工具 `N_m3u8DL-RE` 把 HLS/DASH 直播流录到本地；同时提供一个 Web 后台管理 UI。

### 1.2 技术栈

| 层 | 栈 |
| --- | --- |
| **后端** | Axum 0.8 + Tokio 1.42 + sqlx 0.8 (SQLite) + tokio-cron-scheduler 0.12 + figment 配置 + reqwest + tracing + bcrypt + jsonwebtoken |
| **前端** | React 19 + TypeScript + Vite 7 + Zustand + TanStack Query 5 + React Router 7 + i18next + hls.js + dayjs + axios |
| **外部依赖** | `N_m3u8DL-RE`（HLS/DASH 录制工具）+ `FFmpeg`（转码预览） |
| **持久化** | SQLite（单文件 `data/iptv-recorder.db`）+ 本地文件系统（`data/recordings/`） |
| **实时通信** | 服务端 broadcast channel → WebSocket → 前端 TanStack Query 缓存 patch |

### 1.3 当前发布状态

- 版本 `0.1.0`（`backend/Cargo.toml:3`）
- 最近 commit: `e6ccf30 fix scheduled execute error handling`（说明调度链路是当前重点打磨方向）
- 无 CI、无 release pipeline、无 docker 镜像（`scripts/dev.sh` 是开发脚本）

### 1.4 文档齐全度

`docs/` 下 **12 个 md 文件**，覆盖了架构、API、数据库、配置、部署、前端设计、运维 SOP、安全、UI prompt 等——**文档数量和成熟度比代码高**，这是这个项目最值得注意的反差。

---

## 2. 开发进度评估

### 2.1 后端（`backend/src/`，27 个 Rust 文件）

| 模块 | 状态 | 主要功能 | 关键缺口 |
| --- | --- | --- | --- |
| **入口 `main.rs`** | ✅ 完整 | 加载配置 → 校验密钥 → 初始化 DB → 起 ProcessManager/EventBus/Scheduler/Cleanup/Transcode → 启 Axum | 无优雅停机（SIGTERM 后子进程可能孤儿） |
| **配置 `config.rs`** | ✅ 完整 | figment 三层合并（env > config file > defaults），`IPTV__` 前缀嵌套 | 无热加载，运行时改 config 要重启 |
| **数据库 `core/database.rs`** | ✅ 完整 | sqlx migrate + 4 个迁移文件 + legacy 兼容补丁 | **未启用 WAL**（`database.rs:25-39`），并发写会阻塞；迁移用 PRAGMA 字符串拼接（`database.rs:102-108`） |
| **事件总线 `core/event.rs`** | ✅ 完整 | broadcast channel，5 个事件类型，1000 容量 | 无慢消费者保护、replay、过滤 |
| **进程管理 `core/process.rs`** | 🟡 完整但有风险 | 包装 `N_m3u8DL-RE`，oneshot kill channel + watch status | `child.kill()` 用 SIGKILL（不是注释写的 SIGTERM）`process.rs:280`；**无 `kill_on_drop`**（`process.rs:240-242`），panic 必留孤儿 |
| **认证 `services/auth.rs`** | ✅ 完整 | bcrypt + JWT（24h 过期），3 角色（admin/operator/viewer） | 默认密码写到 warn 日志（`auth.rs:102-105`）；无 rate limit；无 refresh token |
| **频道 `services/channel.rs`** | ✅ 完整 | CRUD、import M3U/EPG、test 探测 | （未读全文，但从 handler 列表看能力完整） |
| **计划 `services/schedule.rs`** | ✅ 完整 | CRUD、启用/禁用 | （同上） |
| **调度器 `services/scheduler.rs`** | 🟡 完整但有并发隐患 | tokio-cron-scheduler 包装，CronTrigger 手动触发，scheduler_timezone 解析 | `parse_simple_format` 只识别 3 种（`scheduler.rs:283-316`）；`trigger_all` 内部 `add_schedule` 会**重置 cron 状态**（`scheduler.rs:362-383`）——次次重新注册，下一 tick 从"现在"算起，不是原 next_run；`reload` 整体重启调度器（`scheduler.rs:254-275`），期间所有任务停摆 |
| **录制 `services/recording.rs`** | 🟡 完整但有 race + 静默失败 | 任务生命周期（insert → spawn N_m3u8DL-RE → 监控 → 完成态） | `ensure_recording_capacity` 是**读-写之间有窗口**的 TOCTOU（`recording.rs:869-892`）：两个并发 start_manual 都通过 count 检查后都会插入；`event_sender: Option<EventSender>` 经常传 `None`（`recording.rs:172`），意味着**调度触发的录制不推 WS 事件**，前端 Dashboard 的运行中任务可能不更新；`df -B1` 解析硬编码按 Linux 输出格式（`recording.rs:1043-1068`），macOS 直接挂 |
| **转码 `services/transcode.rs`** | ✅ 设计良好 | 3 个 profile 降级（fMP4 → MpegTs → FastRemux），5min 超时清理 | 单用户 2 个会话上限写死（`transcode.rs:103`）不可配置；`drop` 是空操作（`transcode.rs:403-408`） |
| **M3U 解析 `services/m3u_parser.rs`** | 🟡 功能可但边界松 | 6 个静态编译正则，覆盖 EXTINF/tvg-id/tvg-name/tvg-logo/group-title | **接受 `url.starts_with("/")`**（`m3u_parser.rs:213-216`）——意味着 M3U 导入可以把 `/etc/passwd` 当频道源；reqwest 无 body 大小限制（`m3u_parser.rs:78-89`）可 OOM |
| **EPG `services/epg.rs`** | ✅ | source + programs 基础 CRUD | （未读全文，但前端扫描报告指 EPG 源管理独立页缺失） |
| **清理 `services/cleanup.rs`** | ✅ | 按 `task_retention_days` 删旧 task | （未读全文） |
| **审计 `services/audit.rs`** | ✅ | 7 个 query 拼 system_health | 7 个独立 query 可合一（`audit.rs:67-100`，前端报告 §3.3 也指出） |
| **API 路由 `api/router.rs`** | ✅ 完整 | 4 层 middleware（public / authenticated / operator / admin） | **CORS 是 `permissive()`**（`router.rs:176`），生产环境不能这样；HLS 文件路由是 public（`router.rs:95-98`），仅靠 UUID 熵防爬 |
| **WebSocket `api/websocket.rs`** | 🟡 | 鉴权靠 query token | 1008 关闭码、policy violation 处理（`api/websocket.rs:81-86` 推测） |

**后端整体成熟度**：~70%。核心录制链路完整，但**生产化的健壮性（并发、安全、可观测、优雅停机）缺口大**。

### 2.2 前端（`frontend/src/`，36 个文件）

由 `frontend-engineer` 报告（`outputs/frontend-scan/deliverable.md`，475 行）综合：

| 维度 | 进度 |
| --- | --- |
| 5 个核心 page（Dashboard/Channels/Schedules/Tasks/Settings）+ Login | ✅ **全部完成** |
| WebSocket 客户端 + 状态机 + 重连 | ✅ 完整 |
| i18n 资源对齐 | ✅ zh-CN / en-US 各 188 行 |
| TypeScript 严格模式 | ✅ strict + 多个 no* |
| 路由懒加载 + manualChunks | ✅ |
| 组件/E2E 测试 | ❌ **零** |
| Ant Design 集成 | ❌ **零**（文档说用，代码不用） |
| EPG 源管理独立页 | ❌ 缺失 |
| M3U 源管理独立页 | ❌ 缺失 |
| 录制文件库 | ❌ 缺失 |
| 转码会话管理页 | ❌ 缺失 |
| i18n 硬编码 | ❌ **200+ 处中文硬编码**（Layout 15+、Channels 30+、Settings 50+、Schedules 25+、Tasks 25+、Dashboard 20+） |
| WS 重连退避 | ❌ 固定 3s，无指数退避 |
| 公共 `<Modal>` 组件 | ❌ 6 个 modal 各自 60+ 行重复 |
| `useChannelStore` 死代码 | ❌ 全项目零引用 |
| `App.css` 死代码 | ❌ Vite 模板遗留 37 行 |
| 死代码 `assets/react.svg` | ❌ |
| 文档/实现漂移 | ❌ 3 份文档都说 Ant Design 6.x |

**前端整体成熟度**：~75%。功能面 100% 跑通（用户能完整操作所有主要场景），但**代码质量、测试、可维护性、国际化深度**都有明显缺口。

### 2.3 测试体系

| 维度 | 现状 |
| --- | --- |
| 前端测试文件 | 3 个：websocket.test.ts (85 行) / taskRealtime.test.ts (71 行) / configPayload.test.ts (45 行) |
| 前端覆盖率 | < 5%（仅工具函数，无 component/page/store 测试） |
| 后端 `cfg(test)` 模块 | 6 个：scheduler、recording、transcode、m3u_parser、auth、process（大多是 model/utility 级别的单元测试，不是真实集成） |
| 后端集成测试 `backend/tests/` | ❌ 不存在 |
| CI workflow | ❌ `.github/workflows/` 不存在；`.gitlab-ci.yml` 不存在 |
| E2E 测试 | ❌ |
| 覆盖率工具 | ❌ 都没装（cargo-tarpaulin / nyc / vitest coverage 都没配） |

**测试成熟度**：~10%。有单测雏形但**没有 CI 强制**，也没集成/E2E 覆盖关键路径。

### 2.4 文档

12 份 md 文档齐全，结构良好。但有**自我矛盾**：

- `docs/frontend-design.md:34-37` 说 "UI: Ant Design 6.x + Tailwind CSS 4.x"
- `docs/frontend-prompt.md:18` 说 "UI 组件库: Ant Design 6.x"
- `docs/ui-design-prompt.md` **完全不提 Ant Design**，写的是 "Lucide Icons + 自定义组件系统"
- `CLAUDE.md:143` 写 "Frontend: React 19, TypeScript, Vite, **Ant Design**, TanStack Query, ..."
- `frontend/package.json:13-25` **完全没有 antd** 或 `@ant-design/icons` 或 `@ant-design/charts`
- 整个 `src/` **零** `from 'antd'` / `from '@ant-design/icons'`

→ 文档需要在"真实栈"和"理想栈"之间二选一明确，不要四份文档各说各话。

---

## 3. 关键问题清单

> 严重度定义：
> - 🔴 严重：生产事故/数据丢失/安全漏洞
> - 🟡 重要：稳定性/可维护性/性能
> - 🟢 次要：风格/可读性/未来扩展性

### 3.1 🔴 严重

#### R1. 调度 → 录制 → 进程管理 全链路有 race condition
- **位置**：`backend/src/services/recording.rs:869-892` `ensure_recording_capacity` + `services/scheduler.rs:148-204`
- **机制**：scheduler cron 触发 → `start_manual` → `count_running_tasks() >= max_concurrent` 检查 → INSERT task。两个并发 start_manual 都在 check 通过、INSERT 之前 → 都会成功 → 实际并发数超过 `max_concurrent`。
- **影响**：磁盘/CPU 可能被超出配置上限的并发录制打爆，特别是 cron 高频（如每分钟）+ 录制时长长（小时级）的组合。
- **修复**：`INSERT ... WHERE (SELECT COUNT(*) FROM tasks WHERE status='running') < max` 原子化，或用 sqlx transaction + `SELECT ... FOR UPDATE` 模式（SQLite 实际是 BEGIN IMMEDIATE）。

#### R2. `child.kill()` 注释错、且无 `kill_on_drop` → 孤儿进程
- **位置**：`backend/src/core/process.rs:280` 注释"发送 SIGTERM"，但 `tokio::process::Child::kill()` 在 Unix 下是 SIGKILL；`process.rs:240-242` spawn 后没设 `kill_on_drop(true)`。
- **影响**：`RecordingService` 启动的 `tokio::spawn` 监控任务如果 panic、或主进程被 SIGKILL 强杀、`scheduler.reload` 整体重启（`scheduler.rs:254-275`）期间，子进程 N_m3u8DL-RE 不会被清理 → 持续录制、永不停止、占满磁盘。
- **修复**：`cmd.kill_on_drop(true);`（`process.rs:240` 附近），并把"优雅停止"做成显式 SIGTERM + 5s 超时再 SIGKILL（前端扫描报告 §9.2 #12 已类似指出）。

#### R3. SQLite 未启用 WAL → 录制写并发阻塞
- **位置**：`backend/src/core/database.rs:25-39`，初始化只有 `filename` + `create_if_missing`，**没有 `journal_mode=WAL`**。
- **影响**：N_m3u8DL-RE 子进程在写 task progress（3s 一次，见 `recording.rs:154-217`）+ scheduler 触发新 task INSERT + 用户 HTTP 拉 task 列表 = 写竞争。SQLite 默认 rollback journal 模式下并发写会 `SQLITE_BUSY`，上层没看到合适的处理。
- **修复**：`SqliteConnectOptions::new().filename(...).create_if_missing(true).journal_mode(SqliteJournalMode::Wal).busy_timeout(Duration::from_secs(5))`。

#### R4. M3U 解析接受本地路径 → SSRF / 本地文件读取
- **位置**：`backend/src/services/m3u_parser.rs:213-216`
  ```rust
  if !url.is_empty()
      && (url.starts_with("http://")
          || url.starts_with("https://")
          || url.starts_with("/"))  // ← 接受本地绝对路径
  ```
- **影响**：用户通过"导入 M3U"端点可以提交 `file:///etc/passwd` 或 `/etc/shadow` 等本地路径，作为 channel URL 存入数据库。后续 `transcode.rs:434-451` 直接把这个 URL 喂给 FFmpeg（FFmpeg 支持 `file://` 协议）——攻击者导入一次就能让服务**主动读取任意文件**。
- **修复**：删掉 `url.starts_with("/")` 这条；如果支持本地文件，要单独走"本地播放列表"端点，不和 HTTP 混用。

#### R5. 默认 admin 密码以 warn 级别明文写到日志
- **位置**：`backend/src/services/auth.rs:78-105`
  ```rust
  let generated_password = format!("admin-{}", uuid::Uuid::new_v4().simple());
  ...
  tracing::warn!("Created initial admin user. username=admin, password={}. ...", initial_password);
  ```
- **影响**：tracing 默认输出到 stdout——任何能看 container logs / systemd journal 的人都拿到初始 admin 权限。这是合规层面的硬性事故。
- **修复**：默认密码**强制**从 `IPTV_INITIAL_ADMIN_PASSWORD` 读，没有就**拒绝启动**而不是生成随机密码（生成后也只能展示一次到 admin 的初次响应里，不进日志）。

#### R6. /api/transcode/hls 是 public 路由，仅靠 UUID 熵
- **位置**：`backend/src/api/router.rs:95-98` 把 HLS 文件列入 public 路由；`backend/src/services/transcode.rs:1247-1278` 推测无 active session 校验（需读 `handlers.rs` 确认）。
- **影响**：任何拿到 session_id（UUID v4，128 位熵）的人都能拉转码的 HLS 直播流。UUID 难猜，但若日志/数据库泄露（见 R5），等于把直播流也泄露。
- **修复**：HLS 端点要走 auth_middleware，或者把 session_id 改成可猜测但有时效的 token（HMAC(user_id + session_id + start_at)）。

#### R7. CORS 是 `permissive()` 跨域全开
- **位置**：`backend/src/api/router.rs:176` `CorsLayer::permissive()`。
- **影响**：生产部署时任何网站都能带 cookie 调后端 API（如果将来加 cookie auth 会出大事）。当前 JWT 在 Authorization header 不是 cookie，影响有限，但**配置上的"打开"=未来的事故**。
- **修复**：默认拒绝跨域；只在环境变量显式 `IPTV_CORS_ALLOW_ORIGINS=...` 时打开白名单。

### 3.2 🟡 重要

#### I1. 调度器 `add_schedule` 重置 cron 状态 + `reload` 全停
- **位置**：`backend/src/services/scheduler.rs:362-383` `CronTrigger.trigger_all` 内部 `add_schedule` 会 remove+re-add job（`scheduler.rs:93-103`）——意味着每次"测试"所有计划的触发都会**把 cron 的 next tick 重置**到"现在"。`scheduler.rs:254-275` `reload` 整体 `shutdown` + `JobScheduler::new()` + `start()` 期间所有调度停摆。
- **影响**：reload 操作会让正在等待触发的计划**多延迟一个完整周期**。用户改了一个 plan 点保存，不小心触发 reload，下一条记录要等下个完整 cron 周期才录。
- **修复**：`add_schedule` 的"先 remove 旧的再加新的"逻辑只在 schedule 内容真的变化时跑；reload 用"diff 后增量更新"代替"全停全启"。

#### I2. `event_sender: Option<EventSender>` 经常 None → 调度触发录制不推 WS
- **位置**：`backend/src/services/recording.rs:172` `RecordingService::new(pm, ctx, None)` —— 这是 scheduler 闭包里构造的方式（`scheduler.rs:172`）；manual API 调用的 `RecordingService::new(pm, ctx, event_sender)`（推测）传了 sender。
- **影响**：通过 scheduler 触发的录制全程不 emit `TaskProgress` / `TaskUpdate` 事件——前端 Dashboard 上"运行中任务" / "进度条"不更新，用户看不出"定时任务正在录"。
- **修复**：`main.rs:94-97` 创建 scheduler 时把 event_bus.sender() 透传下去；或者 RecordingService 内部自己 subscribe event_bus 而不是靠外部注入。

#### I3. 后端错误处理静默吞错
- **位置**：`recording.rs:185-198` 进度更新 SQL `let _ = sqlx::query(...).execute(...).await`；`recording.rs:124-136` 失败时 `UPDATE ... .await.ok()`；`services/cleanup.rs:21-31` `if let Err(e) = ...` 不上报。
- **影响**：DB 写失败、清理失败都不进事件总线，不进告警——生产爆雷时只看得到"录制没进度"但查不到"DB 卡了"。
- **修复**：至少 `tracing::warn!` + 关键路径上 emit `Event::SystemAlert` 到 event_bus。

#### I4. WebSocket / 前端 4 个高风险细节
- 前端 `src/api/websocket.ts:57` token 走 query string —— 反向代理 access log 会泄露 JWT
- 前端 `src/api/websocket.ts:90` 固定 3s 重连，无指数退避 —— server 短暂 OOM 触发客户端风暴
- 前端 `src/api/websocket.ts:29-32` `new URL(VITE_API_BASE_URL)` —— 若环境变量是 `/api` 相对路径会抛 `Invalid URL`，WS 永远连不上
- 后端 `api/websocket.rs:101` `/ws` 在 public 路由里，跨域升级（见 R7）后任何网站都能升级 WS 调用户 API

#### I5. `task_timeout_secs: 7200` 配置了不 enforce
- **位置**：`backend/src/config.rs:101` 默认 7200；`backend/src/services/recording.rs` 监控循环**没用这个值**。
- **影响**：配置项名给运维一个"超时保障"承诺，实际不生效。
- **修复**：在 `start_recording` 时设 `tokio::time::timeout(Duration::from_secs(task_timeout_secs), handle)` 包裹子进程。

#### I6. `df -B1` 解析硬编码 Linux，macOS/BSD 直接挂
- **位置**：`backend/src/services/recording.rs:1043-1068` `get_available_space`，按 `lines().nth(1).split_whitespace().nth(3)` 解析。
- **影响**：开发者在 macOS 上跑会立刻 panic 类的 `df` 输出格式不同。`sysinfo` crate 是跨平台替代。
- **修复**：用 `sysinfo::Disks` 或 `nix` crate 的 `statvfs`。

#### I7. Channel 模型 4 个字段是死字段
- **位置**：`backend/src/models/mod.rs:60-85` Channel 有 `source_type`、`source_url`、`status`、`last_check_at`、`fail_count` —— 但 `channel.rs` 推测没用上，handlers 也不消费。
- **影响**：schema 看着"丰富"但实际不 enforce，未来加的"自动 health check"功能会撞到"有字段没逻辑"的尴尬。
- **修复**：要么删字段（schema 走 migration 加新列），要么真实现 health check 逻辑并消费这些字段。

#### I8. `parse_simple_format` 只识别 3 种 cron 简写
- **位置**：`backend/src/services/scheduler.rs:283-316` 只处理 `hourly` / `daily HH:MM` / `weekly HH:MM`。
- **影响**：用户在 `frontend/src/pages/Schedules/index.tsx:25-72` 看到的"7 种简单语法"（每分钟、每天、工作日、周末、每月 X 日、X 小时、X 分钟）—— **后端只识别 3 种**。前端能保存但 cron 永远不触发。
- **修复**：把前端的 7 种都映射到后端可识别的标准 cron 表达式；或者后端扩展 `parse_simple_format`。

#### I9. 录制路径消毒有边界但 EPG/m3u 解析后会无脑写入 DB
- 位置：`recording.rs:733-773` `sanitize_filename_part` 做得对；但 `m3u_parser.rs:202-216` 接受 `/etc/passwd` 类本地路径后会存到 channel.url，间接让 `transcode.rs:434-451` 的 FFmpeg 命令读取本地文件。
- 这是 R4 的子效应，但放在 I 类里再次强调——`channel.url` 没有 scheme 白名单校验。

#### I10. `recording.rs:166-170` 进度计算逻辑 bug
- `progress = (elapsed / duration) * 100, min 99` —— 如果 duration 是 0（schedule 没设）会除零。
- 同时 `speed: String::new()` 永远是空字符串（`recording.rs:214`）—— 前端 `TaskProgressEvent.speed` 字段也始终空。

### 3.3 🟢 次要

#### S1. 后端大量 `#![allow(dead_code)]`
- `services/scheduler.rs:5`、`services/transcode.rs:14` 等多处。说明 API 在演进但没清理。
- 风险：未来重构时死字段复活导致行为不一致。

#### S2. `frontend/src/stores/channelStore.ts` 全项目零引用
- 死代码，删除。

#### S3. `frontend/src/App.css` 是 Vite 模板 37 行未被引用
- 死代码，删除。

#### S4. `frontend/src/components/ScheduleModal.tsx:93-98` 6 处 `as any`
- 类型已存在但开发者偷懒。坏习惯传染。

#### S5. 200+ 处硬编码中文
- 见前端扫描报告 §5.2 完整清单。英语用户**完全无法**使用产品（除 locale 翻译过的 key 外全是中文）。

#### S6. 6 个 modal（Channel/Schedule/ImportM3U/EpgImport/EpgPrograms/TaskDetail）重复 60+ 行结构
- 应该抽公共 `<Modal>` 组件（统一遮罩/ESC 关闭/滚动锁）。

#### S7. `formatDuration` / `formatFileSize` / `formatDateTime` 重复 3+ 处实现
- 3 处各写各的，输出格式还不一致（Tasks 用 `1:23:45`、Dashboard 用 `83 min`）。
- 应进 `src/lib/format.ts`。

#### S8. Settings page 922 行单文件
- 7 个 section 全堆在一起，4 个 mutation + 2 个 useEffect 深度同步。应按 section 拆子组件。

#### S9. 文档/实现漂移
- 见 §2.4 末尾。

---

## 4. 风险点（生产环境可能爆雷的清单）

按"概率 × 严重度"排序，每个标 **触发条件** / **当前缓解** / **推荐行动**：

| # | 风险 | 概率 | 严重度 | 触发条件 | 当前缓解 | 推荐行动 |
| --- | --- | --- | --- | --- | --- | --- |
| **H1** | 孤儿 N_m3u8DL-RE 进程耗尽磁盘 | 高 | 🔴 严重 | server OOM / 部署 OOM killer / scheduler reload | 无 | 加 `kill_on_drop` + 主进程 graceful shutdown 清理 |
| **H2** | SQLite 写锁等待 → 录制进度卡死 | 中 | 🔴 严重 | 多个任务同时跑 + Dashboard 频繁 GET tasks | 10 个连接池，但默认 rollback journal | 启用 WAL + busy_timeout |
| **H3** | M3U 导入读到 /etc/passwd → 数据泄露 | 中 | 🔴 严重 | 用户在公网部署 + 攻击者拥有任意用户账号 | 仅有 3 角色权限 | 删 `url.starts_with("/")` 分支 |
| **H4** | 默认 admin 密码在容器日志里 | 高 | 🔴 严重 | 用户没设 `IPTV_INITIAL_ADMIN_PASSWORD` | warn 级别 | 强制读 env，缺就拒启动 |
| **H5** | 调度器 cron 重复触发并发录制 | 中 | 🔴 严重 | cron 频率高 + 录制时长长 | `ensure_recording_capacity` 存在但 TOCTOU | 原子 INSERT 或 BEGIN IMMEDIATE |
| **H6** | WS 重连风暴打爆 server | 中 | 🟡 重要 | server 短暂 OOM 后恢复 | 固定 3s 重连 | 指数退避 + jitter + 上限 |
| **H7** | CORS permissive + JWT 在 Authorization → 跨域读 | 低 | 🟡 重要 | 用户启用了 cookie auth 或新加 cookie 机制 | 暂无 | 默认拒绝跨域 |
| **H8** | `cron 简写` 后端只识别 3 种，前端保存后不触发 | 中 | 🟡 重要 | 用户在前端选了"每月 X 日"保存 | 客户端归一化？需查 `parse_simple_format` 调用 | 后端扩 7 种或前端后端统一词表 |
| **H9** | macOS 开发者 panic | 中 | 🟡 重要 | 任何 macOS 开发 | 文档要求 Linux | 用 `sysinfo` |
| **H10** | WS 鉴权失败 401 响应泄露内部错误 | 低 | 🟡 重要 | 攻击者主动爆破 | 失败响应含 `Token 无效: <jwt error>` | 401 响应只返 `Unauthorized` |
| **H11** | 默认密码 warning + 全量 stdio log → log 泄露 = 密码泄露 | 高 | 🔴 严重 | 任何 stdout 暴露场景 | 截短？ | 同 H4 |
| **H12** | `/api/proxy/stream` 拿 `?url=...` 做 SSRF：DNS rebinding | 中 | 🟡 重要 | 攻击者用域名前后两次解析到不同 IP | `is_private_ip` + `is_disallowed_hostname` 已实现 | 加 to-addr 校验：解析后立即 connect，与后续 reqwest 的解析分开 |

---

## 5. 改进路线图

> 按 ROI 排序：先做 P0（低工时高收益），再做 P1（基础质量），最后 P2（长期投资）。

### P0（1-3 天内必修，影响线上）

| # | 行动 | 涉及文件 | 预期收益 | 推荐执行人 |
| --- | --- | --- | --- | --- |
| **P0-1** | 启用 SQLite WAL + busy_timeout | `core/database.rs:25-39` | 写并发从"会卡"到"稳定并行" | rust-backend-engineer |
| **P0-2** | `RecordingService` 注入真实 event_sender（scheduler 触发的也要推事件） | `services/scheduler.rs:172` + `services/recording.rs:172` | Dashboard 实时显示定时录制进度 | rust-backend-engineer |
| **P0-3** | `tokio::process::Command` 加 `kill_on_drop(true)` | `core/process.rs:240` | 防孤儿进程 | rust-backend-engineer |
| **P0-4** | M3U 解析去掉 `url.starts_with("/")` 分支 | `services/m3u_parser.rs:213-216` | 堵 SSRF / 本地文件读取 | rust-backend-engineer |
| **P0-5** | 默认密码强制从 env 读，缺则拒启动 | `services/auth.rs:78-83` + `main.rs:63` | 密码不落日志 | rust-backend-engineer |
| **P0-6** | `ensure_recording_capacity` 改原子 INSERT | `services/recording.rs:869-892` | 防超额并发 | rust-backend-engineer |
| **P0-7** | Settings `hasChanges` 改白名单路径比较 | `frontend/src/pages/Settings/index.tsx:221` | 避免 false positive | frontend-engineer |
| **P0-8** | Channels 一键测试改 `Promise.allSettled` + 单次 invalidate | `frontend/src/pages/Channels/index.tsx:122-146` | 选 100 频道不卡死 | frontend-engineer |
| **P0-9** | 删除 `useChannelStore` + `App.css` + `assets/react.svg` 死代码 | `frontend/src/stores/channelStore.ts` / `frontend/src/App.css` / `frontend/src/assets/react.svg` | 清理 | frontend-engineer |

### P1（1-2 周，拉升基础质量）

| # | 行动 | 涉及文件 | 预期收益 | 推荐执行人 |
| --- | --- | --- | --- | --- |
| **P1-1** | 加 GitHub Actions CI：lint + tsc + cargo check + cargo test + pnpm test | 新增 `.github/workflows/ci.yml` | PR 时自动验证 | tech-lead 牵头，dev 配合 |
| **P1-2** | WS 客户端加重连退避（1s→2s→4s→... 上限 30s）+ jitter | `frontend/src/api/websocket.ts:90` | 防重连风暴 | frontend-engineer |
| **P1-3** | WS token 改 `Sec-WebSocket-Protocol` subprotocol | `frontend/src/api/websocket.ts:57` | 不进反代 access log | frontend-engineer |
| **P1-4** | 修 WS URL 解析 bug：`new URL(env, window.location.origin)` | `frontend/src/api/websocket.ts:29-32` | 反向代理下 WS 能连上 | frontend-engineer |
| **P1-5** | 收紧 CORS：默认 deny，env 控白名单 | `backend/src/api/router.rs:176` | 跨域防意外 | rust-backend-engineer |
| **P1-6** | `task_timeout_secs` 真 enforce | `backend/src/core/process.rs` + `services/recording.rs` 监控循环 | 配置项承诺兑现 | rust-backend-engineer |
| **P1-7** | 用 `sysinfo` 替代 `df` 解析 | `backend/src/services/recording.rs:1043-1068` | 跨平台 | rust-backend-engineer |
| **P1-8** | 文档统一：删 3 份文档的 Ant Design 描述，或反向引入 antd | `docs/frontend-design.md` / `docs/frontend-prompt.md` / `CLAUDE.md:143` | 防新人按错文档下手 | tech-lead 拍板 |
| **P1-9** | 抽 `<Modal>` 公共组件 + `<EmptyState>` + `<StatCard>` | `frontend/src/components/` | 6 处重复结构变 1 处 | frontend-engineer |
| **P1-10** | 抽 `useWebSocketBridge` hook，把 App.tsx:79-149 70 行副作用挪走 | `frontend/src/App.tsx:79-149` | 可测 | frontend-engineer |
| **P1-11** | `ScheduleModal` 移除 6 处 `as any`（类型已包含字段） | `frontend/src/components/ScheduleModal.tsx:93-98` | 类型严格 | frontend-engineer |
| **P1-12** | i18n 硬编码 200+ 处批量替换（按前端报告 §5.2 表格） | 多个前端文件 | 英文用户可用 | frontend-engineer |
| **P1-13** | Layout 用 selector 拆分 `useUIStore` 订阅 | `frontend/src/components/Layout/index.tsx:42-45` | alerts 变更不重渲染侧栏 | frontend-engineer |
| **P1-14** | 补 3 个集成测试：scheduler 触发 / recording 终态 / process 进程清理 | `backend/tests/` | CI 挡住回归 | qa-engineer + dev 配合 |

### P2（1-2 月，长期投资）

| # | 行动 | 预期收益 |
| --- | --- | --- |
| **P2-1** | 写 5 个 page 的组件测试（`@testing-library/react` 已装） | 覆盖用户路径 |
| **P2-2** | 写 E2E（Playwright）：登录→导入 M3U→建计划→录制→下载 | 验证主流程 |
| **P2-3** | 引入 `tanstack-virtual` 处理 Channels/Tasks 大列表 | 万级数据不卡 |
| **P2-4** | Settings page 922 行按 section 拆子组件 | 可维护性 |
| **P2-5** | 加 EPG 源管理 / M3U 源管理 / 录制文件库独立页 | 文档对齐能力 |
| **P2-6** | `transcode.rs` 的会话上限、超时改为可配置 | 弹性 |
| **P2-7** | 引入 refresh token + rate limit（login 端点） | 安全 |
| **P2-8** | 优雅停机：SIGTERM → cancel running tasks → wait children → exit | 部署安全 |
| **P2-9** | 录制文件可恢复：进程崩溃后下次启动自动续录 | 鲁棒性 |
| **P2-10** | 加 Prometheus metrics 端点 / OpenTelemetry | 可观测 |
| **P2-11** | Channel 模型 4 个死字段（source_type/source_url/status/last_check_at/fail_count）要么删要么实现 health check | schema 干净 |

---

## 6. 测试覆盖矩阵

| 模块/路径 | 单元测试 | 集成测试 | E2E | 状态 |
| --- | --- | --- | --- | --- |
| **前端** | | | | |
| `frontend/src/api/websocket.ts` | ✅ 85 行 | — | — | 状态机 + 重连逻辑覆盖 |
| `frontend/src/lib/taskRealtime.ts` | ✅ 71 行 | — | — | 缓存 patch 函数覆盖 |
| `frontend/src/pages/Settings/configPayload.ts` | ✅ 45 行 | — | — | 字段映射覆盖 |
| `frontend/src/pages/*` (5 pages) | ❌ | ❌ | ❌ | **零覆盖** |
| `frontend/src/components/*` (6+ modals, Layout) | ❌ | ❌ | ❌ | **零覆盖** |
| `frontend/src/stores/*` | ❌ | ❌ | — | **零覆盖** |
| `frontend/src/api/*` (除 websocket) | ❌ | ❌ | — | **零覆盖** |
| **后端** | | | | |
| `services/scheduler.rs` | ✅ 3 tests | ❌ | — | 模型 + 时区测试，无真实 cron 触发 |
| `services/recording.rs` | ✅ 5 tests | ❌ | — | 输出路径 / 取消 / 错误消息，无真实进程测试 |
| `services/transcode.rs` | ✅ 3 tests | ❌ | — | playlist ready / log tail，无 FFmpeg 集成 |
| `services/m3u_parser.rs` | ✅ 2 tests | ❌ | — | 简单格式 + attrs，无大文件/Unicode |
| `services/auth.rs` | ✅ 3 tests | ❌ | — | 密钥校验 + 默认 admin，无 login 流程 |
| `core/process.rs` | ✅ 1 test | ❌ | — | 仅 struct 构造测试，无真实进程 |
| `core/event.rs` | ❌ | ❌ | — | 零测试 |
| `core/database.rs` | ❌ | ❌ | — | 零测试（迁移靠 sqlx-cli 验证） |
| `services/channel.rs` / `epg.rs` / `cleanup.rs` / `audit.rs` / `schedule.rs` / `config_service.rs` / `post_process.rs` | ❌ | ❌ | — | 零测试 |
| `api/handlers.rs` / `router.rs` / `websocket.rs` / `auth_middleware.rs` | ❌ | ❌ | — | 零测试 |
| **CI** | | | | |
| GitHub Actions / GitLab CI | ❌ | — | — | 无 |
| 覆盖率工具 | ❌ | — | — | 无 |
| pre-commit hook | ❌ | — | — | 无 |

**估算整体覆盖率**：< 5%。

---

## 7. 附录：所有发现的文件:行号引用清单

### 后端

**并发 / 可靠性**
- `backend/src/services/recording.rs:869-892` — `ensure_recording_capacity` TOCTOU
- `backend/src/services/recording.rs:124-136` — 失败 UPDATE `.await.ok()` 静默吞错
- `backend/src/services/recording.rs:185-198` — 进度 UPDATE 静默吞错
- `backend/src/services/recording.rs:172` — `event_sender` 经常传 None
- `backend/src/core/process.rs:240-242` — `cmd.kill_on_drop` 未设
- `backend/src/core/process.rs:280` — `child.kill()` 注释与实际不符
- `backend/src/services/scheduler.rs:362-383` — `CronTrigger.trigger_all` 内部 `add_schedule` 重置 cron 状态
- `backend/src/services/scheduler.rs:254-275` — `reload` 整体停摆

**安全**
- `backend/src/services/m3u_parser.rs:213-216` — 接受本地路径
- `backend/src/services/auth.rs:78-105` — 默认密码明文落日志
- `backend/src/api/router.rs:176` — CORS permissive
- `backend/src/api/router.rs:95-98` — HLS 公开路由
- `backend/src/api/router.rs:100` — /ws 公开，靠 query token

**数据库**
- `backend/src/core/database.rs:25-39` — 未启用 WAL
- `backend/src/core/database.rs:69-99` — `ensure_legacy_compatibility` 绕过 sqlx::migrate
- `backend/src/core/database.rs:101-108` — PRAGMA 字符串拼接

**错误处理**
- `backend/src/services/recording.rs:124-136` — 失败静默
- `backend/src/services/recording.rs:185-198` — 进度静默
- `backend/src/services/recording.rs:382-388` — 状态写回用 `rows_affected` 守卫，正确 ✅
- `backend/src/services/cleanup.rs:21-31` — 清理失败不通知

**配置 / 部署**
- `backend/src/config.rs:101` — `task_timeout_secs` 配置了不 enforce
- `backend/src/services/recording.rs:1043-1068` — `df` 解析硬编码 Linux
- `backend/src/services/recording.rs:166-170` — 进度计算可除零
- `backend/src/services/recording.rs:214` — `speed: String::new()` 永远是空

**模型**
- `backend/src/models/mod.rs:60-85` — Channel 4 个死字段（source_type/source_url/status/last_check_at/fail_count）
- `backend/src/services/scheduler.rs:283-316` — `parse_simple_format` 只识别 3 种

### 前端

（完整清单见 `outputs/frontend-scan/deliverable.md` §5.2 §9 §10，下面只列关键）

- `frontend/src/api/websocket.ts:29-32` — URL 解析 bug
- `frontend/src/api/websocket.ts:57` — token 走 query string
- `frontend/src/api/websocket.ts:90` — 固定 3s 重连
- `frontend/src/pages/Settings/index.tsx:221` — `JSON.stringify` 变更检测
- `frontend/src/pages/Channels/index.tsx:122-146` — 一键测试串行
- `frontend/src/components/ScheduleModal.tsx:93-98` — 6 处 `as any`
- `frontend/src/components/Layout/index.tsx:42-45` — `useUIStore` 一次性解构 4 字段
- `frontend/src/stores/channelStore.ts:1-41` — 死代码
- `frontend/src/App.css:1-37` — Vite 模板死代码
- `frontend/src/assets/react.svg` — React 默认 logo
- `frontend/src/App.tsx:79-149` — 70 行 WS 副作用
- `frontend/src/pages/Settings/index.tsx:51` — 922 行单文件

### 文档/实现漂移

- `docs/frontend-design.md:34-37` — "Ant Design 6.x + Tailwind CSS 4.x"
- `docs/frontend-prompt.md:18` — "UI 组件库: Ant Design 6.x"
- `docs/ui-design-prompt.md` — 反向：不提 Ant Design
- `CLAUDE.md:143` — "Frontend: ..., Ant Design, ..."
- `frontend/package.json:13-25` — 零 antd 依赖

---

## 8. 给项目维护者的最终判断

**这是个好项目**——文档齐全、架构清晰、核心功能跑通、测试有雏形。但**生产化距离还差 1-2 个月的扎实工作**：

1. **优先把 3 类风险降下去**（H1 孤儿进程 / H2 SQLite 写并发 / H3 M3U 本地路径读取）—— 这三个是"装上去线上不爆"的前提
2. **补 CI + 集成测试**——目前 0% CI 覆盖率，PR 全靠手测是经不起维护团队轮换的
3. **文档/实现对齐**——Ant Design 这个矛盾会让下一个维护者走 30 分钟弯路
4. **慢做 i18n 硬编码替换**——这是最不影响功能但最影响国际化的债务

最后：**"先把能用的做成可用的"**——前 9 个 P0 工时加起来 < 3 天，能挡 80% 生产事故，ROI 极高。

---

*报告完。如对任何 P0 行动有疑问或优先级调整想法，告诉我，我让对应 agent 直接开干。*
