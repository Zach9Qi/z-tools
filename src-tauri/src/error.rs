//! 全局统一错误类型。新增变体时保持中文文案口径，文案面向前端用户直接展示。

/// 全局统一错误类型：可失败命令一律返回 `Result<T, AppError>`，
/// 序列化时输出用户可读的中文文案供前端直接展示。
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// 前端传入的参数不合法（前端是不可信输入源，校验放在命令层边界）
    #[error("参数错误: {0}")]
    InvalidInput(String),

    /// 文件系统等输入输出错误
    #[error("输入输出错误: {0}")]
    Io(#[from] std::io::Error),

    /// Tauri 运行时错误（窗口、事件等）
    #[error("系统错误: {0}")]
    Tauri(#[from] tauri::Error),
}

impl serde::Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 锁定序列化契约：前端直接展示 to_string()，文案前缀不能漂移
    #[test]
    fn invalid_input_message_has_category_prefix() {
        let err = AppError::InvalidInput("名字不能为空".into());
        assert_eq!(err.to_string(), "参数错误: 名字不能为空");
        let json = serde_json::to_string(&err).expect("AppError 应能序列化为字符串");
        assert_eq!(json, "\"参数错误: 名字不能为空\"");
    }
}
