# 更新日志

本文件记录 IPTV Recorder 的所有显著变更。格式参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [0.1.7] - 2026-07-04

### 🔒 安全 / 正确性修复（高优先级）
- **数据库启用 WAL + foreign_keys + synchronous=NORMAL**：此前 SQLite 用默认 DELETE 日志模式（每次写全表锁 + fsync），并发录制心跳、Cron 触发、清理任务并发写时会触发 `database is locked`。改用 WAL（并发读 + 单写）+ busy_timeout=5s（锁冲突等待而非立即报错）+ synchronous=NORMAL（WAL 下足够安全且大幅降 fsync 成本）。同时启用 `foreign_keys=ON`，让 schema 声明的 `ON DELETE CASCADE` 首次真正生效（SQLite 默认关闭，此前删频道会留下孤立的 tasks/schedules）。启动时记录连接级 PRAGMA 实际值便于核查。
- **EPG 导入改为单事务**：此前每条节目单各自一条 INSERT（各自一次 fsync），大型 XMLTV（上万条）极慢且长时间持锁；且源 INSERT 与节目写入不在同一事务，中途失败会留下半截脏源。改为单事务包裹：失败整体回滚，且只需一次 fsync。
- **流式代理改为增量转发**：`/api/proxy/stream` 此前用 `response.bytes().await` 把整条上游媒体流读进内存再转发，长直播/大片是内存耗尽向量。改为 `Body::from_stream(bytes_stream())` 增量转发，内存占用 O(buffer)；移除会杀断长流的整体 30s 超时，改 connect_timeout。
- **HTTP 响应压缩**：tower-http 的 `compression-br` feature 此前已编译但未挂 `CompressionLayer`，JSON 列表（channels/tasks/EPG）、index.html 全部裸传。补全 gzip/deflate feature 并挂层，按 Accept-Encoding 协商，文本响应压缩 70-80%。CompressionLayer 内部按 content-type 跳过已压缩类型，流媒体代理的 .ts/.mp4 不受影响。
- **录制任务僵尸检测与启动恢复**：新增 `TaskLivenessService` 常驻巡检——running 任务若超过 `task_stale_timeout_secs`（默认 90s，可在设置页调 30-3600s）未更新 `updated_at`，判定为僵尸并自动置失败、发通知、释放 migration 0006 占用的录制名额。补充 `reconcile_orphaned_tasks` 在后端重启时一次性清理残留 running 任务（在调度器启动前执行），避免"重启后再也录不了同一个台"。

### 🐛 修复
- **移动端侧边栏导航失效**：汉堡按钮此前没有 onClick、无 mobileOpen 状态，CSS 定义的 `.sidebar.mobile-open` 类永不被应用，导致 ≤768px 下侧边栏 `translateX(-100%)` 永久隐藏，手机用户无法导航到任何页面。新增 mobileNavOpen 状态：点汉堡打开抽屉、点导航项或半透明遮罩关闭。
- **401 改为 SPA 路由化 + 登录后回原页**：Token 失效时此前用 `window.location.href='/login'` 整页硬跳，丢失内存状态（未保存表单 / WS 连接）、直接抠 localStorage 绕过 store，且 ProtectedRoute 已实现的 `from` 回跳机制被废弃（重登后回不到原页）。改为 axios 拦截器走 `useAuthStore.logout()` 正确清理 + SPA 跳转带 `state.from`，登录页 honor `from` 回跳。

### 🚀 性能
- **tasks 列表分页 + 状态筛选 + JOIN channel_name**：`GET /api/tasks` 此前返回全部 Task[]（无分页），每次 WS 任务状态变化都触发全量重拉；前端还要额外发一次全量频道请求只为查 channel_id→name。改造为分页信封（PaginatedTasks，结构对齐 PaginatedChannels）+ LEFT JOIN channels 带 channel_name + 可按 status 筛选下推到 DB。
- **tasks 热查询索引**：补 `created_at`（列表 ORDER BY DESC 走索引免排序）、`schedule_id`（按计划查历史）、`(status, updated_at)` 复合（audit/dashboard 的 failed_tasks_24h 扫描）三组索引。
- **HLS 轮询与目录列举改用 tokio::fs**：转码服务的 HLS 就绪轮询（500ms 循环里 `playlist_ready_state`/`count_hls_segments`/`collect_hls_diagnostics`）和设置页的服务器目录浏览器，此前都用 `std::fs` 在 Tokio worker 线程上做同步 IO，磁盘压力下会卡住整个 async runtime，延迟 WebSocket 推送和 HTTP 响应。改为 `tokio::fs` 交由阻塞线程池。
- **Dashboard 去除全量频道拉取**：此前为给任务行查频道名，Dashboard 额外发 `getAllChannels` 全量拉取（数百频道全表）只为建 channelMap。任务列表 JOIN 已带 channel_name，删除该查询与 channelMap，直接读 task.channel_name。
- **Tasks 页去除双状态实时层**：Tasks 页此前维护本地 `liveProgress` Map + 本地 WS 订阅，与 App.tsx 的全局 WS 补丁重复，每个进度 tick 两次状态写 + 整列重渲染。删除本地 Map 与订阅，信任 App.tsx 经 `taskRealtime`（按根前缀 `setQueriesData` 遍历所有任务信封缓存）的实时补丁。

### 🛠 重构
- **集中式 query-key 工厂**：queryKey 字面量此前散落 13 个文件（如 `['tasks']`、`['channels','all']`），一处拼写错误会让缓存失效/实时更新静默失效。新建 `lib/queryKeys.ts` 统一管理（taskKeys/channelKeys/configKeys/notificationKeys/auditKeys/scheduleKeys/upcomingKeys/epgKeys），保留前缀失效 vs 精确读写的语义。
- **删除死代码**：移除零引用的 `channelStore.ts`（频道全走 TanStack Query）、与 `toastStore` 重复的 `useToast.ts`（仅 ToastItem 类型被 ConfirmDialog 引用，迁入 toastStore 后删除）、Vite 模板残留 `App.css`。

### ♿ 无障碍
- **7 个模态补焦点陷阱 + Esc 关闭**：此前所有模态（TaskDetail/Schedule/Channel/ImportM3U/EpgPrograms/ConfirmDialog/MiniPlayer 大窗）都没有 Esc 关闭、没有焦点陷阱、关闭不还原焦点，键盘/屏幕阅读器用户会被困住。新增零依赖手写的 `useModalA11y` hook（Esc 关闭 + 打开聚焦 + Tab 循环 + 关闭还原焦点），5 个标准模态加 role=dialog/aria-modal/aria-labelledby，ConfirmDialog 加 role=alertdialog，MiniPlayer 大窗 Esc 收回小窗。
- **侧边栏导航可访问化**：导航项从 `<div onClick>` 改为 `<button>` + `aria-current="page"`，键盘可操作、屏幕阅读器可识别；收起按钮补 aria-label/aria-expanded。
- **图标按钮补全 aria-label**：此前大量图标按钮只靠 title（屏幕阅读器不朗读）或无标注。为 Channels/Tasks/Dashboard/ScheduleModal 的图标按钮补 aria-label；删除两个无 onClick 的误导性死按钮（Dashboard TaskRow 与 Channels 卡片的 MoreHorizontal）。

---

## [0.1.6] - 2026-06-29

### 🐛 修复
- **频道下拉只显示 100 个**：新建计划 / Dashboard / 任务页的频道选择器之前只能搜到 100 个频道（即便数据库有数百个）。根因是前端 `getAllChannels` 请求分页接口 `page_size=1000`，但后端分页接口把 `page_size` 强制 clamp 到 100（防止单次查询过大），导致超出部分被截断。新增专门的 `GET /api/channels/all` 接口（调用已有的 `ChannelService.list()`，返回全部频道无截断），前端改用它。分页接口的 100 上限保持不动。

### 🚀 性能
- **私有源首帧延迟大幅优化**：预览 UDP 组播 / 网关源时「加载很久才出画面」（最坏 8+30+40=78s）。根因是默认首选的 FastRemux（纯 copy remux）要求凑齐 3 个分片才判定就绪，但 IPTV GOP 2~6s + `hls_time 6` 要 ~18-24s 才有 3 个分片，而超时只有 8s → 几乎必然超时失败，每次降级到 30s 的全编码。现在 `FastRemux` 起播条件放宽为 1 个分片就绪、超时从 8s 提到 15s，首帧一到（~6-10s）即起播，多数情况不再降级。单个分片含完整 IDR 关键帧可解码，hls.js 会自动追后续分片缓冲。

### 🐛 修复
- **播放稳定性增强（IPv4 锁定 + 空分片过滤 + HTTP 重连）**：针对 UDP-over-HTTP 网关源的三类播放中断做容错。① 很多源域名同时有 AAAA(IPv6) 和 A(IPv4) 记录，而 Docker 容器常常 IPv6 出站不通，FFmpeg 优先尝试 IPv6 会连接超时、产出 0 字节分片导致卡死；现在后端在 Rust 侧显式解析 IPv4 并替换 hostname（3s 超时，失败/IPv6-only 源原样返回）。② HTTP/HTTPS 源（含 UDP-over-HTTP 网关）周期性重置 TCP 连接（实测 ~40-50s 一次），现在对这类源开启 ffmpeg `-reconnect` 自动重连。③ 上游重连的几秒内 ffmpeg 会切出 0~4KB 的空分片，被解码会导致缓冲空洞；现在检测到 <10KB 的 `.ts` 分片返回 404，让 hls.js 走 `fragLoadingMaxRetry` 重试，重试窗口内 ffmpeg 通常已完成重连。前端配套调整重试容错参数。

### 🎨 体验
- **播放器警告提示不再遮挡视频**：之前「私有源中转」与「录制中额外拉流」是占满视频顶部的大横幅，持续遮挡播放内容。现在收成右上角小图标 badge，鼠标悬停 / 键盘聚焦时才弹出完整说明 tooltip。私有源为橙色 Radio 图标、录制中为蓝色 badge + 脉冲红点，大窗与小窗统一风格（小窗用更紧凑的 mini 版）。信息完整保留，只是不再持续占用可视区。

---

## [0.1.5] - 2026-06-29

### 🐛 修复
- **紧急修复：v0.1.4 启动崩溃**。v0.1.4 的静态资源缓存头实现用了 `nest_service("/", ServeDir)` 给 ServeDir 包层挂缓存中间件，但 axum 0.8 禁止在根路径 nest，导致运行期 panic：`Nesting at the root is no longer supported. Use fallback_service instead.`。`cargo check` 不报错（编译期不检查路由冲突），只在容器实际启动时暴露，**v0.1.4 镜像无法启动（restart 循环）**。本版改用 `route_service("/") + fallback_service` 兜底子路径，恢复正常启动，同时保留缓存头功能。
- 顺带记录教训：静态服务/路由改动必须在容器里实际启动验证，不能只靠 `cargo check`。

---

## [0.1.4] - 2026-06-29

> ⚠️ **此版本有启动崩溃 bug，已被 v0.1.5 取代。** 请勿使用 v0.1.4 镜像。

### ✨ 新功能
- **播放器大窗/小窗切换**：点「播放」先出全屏大窗，大窗点「最小化」缩为右下角悬浮小窗（**视频流不中断**），小窗点「还原」回大窗，类似 YouTube/B 站的交互体验。小窗可**拖拽**移动位置、拖**右下角 resize** 改变大小，位置与大小用 localStorage 记忆，下次打开还在上次位置。
- **版本号单一数据源**：关于页版本号改从 `package.json` 读取（构建期注入），不再写死在 i18n 文案里。根治了「三处版本号需手动同步、易漂移」的问题（v0.1.2 就曾因此漏改 i18n），今后发版只需改 `package.json` 一处。

### 🛠 重构
- **播放器架构统一**：大窗与小窗合并为同一个 `MiniPlayer` 组件的两种 CSS 表现，共用同一个 `<video>` 节点与 `usePlayerCore` 播放核心，切换模式时 DOM 不变、流不重连。`playerStore` 扩展为状态机（`channel` + `mode` + `position` + `size`），位置/大小持久化到 localStorage。
- **拖拽/缩放手写实现**：新增 `useDraggable` / `useResizable` 两个 hook，纯手写 `mousedown/mousemove/mouseup` 监听 + 视口边界约束，零第三方依赖，与项目无拖拽库的现状保持一致。

### 🐛 修复
- **发版后静态资源 404**：之前 `index.html` 没有缓存头，浏览器启发式缓存导致发版后用户仍拿到旧 `index.html`，引用旧 hash 的 chunk 文件名 → 新容器里该文件已不存在 → 动态 import 失败报错（如 `ScheduleModal-xxx.js 404`）。现在按业界标准设缓存策略：带 hash 的 `/static/assets/*` 一年强缓存 + `immutable`，无 hash 文件（`index.html`、`logo.png`）`no-cache` 每次回源验证。发版后用户无需手动强刷。

### 🔧 工程
- 删除死代码 `PlayerModal.tsx`（已无任何引用，逻辑早已被 `usePlayerCore` + `MiniPlayer` 取代）；`PlayerModal.css` 保留供 `MiniPlayer` 共用。

---

## [0.1.3] - 2026-06-29

### ✨ 新功能
- **单镜像部署**：Dockerfile 新增前端构建阶段，前端构建产物打包进后端镜像，**前后端同源同端口**（访问 `:3033` 即得完整界面），告别「后端容器 + 前端开发容器」分离部署。容器名统一为 `iptv-recorder`，docker-compose 补显式 `iptv-net` 网络。
- **品牌形象升级**：项目 logo 替换为新设计（蓝紫渐变 + 白色频道横条 + 红色录制圆点），原图 4096×4096/6.7MB 压缩到 1024×1024/554KB（lanczos 算法，锐利无失真）。
- **登录页 logo 统一**：登录页由 lucide 场记板图标改为项目 `logo.png`，与侧边栏、关于页全站统一。

### 🐛 修复
- 修复悬浮迷你播放器未打开时，`usePlayerCore` 解构 `null` 导致页面崩溃的问题。

### 🛠 重构
- **SPA 路由改造**：后端新增 `spa_index_handler`，移除旧的 ASCII art 文本首页；路由改用 `.fallback` 实现 SPA history 模式，刷新任意前端路由（`/channels`、`/tasks` 等）不再 404。前端 vite `base` 设为 `/static/`，对齐后端 `ServeDir` 挂载路径。

### 🚀 性能
- **播放热路径日志降级**：边看边转码场景下，`get_hls_file`（每个分片被请求多次）的冗余 INFO 日志与 PathBuf 格式化会同步阻塞响应；ffmpeg 转码的 stats 输出（每秒数十~上百行）拖慢分片写盘。日志降级到 DEBUG（零开销）/warning，缓解播放卡顿。
- **HLS 缓冲策略调优**：前端播放器多囤缓冲（`maxBufferLength` 30/45 → 40/60）、放宽 remux 分片漂移容差（`maxBufferHole` 0.8 → 2.0）、落后直播边缘多一点（`liveSyncDurationCount` 4 → 5/6），减少后端抖动时的转圈卡顿。

### 🔧 工程
- `.gitignore` 新增 `*.tar` / `*.tar.gz` 规则，避免 `docker save` 导出的镜像污染工作区。
- 版本号三处统一到 `0.1.3`（修正 v0.1.2 发版时遗漏更新版本号的问题）。
- 删除冗余的 `CLAUDE.md`，统一以 `AGENTS.md` 为项目指引文件（两者正文完全重复，删除后避免漂移；`AGENTS.md` 是通用 AI 编码代理规范，覆盖面更广）。
- 修正 `AGENTS.md` 前端结构与依赖描述：补全实际存在的 `components/assets/lib/types/test/i18n` 等目录；删除过时的 "Ant Design"，改为实际使用的 Tailwind CSS 4。

---

## [0.1.2] - 2026-06-29

### ✨ 新功能
- **悬浮迷你播放器**：播放器从全屏遮罩大窗改为屏幕右下角的悬浮小窗，打开播放后**不遮挡背景页面**，可边操作频道列表/切到任务页边看，小窗跨路由常驻不消失。悬停显示控制条（复制地址/画中画/关闭），录制中红点脉冲提示。
- **品牌形象统一**：favicon、标签页标题、侧边栏 logo、关于页 logo 全部替换为项目专属 logo，告别默认 Vite 图标与 `frontend` 标题。

### 🛠 重构
- **播放逻辑抽离**：新增 `usePlayerCore` hook，把 HLS/UDP 转码/错误恢复/token 鉴权等播放核心从 PlayerModal 抽出共享，迷你小窗与大窗复用同一套逻辑，避免重复实现。
- **播放状态全局化**：新增 `playerStore`（zustand）管理播放状态，支撑小窗跨路由保持。

### 🎨 优化
- 审计日志列表改为内部滚动（固定高度 + sticky 表头），100+ 条记录不再撑长整页。

### 🔧 工程
- 新增项目级 `git-workflow` skill，规范 dev/main 双分支模型、conventional commits、功能级提交粒度与发版流程。

---

## [0.1.1] - 2026-06-29

### ✨ 新功能
- **录制并发安全**：新增进程级准入锁 + DB 部分唯一索引双重保险，根治「cron 定时录制与手动录制并发撞车」导致的重复录制、突破 `max_concurrent` 上限问题；不同频道仍可正常并发录制，吞吐不受影响。
- **频道来源筛选**：频道管理页新增「来源」筛选下拉（全部 / 公网源 / 私有源），快速区分同名但地址不同的频道（如内网/外网各一路）。
- **计划频道选择器增强**：新建计划选频道时，每项展示「来源徽标（公网/私有）+ 分组 + URL 摘要」，同名频道一目了然；搜索支持按 URL 匹配。
- **画中画播放**：播放器「外部播放器打开」改为「画中画（PiP）」，视频浮到系统桌面置顶小窗，可边浏览边看，不占满屏。不支持的环境降级提示。
- **输出模板新变量**：录制文件名模板新增 `{source}`（公网/私有）、`{group}`（分组）、`{source_url}`（源地址）三个变量，原有变量不变。
- **更新日志页面**：系统设置新增「更新日志」页，滚动查看历史版本所有改动。

### 🐛 修复
- 修复并发场景下「同频道 / 同定时任务」可能产生重复 running 记录的竞态。
- 修复计划立即执行与手动录制并发时偶发突破全局并发上限的问题。

### 🔒 安全
- 录制准入引入原子化串行临界区，DB 层增加部分唯一索引兜底，防止绕过校验的重复录制。
- **P0 止血**：密钥移出代码——`docker-compose.yml` 删除硬编码 JWT 密钥/密码，改 `env_file: .env`；新增 `scripts/generate-env.{sh,ps1}` 首次生成随机密钥；`.env`/`.env.example` 模板与 `.gitignore` 规则；README 删除默认密码。
- **P0 依赖**：升级 `jsonwebtoken` 9.3.1 → 10.4.0（修复 CVE-2026-25537），启用 `rust_crypto` feature；新增 `cargo-deny` 配置（`deny.toml`）做漏洞/协议/依赖源审计。
- **P1 权限边界**：路由提权——`POST /api/config`、`GET /api/system/directories` 收紧为 admin；转码 start/stop 收紧为 operator。
- **P1 SSRF 防护**：抽取共享模块 `services/url_safety.rs`，接入频道 create/import（严格 + 轻量两级）、EPG 导入、流代理；统一拦截云元数据（`169.254.169.254`）/环回地址，放行合法内网网段；`private_server_only` 改为只校 scheme 的宽松校验。
- **P1 路径越界**：`output_dir` 词法规范化 + `starts_with` 录制根校验，拒绝逃逸路径（如 `../../etc`）。
- **P2 认证加固**：JWT 增加 `token_version`/`iss`/`aud` 声明（migration 0007），改密后 `token_version +1` 使旧 token 立即失效（实测 401）；`verify_token_with_db` 统一校验中间件/WS/流代理。
- **P2 错误脱敏**：`internal_error`/`not_found_error` 不再回传原始错误链，客户端只收通用文案，原始错误进 `tracing` 日志。
- **P2 日志脱敏**：录制 URL、初始管理员密码、TraceLayer 请求 URI 中的 `?token=xxx` → `?token=***`。
- **P3 容器/传输**：容器 rootless 化；CORS 收敛 + 安全响应头；登录接口限流；WebSocket 连接加固。
- **P4 前端 XSS**：Markdown 渲染器增加 XSS 防护（HTML 转义 + URL scheme 过滤）。

### 🛠 重构
- 将录制准入逻辑（锁 + 检查 + 插入）提取为 `admit_recording` 方法，便于并发测试直接驱动。

### 🧪 测试
- 新增并发安全测试：锁串行化（20 频道并发 → 恰好 `max_concurrent` 个成功）、DB 索引去重、终态释放可重录等共 5 项。
- 新增频道来源筛选、输出模板变量渲染测试。

---

## [0.1.0] - 初始正式版

- IPTV M3U 频道管理与定时录制系统首个正式发布。
- 支持频道 CRUD、M3U 导入、cron 定时录制、UDP→HLS 转码、WebSocket 实时更新、通知中心、审计日志、应用内通知等。
