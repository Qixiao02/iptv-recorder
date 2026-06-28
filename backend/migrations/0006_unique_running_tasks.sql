-- 录制并发安全：兜底防止「同频道 / 同定时任务」出现两条 running 记录。
--
-- 背景：ensure_recording_capacity 是 check-then-act，cron 触发与 HTTP 手动录制
-- 并发进入时，可能在两条请求之间穿插，导致重复录制或突破 max_concurrent。
-- 进程内准入锁（ADMISSION_LOCK）负责串行化「检查→插入」；这里用部分唯一索引
-- 作为最终兜底，即便未来有多实例或锁失效，DB 仍会拒绝重复 running 记录。
--
-- 关键设计：
-- 1) 索引的 WHERE 子句限定 status='running'。当任务进入终态
--    （completed/failed/cancelled）时自动脱离约束，该频道可立即开始下一次录制。
--    这与 cancel()「先 WHERE status='running' 原子切到 cancelled」的语义一致：
--    状态切换后索引立即释放，取消后立即可重录。
-- 2) schedule_id 可为 NULL（手动录制无 schedule）。SQLite 的 UNIQUE 对多行 NULL
--    视为「互不相等」，故 NULL schedule 不会互相冲突，符合预期。
-- 3) max_concurrent（全局数量上限）无法用索引表达，仍由进程内锁的 COUNT(*) 兜底。

CREATE UNIQUE INDEX IF NOT EXISTS uniq_running_per_channel
    ON tasks(channel_id)
    WHERE status = 'running';

CREATE UNIQUE INDEX IF NOT EXISTS uniq_running_per_schedule
    ON tasks(schedule_id)
    WHERE status = 'running' AND schedule_id IS NOT NULL;
