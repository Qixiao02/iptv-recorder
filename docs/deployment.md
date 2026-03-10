# 部署指南

## 系统要求

### 最低要求

| 资源 | 最低配置 |
|------|---------|
| CPU | 2 核 |
| 内存 | 512MB |
| 磁盘 | 10GB 可用空间 |
| 系统 | Linux/macOS/Windows |

### 推荐配置

| 资源 | 推荐配置 |
|------|---------|
| CPU | 4 核+ |
| 内存 | 2GB+ |
| 磁盘 | 100GB+ (视录制需求) |
| 系统 | Ubuntu 22.04 LTS / Debian 12 |

## 依赖工具

### N_m3u8DL-RE

录制核心工具，必须安装：

```bash
# Linux/macOS
wget https://github.com/nilaoda/N_m3u8DL-RE/releases/latest/download/N_m3u8DL-RE_Linux_64bit.tar.gz
tar -xzf N_m3u8DL-RE_Linux_64bit.tar.gz
chmod +x N_m3u8DL-RE
sudo mv N_m3u8DL-RE /usr/local/bin/

# 验证
N_m3u8DL-RE --version
```

### FFmpeg (可选)

用于转码和切片：

```bash
# Ubuntu/Debian
sudo apt install ffmpeg

# CentOS/RHEL
sudo yum install ffmpeg

# macOS
brew install ffmpeg
```

## 编译部署

### 1. 交叉编译

在开发机器上为目标平台编译：

```bash
# Linux x86_64
cargo build --release --target x86_64-unknown-linux-gnu

# macOS ARM64
cargo build --release --target aarch64-apple-darwin

# Windows x86_64
cargo build --release --target x86_64-pc-windows-msvc
```

### 2. 本地编译

在目标机器上直接编译：

```bash
# 安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# 克隆项目
git clone https://github.com/yourusername/iptv-recorder.git
cd iptv-recorder

# 编译
cargo build --release

# 二进制文件位置
ls -lh target/release/iptv-recorder
```

### 3. 目录结构

```bash
/opt/iptv-recorder/
├── bin/
│   └── iptv-recorder          # 主程序
├── config/
│   └── config.toml            # 配置文件
├── data/
│   ├── iptv-recorder.db       # 数据库
│   ├── recordings/            # 录制文件
│   └── .tmp/                  # 临时文件
├── logs/                      # 日志目录
└── scripts/                   # 管理脚本
```

## 系统服务配置

### systemd (Linux)

创建 `/etc/systemd/system/iptv-recorder.service`：

```ini
[Unit]
Description=IPTV Recorder Service
After=network.target

[Service]
Type=simple
User=iptv
Group=iptv
WorkingDirectory=/opt/iptv-recorder
Environment="RUST_LOG=info"
ExecStart=/opt/iptv-recorder/bin/iptv-recorder
Restart=on-failure
RestartSec=5

# 资源限制
LimitNOFILE=65536
MemoryMax=2G

# 安全加固
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ReadWritePaths=/opt/iptv-recorder/data

[Install]
WantedBy=multi-user.target
```

管理服务：

```bash
# 重载配置
sudo systemctl daemon-reload

# 启动服务
sudo systemctl start iptv-recorder

# 开机自启
sudo systemctl enable iptv-recorder

# 查看状态
sudo systemctl status iptv-recorder

# 查看日志
sudo journalctl -u iptv-recorder -f
```

### OpenRC (Gentoo/Alpine)

创建 `/etc/init.d/iptv-recorder`：

```bash
#!/sbin/openrc-run

name="iptv-recorder"
description="IPTV Recorder Service"
command="/opt/iptv-recorder/bin/iptv-recorder"
command_args=""
command_background=true
pidfile="/run/${RC_SVCNAME}.pid"
output_log="/var/log/iptv-recorder.log"
error_log="/var/log/iptv-recorder.err"

depend() {
    need net
    after firewall
}
```

### Windows 服务

使用 NSSM (Non-Sucking Service Manager)：

```cmd
REM 下载 nssm
https://nssm.cc/download

REM 安装服务
nssm install IPTVRecorder C:\iptv-recorder\iptv-recorder.exe
nssm set IPTVRecorder AppDirectory C:\iptv-recorder
nssm set IPTVRecorder Environment RUST_LOG=info

REM 启动服务
nssm start IPTVRecorder
```

## Docker 部署

### Dockerfile

```dockerfile
# 构建阶段
FROM rust:1.75-alpine AS builder

WORKDIR /build
RUN apk add --no-cache musl-dev sqlite-dev

COPY . .
RUN cargo build --release

# 运行阶段
FROM alpine:3.19

RUN apk add --no-cache sqlite ca-certificates

WORKDIR /app

COPY --from=builder /build/target/release/iptv-recorder /app/
COPY config/default.toml /app/config/

EXPOSE 3000

ENV IPTV__SERVER__HOST=0.0.0.0
ENV IPTV__SERVER__PORT=3000

CMD ["./iptv-recorder"]
```

### Docker Compose

```yaml
version: '3.8'

services:
  iptv-recorder:
    build: .
    container_name: iptv-recorder
    restart: unless-stopped
    ports:
      - "3000:3000"
    environment:
      - RUST_LOG=info
      - IPTV__RECORDER__MAX_CONCURRENT=5
    volumes:
      - ./data:/app/data
      - ./config:/app/config
      - ./recordings:/app/data/recordings
    healthcheck:
      test: ["CMD", "wget", "-q", "--spider", "http://localhost:3000/api/health"]
      interval: 30s
      timeout: 10s
      retries: 3
```

### 运行命令

```bash
# 构建镜像
docker build -t iptv-recorder:latest .

# 运行容器
docker run -d \
  --name iptv-recorder \
  -p 3000:3000 \
  -v $(pwd)/data:/app/data \
  -v $(pwd)/config:/app/config \
  -e IPTV__RECORDER__MAX_CONCURRENT=10 \
  iptv-recorder:latest

# 使用 Docker Compose
docker-compose up -d

# 查看日志
docker logs -f iptv-recorder
```

## Nginx 反向代理

### 配置示例

```nginx
# /etc/nginx/conf.d/iptv-recorder.conf

server {
    listen 80;
    server_name iptv.example.com;

    # 请求体大小限制（上传 M3U 文件）
    client_max_body_size 10M;

    # 代理 API
    location / {
        proxy_pass http://127.0.0.1:3000;
        proxy_http_version 1.1;

        # WebSocket 支持
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";

        # 通用代理头
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        # 超时设置
        proxy_connect_timeout 60s;
        proxy_send_timeout 60s;
        proxy_read_timeout 300s;
    }

    # 静态文件缓存
    location /static/ {
        proxy_pass http://127.0.0.1:3000;
        expires 7d;
        add_header Cache-Control "public, immutable";
    }

    # 录制文件直接访问
    location /recordings/ {
        alias /opt/iptv-recorder/data/recordings/;
        autoindex off;

        # 视频流支持
        types {
            video/mp4 mp4;
            video/mpeg ts;
            application/x-mpegURL m3u8;
        }
        add_header Cache-Control "public";
    }
}
```

### HTTPS 配置 (Let's Encrypt)

```bash
# 安装 certbot
sudo apt install certbot python3-certbot-nginx

# 获取证书
sudo certbot --nginx -d iptv.example.com

# 自动续期
sudo certbot renew --dry-run
```

## 监控与日志

### 日志配置

使用 `tracing-appender` 实现日志轮转：

```toml
# config.toml 中配置
[logging]
# 日志目录
dir = "/var/log/iptv-recorder"
# 日志级别
level = "info"
# 日志格式
format = "json"  # 或 "pretty"
# 日志轮转
rotating = true
max_files = 7  # 保留 7 天
```

### Prometheus 指标

暴露指标端点 `/metrics`：

```rust
// 暂未实现，规划中
// iptv_channels_total{status="online"} 156
// iptv_recordings_active 5
// iptv_recordings_completed_total 1024
```

### 健康检查

```bash
# HTTP 端点
curl http://localhost:3000/health

# 响应
{"status": "ok", "timestamp": "2024-02-18T12:00:00Z"}
```

## 备份策略

### 自动备份脚本

```bash
#!/bin/bash
# /opt/iptv-recorder/scripts/backup.sh

BACKUP_DIR="/backup/iptv-recorder"
DATE=$(date +%Y%m%d-%H%M%S)

mkdir -p "$BACKUP_DIR"

# 备份数据库
cp /opt/iptv-recorder/data/iptv-recorder.db \
   "$BACKUP_DIR/iptv-recorder-$DATE.db"

# 压缩录制文件（可选）
tar -czf "$BACKUP_DIR/recordings-$DATE.tar.gz" \
    -C /opt/iptv-recorder/data/ recordings/

# 清理 7 天前的备份
find "$BACKUP_DIR" -mtime +7 -delete

echo "Backup completed: $DATE"
```

### 定时任务

```bash
# crontab -e
# 每天凌晨 3 点执行备份
0 3 * * * /opt/iptv-recorder/scripts/backup.sh >> /var/log/backup.log 2>&1
```

## 故障排除

### 常见问题

| 问题 | 原因 | 解决方案 |
|------|------|---------|
| 服务无法启动 | 端口被占用 | `netstat -tlnp \| grep 3000` |
| 录制失败 | N_m3u8DL-RE 路径错误 | 检查配置中 executable 路径 |
| 磁盘占满 | 未自动清理 | 检查 task_retention_days 配置 |
| 内存占用高 | 并发任务过多 | 降低 max_concurrent 值 |

### 日志分析

```bash
# 查看最近错误
journalctl -u iptv-recorder --since "1 hour ago" | grep -i error

# 查看录制相关日志
journalctl -u iptv-recorder -f | grep -i recording
```

### 性能分析

```bash
# CPU 使用
top -p $(pidof iptv-recorder)

# 内存使用
pmap $(pidof iptv-recorder)

# 网络连接
ss -tnp | grep iptv-recorder
```

## 安全加固

### 防火墙

```bash
# UFW
sudo ufw allow 3000/tcp
sudo ufw enable

# firewalld
sudo firewall-cmd --permanent --add-port=3000/tcp
sudo firewall-cmd --reload
```

### 文件权限

```bash
# 限制配置文件权限
chmod 600 /opt/iptv-recorder/config/config.toml

# 数据目录权限
chown -R iptv:iptv /opt/iptv-recorder/data
chmod 750 /opt/iptv-recorder/data
```

## 升级指南

### 滚动升级

```bash
# 1. 备份数据
./scripts/backup.sh

# 2. 停止服务
sudo systemctl stop iptv-recorder

# 3. 下载新版本
wget https://github.com/xxx/iptv-recorder/releases/latest/download/iptv-recorder

# 4. 替换二进制
mv iptv-recorder /opt/iptv-recorder/bin/
chmod +x /opt/iptv-recorder/bin/iptv-recorder

# 5. 数据库迁移（如需要）
./iptv-recorder --migrate

# 6. 启动服务
sudo systemctl start iptv-recorder
```

### 数据库迁移

```bash
# 运行迁移
./iptv-recorder migrate

# 查看迁移状态
./iptv-recorder migrate status
```
