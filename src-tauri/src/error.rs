// 应用错误类型：结构化（code + message），跨 Rust 内部与 Tauri 边界
// App error: structured (code + message), crosses Rust internals and Tauri boundary
// 前端根据 code 决定提示文案（i18n key），根据 message 展示详情
// Frontend uses code for i18n message key and message for details
use serde::Serialize;
use std::fmt;

// ============================================================
// Error kind
// ============================================================

/// 错误分类（影响前端 UI 行为：toast 颜色、是否重试、是否重定向等）
/// Error category (drives UI behavior: toast color, retry, redirect)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AppErrorKind {
    /// 用户可修正：参数错误、未找到、校验失败
    /// User-fixable: validation, not found, bad input
    #[serde(rename = "user")]
    UserError,
    /// 系统/资源：DB 错、IO 错、超时
    /// System: db, io, timeout
    #[serde(rename = "system")]
    SystemError,
    /// 工作流/业务规则：禁用、环、配置不合法
    /// Business rule: disabled, cycle, invalid config
    #[serde(rename = "business")]
    BusinessError,
}

impl AppErrorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            AppErrorKind::UserError => "user",
            AppErrorKind::SystemError => "system",
            AppErrorKind::BusinessError => "business",
        }
    }
}

// ============================================================
// AppError
// ============================================================

/// 应用错误：code 机器可读，message 人类可读，kind 分类
/// App error: machine-readable code, human-readable message, kind for category
///
/// `Serialize` 序列化为 `{code, message, kind}` 供 Tauri 边界使用
/// `Serialize` as `{code, message, kind}` for Tauri command boundary
#[derive(Debug, Clone, Serialize)]
pub struct AppError {
    /// 机器可读错误码（前端 i18n key 的后半段）
    /// Machine-readable error code (frontend i18n key suffix)
    pub code: String,
    pub message: String,
    pub kind: AppErrorKind,
}

impl AppError {
    pub fn new(code: impl Into<String>, message: impl Into<String>, kind: AppErrorKind) -> Self {
        Self { code: code.into(), message: message.into(), kind }
    }

    /// 简化构造（按 UserError 默认分类）
    /// Shorthand for UserError
    pub fn user(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(code, message, AppErrorKind::UserError)
    }

    /// 简化构造（按 SystemError 分类）
    /// Shorthand for SystemError
    pub fn system(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(code, message, AppErrorKind::SystemError)
    }

    /// 简化构造（按 BusinessError 分类）
    /// Shorthand for BusinessError
    pub fn business(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(code, message, AppErrorKind::BusinessError)
    }

    /// 携带新 code 重新构造（保留 kind + message）
    /// Reconstruct with a new code (preserves kind + message)
    pub fn with_code(self, code: impl Into<String>) -> Self {
        Self { code: code.into(), message: self.message, kind: self.kind }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}: {}", self.kind.as_str(), self.code, self.message)
    }
}

impl std::error::Error for AppError {}

// ============================================================
// AppErrorWire — Tauri 序列化用，{ code, message, kind }
// ============================================================

/// Tauri 命令边界错误格式： `{ "code": "CODE", "message": "...", "kind": "user|system|business" }`
/// Tauri command boundary error format
#[derive(Debug, Clone, Serialize)]
pub struct AppErrorWire {
    pub code: String,
    pub message: String,
    pub kind: String,
}

impl From<AppError> for AppErrorWire {
    fn from(e: AppError) -> Self {
        Self { code: e.code, message: e.message, kind: e.kind.as_str().to_string() }
    }
}

impl From<AppError> for String {
    fn from(e: AppError) -> Self {
        e.to_string()
    }
}

impl From<AppErrorWire> for AppError {
    fn from(_w: AppErrorWire) -> Self {
        // AppErrorWire 只用于 Tauri 命令边界序列化；反序列化走 AppError 构造函数
        // AppErrorWire is only for Tauri command boundary serialization
        AppError::system("Unknown", "deserialization not supported")
    }
}

// ============================================================
// From<String> — 兼容老式 `Result<_, String>` 代码
// ============================================================

/// 转换 String → AppError
/// "CodeName: message" → code=CodeName, kind=SystemError
/// 其他 → code=Unknown, kind=SystemError
impl From<String> for AppError {
    fn from(s: String) -> Self {
        let msg = if let Some((prefix, rest)) = s.split_once(':') {
            let p = prefix.trim();
            if !p.is_empty() && !p.contains(' ') {
                // 第一个无空格词作为 code，其余作为 message
                // First space-free word is code; rest is message
                return AppError::new(p, rest.trim().to_string(), AppErrorKind::SystemError);
            }
            s
        } else {
            s
        };
        AppError::new("Unknown", msg, AppErrorKind::SystemError)
    }
}

impl From<&str> for AppError {
    fn from(s: &str) -> Self {
        Self::from(s.to_string())
    }
}

// ============================================================
// CmdResult — Tauri 异步命令返回类型
// ============================================================

/// Tauri 异步命令返回类型（替代 `Result<T, String>`）
/// Tauri async command result type (replaces `Result<T, String>`)
pub type CmdResult<T> = std::result::Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_prefix_extraction() {
        let e: AppError = "CycleDetected: workflow has a cycle".to_string().into();
        assert_eq!(e.code, "CycleDetected");
        assert_eq!(e.kind, AppErrorKind::SystemError);
        assert_eq!(e.message, "workflow has a cycle");
    }

    #[test]
    fn no_prefix_defaults_to_unknown() {
        let e: AppError = "oops".to_string().into();
        assert_eq!(e.code, "Unknown");
    }

    #[test]
    fn space_in_prefix_keeps_whole_string_as_message() {
        // 含空格的"前缀"保持原样作为 message（避免误判）
        let e: AppError = "two words: rest".to_string().into();
        assert_eq!(e.code, "Unknown");
        assert_eq!(e.message, "two words: rest");
    }

    #[test]
    fn kind_helpers() {
        assert_eq!(AppError::user("X", "y").kind, AppErrorKind::UserError);
        assert_eq!(AppError::business("X", "y").kind, AppErrorKind::BusinessError);
        assert_eq!(AppError::system("X", "y").kind, AppErrorKind::SystemError);
    }

    #[test]
    fn wire_format() {
        // AppError 本身已 Serialize，wire format 与 AppErrorWire 字段一致
        let e = AppError::user("NotFound", "workflow wf-123 not found");
        let json = serde_json::to_value(&e).unwrap();
        assert_eq!(json["code"], "NotFound");
        assert_eq!(json["kind"], "user");
        assert_eq!(json["message"], "workflow wf-123 not found");
    }

    #[test]
    fn wire_round_trip() {
        let e = AppError::business("CycleDetected", "graph has cycle a→b→a");
        let wire: AppErrorWire = e.into();
        let json = serde_json::to_value(&wire).unwrap();
        assert_eq!(json["code"], "CycleDetected");
        assert_eq!(json["kind"], "business");
        assert_eq!(json["message"], "graph has cycle a→b→a");
    }

    #[test]
    fn with_code_preserves_kind_and_message() {
        let e = AppError::business("Original", "msg").with_code("New");
        assert_eq!(e.code, "New");
        assert_eq!(e.message, "msg");
        assert_eq!(e.kind, AppErrorKind::BusinessError);
    }
}
