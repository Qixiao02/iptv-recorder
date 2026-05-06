ALTER TABLE channels ADD COLUMN source_visibility TEXT NOT NULL DEFAULT 'public';
ALTER TABLE channels ADD COLUMN playback_strategy TEXT NOT NULL DEFAULT 'auto';
