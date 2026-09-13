# Linux 平台支持与适配

## 目标 (Goal)

为 `z-tools` 增加 Linux 平台支持，在代码架构上遵循项目的平台抽象分层规范，在 Linux (X11) 环境下提供与 Windows 完全对齐的启动器和剪贴板历史核心体验，并在 Wayland 环境下提供可靠的优雅降级。

## 背景与已有事实 (Background & Confirmed Facts)

1. **项目规范与分工原则**（参见 `.trellis/spec/backend/directory-structure.md`）：
   - 跨平台能力（arboard 读写剪贴板、SQLite 存储、图片编解码）位于通用模块，无 `#[cfg]` 分支。
   - 平台专属能力（剪贴板监听、前台窗口追踪与激活、模拟粘贴、平台钩子）按平台拆分为 `src/<domain>/{windows,linux}.rs`，父模块通过同名函数与导出结构体进行统一路由。
   - 平台 target 语法约定使用 `#[cfg(target_os = "linux")]`。
2. **Windows 现有平台实现**：
   - `src-tauri/src/clipboard/windows.rs`：`run_monitor`、`stop_monitor`、`send_paste`、`ClipboardWatcher`。
   - `src-tauri/src/launcher/windows.rs`：`current_foreground`、`activate`、`suppress_alt_sysmenu`、`is_cursor_on_notification_chevron`、`PreviousForeground`。
   - `src-tauri/src/lib.rs`：`setup_desktop` 中的平台钩子与前台窗口状态托管，`RunEvent::Exit` 停止监听。
3. **Linux 平台特性与差异**：
   - X11 下具备成熟的 EWMH 协议（`_NET_ACTIVE_WINDOW`）、XFixes 扩展（剪贴板变化通知）、XTest 扩展（按键事件注入）以及窗口绝对坐标定位。
   - Wayland 原生环境具有严格的应用间安全隔离，默认禁止后台剪贴板窃听、跨窗口焦点窃取与全局按键模拟。

## 关键决策 (Key Decisions)

- **D1 (显示协议与兼容器策略)**：采用 **X11 全功能优先 + Wayland 安全降级**。在 X11 / XWayland 环境下提供与 Windows 完全对齐的功能；在纯 Wayland 环境下执行安全降级（内容写入剪贴板后隐藏面板，提示用户手动粘贴，不强行注入模拟按键与切换焦点）。
- **D2 (底层依赖选型)**：选用纯 Rust 实现的 `x11rb` crate（开启 `xfixes` 与 `xtest` feature）。相比 C 绑定的 `x11` 库，`x11rb` 内存安全、零额外 C 头文件依赖、易于交叉编译且性能优异。
- **D3 (平台抽象契约)**：不使用宏或全局单例，沿用现有的 `app.manage` 状态托管模式（如 `PreviousForeground`、`ClipboardWatcher` 在对应平台分别声明与托管）。

## 范围 (Scope)

### 包含 (In Scope)

1. **平台抽象解耦**：
   - 消除 `launcher.rs`、`clipboard.rs` 和 `lib.rs` 中仅面向 Windows 的硬编码。
   - 抽象前台窗口句柄类型（Windows 为 `isize`，Linux 为 X11 `u32` 窗口 ID 或跨平台统一载体）。
2. **Linux 剪贴板平台实现 (`clipboard/linux.rs`)**：
   - 使用 `x11rb` + XFixes 扩展实现后台剪贴板监听器 `run_monitor` 与 `stop_monitor`。
   - 使用 `x11rb` + XTest 扩展实现按键模拟 `send_paste`（模拟 `Ctrl+V`）。
   - 实现 Wayland 环境检测与安全降级。
3. **Linux 启动器平台实现 (`launcher/linux.rs`)**：
   - 基于 EWMH 协议实现 `current_foreground`（查询根窗口 `_NET_ACTIVE_WINDOW`）。
   - 基于 EWMH ClientMessage 实现 `activate`（向根窗口发送激活请求）。
   - 提供符合 Linux 行为的窗口与托盘辅助函数（如空实现的 `suppress_alt_sysmenu`）。
4. **生命周期装配与配置**：
   - 在 `lib.rs` 中适配 Linux 下的 `setup_clipboard`、`setup_desktop` 及 `RunEvent::Exit` 监听退出。
   - `Cargo.toml` 中配置 Linux 目标依赖。

### 不包含 (Out of Scope)

- macOS 平台支持（保留待后续专属任务实现）。
- Wayland 原生环境下通过特权 daemon（如 ydotool、uinput 驱动）强制模拟按键。
- 启动器全局快捷键配置 UI（由全局设置任务单独处理）。

## 验收标准 (Acceptance Criteria)

- [ ] **AC-1 (X11 剪贴板监听)**：在 Linux (X11) 环境下，复制任意文本、图片或文件，后台监听线程能在 100ms 内捕获并触发 `record()` 入库，前端能收到 `clipboard://changed` 广播。
- [ ] **AC-2 (X11 自动粘贴)**：在 Linux (X11) 环境下，从剪贴板历史面板点击或按回车粘贴条目，启动器能成功将焦点交还给原前台窗口，并模拟 `Ctrl+V` 完成粘贴。
- [ ] **AC-3 (Wayland 优雅降级)**：在 Linux (Wayland) 环境下执行粘贴命令时，条目正常写回剪贴板并收起面板，返回成功，不发生 panic 或悬挂。
- [ ] **AC-4 (窗口与托盘)**：在 Linux (X11) 下启动器面板按当前工作区水平居中、顶边 1/4 定位；失焦时自动隐藏；托盘左键点击可正常切换显隐。
- [ ] **AC-5 (进程生命周期)**：应用在 Linux 托盘退出或关闭时，后台 X11 监听连接能够正确关闭并退出线程，无孤儿线程或资源泄漏。
- [ ] **AC-6 (Windows 回归无损)**：Windows 平台所有既有编译、单测与运行时功能保持正常（`cargo clippy`、`cargo test` 全通过）。
- [ ] **AC-7 (代码质量门禁)**：符合项目规范，注释全中文，unsafe 均有前置条件安全说明，无未处理警告。
