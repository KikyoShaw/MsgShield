//! 业务错误：code + 用户可读中文文案，序列化给前端
//! code 集合对齐 C# BusinessException 的 ErrorCode

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct PatchError {
    /// 业务错误码（installed / not_support / maybe_modified / match_* / io 等）
    pub code: String,
    /// 用户可读文案（中文，对齐原版）
    pub message: String,
}

impl PatchError {
    pub fn business(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
        }
    }

    /// IO 类失败：文案追加管理员/关闭应用提示（对齐 FormMain catch 逻辑）
    pub fn io(context: &str, err: &std::io::Error) -> Self {
        Self {
            code: "io".to_string(),
            message: format!("{context} 失败：{err}。请以管理员权限启动本程序，并确认当前应用（微信/QQ/TIM）处于关闭状态。"),
        }
    }
}

impl std::fmt::Display for PatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for PatchError {}

pub type PatchResult<T> = Result<T, PatchError>;