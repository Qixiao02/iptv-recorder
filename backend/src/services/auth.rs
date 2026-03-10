//! 认证服务

use anyhow::{anyhow, Result};
use bcrypt::{hash, verify, DEFAULT_COST};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Sqlite};
use uuid::Uuid;

use crate::models::{User, LoginRequest, LoginResponse, UserInfo, ChangePasswordRequest};

/// JWT 配置
const JWT_EXPIRATION_HOURS: i64 = 24;

fn jwt_secret() -> Vec<u8> {
    std::env::var("IPTV_JWT_SECRET")
        .unwrap_or_else(|_| "iptv-recorder-jwt-secret-key-2024".to_string())
        .into_bytes()
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

/// 认证服务
pub struct AuthService {
    db: Pool<Sqlite>,
}

impl AuthService {
    pub fn new(db: Pool<Sqlite>) -> Self {
        Self { db }
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
            let password_hash = hash("admin001", DEFAULT_COST)?;
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

            tracing::info!("Created default admin user: admin / admin001");
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
            &EncodingKey::from_secret(&jwt_secret()),
        )
        .map_err(|e| anyhow!("生成 Token 失败: {}", e))
    }

    /// 验证 JWT Token
    pub fn verify_token(token: &str) -> Result<Claims> {
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(&jwt_secret()),
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
