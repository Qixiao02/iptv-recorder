CREATE TABLE IF NOT EXISTS epg_sources (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    source_url TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS epg_programs (
    id TEXT PRIMARY KEY,
    source_id TEXT NOT NULL REFERENCES epg_sources(id) ON DELETE CASCADE,
    channel_ref TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT,
    category TEXT,
    start_at TEXT NOT NULL,
    end_at TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_epg_programs_channel_ref ON epg_programs(channel_ref);
CREATE INDEX IF NOT EXISTS idx_epg_programs_start_at ON epg_programs(start_at);
