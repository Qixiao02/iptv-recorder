# 更新日志

本文件记录 IPTV Recorder 的所有显著变更。格式参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

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
