//! 认证中间件

use axum::{
    extract::Request,
    http::{header, StatusCode},
    middleware::Next,
    response::Json,
};
use serde::Serialize;

use crate::services::AuthService;

/// 错误响应
#[derive(Serialize)]
pub struct AuthError {
    pub error: String,
    pub details: Option<String>,
}

/// 认证中间件
pub async fn auth_middleware(
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

    // 验证 Token
    match AuthService::verify_token(token) {
        Ok(claims) => {
            // 将用户信息添加到 request extensions
            let mut request = request;
            request.extensions_mut().insert(claims);
            Ok(next.run(request).await)
        }
        Err(e) => Err((
            StatusCode::UNAUTHORIZED,
            Json(AuthError {
                error: "unauthorized".to_string(),
                details: Some(e.to_string()),
            }),
        )),
    }
}
