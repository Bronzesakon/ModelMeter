//! 全后端统一的结构化错误类型。
//!
//! 内部模块返回 `Result<T, AppError>`，命令层通过 `From<AppError> for String`
//! 自动转为前端可读的错误信息。`is_auth_error()` 用于识别登录态失效，便于
//! 触发重新登录 / 清除会话等降级逻辑。
//!
//! `NetworkError` / `DecodingError` 等变体名与历史代码保持兼容，
//! `ds::api` / `mimo::api` 通过 `pub use crate::error::AppError as ApiError;`
//! 复用本类型，保证全后端只有一种错误类型。

use std::fmt;

#[derive(Debug, Clone)]
pub enum AppError {
    /// 认证无效或已过期（DeepSeek API Key）
    Unauthorized,
    /// 平台登录态已失效（网页 Token / Cookie / MiMo 会话）
    PlatformUnauthorized,
    /// 请求过于频繁
    RateLimited,
    /// 服务器错误
    ServerError(u16),
    /// HTTP 错误
    HttpError(u16),
    /// 网络错误
    NetworkError(String),
    /// 响应解析失败
    DecodingError(String),
    /// 服务器返回无效响应
    InvalidResponse,
    /// 平台 API HTTP 错误
    PlatformError(u16, String),
    /// 通用请求失败
    Http(String),
    /// 请求超时
    Timeout,
    /// 配置错误
    Config(String),
    /// IO 错误
    Io(String),
    /// 凭据加解密失败
    Crypto(String),
    /// 其他
    Other(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Unauthorized => write!(f, "认证无效或已过期，请重新登录"),
            AppError::PlatformUnauthorized => write!(f, "登录无效或已过期，请重新登录"),
            AppError::RateLimited => write!(f, "请求过于频繁，请稍后重试"),
            AppError::ServerError(c) => write!(f, "服务器错误 ({})", c),
            AppError::HttpError(c) => write!(f, "HTTP 错误 ({})", c),
            AppError::NetworkError(_) => write!(f, "网络连接失败，请检查网络设置"),
            AppError::DecodingError(m) => write!(f, "数据解析错误: {}", m),
            AppError::InvalidResponse => write!(f, "服务器返回无效响应"),
            AppError::PlatformError(c, m) => write!(f, "平台API HTTP {}: {}", c, m),
            AppError::Http(msg) => write!(f, "请求失败: {}", msg),
            AppError::Timeout => write!(f, "请求超时"),
            AppError::Config(m) => write!(f, "配置错误: {}", m),
            AppError::Io(m) => write!(f, "IO 错误: {}", m),
            AppError::Crypto(m) => write!(f, "凭据加解密失败: {}", m),
            AppError::Other(m) => write!(f, "{}", m),
        }
    }
}

impl AppError {
    /// 是否为登录态/认证失效类错误。
    pub fn is_auth_error(&self) -> bool {
        matches!(self, AppError::PlatformUnauthorized | AppError::Unauthorized)
    }
}

impl From<String> for AppError {
    fn from(s: String) -> Self {
        AppError::Other(s)
    }
}

impl From<&str> for AppError {
    fn from(s: &str) -> Self {
        AppError::Other(s.to_string())
    }
}

impl From<AppError> for String {
    fn from(e: AppError) -> String {
        e.to_string()
    }
}

impl std::error::Error for AppError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_texts() {
        assert_eq!(AppError::PlatformUnauthorized.to_string(), "登录无效或已过期，请重新登录");
        assert_eq!(AppError::Timeout.to_string(), "请求超时");
    }

    #[test]
    fn auth_error_detection() {
        assert!(AppError::PlatformUnauthorized.is_auth_error());
        assert!(AppError::Unauthorized.is_auth_error());
        assert!(!AppError::RateLimited.is_auth_error());
        assert!(!AppError::Timeout.is_auth_error());
    }

    #[test]
    fn convert_to_string() {
        let s: String = AppError::Other("自定义错误".to_string()).into();
        assert_eq!(s, "自定义错误");
    }
}