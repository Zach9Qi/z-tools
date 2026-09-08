//! 示例命令：问候。演示「参数校验 → 返回 Result<T, AppError>」的最小写法。

use crate::error::AppError;

/// 向指定名字问好。
///
/// `name` 为用户输入，去除首尾空白后为空则返回参数错误，由前端直接展示文案。
#[tauri::command]
pub fn greet(name: &str) -> Result<String, AppError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::InvalidInput("名字不能为空".into()));
    }
    Ok(format!("你好，{name}！来自 Rust 的问候。"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_name_is_rejected() {
        let err = greet("   ").expect_err("空名字应当返回错误");
        assert!(matches!(err, AppError::InvalidInput(_)));
        assert_eq!(err.to_string(), "参数错误: 名字不能为空");
    }

    #[test]
    fn greets_trimmed_name() {
        let msg = greet("  小明 ").expect("正常名字应当返回问候");
        assert_eq!(msg, "你好，小明！来自 Rust 的问候。");
    }
}
