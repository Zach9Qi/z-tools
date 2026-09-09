//! 剪贴板历史命令：列表 / 全文 / 粘贴 / 删除 / 收藏，只做参数校验与转发，逻辑在 `crate::clipboard`。
//!
//! 本层**不** emit 事件：删除 / 收藏由前端在命令成功后直接改本地列表；
//! `clipboard://changed` 只由领域层 `record()`（监听器录入）发出。

use tauri::{AppHandle, Runtime, State};

use crate::clipboard::{self, ClipboardItem, ClipboardStore, ListQuery};
use crate::error::AppError;

/// 单页条数允许范围；前端默认 100
const LIMIT_RANGE: std::ops::RangeInclusive<u32> = 1..=200;

/// 按 kind × favoriteOnly × query 叠加筛选并游标分页，`ORDER BY copied_at DESC, id DESC`。
///
/// `limit` 不在 1..=200 内返回参数错误；关键字先 trim，只有空白时视为不过滤。
#[tauri::command]
pub async fn list_clipboard_items(
    store: State<'_, ClipboardStore>,
    mut query: ListQuery,
) -> Result<Vec<ClipboardItem>, AppError> {
    validate_limit(query.limit)?;
    query.query = query.query.trim().to_owned();
    store.list(&query).await
}

/// 文本条目全文（原样不 trim），供展开态显示。不存在 / 非文本条目返回参数错误；无副作用。
#[tauri::command]
pub async fn get_clipboard_text(
    store: State<'_, ClipboardStore>,
    id: i64,
) -> Result<String, AppError> {
    store.get_text(id).await
}

/// 写回剪贴板 → 激活原前台窗口 → 收起面板 → 模拟 Ctrl+V；原窗口不可用时只写回并收起，仍返回 Ok。
///
/// 非 Windows 平台返回「当前平台暂不支持」。
#[tauri::command]
pub async fn paste_clipboard_item<R: Runtime>(
    app: AppHandle<R>,
    store: State<'_, ClipboardStore>,
    id: i64,
) -> Result<(), AppError> {
    clipboard::paste(&app, &store, id).await
}

/// 删除一条（含图片文件）；不存在返回参数错误。
#[tauri::command]
pub async fn delete_clipboard_item(
    store: State<'_, ClipboardStore>,
    id: i64,
) -> Result<(), AppError> {
    clipboard::delete_item(&store, id).await
}

/// 设置 / 取消收藏；只改标记不动 `copied_at`。不存在返回参数错误。
#[tauri::command]
pub async fn set_clipboard_item_favorite(
    store: State<'_, ClipboardStore>,
    id: i64,
    favorite: bool,
) -> Result<(), AppError> {
    store.set_favorite(id, favorite).await
}

fn validate_limit(limit: u32) -> Result<(), AppError> {
    if LIMIT_RANGE.contains(&limit) {
        Ok(())
    } else {
        Err(AppError::InvalidInput(format!(
            "每页条数必须在 {}..={} 之间",
            LIMIT_RANGE.start(),
            LIMIT_RANGE.end()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limit_out_of_range_is_rejected_with_chinese_message() {
        for bad in [0, 201, u32::MAX] {
            let err = validate_limit(bad).expect_err("越界应被拒");
            assert_eq!(err.to_string(), "参数错误: 每页条数必须在 1..=200 之间");
        }
    }

    #[test]
    fn limit_in_range_is_accepted() {
        for ok in [1, 100, 200] {
            assert!(validate_limit(ok).is_ok());
        }
    }
}
