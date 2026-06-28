-- 用户表增加 token_version 字段,用于 JWT 吊销机制。
-- 改密码/降权限时 token_version + 1,使旧 token 失效。
ALTER TABLE users ADD COLUMN token_version INTEGER NOT NULL DEFAULT 0;
