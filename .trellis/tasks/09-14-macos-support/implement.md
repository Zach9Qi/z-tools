# macOS 平台支持实现计划 (Implement Plan)

## 实施阶段与步骤清单

### 阶段一：依赖与平台层抽象对齐
- [x] 1.1 在 `src-tauri/Cargo.toml` 中配置 `[target.'cfg(target_os = "macos")'.dependencies]`，引入 `objc2`、`objc2-app-kit`、`objc2-foundation` 与 `core-graphics`，并添加规范的中文选型注释。
- [x] 1.2 重构 `src-tauri/src/launcher.rs`：
  - 将 `PreviousForeground` 的 `#[cfg(any(windows, target_os = "linux"))]` 扩展为 `#[cfg(any(windows, target_os = "linux", target_os = "macos"))]`。
  - 在 `activate_window`、`remember_foreground` 中引入 macOS 分支路由。
- [x] 1.3 重构 `src-tauri/src/clipboard.rs`：
  - 将 `paste` 编排、`ClipboardWatcher` 与监听函数导出向 macOS 开放。
  - 在 `platform_write` 与 `platform_send_paste` 中接入 macOS 分支。
- [x] 1.4 重构 `src-tauri/src/lib.rs`：
  - 在 `setup_clipboard`、`setup_desktop` 与 `RunEvent::Exit` 中将条件编译扩展至包含 macOS。

### 阶段二：macOS 平台实现代码
- [x] 2.1 编写 `src-tauri/src/launcher/macos.rs`：
  - 实现基于 `NSWorkspace` 的前台应用 PID 获取 `current_foreground`。
  - 实现基于 `NSRunningApplication` 的原应用激活 `activate`。
  - 实现自身进程过滤 `is_own_process`。
  - 实现平台钩子 `suppress_alt_sysmenu`（no-op）与 `is_cursor_on_notification_chevron`（返回 `false`）。
- [x] 2.2 编写 `src-tauri/src/clipboard/macos.rs`：
  - 实现基于 `NSPasteboard` 的 `changeCount` 轻量轮询监听循环 `run_monitor` 与退出控制 `stop_monitor`。
  - 实现基于 `CGEventPost` 的 `Cmd+V` 按键模拟 `send_paste`。
  - 实现未授予辅助功能权限时的优雅降级（记日志，返回 `Ok`，不阻塞粘贴流程）。

### 阶段三：质量门禁与跨平台验证
- [x] 3.1 运行本地质量门禁：
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
- [x] 3.2 运行跨平台语法与类型检查（确认无编译与语法破坏）。
  - Windows 宿主全绿；`aarch64-apple-darwin` 交叉检查因本机无 Apple SDK / `cc` 未能完成链接（预期，需在 macOS 上验证运行时 AC）。
- [x] 3.3 回写文档与规范（若有平台适配总结）。
  - 已更新 `.trellis/spec/backend/directory-structure.md` 与 `state-events-async.md` 纳入 macOS 平台文件与 cfg。

---

## 验证命令 (Validation Commands)

```bash
# 代码格式检查
cargo fmt --check

# 全目标代码规范检查（必须 0 告警）
cargo clippy --all-targets -- -D warnings

# 单元测试与集成测试
cargo test

# macOS 目标语法检查（若本地安装了 target）
rustup target add aarch64-apple-darwin 2>/dev/null || true
cargo check --target aarch64-apple-darwin 2>/dev/null || true
```

---

## 风险文件与回滚锚点 (Risky Files & Rollback Points)

- **核心改动文件**：
  - `src-tauri/src/lib.rs`
  - `src-tauri/src/launcher.rs`
  - `src-tauri/src/clipboard.rs`
  - `src-tauri/Cargo.toml`
- **新建文件**：
  - `src-tauri/src/launcher/macos.rs`
  - `src-tauri/src/clipboard/macos.rs`
- **回滚点**：
  - 若抽象调整影响到 Windows / Linux 行为，可通过 `git checkout -- src-tauri/src/lib.rs src-tauri/src/launcher.rs src-tauri/src/clipboard.rs src-tauri/Cargo.toml` 立即还原。
