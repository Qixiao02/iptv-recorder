-- 通知中心持久化表
-- 应用内通知：录制完成 / 录制失败 / 磁盘空间警告 / 系统消息
CREATE TABLE IF NOT EXISTS notifications (
    id          TEXT PRIMARY KEY,
    category    TEXT NOT NULL,             -- 'recording_complete' | 'recording_failed' | 'disk_warning' | 'system'
    level       TEXT NOT NULL DEFAULT 'info', -- 'info' | 'warning' | 'error'
    title       TEXT NOT NULL,
    message     TEXT NOT NULL,
    details     TEXT,                       -- JSON 串：任务 ID / 文件大小 / 磁盘数值等
    task_id     TEXT,
    read        INTEGER NOT NULL DEFAULT 0, -- 0 = 未读, 1 = 已读
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_notifications_created_at ON notifications(created_at);
CREATE INDEX IF NOT EXISTS idx_notifications_read      ON notifications(read);
CREATE INDEX IF NOT EXISTS idx_notifications_category  ON notifications(category);
