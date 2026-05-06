//! 用户模型

use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

/// 用户角色
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    Viewer,
    Operator,
    Admin,
}

impl UserRole {
    pub fn can_manage_content(self) -> bool {
        matches!(self, Self::Operator | Self::Admin)
    }

    pub fn can_manage_security(self) -> bool {
        matches!(self, Self::Admin)
    }
}

impl fmt::Display for UserRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Viewer => write!(f, "viewer"),
            Self::Operator => write!(f, "operator"),
            Self::Admin => write!(f, "admin"),
        }
    }
}

impl FromStr for UserRole {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "viewer" => Ok(Self::Viewer),
            "operator" => Ok(Self::Operator),
            "admin" => Ok(Self::Admin),
            _ => Err(()),
        }
    }
}

/// 用户
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: String,
    pub username: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub nickname: Option<String>,
    pub role: String,
    pub created_at: String,
    pub updated_at: String,
}

/// 登录请求
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// 登录响应
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: UserInfo,
}

/// 用户信息（不含敏感信息）
#[derive(Debug, Serialize)]
pub struct UserInfo {
    pub id: String,
    pub username: String,
    pub nickname: Option<String>,
    pub role: String,
}

impl From<User> for UserInfo {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            username: user.username,
            nickname: user.nickname,
            role: user.role,
        }
    }
}

/// 修改密码请求
#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub old_password: String,
    pub new_password: String,
}
