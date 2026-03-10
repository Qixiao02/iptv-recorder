# IPTV Recorder 项目文档

## 文档目录

| 文档 | 描述 |
|------|------|
| [架构设计](./architecture.md) | 系统整体架构、模块划分、技术选型 |
| [API 文档](./api.md) | REST API 和 WebSocket 接口说明 |
| [数据库设计](./database.md) | 数据表结构、索引设计、迁移策略 |
| [配置说明](./configuration.md) | 配置项详解、环境变量、部署配置 |
| [开发指南](./development.md) | 开发环境搭建、代码规范、贡献指南 |
| [部署指南](./deployment.md) | 生产环境部署、Docker 部署、运维监控 |

## 快速开始

### 环境要求

- **Rust**: 1.75+
- **SQLite**: 3.0+ (内置)
- **N_m3u8DL-RE**: HLS 流录制工具
- **操作系统**: Windows / Linux / macOS

### 安装 N_m3u8DL-RE

```bash
# 下载最新版本
# Windows: 下载 N_m3u8DL-RE.exe 到项目目录
# Linux/macOS: 下载对应二进制文件

# 或使用环境变量指定路径
export IPTV__RECORDER__EXECUTABLE=/path/to/N_m3u8DL-RE
```

### 安装运行

```bash
# 克隆项目
cd E:/WrenPorject/iptv-recorder

# 编译并运行
cargo run

# 访问服务
# 浏览器打开: http://localhost:3000
```

### 首次启动

首次启动时，程序会自动：
1. 创建 `data/` 目录
2. 初始化 SQLite 数据库
3. 创建所有数据表和索引
4. 启动 Cron 调度器
5. 启动 Web 服务器

```
🚀 IPTV Recorder starting...
📝 Configuration loaded from: None
Created data directory: E:\WrenPorject\iptv-recorder\data
Connecting to database: E:\WrenPorject\iptv-recorder\data\iptv-recorder.db
Database migrations completed
🗄️  Database initialized: data/iptv-recorder.db
🎬 Process Manager initialized
📅 Cron Scheduler started
🌐 Web server listening on http://127.0.0.1:3000
```

## 功能特性

| 模块 | 状态 | 描述 |
|------|------|------|
| 频道管理 | ✅ 已完成 | 创建、查询、更新、删除频道 |
| M3U 解析 | ✅ 已完成 | 从 URL 或内容导入 M3U/M3U8 频道列表 |
| 定时调度 | ✅ 已完成 | 基于 Cron 表达式的定时录制计划 |
| 录制引擎 | ✅ 已完成 | 集成 N_m3u8DL-RE，支持手动/自动录制 |
| 任务管理 | ✅ 已完成 | 任务查询、取消任务、状态监控 |
| WebSocket | ✅ 基础完成 | WebSocket 连接（待扩展实时推送） |
| Web 界面 | ⏳ 待开发 | 管理界面 |

## 系统概览

IPTV Recorder 是一个基于 Rust 开发的 IPTV 录制管理系统，主要功能包括：

- **M3U 频道管理**: 导入、编辑、分组、健康检测
- **定时录制**: 基于 Cron 表达式的灵活调度
- **手动录制**: 立即启动录制任务
- **实时监控**: 任务状态跟踪、进程生命周期管理
- **资源控制**: 并发限制、磁盘监控、自动清理

## 技术栈

| 层级 | 技术 | 版本 |
|------|------|------|
| Web 框架 | Axum | 0.8 |
| 异步运行时 | Tokio | 1.42 |
| 数据库 | SQLite (sqlx) | 0.8 |
| 配置管理 | figment | 0.10 |
| 日志 | tracing | 0.1 |
| HTTP 客户端 | reqwest | 0.12 |
| 定时任务 | tokio-cron-scheduler | 0.12 |
| Cron 解析 | cron | 0.12 |
| 时区处理 | chrono-tz | 0.10 |

## 项目结构

```
iptv-recorder/
├── src/
│   ├── main.rs              # 程序入口
│   ├── config.rs            # 配置管理
│   ├── api/                 # API 层
│   │   ├── mod.rs           # 模块导出
│   │   ├── router.rs        # 路由定义
│   │   ├── handlers.rs      # HTTP 处理器
│   │   └── websocket.rs     # WebSocket 处理
│   ├── core/                # 核心基础设施
│   │   ├── mod.rs           # 模块导出
│   │   ├── database.rs      # 数据库初始化
│   │   ├── event.rs         # 事件总线
│   │   └── process.rs       # 进程管理（N_m3u8DL-RE）
│   ├── services/            # 业务服务层
│   │   ├── mod.rs           # 模块导出 + ServiceContext
│   │   ├── channel.rs       # 频道服务
│   │   ├── schedule.rs      # 计划服务
│   │   ├── recording.rs     # 录制服务
│   │   ├── m3u_parser.rs    # M3U/M3U8 解析器
│   │   └── scheduler.rs     # Cron 调度器管理
│   └── models/              # 数据模型
├── templates/               # Askama 模板
├── static/                  # 静态资源
├── config/                  # 配置文件
├── migrations/              # 数据库迁移
├── docs/                    # 项目文档
└── Cargo.toml               # 依赖配置
```

## API 快速测试

```bash
# 获取频道列表
curl http://localhost:3000/api/channels

# 创建频道
curl -X POST http://localhost:3000/api/channels \
  -H "Content-Type: application/json" \
  -d '{
    "name": "CCTV-1",
    "url": "http://example.com/stream.m3u8",
    "group_name": "央视"
  }'

# 从 URL 导入 M3U
curl -X POST http://localhost:3000/api/channels/import/url \
  -H "Content-Type: application/json" \
  -d '{
    "url": "http://example.com/playlist.m3u",
    "overwrite": false
  }'

# 创建录制计划
curl -X POST http://localhost:3000/api/schedules \
  -H "Content-Type: application/json" \
  -d '{
    "name": "新闻联播",
    "channel_id": "channel-uuid",
    "cron_expression": "0 19 * * *",
    "duration_seconds": 1800
  }'

# 手动录制
curl -X POST http://localhost:3000/api/tasks/manual \
  -H "Content-Type: application/json" \
  -d '{
    "channel_id": "channel-uuid",
    "duration_seconds": 3600
  }'

# 查询任务状态
curl http://localhost:3000/api/tasks

# 取消任务
curl -X POST http://localhost:3000/api/tasks/{task_id}/cancel

# 获取即将执行的任务
curl http://localhost:3000/api/scheduler/upcoming
```

## 配置文件示例

创建 `config.toml`:

```toml
[server]
host = "0.0.0.0"
port = 3000
workers = 4

[database]
path = "data/iptv-recorder.db"
pool_size = 10

[storage]
recordings_dir = "data/recordings"
temp_dir = "data/.tmp"
min_free_space_mb = 1024

[recorder]
executable = "N_m3u8DL-RE"
max_concurrent = 5
task_timeout_secs = 7200

[scheduler]
timezone = "Asia/Shanghai"
task_retention_days = 30
```

## 常见问题

### 端口被占用

修改配置文件或设置环境变量：

```bash
# Windows
set IPTV__SERVER__PORT=8080

# Linux/macOS
export IPTV__SERVER__PORT=8080
```

### 数据库文件位置

默认位置：`data/iptv-recorder.db`（相对于运行目录）

可通过配置修改：

```toml
[database]
path = "/custom/path/to/database.db"
```

### 录制文件位置

默认位置：`data/recordings/`（相对于运行目录）

文件名格式：`{频道名}_{日期}_{时间}.mp4`

### 查看日志

```bash
# 调试模式
RUST_LOG=debug cargo run

# JSON 格式日志
RUST_FORMAT=json cargo run
```

## 实现细节

### M3U 解析器

支持从 URL 或直接内容解析 M3U/M3U8 文件：
- 解析 `#EXTINF` 条目
- 提取 `tvg-id`, `tvg-logo`, `group-title` 属性
- 自动去重和分组

### Cron 调度器

- 基于 `tokio-cron-scheduler` 实现
- 支持标准 Cron 表达式（5 段式）
- 支持时区配置
- 启动时自动加载启用的计划

### 录制引擎

- 集成 N_m3u8DL-RE 作为录制工具
- 进程生命周期管理
- 任务状态实时跟踪
- 支持自定义 User-Agent、代理、线程数

## 已知问题

| 问题 | 状态 | 解决方案 |
|------|------|---------|
| Windows 路径问题 | ✅ 已修复 | 使用 `SqliteConnectOptions` |
| Axum 0.8 路由语法 | ✅ 已修复 | 使用 `{id}` 替代 `:id` |
| 任务取消功能 | ⏳ 待完善 | 需要添加任务跟踪机制 |
| WebSocket 实时推送 | ⏳ 待完善 | 需要集成事件总线 |

## 获取帮助

- **GitHub Issues**: 报告 Bug 或功能请求
- **文档**: 查看 `/docs` 目录下的详细文档

## 依赖的外部工具

- **N_m3u8DL-RE**: HLS 流下载工具
  - 下载: https://github.com/nilaoda/N_m3u8DL-RE/releases
  - 功能: 支持 HLS/DASH 直播流录制

## 开发路线图

- [x] M3U 文件解析与导入
- [x] Cron 调度器集成
- [x] N_m3u8DL-RE 进程管理
- [x] 手动录制功能
- [ ] 频道健康检测（HEAD 请求）
- [ ] 录制进度实时 WebSocket 推送
- [ ] Web 管理界面（HTMX + Alpine.js）
- [ ] EPG 节目单支持
- [ ] 录制文件自动清理
- [ ] 用户认证与权限

## 许可证

MIT License
