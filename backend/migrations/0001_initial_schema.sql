CREATE TABLE IF NOT EXISTS channels (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    url TEXT NOT NULL,
    group_name TEXT DEFAULT 'Uncategorized',
    logo_url TEXT,
    source_type TEXT DEFAULT 'remote_url',
    source_url TEXT,
    status TEXT DEFAULT 'unknown',
    last_check_at TEXT,
    fail_count INTEGER DEFAULT 0,
    metadata TEXT DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS schedules (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    channel_id TEXT NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    cron_expression TEXT NOT NULL,
    duration_seconds INTEGER NOT NULL DEFAULT 3600,
    output_template TEXT DEFAULT '{channel_name}_{date}_{time}.mp4',
    output_dir TEXT,
    priority INTEGER DEFAULT 5,
    enabled INTEGER DEFAULT 1,
    max_retry INTEGER DEFAULT 3,
    notify_on_complete INTEGER DEFAULT 0,
    video_quality TEXT DEFAULT 'best',
    audio_quality TEXT DEFAULT 'best',
    max_speed TEXT,
    thread_count INTEGER DEFAULT 20,
    transcode_mode TEXT DEFAULT 'off',
    transcode_preset TEXT DEFAULT 'medium',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS tasks (
    id TEXT PRIMARY KEY,
    schedule_id TEXT REFERENCES schedules(id) ON DELETE SET NULL,
    channel_id TEXT NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    status TEXT DEFAULT 'pending',
    started_at TEXT,
    ended_at TEXT,
    exit_code INTEGER,
    error_message TEXT,
    output_path TEXT,
    file_size INTEGER DEFAULT 0,
    duration_recorded INTEGER DEFAULT 0,
    progress_percent INTEGER DEFAULT 0,
    current_speed TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS recordings (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    pid INTEGER,
    started_at TEXT NOT NULL,
    temp_path TEXT,
    log_path TEXT,
    last_progress_at TEXT,
    is_healthy INTEGER DEFAULT 1
);

CREATE TABLE IF NOT EXISTS system_config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    nickname TEXT,
    role TEXT DEFAULT 'user',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_channels_status ON channels(status);
CREATE INDEX IF NOT EXISTS idx_schedules_channel ON schedules(channel_id);
CREATE INDEX IF NOT EXISTS idx_schedules_enabled ON schedules(enabled);
CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
CREATE INDEX IF NOT EXISTS idx_tasks_channel ON tasks(channel_id);
CREATE INDEX IF NOT EXISTS idx_tasks_started ON tasks(started_at);
CREATE INDEX IF NOT EXISTS idx_users_username ON users(username);

DELETE FROM channels
WHERE id IN (
    SELECT older.id
    FROM channels older
    JOIN channels newer
      ON older.url = newer.url
     AND older.id <> newer.id
     AND (
            older.updated_at < newer.updated_at
         OR (older.updated_at = newer.updated_at AND older.rowid < newer.rowid)
     )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_channels_url_unique ON channels(url);

INSERT OR IGNORE INTO system_config (key, value) VALUES
    ('storage.recordings_path', './data/recordings'),
    ('storage.auto_cleanup_days', '30'),
    ('storage.min_free_space_gb', '10'),
    ('recording.default_duration_minutes', '60'),
    ('recording.n_m3u8dl_re_path', 'N_m3u8DL-RE'),
    ('recording.max_retry', '3'),
    ('recording.thread_count', '4'),
    ('notification.on_complete', 'true'),
    ('notification.on_failure', 'true'),
    ('notification.disk_warning', 'true');
