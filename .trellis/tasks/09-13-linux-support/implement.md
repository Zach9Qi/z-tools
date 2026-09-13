# Linux 平台支持实现计划 (Implement Plan)

## 实施阶段与步骤清单

### 阶段一：依赖与平台层抽象重构
- [ ] 1.1 在 `src-tauri/Cargo.toml` 中配置 `[target.'cfg(target_os = "linux")'.dependencies]`，引入 `x11rb`（features: `["allow-unsafe-code", "xfixes", "xtest"]`），并添加中文用途注释。
- [ ] 1.2 重构 `src-tauri/src/launcher.rs`：
  - 将 `PreviousForeground` 的 `#[cfg(windows)]` 调整为 `#[cfg(any(windows, target_os = "linux"))]`。
  - 规范化 `current_foreground`、`activate_window`、`suppress_alt_sysmenu` 在 Linux 下的调用映射。
- [ ] 1.3 重构 `src-tauri/src/clipboard.rs`：
  - 将 `paste` 编排中的前台切换与 `send_paste` 解耦，支持 Linux。
  - 将 `ClipboardWatcher`、`run_monitor`、`stop_monitor` 的导出对 Linux 开放。
- [ ] 1.4 重构 `src-tauri/src/lib.rs`：
  - 在 `setup_clipboard` 与 `setup_desktop` 中对 Linux 平台开放监听与前台状态托管。
  - 在 `RunEvent::Exit` 中针对 Linux 触发 `stop_monitor`。

### 阶段二：Linux 平台实现
- [ ] 2.1 编写 `src-tauri/src/launcher/linux.rs`：
  - 实现基于 EWMH `_NET_ACTIVE_WINDOW` 的当前前台窗口获取与窗口激活。
  - 实现平台钩子 `suppress_alt_sysmenu`（no-op）与托盘角标过滤（返回 `false`）。
- [ ] 2.2 编写 `src-tauri/src/clipboard/linux.rs`：
  - 实现基于 `x11rb` + XFixes 扩展的 `run_monitor` 与 `stop_monitor` 监听循环。
  - 实现基于 `x11rb` + XTest 扩展的 `send_paste`（模拟 `Ctrl+V`）。
  - 实现 Wayland 环境检测及安全降级。

### 阶段三：质量门禁与跨平台验证
- [ ] 3.1 运行 Windows 本地质量门禁：
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
- [ ] 3.2 运行 Linux 目标交叉检查（或代码语法/类型跨平台校验）：
  - `cargo check --target x86_64-unknown-linux-gnu`（如已安装 target）
  - 确认 Windows 下现有测试用例无破坏。

---

## 验证命令 (Validation Commands)

```bash
# 格式检查
cargo fmt --check

# 全目标代码规范检查（必须 0 告警）
cargo clippy --all-targets -- -D warnings

# 单元测试与集成测试
cargo test

# Linux 交叉编译检查（如果本地安装了目标工具链）
rustup target add x86_64-unknown-linux-gnu 2>/dev/null || true
cargo check --target x86_64-unknown-linux-gnu
```

---

## 风险文件与回滚锚点 (Risky Files & Rollback Points)

- **核心改动文件**：
  - `src-tauri/src/lib.rs`
  - `src-tauri/src/launcher.rs`
  - `src-tauri/src/clipboard.rs`
  - `src-tauri/Cargo.toml`
- **新建文件**：
  - `src-tauri/src/launcher/linux.rs`
  - `src-tauri/src/clipboard/linux.rs`
- **回滚点**：
  - 若架构重构影响 Windows 行为，可直接通过 `git checkout -- src-tauri/src/lib.rs src-tauri/src/launcher.rs src-tauri/src/clipboard.rs` 还原。
