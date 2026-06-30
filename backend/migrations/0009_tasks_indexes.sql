-- tasks 热查询索引补充
--
-- 背景:
--   list_tasks 用 `SELECT * FROM tasks ORDER BY created_at DESC` 做分页,但 created_at
--   此前无索引(0001 只建了 status/channel/started_at),任务表增长后每页都要全表排序。
--   此外:
--   - 查某 schedule 的历史任务(tasks.schedule_id)无索引,0006 的 partial unique 索引
--     只覆盖 status='running' 行,历史查询仍扫全表。
--   - audit/dashboard 的 failed_tasks_24h(`WHERE status='failed' AND updated_at>=...`)
--     只有 status 单列索引,updated_at 范围扫描未走索引。
--
-- 本迁移补三组索引:
--   1) idx_tasks_created_at         —— 列表分页 ORDER BY created_at DESC 走索引,免排序;
--   2) idx_tasks_schedule_id        —— 按 schedule 查历史任务;
--   3) idx_tasks_status_updated_at  —— (status, updated_at) 复合,覆盖失败/近期扫描。

CREATE INDEX IF NOT EXISTS idx_tasks_created_at ON tasks(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_tasks_schedule_id ON tasks(schedule_id);
CREATE INDEX IF NOT EXISTS idx_tasks_status_updated_at ON tasks(status, updated_at);
