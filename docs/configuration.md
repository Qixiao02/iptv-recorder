# 配置说明文档

## 配置优先级

配置按以下优先级加载（高优先级覆盖低优先级）：

```
默认值 → 配置文件 → 环境变量
```

## 配置文件

### 位置

配置文件按以下顺序查找（使用第一个找到的）：

1. `./config.toml` - 当前目录
2. `./config.yaml` - 当前目录
3. `~/.iptv-recorder/config.toml` - 用户目录
4. `~/.iptv-recorder/config.yaml` - 用户目录
5. `/etc/iptv-recorder/config.toml` - 系统目录
6. `/etc/iptv-recorder/config.yaml` - 系统目录

### TOML 格式

```toml
# config/config.toml

[server]
# 服务器监听地址
host = "127.0.0.1"
# 服务器监听端口
port = 3000
# Worker 线程数 (0 = CPU 核心数)
workers = 4

[database]
# SQLite 数据库文件路径
path = "data/iptv-recorder.db"
# 连接池最大连接数
pool_size = 10

[storage]
# 录制文件存储目录
recordings_dir = "data/recordings"
# 临时文件目录
temp_dir = "data/.tmp"
# 最小剩余空间 (MB)，低于此值暂停新任务
min_free_space_mb = 1024

[recorder]
# N_m3u8DL-RE 可执行文件路径
# Windows: "N_m3u8DL-RE.exe" 或完整路径
# Linux: "/usr/local/bin/N_m3u8DL-RE"
executable = "N_m3u8DL-RE"
# 全局最大并发录制数
max_concurrent = 5
# 单任务超时时间 (秒)
task_timeout_secs = 7200

[scheduler]
# 默认时区
timezone = "Asia/Shanghai"
# 任务记录保留天数
task_retention_days = 30

# [notification]
# Webhook URL (录制完成通知)
# webhook_url = "http://your-server.com/webhook"
```

### YAML 格式

```yaml
# config/config.yaml

server:
  host: "127.0.0.1"
  port: 3000
  workers: 4

database:
  path: "data/iptv-recorder.db"
  pool_size: 10

storage:
  recordings_dir: "data/recordings"
  temp_dir: "data/.tmp"
  min_free_space_mb: 1024

recorder:
  executable: "N_m3u8DL-RE"
  max_concurrent: 5
  task_timeout_secs: 7200

scheduler:
  timezone: "Asia/Shanghai"
  task_retention_days: 30
```

## 环境变量

### 命名规则

环境变量使用 `IPTV__` 前缀，双下划线表示嵌套：

```
IPTV__SECTION__KEY=value
```

### 完整列表

| 环境变量 | 配置项 | 示例 |
|---------|-------|------|
| `IPTV__SERVER__HOST` | server.host | `0.0.0.0` |
| `IPTV__SERVER__PORT` | server.port | `8080` |
| `IPTV__SERVER__WORKERS` | server.workers | `4` |
| `IPTV__DATABASE__PATH` | database.path | `/data/db.sqlite` |
| `IPTV__DATABASE__POOL_SIZE` | database.pool_size | `20` |
| `IPTV__STORAGE__RECORDINGS_DIR` | storage.recordings_dir | `/data/recordings` |
| `IPTV__STORAGE__TEMP_DIR` | storage.temp_dir | `/data/tmp` |
| `IPTV__STORAGE__MIN_FREE_SPACE_MB` | storage.min_free_space_mb | `2048` |
| `IPTV__RECORDER__EXECUTABLE` | recorder.executable | `/usr/bin/N_m3u8DL-RE` |
| `IPTV__RECORDER__MAX_CONCURRENT` | recorder.max_concurrent | `10` |
| `IPTV__RECORDER__TASK_TIMEOUT_SECS` | recorder.task_timeout_secs | `3600` |
| `IPTV__SCHEDULER__TIMEZONE` | scheduler.timezone | `Asia/Shanghai` |
| `IPTV__SCHEDULER__TASK_RETENTION_DAYS` | scheduler.task_retention_days | `7` |

### 使用示例

```bash
# Linux/macOS
export IPTV__SERVER__PORT=8080
export IPTV__RECORDER__MAX_CONCURRENT=10

# Windows (CMD)
set IPTV__SERVER__PORT=8080
set IPTV__RECORDER__MAX_CONCURRENT=10

# Windows (PowerShell)
$env:IPTV__SERVER__PORT=8080
$env:IPTV__RECORDER__MAX_CONCURRENT=10
```

## 配置项详解

### [server] 服务器配置

| 配置项 | 类型 | 默认值 | 说明 |
|-------|------|-------|------|
| host | string | `127.0.0.1` | 监听地址，`0.0.0.0` 监听所有接口 |
| port | integer | `3000` | 监听端口 (1-65535) |
| workers | integer | `4` | Worker 线程数，`0` 表示 CPU 核心数 |

**端口说明**:
- 确保端口未被占用
- 1024 以下端口需要 root 权限

### [database] 数据库配置

| 配置项 | 类型 | 默认值 | 说明 |
|-------|------|-------|------|
| path | string | `data/iptv-recorder.db` | SQLite 数据库文件路径 |
| pool_size | integer | `10` | 连接池最大连接数 |

**路径说明**:
- 相对路径相对于程序运行目录
- 自动创建父目录
- 支持绝对路径

### [storage] 存储配置

| 配置项 | 类型 | 默认值 | 说明 |
|-------|------|-------|------|
| recordings_dir | string | `data/recordings` | 录制文件存储目录 |
| temp_dir | string | `data/.tmp` | 临时文件目录 |
| min_free_space_mb | integer | `1024` | 最小剩余空间 (MB) |

**空间说明**:
- 低于最小值时暂停新任务
- 自动检测磁盘空间
- 建议设置为录制文件大小的 2-3 倍

### [recorder] 录制器配置

| 配置项 | 类型 | 默认值 | 说明 |
|-------|------|-------|------|
| executable | string | `N_m3u8DL-RE` | N_m3u8DL-RE 可执行文件路径 |
| max_concurrent | integer | `5` | 全局最大并发录制数 |
| task_timeout_secs | integer | `7200` | 单任务超时时间（秒） |

**并发说明**:
- 建议值：CPU 核心数 × 1.5
- 同一频道的录制任务串行执行
- 不同频道的录制任务并行执行

**N_m3u8DL-RE 下载**:
- GitHub: https://github.com/nilaoda/N_m3u8DL-RE
- 放到系统 PATH 或配置完整路径

### [scheduler] 调度器配置

| 配置项 | 类型 | 默认值 | 说明 |
|-------|------|-------|------|
| timezone | string | `Asia/Shanghai` | 默认时区 |
| task_retention_days | integer | `30` | 任务记录保留天数 |

**时区支持**:
- 使用 IANA 时区标识
- 常见值: `Asia/Shanghai`, `Asia/Tokyo`, `America/New_York`
- 完整列表: https://en.wikipedia.org/wiki/List_of_tz_database_time_zones

## 常见配置场景

### 场景 1: 本地开发

```toml
[server]
host = "127.0.0.1"
port = 3000

[database]
path = "data/dev.db"

[storage]
recordings_dir = "data/dev/recordings"
```

### 场景 2: 家庭服务器

```toml
[server]
host = "0.0.0.0"
port = 8080

[storage]
recordings_dir = "/mnt/hdd/recordings"
min_free_space_mb = 10240  # 10GB

[recorder]
max_concurrent = 10
```

### 场景 3: 低配置设备

```toml
[server]
workers = 2

[recorder]
max_concurrent = 2
task_timeout_secs = 3600
```

### 场景 4: Docker 部署

```yaml
# docker-compose.yml
version: '3'
services:
  iptv-recorder:
    image: iptv-recorder:latest
    environment:
      - IPTV__SERVER__HOST=0.0.0.0
      - IPTV__SERVER__PORT=3000
      - IPTV__RECORDER__MAX_CONCURRENT=5
    volumes:
      - ./data:/app/data
      - ./config:/app/config
```

## 配置验证

### 检查配置

```bash
# 启动时自动验证
cargo run

# 输出示例
# 🚀 IPTV Recorder starting...
# 📝 Configuration loaded from: Some("config.toml")
# 🗄️  Database initialized: data/iptv-recorder.db
# 🌐 Web server listening on http://127.0.0.1:3000
```

### 常见错误

| 错误 | 原因 | 解决方案 |
|------|------|---------|
| `Address already in use` | 端口被占用 | 更换端口或关闭占用进程 |
| `Permission denied` | 无写权限 | 检查目录权限 |
| `Cannot find recorder` | 录制器路径错误 | 检查 N_m3u8DL-RE 路径 |
| `Disk full` | 磁盘空间不足 | 清理空间或增大 min_free_space_mb |

## 热重载

当前版本不支持热重载，修改配置后需要重启服务：

```bash
# 重启服务
systemctl restart iptv-recorder
# 或
kill -HUP $(pidof iptv-recorder)
```

未来版本将支持配置热重载。
