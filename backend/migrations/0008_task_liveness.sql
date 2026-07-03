-- 录制任务僵尸检测与恢复
--
-- 背景：录制进度由一个后台监控任务每 3 秒更新 updated_at。当后端重启/崩溃时，
-- 该监控任务随之死亡，updated_at 不再前进，而任务行永久停留在 status='running'。
-- 更糟的是 migration 0006 的部分唯一索引会让这些僵尸 running 行永久占用
-- 该频道/计划的录制名额，导致再也录不了同一个台。
--
-- 本迁移：
-- 1) 为 tasks.updated_at 建索引 —— 僵尸巡检服务用它扫描"长时间未更新"的 running 任务。
-- 2) 写入默认停滞阈值 task_stale_timeout_secs=90s（= 30 次 3s 心跳漏更新），
--    可在设置页调整。监控任务一旦死亡，90s 后即被判定为僵尸并清理。

CREATE INDEX IF NOT EXISTS idx_tasks_updated_at ON tasks(updated_at);

INSERT OR IGNORE INTO system_config(key, value, updated_at)
VALUES('task_stale_timeout_secs', '90', datetime('now'));
