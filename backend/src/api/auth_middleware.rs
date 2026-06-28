//! 认证中间件

use axum::{
    extract::Request,
    http::{header, StatusCode},
    middleware::Next,
    response::Json,
    Extension,
};
use serde::Serialize;
use sqlx::{Pool, Sqlite};

use crate::services::{AuthService, Claims};

/// 错误响应
#[derive(Serialize)]
pub struct AuthError {
    pub error: String,
    pub details: Option<String>,
}

/// 认证中间件:校验 JWT 签名 + 过期 + iss/aud + token_version(吊销检查)
pub async fn auth_middleware(
    Extension(db): Extension<Pool<Sqlite>>,
    request: Request,
    next: Next,
) -> Result<axum::response::Response, (StatusCode, Json<AuthError>)> {
    // 从 Header 获取 Token
    let auth_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok());

    let token = match auth_header {
        Some(h) if h.starts_with("Bearer ") => &h[7..],
        _ => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(AuthError {
                    error: "unauthorized".to_string(),
                    details: Some("缺少认证 Token".to_string()),
                }),
            ));
        }
    };

    // 验证 Token:签名 + 过期 + iss/aud + token_version(吊销检查),复用 AuthService
    let auth_service = AuthService::new(db);
    let claims = match auth_service.verify_token_with_db(token).await {
        Ok(c) => c,
        Err(e) => {
            let is_revoked = e.to_string().contains("失效");
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(AuthError {
                    error: if is_revoked {
                        "token_revoked".to_string()
                    } else {
                        "unauthorized".to_string()
                    },
                    details: Some(e.to_string()),
                }),
            ));
        }
    };

    // 将用户信息添加到 request extensions
    let mut request = request;
    request.extensions_mut().insert(claims);
    Ok(next.run(request).await)
}

/// 需要 operator/admin 权限的中间件
pub async fn operator_middleware(
    request: Request,
    next: Next,
) -> Result<axum::response::Response, (StatusCode, Json<AuthError>)> {
    let claims = request
        .extensions()
        .get::<Claims>()
        .cloned()
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(AuthError {
                    error: "unauthorized".to_string(),
                    details: Some("缺少认证上下文".to_string()),
                }),
            )
        })?;

    if !claims.can_manage_content() {
        return Err((
            StatusCode::FORBIDDEN,
            Json(AuthError {
                error: "forbidden".to_string(),
                details: Some("需要 operator 或 admin 权限".to_string()),
            }),
        ));
    }

    Ok(next.run(request).await)
}

/// 需要 admin 权限的中间件
pub async fn admin_middleware(
    request: Request,
    next: Next,
) -> Result<axum::response::Response, (StatusCode, Json<AuthError>)> {
    let claims = request
        .extensions()
        .get::<Claims>()
        .cloned()
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(AuthError {
                    error: "unauthorized".to_string(),
                    details: Some("缺少认证上下文".to_string()),
                }),
            )
        })?;

    if !claims.can_manage_security() {
        return Err((
            StatusCode::FORBIDDEN,
            Json(AuthError {
                error: "forbidden".to_string(),
                details: Some("需要 admin 权限".to_string()),
            }),
        ));
    }

    Ok(next.run(request).await)
}
