# 后端修复 02：安全加固

> 优先级：**P0 + P1 + 部分 P2**
> 预计工时：3-4 天
> 推荐执行人：`rust-backend-engineer`
> 配套审计章节：`deep-analysis-2026-06-02.md` §3.1 R3-R7, §3.2 I4

## 范围与背景

本文件覆盖后端的**安全漏洞与配置安全默认值**。共 4 个子任务：M3U SSRF 防御、admin 密码强制从环境变量、CORS 收紧、登录限流 + refresh token。任意一个**单独**就足以让一个公网部署的项目被攻破。

## 子任务清单

### 子任务 2.1：M3U 解析去掉 `url.starts_with("/")` 分支（**P0**）

**审计引用**：`backend/src/services/m3u_parser.rs:213-216`、§3.1 R3

**问题**：
```rust
if !url.is_empty()
    && (url.starts_with("http://")
        || url.starts_with("https://")
        || url.starts_with("/"))   // ← 接受本地路径
{
    // 存入 channel.url
}
```
后果：用户通过"导入 M3U"端点可以提交 `/etc/passwd`、`/etc/shadow` 或 `file:///etc/shadow` 当频道 URL。这些 URL 后续会喂给 `transcode.rs:434-451` 的 FFmpeg——**FFmpeg 支持 file:// 协议**。

**修复方案**：
```rust
// backend/src/services/m3u_parser.rs:213
if !url.is_empty()
    && (url.starts_with("http://")
        || url.starts_with("https://"))  // 删掉 || url.starts_with("/")
{
    // 已有逻辑
} else {
    result.failed += 1;
    result.errors.push(format!("不支持的 URL scheme: {}", url));
}
```

**进一步建议**：增加 `url.parse::<url::Url>()` 校验，强制 `scheme ∈ {http, https}`：
```rust
match url::Url::parse(&url) {
    Ok(parsed) if matches!(parsed.scheme(), "http" | "https") => {
        // 接受
    }
    _ => {
        // 拒绝
    }
}
```

**类似要修的地方**：
- `backend/src/api/handlers.rs` 推测的 `import_m3u_url` / `import_m3u_content` 处理函数——读完后调 `M3UParser::parse`，也会进入同一分支
- `backend/src/services/channel.rs` 创建 channel 的 URL 字段也要校验

**验收**：
- [ ] 单元测试：传 `{"url": "/etc/passwd"}` 给 M3U 解析，返回 `failed` 不入库
- [ ] 单元测试：传 `{"url": "file:///etc/shadow"}` 同样拒绝
- [ ] 单元测试：传 `{"url": "http://example.com/playlist.m3u8"}` 仍然通过
- [ ] 数据库扫一遍：确认现存 channel 没有任何 `url` 是 `/xxx` 形式；如有，单独发 migration 清理（**这一步手动**）

**风险**：低。只是删除一个分支 + 加 scheme 校验。**如果仓库里已经有用户导入过 `/etc/passwd` 之类的脏数据需要先清理再修**——先跑 `SELECT id, url FROM channels WHERE url NOT LIKE 'http%'` 看一下。

---

### 子任务 2.2：默认 admin 密码强制从 env 读，缺则拒启动（**P0**）

**审计引用**：`backend/src/services/auth.rs:78-105`、`backend/src/main.rs:63`、§3.1 R5

**问题**：
```rust
let generated_password = format!("admin-{}", uuid::Uuid::new_v4().simple());
let initial_password = std::env::var("IPTV_INITIAL_ADMIN_PASSWORD")
    .ok()
    .filter(|password| password.trim().len() >= 8)
    .unwrap_or(generated_password);  // ← 默认生成随机密码
// ...
tracing::warn!("Created initial admin user. username=admin, password={}. ...", initial_password);
```
后果：随机生成的密码以 `warn!` 级别写到 stdout——任何能看容器日志的人都拿到初始 admin 权限。

**修复方案**：
```rust
// backend/src/services/auth.rs
pub async fn init_default_admin(&self) -> Result<()> {
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE username = 'admin')")
            .fetch_one(&self.db)
            .await?;

    if !exists {
        // 强制从 env 读
        let initial_password = std::env::var("IPTV_INITIAL_ADMIN_PASSWORD")
            .map_err(|_| anyhow!(
                "未设置环境变量 IPTV_INITIAL_ADMIN_PASSWORD，请配置至少 8 位的初始 admin 密码"
            ))?;

        if initial_password.trim().len() < 8 {
            return Err(anyhow!(
                "环境变量 IPTV_INITIAL_ADMIN_PASSWORD 长度不足，至少需要 8 位"
            ));
        }

        // 写入后绝不打印密码本身
        let password_hash = hash(&initial_password, DEFAULT_COST)?;
        // ... INSERT ...

        // 只打印提示，让用户去日志外的地方设置
        tracing::info!(
            "✅ Created initial admin user from IPTV_INITIAL_ADMIN_PASSWORD. \
             密码未写入日志，请从环境变量持有者处获取并立即修改。"
        );
    }
    Ok(())
}
```

**main.rs 改动**：
```rust
// backend/src/main.rs:63
// 把 init_default_admin 提到 validate_runtime_config 之后，
// 一旦失败 main 直接 return 退出
services::AuthService::validate_runtime_config()?;
let auth_service = services::AuthService::new(db.clone());
auth_service.init_default_admin().await?;  // 失败就 bail
```

**文档更新**：
- `docs/configuration.md` / `docs/deployment.md` / `README.md`：在"首次部署"章节加**必须**设 `IPTV_INITIAL_ADMIN_PASSWORD`
- 加进 `docs/operations-runbook.md` 的"故障排查"清单

**验收**：
- [ ] 不设 env 启动 → main 退出，错误信息明确
- [ ] 设了 < 8 位 → 同样退出
- [ ] 设了 ≥ 8 位 → admin 用户创建成功，**日志里不出现密码本身**
- [ ] 现有数据库已有 admin → 不报错不重建

**风险**：低。**破坏性变更**——以前"首次启动自动生成密码"的行为不再存在。需要在 release notes / commit message 显著标注，并确保部署文档先更新。

---

### 子任务 2.3：CORS 默认收紧（**P1**）

**审计引用**：`backend/src/api/router.rs:176`、§3.1 R7

**问题**：`CorsLayer::permissive()` 跨域全开。当前 JWT 在 Authorization header 影响有限，但**这是配置上的"打开"=未来的事故**——如果将来加 cookie auth、或者前端的同源策略改变，会出大事。

**修复方案**：
```rust
// backend/src/config.rs 增
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    // ... 现有字段
    /// CORS 允许的源（逗号分隔）。空 = 拒绝所有跨域。
    #[serde(default)]
    pub cors_allowed_origins: Vec<String>,
}
```

```rust
// backend/src/api/router.rs:166-178
use tower_http::cors::{CorsLayer, AllowOrigin, AllowMethods, AllowHeaders};

let cors_layer = if config.server.cors_allowed_origins.is_empty() {
    // 拒绝所有跨域
    CorsLayer::new()
} else {
    let origins: Vec<_> = config.server.cors_allowed_origins
        .iter()
        .filter_map(|o| o.parse().ok())
        .collect();
    CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods(AllowMethods::LIST)
        .allow_headers(AllowHeaders::list(vec![http::header::AUTHORIZATION, http::header::CONTENT_TYPE]))
        .allow_credentials(false)  // 当前不用 cookie auth
};
```

```toml
# backend/config/default.toml
[server]
# CORS 允许的源（逗号分隔）。生产环境应留空，只允许同源访问。
# 开发场景：cors_allowed_origins = ["http://localhost:5173", "http://127.0.0.1:5173"]
cors_allowed_origins = []
```

```bash
# 生产部署用环境变量
IPTV__SERVER__CORS_ALLOWED_ORIGINS=https://iptv.example.com,https://admin.example.com
```

**验收**：
- [ ] 不设 env → `OPTIONS` 跨域预检返回 403
- [ ] 设了 `http://localhost:5173` → 预检通过且 `Access-Control-Allow-Origin: http://localhost:5173`
- [ ] 设了不在白名单的 origin → 预检不通过
- [ ] 同源请求（前端直接部署到后端 `/`）→ 不受影响

**风险**：低。**当前默认是 permissive = 跨域全过；改后默认是 deny = 同源才过**。开发环境会需要显式开。

---

### 子任务 2.4：登录限流 + refresh token（**P2**）

**审计引用**：`backend/src/services/auth.rs:111-135`、§3.2 I10

**问题**：
- `/api/auth/login` 无 rate limit——bcrypt verify 是 CPU bound，攻击者可以用大字典做 bcrypt 爆破
- JWT 24h 过期无 refresh——用户体验差 + 长 token 泄露窗口大

**修复方案**（这部分是 P2 但建议一起做）：

#### 2.4a：登录限流

在 `services/auth.rs` 加内存 rate limiter（项目小用 `dashmap` + 滑动窗口就够）：
```rust
use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct LoginRateLimiter {
    attempts: DashMap<String, Vec<Instant>>,
}

impl LoginRateLimiter {
    pub fn check_and_record(&self, key: &str, max_attempts: usize, window: Duration) -> bool {
        let now = Instant::now();
        let mut entry = self.attempts.entry(key.to_string()).or_default();
        entry.retain(|t| now.duration_since(*t) < window);
        if entry.len() >= max_attempts {
            return false;  // 限流
        }
        entry.push(now);
        true
    }
}
```

在 `auth_service.login` 里加：
```rust
pub async fn login(&self, req: LoginRequest) -> Result<LoginResponse> {
    if !self.rate_limiter.check_and_record(
        &format!("login:{}", req.username),
        5,  // 5 次
        Duration::from_secs(60),  // 每 60s 窗口
    ) {
        return Err(anyhow!("登录尝试过于频繁，请稍后重试"));
    }
    // ... 现有 login 逻辑
}
```

更精确应该按 IP 限流（不是 username）——`axum::extract::ConnectInfo<SocketAddr>` 拿 IP。

#### 2.4b：Refresh token

新增长期 refresh token + 短时 access token：
- `access_token` 有效期 15 分钟
- `refresh_token` 有效期 30 天，存数据库（不在 JWT 里）
- `/api/auth/refresh` 端点用 refresh token 换新 access token

需要在 `models` 加 `refresh_tokens` 表：
```sql
CREATE TABLE refresh_tokens (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    token_hash TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL,
    last_used_at TEXT,
    revoked_at TEXT,
    FOREIGN KEY (user_id) REFERENCES users(id)
);
CREATE INDEX idx_refresh_tokens_user ON refresh_tokens(user_id);
CREATE INDEX idx_refresh_tokens_hash ON refresh_tokens(token_hash);
```

前端要做的事：
- login 拿到 `{access_token, refresh_token}`
- access token 401 → 自动调 `/api/auth/refresh` 换新
- 30 天没活动就强制重新登录

**验收**（仅 2.4a，因为 2.4b 工时大可分独立 task）：
- [ ] 单元测试：6 次连续错误密码登录 → 第 6 次返回 "登录尝试过于频繁"
- [ ] 60 秒后重试 → 成功
- [ ] 不同 username 不互相影响

**风险**：中。rate limiter 用内存意味着**多实例部署不共享**——生产应换 Redis；本任务先满足单机场景。

---

## 测试要求

每子任务完成后必须新增测试：

| 子任务 | 测试 |
| --- | --- |
| 2.1 | `services/m3u_parser.rs` 加 3 个测试（拒绝 /etc/、拒绝 file://、接受 https） |
| 2.2 | `services/auth.rs` 加 3 个测试（缺 env / 短密码 / 正常）|
| 2.3 | `api/router.rs` 加 2 个测试（空配置拒绝 / 白名单通过）|
| 2.4a | `services/auth.rs` 加 1 个 rate limit 测试 |

## 提交策略

- 每个子任务一个 commit
- 2.2 是**破坏性变更**，commit message 加 `BREAKING:` 前缀
- 2.3 默认行为改变（deny）属于兼容性变更，需要 release notes 强调

## 风险汇总

| 子任务 | 风险 | 缓解 |
| --- | --- | --- |
| 2.1 | 历史脏数据没清 | 修前先 `SELECT url FROM channels` 排查 |
| 2.2 | 部署文档没更新导致用户启动失败 | 文档先行 + 部署示例带 IPTV_INITIAL_ADMIN_PASSWORD |
| 2.3 | 开发体验变差 | dev 文档说明显式开 `cors_allowed_origins` |
| 2.4 | 内存 rate limit 不支持多实例 | 文档说明 + 留 TODO 用 Redis |

---

*执行入口：2.1 → 2.2 → 2.3 → 2.4。*
