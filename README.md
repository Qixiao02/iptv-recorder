# 📺 IPTV Recorder

基于 Rust + React 的 IPTV M3U 管理与定时录制系统。支持频道导入管理、Cron 定时录制、实时转码预览、应用内通知中心，以及完整的 Web 管理界面。

![tech](https://img.shields.io/badge/backend-Rust%2FAxum-orange) ![tech](https://img.shields.io/badge/frontend-React%2019%2FVite-blue) ![tech](https://img.shields.io/badge/db-SQLite-green)

> 单文件 SQLite，零外部数据库依赖；双录制引擎（N_m3u8DL-RE + FFmpeg）自动选择；中/英双语界面。

---

## 目录

- [✨ 功能特性](#-功能特性)
- [🚀 快速开始](#-快速开始)
- [🐳 Docker 部署](#-docker-部署)
- [⚙️ 配置](#️-配置)
- [📡 API 概览](#-api-概览)
- [🏗️ 技术栈](#️-技术栈)
- [📂 项目结构](#-项目结构)
- [🔧 开发命令](#-开发命令)
- [📝 数据流](#-数据流)

---

## ✨ 功能特性

### 频道管理
- M3U / M3U8 播放单导入（URL 或文本），支持 `tvg-id`、`tvg-logo`、`group-title` 等扩展属性
- 频道分页、分组筛选、名称搜索
- 频道连通性测试
- **可搜索频道选择器**：新建计划时输入关键词模糊搜索，无需在数百个频道里滚动翻找

### 定时录制
- Cron 表达式调度，支持简单格式（如 `daily 19:00`）自动转换
- 时区感知（默认 `Asia/Shanghai`），避免 UTC 偏移导致的录制时间错位
- 每任务可配置：时长、视频/音频质量、并发线程、转码模式
- 全局并发上限保护，避免同时拉太多流压垮机器
- **「立即执行」按钮常驻卡片**：不用展开详情即可触发录制

### 录制引擎
- **N_m3u8DL-RE**（HLS / DASH 源）+ **FFmpeg**（UDP / RTMP / 原始流）双引擎自动选择
- 录制命令加固：显式 `-map` 选流（防止多 Program 流选台漂移）、`-reconnect` 自动重连、独立临时目录隔离（防止并发录制分片互相覆盖）
- 后处理转码：off / realtime / post 三种模式，支持质量预设

### 转码预览
- UDP 流实时转 HLS 供 Web 端播放（FFmpeg 多档 profile 自适应：FastRemux / StableFmp4 / CompatibleMpegTs）
- 预览 HLS 优先写入内存文件系统（`/dev/shm`），减少硬盘写入
- 同频道录制与预览互不干扰

### 通知中心
- **应用内持久化通知**：录制完成 / 失败、磁盘空间告警、系统消息
- WebSocket 实时推送，**前端通知列表自动更新**（无需手动刷新页面）
- 未读角标、分页历史、标记已读 / 删除
- 三个独立开关：完成通知、失败通知、磁盘告警

### 后台巡检（Heartbeat）
- **独立后台窗口**：每 10 分钟定时巡检磁盘剩余空间
- 低于阈值发警告通知，同级别 1 小时内去重（不刷屏）
- 与录制主流程解耦，不挤进录制输出

### 安全与审计
- JWT 认证 + 三级权限（admin / operator / viewer）
- 关键操作审计日志（分页查询，默认 20 条/页，可选 20/50/100）
- 流代理 URL 安全校验（拒绝 localhost / 非 http(s) scheme，防 SSRF）
- 初始管理员账号自动初始化

### 存储
- SQLite 单文件存储，零运维
- 录制路径**支持本地路径和网络路径**（UNC `\\server\share`、NFS/SMB 挂载点 `/mnt/nas`）
- Docker 下可挂载宿主机磁盘到容器，Web UI 直接浏览选择宿主路径
- 路径保存前**预校验**（创建目录 + 写权限验证），配错即时拦截
- 自动清理过期录制（按天数）+ 最小剩余空间保护

### 界面
- 中/英双语，深色 / 浅色主题切换
- 状态语义色：**录制中（橙）**、已完成（绿）、失败（红），主题对比度优化

---

## 🚀 快速开始

### 方式一：Docker（推荐）

```bash
git clone <repo-url> && cd iptv-recorder

# 首次部署:生成随机密钥与初始密码(写入 .env,已被 git 忽略)
bash scripts/generate-env.sh        # Linux / Git Bash
# 或: powershell -ExecutionPolicy Bypass -File scripts/generate-env.ps1   # Windows

docker compose up -d --build
```

- **后端 API**：`http://localhost:3033`（容器内 3000，宿主映射 3033）
- **默认账号**：`admin`（密码由生成脚本输出,或自行在 `.env` 设置 `IPTV_INITIAL_ADMIN_PASSWORD`;登录后请立即在「账户」页修改）
- ⚠️ **务必先运行生成脚本**:不生成 `.env` 则后端会因缺少 `IPTV_JWT_SECRET` 拒绝启动

> 生产构建已把前端打包进后端镜像，访问 `:3033` 即得完整界面。
> 如需前端热更新开发，见 [开发命令](#-开发命令)。

### 方式二：本地开发

```bash
# 终端 1 - 后端
cd backend && cargo run

# 终端 2 - 前端
cd frontend && pnpm install && pnpm dev
```

- 前端开发服务器：`http://localhost:5173`
- 自动代理 `/api` 和 `/ws` 到后端（默认 `127.0.0.1:3033`，可用 `VITE_BACKEND_URL` 覆盖）

---

## 🐳 Docker 部署

### 录制到网络路径 / 宿主机路径

容器默认只能看到挂载的卷。如需录制到网络路径（UNC / NFS / SMB）或宿主机其它盘符，需先把路径挂载进容器，再在 Web UI「设置 > 存储」里填写容器内路径：

```yaml
volumes:
  - ./backend/data:/app/data
  # Windows Docker Desktop（需在 Settings → File Sharing 启用盘符）：
  - /d:/mnt/host/d
  # Linux 宿主机：
  # - /:/mnt/host
  # 网络盘先在宿主机挂载，再映射进容器：
  # - /mnt/nas/recordings:/mnt/nas/recordings
```

挂载后，在 Web UI「设置 > 存储」点「选择目录」→「宿主机」按钮，从 `/mnt/host` 开始浏览即可选到宿主机路径。录制工具支持的网络路径：
- Windows：本地盘符、UNC 共享（`\\server\share`，自动探测可用空间）
- Linux：挂载点（`/mnt/nas`、NFS、SMB）

### 前端开发容器（源码外挂 + HMR）

```bash
cd frontend
docker compose -f docker-compose.dev.yml up -d
# 访问 http://localhost:5173
```

容器内使用容器自己的 `node_modules`（匿名 volume 隔离，避免 Windows 二进制不兼容），源码通过 bind mount 实时热更新。

---

## ⚙️ 配置

配置优先级：**环境变量 > 配置文件 > 默认值**。

- **本地开发**：复制 `backend/config/default.toml` 为 `backend/config/config.toml` 自定义
- **Docker 部署**：用环境变量（见下表）

环境变量前缀 `IPTV__`，双下划线 `__` 表示嵌套：

| 环境变量 | 说明 | 默认值 |
|---------|------|-------|
| `IPTV__SERVER__HOST` | 监听地址 | `127.0.0.1` |
| `IPTV__SERVER__PORT` | 监听端口 | `3000` |
| `IPTV__DATABASE__PATH` | SQLite 路径 | `data/iptv-recorder.db` |
| `IPTV__STORAGE__RECORDINGS_DIR` | 录制保存目录 | `data/recordings` |
| `IPTV__STORAGE__TEMP_DIR` | 临时文件目录 | `data/.tmp` |
| `IPTV__STORAGE__PREVIEW_TEMP_DIR` | 预览 HLS 临时目录 | 自动（`/dev/shm`） |
| `IPTV__RECORDER__EXECUTABLE` | N_m3u8DL-RE 路径 | `N_m3u8DL-RE` |
| `IPTV__RECORDER__MAX_CONCURRENT` | 最大并发录制数 | `5` |
| `IPTV__SCHEDULER__TIMEZONE` | 调度时区 | `Asia/Shanghai` |
| `IPTV_JWT_SECRET` | JWT 签名密钥（**必填**,≥32 字符,用 `scripts/generate-env` 生成） | — |
| `IPTV_INITIAL_ADMIN_PASSWORD` | 初始管理员密码（不设则随机生成） | — |

> 录制保存路径、清理天数、最小剩余空间、通知开关等**运行时可调项**，存于数据库 `system_config` 表，通过 Web UI「设置」修改即可，无需重启。

---

## 📡 API 概览

Base URL：`http://localhost:3033/api`（除登录外需 `Authorization: Bearer <token>`）

### 鉴权
| 方法 | 路径 | 说明 |
|------|------|------|
| POST | `/api/auth/login` | 登录获取 token |
| GET | `/api/auth/me` | 当前用户信息 |
| POST | `/api/auth/password` | 修改密码 |

### 频道
| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/channels` | 分页列表（`?page=&page_size=&group=&search=`） |
| GET | `/api/channels/all` | 全部频道 |
| POST/PUT/DELETE | `/api/channels[/{id}]` | 频道 CRUD |
| POST | `/api/channels/import/url` | 从 URL 导入 M3U |
| POST | `/api/channels/import/content` | 从文本导入 M3U |
| POST | `/api/channels/{id}/test` | 测试连通性 |

### 计划与任务
| 方法 | 路径 | 说明 |
|------|------|------|
| GET/POST | `/api/schedules` | 录制计划 |
| POST | `/api/schedules/{id}/toggle` | 启用/禁用 |
| GET | `/api/tasks` | 录制任务列表 |
| POST | `/api/tasks/manual` | 立即录制 |
| POST | `/api/tasks/{id}/cancel` | 取消录制 |
| GET | `/api/scheduler/upcoming` | 即将执行的任务 |

### 通知中心
| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/notifications` | 分页通知列表 |
| GET | `/api/notifications/unread-count` | 未读数 |
| POST | `/api/notifications/read-all` | 全部已读 |
| POST | `/api/notifications/{id}/read` | 标记已读 |
| DELETE | `/api/notifications/{id}` | 删除通知 |

### 系统
| 方法 | 路径 | 说明 |
|------|------|------|
| GET/POST | `/api/config` | 读取/更新配置 |
| GET | `/api/system/health` | 系统健康 |
| GET | `/api/system/directories` | 目录浏览（选择器） |
| GET | `/api/audit/logs` | 审计日志（分页） |
| POST | `/api/system/cleanup/run` | 手动触发清理 |

### 实时
| 方法 | 路径 | 说明 |
|------|------|------|
| WS | `/ws` | WebSocket（任务进度、状态变更、通知推送） |

完整字段定义见 `backend/src/api/handlers.rs` 与 `backend/src/models/`。

---

## 🏗️ 技术栈

**后端**（`backend/`）
- Rust + Axum 0.8 + Tokio
- SQLx 0.8 + SQLite（带 migration）
- tokio-cron-scheduler 定时调度
- figment 分层配置
- tracing 日志 + JWT + bcrypt

**前端**（`frontend/`）
- React 19 + TypeScript + Vite 7
- TanStack Query 5（数据获取/缓存）
- Zustand（状态管理）
- React Router 7 + i18next（中/英双语，模块化命名空间）
- lucide-react 图标，纯 CSS 样式（无 UI 框架依赖）

**外部工具**
- [N_m3u8DL-RE](https://github.com/nilaoda/N_m3u8DL-RE) — HLS/DASH 下载
- [FFmpeg](https://ffmpeg.org/) — 转码 / UDP 流录制

---

## 📂 项目结构

```
iptv-recorder/
├── backend/
│   ├── config/default.toml        # 默认配置
│   ├── migrations/                # SQL 迁移(0001 schema ~ 0005 notifications)
│   └── src/
│       ├── api/                   # HTTP 路由 / handlers / WebSocket
│       ├── core/                  # 数据库 / 事件总线 / 进程管理
│       ├── models/                # 数据模型
│       ├── services/              # 业务逻辑
│       │   ├── channel.rs         # 频道 CRUD + M3U 导入
│       │   ├── recording.rs       # 录制执行 + 终态通知
│       │   ├── scheduler.rs       # Cron 调度
│       │   ├── transcode.rs       # UDP→HLS 转码预览
│       │   ├── notification.rs    # 应用内通知中心
│       │   ├── heartbeat.rs       # 后台巡检(磁盘空间)
│       │   └── cleanup.rs         # 自动清理
│       └── main.rs                # 入口:编排各服务启动
├── frontend/
│   └── src/
│       ├── api/                   # API 客户端
│       ├── components/            # 通用组件(Modal / 铃铛 / 计划弹窗)
│       ├── pages/                 # 页面(Dashboard / Channels / Schedules / Tasks / Settings)
│       ├── stores/                # Zustand 状态
│       └── i18n/                  # 模块化双语文案
├── docker-compose.yml             # 后端编排(含宿主机/网络路径挂载示例)
├── Dockerfile                     # 多阶段构建(builder + runtime)
└── frontend/docker-compose.dev.yml # 前端开发容器(HMR + 源码外挂)
```

---

## 🔧 开发命令

### 后端
```bash
cd backend
cargo run                  # 开发运行
cargo build --release      # 发布构建
cargo test                 # 运行测试
cargo clippy               # Lint 检查
cargo fmt                  # 格式化
```

### 前端
```bash
cd frontend
pnpm install               # 安装依赖
pnpm dev                   # 开发服务器(5173)
pnpm build                 # 生产构建
pnpm lint                  # ESLint
pnpm tsc --noEmit          # 类型检查
```

---

## 📝 数据流

```
定时录制:  Cron 触发 → 检查频道/并发 → 启动 N_m3u8DL-RE/FFmpeg → 监控进度
              → 终态写库 → 落库通知 + WebSocket 推送 → 前端实时刷新

实时更新:  事件总线(broadcast) → WebSocket 转发 → 前端 TanStack Query 自动失效刷新

磁盘巡检:  Heartbeat(每10分钟) → 探测录制目录空间 → 低于阈值 → 发警告通知
```

---

## 📄 License

私有项目。

---

## 🙏 致谢

- [N_m3u8DL-RE](https://github.com/nilaoda/N_m3u8DL-RE) — 强大的 HLS/DASH 下载工具
- [FFmpeg](https://ffmpeg.org/) — 多媒体处理瑞士军刀
