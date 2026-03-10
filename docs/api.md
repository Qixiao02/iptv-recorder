# IPTV Recorder API 文档

> 版本: 3.7
> 更新时间: 2026-02-22
> Base URL: `http://localhost:3000/api`

---

## 目录

1. [基础信息](#基础信息)
2. [频道管理 API](#频道管理-api)
3. [录制计划 API](#录制计划-api)
4. [录制任务 API](#录制任务-api)
5. [调度器 API](#调度器-api)
6. [系统配置 API](#系统配置-api)
7. [流代理 API](#流代理-api)
8. [转码 API](#转码-api)
9. [WebSocket API](#websocket-api)
10. [数据模型](#数据模型)
11. [错误码](#错误码)
12. [示例代码](#示例代码)

---

## 基础信息

### 请求格式

- **Content-Type**: `application/json`
- **字符编码**: `UTF-8`
- **ID 格式**: UUID v4

### 响应格式

**成功响应** (200/201):
```json
直接返回数据对象或数组
```

**错误响应** (4xx/5xx):
```json
{
  "error": "error_code",
  "details": "详细错误信息"
}
```

### HTTP 状态码

| 状态码 | 说明 |
|--------|------|
| 200 | 成功 |
| 201 | 创建成功 |
| 204 | 成功（无返回内容） |
| 400 | 请求参数错误 |
| 404 | 资源不存在 |
| 500 | 服务器内部错误 |

---

## 频道管理 API

### 获取频道列表

```http
GET /api/channels
GET /api/channels?page=1&page_size=20&group=央视&search=CCTV
```

**查询参数**:
| 参数 | 类型 | 必填 | 默认值 | 说明 |
|------|------|------|--------|------|
| page | number | ❌ | 1 | 页码 |
| page_size | number | ❌ | 20 | 每页数量 |
| group | string | ❌ | - | 按分组筛选 |
| search | string | ❌ | - | 搜索关键词 |

**响应**:
```json
{
  "items": [
    { "id": "uuid", "name": "CCTV-1", ... }
  ],
  "total": 150,
  "page": 1,
  "page_size": 20,
  "total_pages": 8
}
```

### 获取所有频道（不分页）

```http
GET /api/channels/all
```

**响应**: `Channel[]`

### 获取单个频道

```http
GET /api/channels/{id}
```

**路径参数**:
- `id` (string) - 频道 UUID

**响应**: `Channel`

### 创建频道

```http
POST /api/channels
Content-Type: application/json
```

**请求体**:
```json
{
  "name": "CCTV-1",
  "url": "http://example.com/stream.m3u8",
  "group_name": "央视",
  "logo_url": "http://example.com/logo.png"
}
```

| 字段 | 类型 | 必填 | 默认值 | 说明 |
|------|------|------|--------|------|
| name | string | ✅ | - | 频道名称 |
| url | string | ✅ | - | 直播源 URL |
| group_name | string | ❌ | "Uncategorized" | 分组名称 |
| logo_url | string | ❌ | null | Logo URL |

**响应**: `Channel`

### 更新频道

```http
PUT /api/channels/{id}
Content-Type: application/json
```

**路径参数**:
- `id` (string) - 频道 UUID

**请求体**: 同创建频道

**响应**: `Channel`

### 删除频道

```http
DELETE /api/channels/{id}
```

**路径参数**:
- `id` (string) - 频道 UUID

**响应**: 204 No Content

### 获取频道分组

```http
GET /api/channels/groups
```

**响应**: `string[]`

### 测试频道连接

```http
POST /api/channels/{id}/test
```

**路径参数**:
- `id` (string) - 频道 UUID

**响应**: `ChannelTestResult`

```json
{
  "channel_id": "uuid",
  "status": "online",
  "response_time_ms": 33,
  "error": null
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| channel_id | string | 频道 UUID |
| status | string | 状态: online/offline |
| response_time_ms | number\|null | 响应时间（毫秒） |
| error | string\|null | 错误信息（如果失败） |

### 从 URL 导入 M3U

```http
POST /api/channels/import/url
Content-Type: application/json
```

**请求体**:
```json
{
  "url": "http://example.com/playlist.m3u",
  "overwrite": false
}
```

| 字段 | 类型 | 必填 | 默认值 | 说明 |
|------|------|------|--------|------|
| url | string | ✅ | - | M3U 文件 URL |
| overwrite | boolean | ❌ | false | 是否覆盖已存在频道 |

**响应**:
```json
{
  "imported": 120,
  "skipped": 5,
  "failed": 2,
  "errors": ["频道 XXX 已存在", "频道 YYY URL 无效"]
}
```

### 从内容导入 M3U

```http
POST /api/channels/import/content
Content-Type: application/json
```

**请求体**:
```json
{
  "content": "#EXTM3U\n#EXTINF:-1,CCTV-1\nhttp://example.com/stream.m3u8\n...",
  "overwrite": false
}
```

| 字段 | 类型 | 必填 | 默认值 | 说明 |
|------|------|------|--------|------|
| content | string | ✅ | - | M3U 文件内容 |
| overwrite | boolean | ❌ | false | 是否覆盖已存在频道 |

**响应**: 同 URL 导入

---

## 录制计划 API

### 获取计划列表

```http
GET /api/schedules
```

**响应**: `Schedule[]`

### 获取单个计划

```http
GET /api/schedules/{id}
```

**路径参数**:
- `id` (string) - 计划 UUID

**响应**: `Schedule`

### 创建计划

```http
POST /api/schedules
Content-Type: application/json
```

**请求体**:
```json
{
  "name": "新闻联播",
  "channel_id": "channel-uuid",
  "cron_expression": "0 19 * * *",
  "duration_seconds": 3600,
  "output_template": "{channel_name}_{date}_{time}.mp4",
  "priority": 5
}
```

| 字段 | 类型 | 必填 | 默认值 | 说明 |
|------|------|------|--------|------|
| name | string | ✅ | - | 计划名称 |
| channel_id | string | ✅ | - | 频道 UUID |
| cron_expression | string | ✅ | - | Cron 表达式 (标准 5 字段) |
| duration_seconds | number | ✅ | - | 录制时长（秒） |
| output_template | string | ❌ | 见配置 | 输出文件名模板 |
| priority | number | ❌ | 5 | 优先级 (1-10) |

**Cron 表达式格式** (标准 5 字段):
```
分 时 日 月 周
* * * * *
│ │ │ │ │
│ │ │ │ └─ 周几 (0-6, 0=周日)
│ │ │ └─── 月份 (1-12)
│ │ └───── 日期 (1-31)
│ └─────── 小时 (0-23)
└───────── 分钟 (0-59)
```

**示例**:
- `0 19 * * *` - 每天 19:00
- `30 8 * * 1-5` - 工作日 08:30
- `0 */2 * * *` - 每 2 小时
- `0 20 * * 6,0` - 周六和周日 20:00

**响应**: `Schedule`

### 更新计划

```http
PUT /api/schedules/{id}
Content-Type: application/json
```

**路径参数**:
- `id` (string) - 计划 UUID

**请求体**: 同创建计划

**响应**: `Schedule`

### 删除计划

```http
DELETE /api/schedules/{id}
```

**路径参数**:
- `id` (string) - 计划 UUID

**响应**: 204 No Content

### 切换计划状态

```http
POST /api/schedules/{id}/toggle
```

**路径参数**:
- `id` (string) - 计划 UUID

**响应**: `Schedule` (更新后的计划)

---

## 录制任务 API

### 获取任务列表

```http
GET /api/tasks
```

**响应**: `Task[]`

### 获取单个任务

```http
GET /api/tasks/{id}
```

**路径参数**:
- `id` (string) - 任务 UUID

**响应**: `Task`

### 取消任务

```http
POST /api/tasks/{id}/cancel
```

**路径参数**:
- `id` (string) - 任务 UUID

**响应**: 204 No Content

### 手动录制

```http
POST /api/tasks/manual
Content-Type: application/json
```

**请求体**:
```json
{
  "channel_id": "channel-uuid",
  "duration_seconds": 3600,
  "output_name": "我的录制"
}
```

| 字段 | 类型 | 必填 | 默认值 | 说明 |
|------|------|------|--------|------|
| channel_id | string | ✅ | - | 频道 UUID |
| duration_seconds | number | ❌ | 3600 | 录制时长（秒） |
| output_name | string | ❌ | 自动生成 | 输出文件名（不含扩展名） |

**响应**: `Task`

---

## 调度器 API

### 获取即将执行的任务

```http
GET /api/scheduler/upcoming
```

**响应**: `UpcomingTask[]`

```json
[
  {
    "schedule_id": "uuid",
    "schedule_name": "新闻联播",
    "channel_id": "channel-uuid",
    "next_run": "2025-02-19T19:00:00+08:00",
    "duration_seconds": 3600
  }
]
```

### 重新加载调度器

```http
POST /api/scheduler/reload
```

重新加载所有启用的录制计划到调度器。

**响应**:
```json
{
  "status": "ok",
  "message": "调度器已重新加载"
}
```

---

## 系统配置 API

### 字段说明

| 模块 | 字段 | 可修改 | 说明 |
|------|------|--------|------|
| server | host, port | ❌ | 只读，从启动配置读取 |
| storage | recordings_path | ✅ | 录制文件保存目录 |
| storage | auto_cleanup_days | ✅ | 自动清理天数 (0=禁用) |
| storage | min_free_space_gb | ✅ | 最小剩余空间警告阈值 |
| recording | default_duration_minutes | ✅ | 手动录制默认时长 |
| recording | n_m3u8dl_re_path | ✅ | 下载工具路径 |
| recording | max_retry | ✅ | 录制失败最大重试次数 |
| recording | thread_count | ✅ | 并发下载线程数 |
| notification | on_complete | ✅ | 录制完成通知 |
| notification | on_failure | ✅ | 录制失败通知 |
| notification | disk_warning | ✅ | 磁盘空间警告 |

### 获取系统配置

```http
GET /api/config
```

**响应**: `SystemConfig`

```json
{
  "server": {
    "host": "0.0.0.0",
    "port": 3000
  },
  "storage": {
    "recordings_path": "./data/recordings",
    "auto_cleanup_days": 30,
    "min_free_space_gb": 10
  },
  "recording": {
    "default_duration_minutes": 60,
    "n_m3u8dl_re_path": "N_m3u8DL-RE",
    "max_retry": 3,
    "thread_count": 4
  },
  "notification": {
    "on_complete": true,
    "on_failure": true,
    "disk_warning": true
  }
}
```

### 更新系统配置

```http
POST /api/config
Content-Type: application/json
```

**请求体**: 支持部分更新

```json
{
  "storage": {
    "recordings_path": "E:/Recordings",
    "auto_cleanup_days": 60
  }
}
```

**响应**: `SystemConfig` (更新后的完整配置)

---

## 流代理 API

流代理用于绕过 CORS 限制，允许前端播放 HLS 流媒体。

### 代理流请求

```http
GET /api/proxy/stream?url={encoded_url}
```

**查询参数**:
- `url` (string, 必填) - URL 编码后的流地址

**说明**:
- 用于代理 HTTP 流媒体请求，解决浏览器的 CORS 限制
- 支持 HLS (.m3u8) 和其他 HTTP 流媒体格式
- **注意**: UDP 组播流 (`/udp/` 路径) 无法在浏览器中直接播放，需要使用外部播放器（如 VLC）

**响应头**:
```
Content-Type: <原始内容的 Content-Type>
Access-Control-Allow-Origin: *
Access-Control-Allow-Methods: GET, OPTIONS
Access-Control-Allow-Headers: *
```

**示例**:
```bash
# 代理 HLS 流
curl "http://localhost:3000/api/proxy/stream?url=http%3A%2F%2Fexample.com%2Fstream.m3u8"
```

**错误响应**:
- `500 Internal Server Error` - 请求目标流失败

---

## 转码 API

转码服务用于将 UDP 组播流实时转码为 HLS 格式，供浏览器播放。

### 启动转码

```http
POST /api/transcode/start
Content-Type: application/json
```

**请求体**:
```json
{
  "channel_id": "channel-uuid",
  "url": "http://192.168.0.211:4022/udp/239.77.1.17:5146"
}
```

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| channel_id | string | ✅ | 频道 UUID |
| url | string | ✅ | 流 URL |

**响应**:
```json
{
  "session_id": "uuid",
  "playlist_url": "/api/transcode/hls/{session_id}/stream.m3u8"
}
```

**说明**:
- 如果该频道已有活动的转码会话，将返回现有会话
- 转码使用 FFmpeg，需确保服务器已安装
- 输出配置：
  - 视频编码: H.264 (ultrafast preset, zerolatency tune)
  - 音频编码: AAC 128kbps
  - HLS 分片: 2秒
  - 播放列表: 保留最近 20 个分片
  - 关键帧间隔: 50 帧

### 停止转码

```http
POST /api/transcode/{session_id}
```

**路径参数**:
- `session_id` (string) - 转码会话 UUID

**响应**: 204 No Content

### 获取 HLS 文件

```http
GET /api/transcode/hls/{session_id}/{filename}
```

**路径参数**:
- `session_id` (string) - 转码会话 UUID
- `filename` (string) - 文件名（stream.m3u8 或 segment_XXX.ts）

**响应**:
- `.m3u8` 文件: `application/vnd.apple.mpegurl`
- `.ts` 文件: `video/mp2t`

**示例**:
```bash
# 获取播放列表
curl http://localhost:3000/api/transcode/hls/{session_id}/stream.m3u8

# 获取视频分片
curl http://localhost:3000/api/transcode/hls/{session_id}/segment_000.ts
```

---

## WebSocket API

### 连接

```http
WS /ws
```

### 消息格式

所有消息为 JSON 格式：

```json
{
  "type": "message_type",
  "data": { ... }
}
```

### 消息类型

#### task.update - 任务状态更新

```json
{
  "type": "task.update",
  "data": {
    "task_id": "uuid",
    "status": "running",
    "error_message": null
  }
}
```

#### task.progress - 任务进度更新

```json
{
  "type": "task.progress",
  "data": {
    "task_id": "uuid",
    "percent": 45,
    "downloaded_bytes": 52428800,
    "speed": "2.5MB/s",
    "eta_seconds": 300
  }
}
```

#### channel.status - 频道状态变更

```json
{
  "type": "channel.status",
  "data": {
    "channel_id": "uuid",
    "status": "offline"
  }
}
```

#### system.alert - 系统告警

```json
{
  "type": "system.alert",
  "data": {
    "level": "warning",
    "message": "磁盘空间不足",
    "details": "仅剩 2 GB 可用空间"
  }
}
```

**告警级别**: `info` | `warning` | `error` | `critical`

#### ping/pong - 心跳

客户端发送:
```json
{ "type": "ping" }
```

服务器响应:
```json
{
  "type": "pong",
  "data": {
    "timestamp": "2025-02-19T10:00:00Z"
  }
}
```

---

## 数据模型

### Channel

| 字段 | 类型 | 说明 |
|------|------|------|
| id | string | 频道 UUID |
| name | string | 频道名称 |
| url | string | 直播源 URL |
| group_name | string | 分组名称 |
| logo_url | string\|null | Logo URL |
| source_type | string | 来源类型 |
| source_url | string\|null | 来源 URL |
| status | string | 状态: online/offline/slow/unknown |
| last_check_at | string\|null | 最后检查时间 (ISO 8601) |
| fail_count | number | 失败次数 |
| metadata | object | 元数据 |
| created_at | string | 创建时间 (ISO 8601) |
| updated_at | string | 更新时间 (ISO 8601) |

### Schedule

| 字段 | 类型 | 说明 |
|------|------|------|
| id | string | 计划 UUID |
| name | string | 计划名称 |
| channel_id | string | 频道 UUID |
| cron_expression | string | Cron 表达式 |
| duration_seconds | number | 录制时长（秒） |
| output_template | string | 输出文件名模板 |
| priority | number | 优先级 (1-10) |
| enabled | boolean | 是否启用 |
| max_retry | number | 最大重试次数 |
| notify_on_complete | boolean | 完成时通知 |
| created_at | string | 创建时间 (ISO 8601) |
| updated_at | string | 更新时间 (ISO 8601) |

### Task

| 字段 | 类型 | 说明 |
|------|------|------|
| id | string | 任务 UUID |
| schedule_id | string\|null | 计划 UUID |
| channel_id | string | 频道 UUID |
| status | string | 状态: pending/running/completed/failed/cancelled |
| started_at | string\|null | 开始时间 (ISO 8601) |
| ended_at | string\|null | 结束时间 (ISO 8601) |
| exit_code | number\|null | 退出码 |
| error_message | string\|null | 错误信息 |
| output_path | string\|null | 输出文件路径 |
| file_size | number | 文件大小（字节） |
| duration_recorded | number | 已录制时长（秒） |
| progress_percent | number | 进度百分比 (0-100) |
| current_speed | string\|null | 当前下载速度 |
| created_at | string | 创建时间 (ISO 8601) |
| updated_at | string | 更新时间 (ISO 8601) |

### UpcomingTask

| 字段 | 类型 | 说明 |
|------|------|------|
| schedule_id | string | 计划 UUID |
| schedule_name | string | 计划名称 |
| channel_id | string | 频道 UUID |
| next_run | string | 下次运行时间 (ISO 8601) |
| duration_seconds | number | 录制时长（秒） |

### ChannelTestResult

| 字段 | 类型 | 说明 |
|------|------|------|
| channel_id | string | 频道 UUID |
| status | string | 状态: online/offline |
| response_time_ms | number\|null | 响应时间（毫秒） |
| error | string\|null | 错误信息 |

### SystemConfig

| 字段 | 类型 | 说明 |
|------|------|------|
| server.host | string | 服务器地址 |
| server.port | number | 服务器端口 |
| storage.recordings_path | string | 录制保存路径 |
| storage.auto_cleanup_days | number | 自动清理天数 |
| storage.min_free_space_gb | number | 最小可用空间 (GB) |
| recording.default_duration_minutes | number | 默认录制时长（分钟） |
| recording.n_m3u8dl_re_path | string | N_m3u8DL-RE 路径 |
| recording.max_retry | number | 最大重试次数 |
| recording.thread_count | number | 线程数 |
| notification.on_complete | boolean | 完成时通知 |
| notification.on_failure | boolean | 失败时通知 |
| notification.disk_warning | boolean | 磁盘空间警告 |

---

## 错误码

| 错误码 | 说明 |
|--------|------|
| `internal_error` | 服务器内部错误 |
| `not_found` | 资源不存在 |
| `invalid_params` | 请求参数无效 |
| `channel_offline` | 频道离线 |
| `concurrent_limit` | 并发限制 |
| `disk_full` | 磁盘空间不足 |
| `invalid_cron` | Cron 表达式无效 |
| `missing_url` | 缺少 URL 参数 |
| `missing_content` | 缺少内容参数 |

---

## 示例代码

### JavaScript / TypeScript

```typescript
const BASE_URL = 'http://localhost:3000/api';

// 类型定义
interface Channel {
  id: string;
  name: string;
  url: string;
  group_name: string;
  logo_url: string | null;
  status: string;
}

// 获取频道列表
const getChannels = async (): Promise<Channel[]> => {
  const res = await fetch(`${BASE_URL}/channels`);
  return res.json();
};

// 创建频道
const createChannel = async (data: {
  name: string;
  url: string;
  group_name?: string;
}): Promise<Channel> => {
  const res = await fetch(`${BASE_URL}/channels`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(data),
  });
  return res.json();
};

// WebSocket 连接
const ws = new WebSocket('ws://localhost:3000/ws');
ws.onmessage = (event) => {
  const message = JSON.parse(event.data);
  console.log(message.type, message.data);
};
```

### Python

```python
import requests

BASE_URL = "http://localhost:3000"

# 获取频道列表
channels = requests.get(f"{BASE_URL}/channels").json()

# 创建频道
channel = requests.post(f"{BASE_URL}/channels", json={
    "name": "CCTV-1",
    "url": "http://example.com/stream.m3u8",
    "group_name": "央视"
}).json()

# 导入 M3U
result = requests.post(f"{BASE_URL}/channels/import/url", json={
    "url": "http://example.com/playlist.m3u"
}).json()
print(f"导入: {result['imported']}, 跳过: {result['skipped']}")
```

### cURL

```bash
# 创建频道
curl -X POST http://localhost:3000/api/channels \
  -H "Content-Type: application/json" \
  -d '{"name":"CCTV-1","url":"http://example.com/stream.m3u8","group_name":"央视"}'

# 导入 M3U
curl -X POST http://localhost:3000/api/channels/import/url \
  -H "Content-Type: application/json" \
  -d '{"url":"http://example.com/playlist.m3u"}'

# 创建计划
curl -X POST http://localhost:3000/api/schedules \
  -H "Content-Type: application/json" \
  -d '{"name":"新闻联播","channel_id":"uuid","cron_expression":"0 19 * * *","duration_seconds":3600}'

# 手动录制
curl -X POST http://localhost:3000/api/tasks/manual \
  -H "Content-Type: application/json" \
  -d '{"channel_id":"uuid","duration_seconds":3600}'
```

---

*文档版本: 3.7*
*最后更新: 2026-02-22*
