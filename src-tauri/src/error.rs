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

    /// SQLite（sqlx）读写失败：剪贴板历史库损坏、磁盘满、迁移失败等，由存储层 `?` 透传到命令层
    #[error("数据库错误: {0}")]
    Database(#[from] sqlx::Error),

    /// 剪贴板读写失败（arboard 报错 / 图片编解码失败 / Win32 监听窗口创建失败），内容为底层错误的 `to_string()`
    #[error("剪贴板错误: {0}")]
    Clipboard(String),

    /// 当前操作系统没有实现该能力（如非 Windows 上的剪贴板粘贴），前端直接展示，不重试
    #[error("当前平台暂不支持: {0}")]
    Unsupported(String),
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

    /// 剪贴板错误文案前缀锁定
    #[test]
    fn clipboard_message_has_category_prefix() {
        let err = AppError::Clipboard("无法打开剪贴板".into());
        assert_eq!(err.to_string(), "剪贴板错误: 无法打开剪贴板");
        let json = serde_json::to_string(&err).expect("AppError 应能序列化为字符串");
        assert_eq!(json, "\"剪贴板错误: 无法打开剪贴板\"");
    }

    /// 平台不支持文案前缀锁定
    #[test]
    fn unsupported_message_has_category_prefix() {
        let err = AppError::Unsupported("剪贴板粘贴".into());
        assert_eq!(err.to_string(), "当前平台暂不支持: 剪贴板粘贴");
    }

    /// 数据库错误由 sqlx::Error 自动转换，文案带类别前缀
    #[test]
    fn database_message_has_category_prefix() {
        let err: AppError = sqlx::Error::RowNotFound.into();
        assert!(matches!(err, AppError::Database(_)));
        assert!(err.to_string().starts_with("数据库错误: "));
    }
}
