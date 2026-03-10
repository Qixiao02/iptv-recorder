# 数据库设计文档

## 概述

IPTV Recorder 使用 SQLite 作为存储引擎，具有以下优势：
- 无需独立数据库服务
- 单文件存储，便于备份和迁移
- 支持 WAL 模式，并发读写性能良好

## 数据表

### channels - 频道表

存储 IPTV 频道信息。

| 字段 | 类型 | 约束 | 说明 |
|------|------|------|------|
| id | TEXT | PRIMARY KEY | UUID v4 |
| name | TEXT | NOT NULL | 频道名称 |
| url | TEXT | NOT NULL | 频道流地址 |
| group_name | TEXT | DEFAULT 'Uncategorized' | 分组名称 |
| logo_url | TEXT | | Logo 地址 |
| source_type | TEXT | DEFAULT 'remote_url' | 源类型: remote_url/local_file |
| source_url | TEXT | | 源地址（M3U 文件 URL） |
| status | TEXT | DEFAULT 'unknown' | 状态: unknown/online/offline |
| last_check_at | TEXT | | 最后检测时间 (ISO 8601) |
| fail_count | INTEGER | DEFAULT 0 | 连续失败次数 |
| metadata | TEXT | DEFAULT '{}' | 扩展元数据 (JSON) |
| created_at | TEXT | NOT NULL | 创建时间 |
| updated_at | TEXT | NOT NULL | 更新时间 |

**索引**:
- `idx_channels_status` - 按状态查询
- `idx_channels_group` - 按分组查询（建议添加）

### schedules - 录制计划表

存储定时录制计划。

| 字段 | 类型 | 约束 | 说明 |
|------|------|------|------|
| id | TEXT | PRIMARY KEY | UUID v4 |
| name | TEXT | NOT NULL | 计划名称 |
| channel_id | TEXT | NOT NULL, FOREIGN KEY | 关联频道 ID |
| cron_expression | TEXT | NOT NULL | Cron 表达式 |
| duration_seconds | INTEGER | DEFAULT 3600 | 录制时长（秒） |
| output_template | TEXT | | 输出文件名模板 |
| priority | INTEGER | DEFAULT 5 | 优先级 (1-10) |
| enabled | INTEGER | DEFAULT 1 | 是否启用 (0/1) |
| max_retry | INTEGER | DEFAULT 3 | 最大重试次数 |
| notify_on_complete | INTEGER | DEFAULT 0 | 完成通知 (0/1) |
| created_at | TEXT | NOT NULL | 创建时间 |
| updated_at | TEXT | NOT NULL | 更新时间 |

**索引**:
- `idx_schedules_channel` - 按频道查询
- `idx_schedules_enabled` - 按启用状态查询

### tasks - 任务实例表

存储录制任务执行记录。

| 字段 | 类型 | 约束 | 说明 |
|------|------|------|------|
| id | TEXT | PRIMARY KEY | UUID v4 |
| schedule_id | TEXT | FOREIGN KEY | 关联计划 ID（可选） |
| channel_id | TEXT | NOT NULL, FOREIGN KEY | 关联频道 ID |
| status | TEXT | DEFAULT 'pending' | 任务状态 |
| started_at | TEXT | | 开始时间 |
| ended_at | TEXT | | 结束时间 |
| exit_code | INTEGER | | 进程退出码 |
| error_message | TEXT | | 错误信息 |
| output_path | TEXT | | 输出文件路径 |
| file_size | INTEGER | DEFAULT 0 | 文件大小（字节） |
| duration_recorded | INTEGER | DEFAULT 0 | 实际录制时长（秒） |
| progress_percent | INTEGER | DEFAULT 0 | 进度百分比 (0-100) |
| current_speed | TEXT | | 当前下载速度 |
| created_at | TEXT | NOT NULL | 创建时间 |
| updated_at | TEXT | NOT NULL | 更新时间 |

**状态值**: `pending` | `running` | `completed` | `failed` | `cancelled`

**索引**:
- `idx_tasks_status` - 按状态查询
- `idx_tasks_channel` - 按频道查询
- `idx_tasks_started` - 按开始时间查询
- `idx_tasks_schedule` - 按计划查询（建议添加）

### recordings - 录制进程表

存储当前运行的录制进程信息（运行时数据）。

| 字段 | 类型 | 约束 | 说明 |
|------|------|------|------|
| id | TEXT | PRIMARY KEY | UUID v4 |
| task_id | TEXT | NOT NULL, FOREIGN KEY | 关联任务 ID |
| pid | INTEGER | | 进程 ID |
| started_at | TEXT | NOT NULL | 启动时间 |
| temp_path | TEXT | | 临时文件路径 |
| log_path | TEXT | | 日志文件路径 |
| last_progress_at | TEXT | | 最后进度更新时间 |
| is_healthy | INTEGER | DEFAULT 1 | 进程健康状态 (0/1) |

## ER 图

```
┌─────────────┐         ┌─────────────┐         ┌─────────────┐
│  channels   │1       *│  schedules  │1       *│    tasks    │
│─────────────│─────────│─────────────│─────────│─────────────│
│ id          │         │ id          │         │ id          │
│ name        │         │ name        │         │ status      │
│ url         │         │ channel_id  │         │ started_at  │
│ group_name  │         │ cron_expr   │         │ ended_at    │
│ status      │         │ enabled     │         │ output_path │
└─────────────┘         └─────────────┘         └─────────────┘
                                                       │1
                                                       │
                                                       │*
                                               ┌─────────────┐
                                               │ recordings  │
                                               │─────────────│
                                               │ id          │
                                               │ task_id     │
                                               │ pid         │
                                               │ is_healthy  │
                                               └─────────────┘
```

## 迁移策略

### 初始化脚本

数据库表结构通过代码中的 SQL 初始化，位于 `src/core/database.rs`：

```rust
sqlx::query(
    r#"
    CREATE TABLE IF NOT EXISTS channels (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        ...
    );
    "#
)
.execute(pool)
.await?;
```

### 版本控制

当前版本使用硬编码 SQL 创建表。未来建议：

1. 引入 `sqlx-cli` 管理迁移：
```bash
cargo install sqlx-cli
sqlx migrate create create_channels_table
```

2. 迁移文件结构：
```
migrations/
├── 001_initial_schema.sql
├── 002_add_epg_table.sql
├── 003_add_recording_index.sql
└── ...
```

### 升级步骤

1. 备份现有数据库：
```bash
cp data/iptv-recorder.db data/backup-$(date +%Y%m%d).db
```

2. 运行迁移：
```bash
sqlx migrate run --database-url sqlite://data/iptv-recorder.db
```

## 性能优化

### 索引建议

```sql
-- 建议添加的索引
CREATE INDEX IF NOT EXISTS idx_channels_group ON channels(group_name);
CREATE INDEX IF NOT EXISTS idx_tasks_schedule ON tasks(schedule_id);
CREATE INDEX IF NOT EXISTS idx_tasks_created ON tasks(created_at DESC);
```

### 分表策略

当 `tasks` 表记录超过 100 万时，考虑按月分表：

```sql
-- 示例：2024 年 2 月的任务表
CREATE TABLE tasks_202402 AS SELECT * FROM tasks WHERE ...;
```

### 数据清理

定期清理旧任务记录：

```sql
-- 删除 30 天前已完成的任务
DELETE FROM tasks
WHERE status IN ('completed', 'cancelled', 'failed')
AND datetime(ended_at) < datetime('now', '-30 days');
```

## 备份策略

### 热备份（WAL 模式）

SQLite 在 WAL 模式下支持在线备份：

```bash
# 检查是否启用 WAL
PRAGMA journal_mode;

# 启用 WAL
PRAGMA journal_mode = WAL;
```

### 备份脚本

```bash
#!/bin/bash
BACKUP_DIR="/backup/iptv-recorder"
DATE=$(date +%Y%m%d)

# 创建备份目录
mkdir -p $BACKUP_DIR

# 备份数据库
cp data/iptv-recorder.db $BACKUP_DIR/iptv-recorder-$DATE.db

# 备份录制文件
tar -czf $BACKUP_DIR/recordings-$DATE.tar.gz data/recordings/

# 清理 7 天前的备份
find $BACKUP_DIR -mtime +7 -delete
```

### 恢复

```bash
# 停止服务
systemctl stop iptv-recorder

# 恢复数据库
cp /backup/iptv-recorder-20240218.db data/iptv-recorder.db

# 启动服务
systemctl start iptv-recorder
```

## 查询示例

### 常用查询

```sql
-- 获取所有在线频道
SELECT * FROM channels WHERE status = 'online';

-- 获取启用的录制计划
SELECT s.*, c.name as channel_name
FROM schedules s
JOIN channels c ON s.channel_id = c.id
WHERE s.enabled = 1;

-- 获取运行中的任务
SELECT t.*, c.name as channel_name
FROM tasks t
JOIN channels c ON t.channel_id = c.id
WHERE t.status = 'running';

-- 获取频道录制历史
SELECT * FROM tasks
WHERE channel_id = ?
ORDER BY started_at DESC
LIMIT 20;

-- 统计任务成功率
SELECT
    status,
    COUNT(*) as count
FROM tasks
WHERE created_at > datetime('now', '-7 days')
GROUP BY status;
```

### 性能分析

```sql
-- 查看表大小
SELECT
    name,
    (pgsize * 1024) as size_bytes
FROM sqlite_master
JOIN dbstat ON name = dbstat.name
GROUP BY name;

-- 查看索引使用情况
EXPLAIN QUERY PLAN
SELECT * FROM tasks WHERE status = 'running';
```

## 数据一致性

### 外键约束

```sql
-- 启用外键约束（默认关闭）
PRAGMA foreign_keys = ON;

-- 级联删除规则
-- 删除频道 → 删除关联计划和任务
ON DELETE CASCADE

-- 删除计划 → 任务保留，schedule_id 置空
ON DELETE SET NULL
```

### 事务处理

```sql
BEGIN TRANSACTION;
-- 多条操作
INSERT INTO channels (...) VALUES (...);
INSERT INTO schedules (...) VALUES (...);
COMMIT;
-- 或 ROLLBACK;
```
