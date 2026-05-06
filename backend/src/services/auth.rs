//! 认证服务

use anyhow::{anyhow, Result};
use bcrypt::{hash, verify, DEFAULT_COST};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Sqlite};

use crate::models::{User, UserRole, LoginRequest, LoginResponse, ChangePasswordRequest};

/// JWT 配置
const JWT_EXPIRATION_HOURS: i64 = 24;
const MIN_JWT_SECRET_LEN: usize = 32;

fn jwt_secret() -> Result<Vec<u8>> {
    let secret = std::env::var("IPTV_JWT_SECRET")
        .map_err(|_| anyhow!("缺少环境变量 IPTV_JWT_SECRET，请配置至少 32 位的 JWT 密钥"))?;

    if secret.trim().len() < MIN_JWT_SECRET_LEN {
        return Err(anyhow!(
            "环境变量 IPTV_JWT_SECRET 长度不足，至少需要 {} 位字符",
            MIN_JWT_SECRET_LEN
        ));
    }

    Ok(secret.into_bytes())
}

/// JWT Claims
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,      // user id
    pub username: String,
    pub role: String,
    pub exp: usize,       // expiration time
    pub iat: usize,       // issued at
}

impl Claims {
    pub fn role(&self) -> UserRole {
        self.role.parse().unwrap_or(UserRole::Viewer)
    }

    pub fn can_manage_content(&self) -> bool {
        self.role().can_manage_content()
    }

    pub fn can_manage_security(&self) -> bool {
        self.role().can_manage_security()
    }
}

/// 认证服务
pub struct AuthService {
    db: Pool<Sqlite>,
}

impl AuthService {
    pub fn new(db: Pool<Sqlite>) -> Self {
        Self { db }
    }

    /// 校验 JWT 密钥配置
    pub fn validate_runtime_config() -> Result<()> {
        jwt_secret().map(|_| ())
    }

    /// 初始化默认管理员账号
    pub async fn init_default_admin(&self) -> Result<()> {
        // 检查是否已存在 admin 用户
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM users WHERE username = 'admin')"
        )
        .fetch_one(&self.db)
        .await?;

        if !exists {
            let generated_password = format!("admin-{}", uuid::Uuid::new_v4().simple());
            let initial_password = std::env::var("IPTV_INITIAL_ADMIN_PASSWORD")
                .ok()
                .filter(|password| password.trim().len() >= 8)
                .unwrap_or(generated_password);
            let password_hash = hash(&initial_password, DEFAULT_COST)?;
            let now = Utc::now().to_rfc3339();

            sqlx::query(
                r#"
                INSERT INTO users (id, username, password_hash, nickname, role, created_at, updated_at)
                VALUES (?, ?, ?, ?, ?, ?, ?)
                "#
            )
            .bind("admin")
            .bind("admin")
            .bind(&password_hash)
            .bind("管理员")
            .bind("admin")
            .bind(&now)
            .bind(&now)
            .execute(&self.db)
            .await?;

            tracing::warn!(
                "Created initial admin user. username=admin, password={}. 请立即登录并修改密码，或通过 IPTV_INITIAL_ADMIN_PASSWORD 显式设置首个管理员密码。",
                initial_password
            );
        }

        Ok(())
    }

    /// 用户登录
    pub async fn login(&self, req: LoginRequest) -> Result<LoginResponse> {
        // 查找用户
        let user = sqlx::query_as::<_, User>(
            "SELECT * FROM users WHERE username = ?"
        )
        .bind(&req.username)
        .fetch_optional(&self.db)
        .await?
        .ok_or_else(|| anyhow!("用户名或密码错误"))?;

        // 验证密码
        let valid = verify(&req.password, &user.password_hash)
            .map_err(|_| anyhow!("密码验证失败"))?;

        if !valid {
            return Err(anyhow!("用户名或密码错误"));
        }

        // 生成 JWT
        let token = self.generate_token(&user)?;

        Ok(LoginResponse {
            token,
            user: user.into(),
        })
    }

    /// 生成 JWT Token
    fn generate_token(&self, user: &User) -> Result<String> {
        let now = Utc::now();
        let exp = now + Duration::hours(JWT_EXPIRATION_HOURS);

        let claims = Claims {
            sub: user.id.clone(),
            username: user.username.clone(),
            role: user.role.clone(),
            exp: exp.timestamp() as usize,
            iat: now.timestamp() as usize,
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(&jwt_secret()?),
        )
        .map_err(|e| anyhow!("生成 Token 失败: {}", e))
    }

    /// 验证 JWT Token
    pub fn verify_token(token: &str) -> Result<Claims> {
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(&jwt_secret()?),
            &Validation::default(),
        )
        .map_err(|e| anyhow!("Token 无效: {}", e))?;

        Ok(token_data.claims)
    }

    /// 获取当前用户
    pub async fn get_current_user(&self, user_id: &str) -> Result<User> {
        let user = sqlx::query_as::<_, User>(
            "SELECT * FROM users WHERE id = ?"
        )
        .bind(user_id)
        .fetch_optional(&self.db)
        .await?
        .ok_or_else(|| anyhow!("用户不存在"))?;

        Ok(user)
    }

    /// 修改密码
    pub async fn change_password(&self, user_id: &str, req: ChangePasswordRequest) -> Result<()> {
        // 获取用户
        let user = self.get_current_user(user_id).await?;

        // 验证旧密码
        let valid = verify(&req.old_password, &user.password_hash)
            .map_err(|_| anyhow!("密码验证失败"))?;

        if !valid {
            return Err(anyhow!("原密码错误"));
        }

        // 哈希新密码
        let new_hash = hash(&req.new_password, DEFAULT_COST)?;
        let now = Utc::now().to_rfc3339();

        // 更新密码
        sqlx::query(
            "UPDATE users SET password_hash = ?, updated_at = ? WHERE id = ?"
        )
        .bind(&new_hash)
        .bind(&now)
        .bind(user_id)
        .execute(&self.db)
        .await?;

        tracing::info!("User {} changed password", user.username);

        Ok(())
    }

    /// 更新用户信息
    pub async fn update_profile(&self, user_id: &str, nickname: Option<&str>) -> Result<User> {
        let now = Utc::now().to_rfc3339();

        if let Some(name) = nickname {
            sqlx::query(
                "UPDATE users SET nickname = ?, updated_at = ? WHERE id = ?"
            )
            .bind(name)
            .bind(&now)
            .bind(user_id)
            .execute(&self.db)
            .await?;
        }

        self.get_current_user(user_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::Config, core::database};
    use std::{path::PathBuf, sync::{Mutex, OnceLock}, time::{SystemTime, UNIX_EPOCH}};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn temp_db_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        std::env::temp_dir().join(format!("iptv-recorder-{name}-{nanos}.db"))
    }

    #[test]
    fn validate_runtime_config_requires_secret() {
        let _guard = env_lock().lock().expect("lock poisoned");
        std::env::remove_var("IPTV_JWT_SECRET");

        let result = AuthService::validate_runtime_config();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("IPTV_JWT_SECRET"));
    }

    #[test]
    fn validate_runtime_config_rejects_short_secret() {
        let _guard = env_lock().lock().expect("lock poisoned");
        std::env::set_var("IPTV_JWT_SECRET", "too-short-secret");

        let result = AuthService::validate_runtime_config();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("至少需要 32 位"));
    }

    #[tokio::test]
    async fn init_default_admin_uses_configured_password() {
        let _guard = env_lock().lock().expect("lock poisoned");
        std::env::set_var("IPTV_JWT_SECRET", "abcdefghijklmnopqrstuvwxyz123456");
        std::env::set_var("IPTV_INITIAL_ADMIN_PASSWORD", "super-secure-admin");

        let db_path = temp_db_path("auth");
        let db = database::init(db_path.to_str().expect("utf8 path"), 1)
            .await
            .expect("db init");
        let service = AuthService::new(db);

        service.init_default_admin().await.expect("init admin");
        let login = service.login(LoginRequest {
            username: "admin".to_string(),
            password: "super-secure-admin".to_string(),
        }).await;

        assert!(login.is_ok());

        let _ = tokio::fs::remove_file(db_path).await;
        std::env::remove_var("IPTV_INITIAL_ADMIN_PASSWORD");
        let _ = Config::default(); // keep config module linked in test builds
    }
}
