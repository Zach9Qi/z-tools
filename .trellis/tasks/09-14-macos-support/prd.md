# macOS 平台支持与适配

## 目标 (Goal)

为 `z-tools` 增加 macOS 平台支持，遵循项目的平台抽象分层规范（`.trellis/spec/backend/directory-structure.md`），在 macOS 环境下提供与 Windows / Linux (X11) 对齐的启动器窗口管理和剪贴板历史核心功能，并在未获系统辅助功能权限时提供优雅降级。

## 背景与已有事实 (Background & Confirmed Facts)

1. **项目规范与分工原则**（参见 `.trellis/spec/backend/directory-structure.md`）：
   - 跨平台能力（arboard 读写剪贴板、SQLite 存储、图片编解码）位于通用模块，无 `#[cfg]` 分支。
   - 平台专属能力（剪贴板监听、前台窗口/应用追踪与激活、模拟粘贴、平台钩子）按平台拆分为 `src/<domain>/{windows,linux,macos}.rs`，父模块通过同名函数与导出结构体进行统一路由。
   - 平台 target 语法约定使用 `#[cfg(target_os = "macos")]`。
2. **已有平台实现**：
   - Windows：`src-tauri/src/clipboard/windows.rs`、`src-tauri/src/launcher/windows.rs`。
   - Linux：`src-tauri/src/clipboard/linux.rs`、`src-tauri/src/launcher/linux.rs`。
   - 平台抽象：`launcher.rs` 中通过 `PreviousForeground`（`AtomicIsize`）记录前台焦点，`clipboard.rs` 中通过 `ClipboardWatcher` 托管监听器退出句柄，`paste()` 编排通用三步（写回 -> 激活前台 -> 隐藏面板 -> 模拟按键）。
3. **macOS 平台特性与机制**：
   - **剪贴板监听**：macOS 缺乏全局剪贴板变更通知系统事件。业界方案（Raycast、Alfred、Maccy、Clipy 等）普遍通过后台轻量轮询 `NSPasteboard.generalPasteboard.changeCount` 实现（单次仅为内存/轻量 IPC 整数读取，开销极低）。
   - **前台焦点追踪与切回**：macOS 前台对象为应用程序（`NSRunningApplication`），通过 `NSWorkspace.sharedWorkspace.frontmostApplication` 获取其 PID 并存入 `PreviousForeground`；切回时通过 `NSRunningApplication.runningApplicationWithProcessIdentifier(pid).activateWithOptions(...)` 激活。
   - **模拟按键粘贴与权限**：macOS 下通过 CoreGraphics `CGEventCreateKeyboardEvent` + `CGEventPost` 模拟按下与释放 `Cmd+V`。跨进程注入按键受系统辅助功能（Accessibility）权限控制（`AXIsProcessTrusted`）。

## 关键决策 (Key Decisions)

- **D1 (剪贴板监听机制)**：采用 `NSPasteboard` 的 `changeCount` 后台轻量轮询（250~300ms 间隔）。在后台 `spawn_blocking` 线程中监听，退出时通过原子状态唤醒并平稳退出。
- **D2 (前台应用标识)**：在 `PreviousForeground` 中以 `pid_t as isize` 存放前台应用进程 ID，统一抹平与 Windows HWND、Linux XID 的平台差异。
- **D3 (模拟按键与辅助功能权限策略)**：采用 **优雅降级模式**。在调用 `send_paste` 时检测权限；若尚未获得系统辅助功能权限，不弹窗打扰用户，记录提示日志并直接将条目写回剪贴板、切回原应用并收起面板（用户手动按一次 `Cmd+V` 即可粘贴）；当用户在系统隐私设置中授权后，自动生效完整按键模拟。
- **D4 (依赖选型)**：利用已在 `Cargo.lock` 中随 Tauri 间接引入的 `objc2`、`objc2-app-kit`、`objc2-foundation` 与 `core-graphics` crate，避免引入额外未经审计的重量级第三方库。
- **D5 (全局快捷键)**：遵循现有单一来源常量 `DEFAULT_TOGGLE_SHORTCUT = "alt+enter"`（在 macOS 下 Tauri 自动映射为 `Option+Enter`）。

## 范围 (Scope)

### 包含 (In Scope)

1. **平台抽象解耦**：
   - 在 `launcher.rs`、`clipboard.rs` 和 `lib.rs` 中引入 `#[cfg(target_os = "macos")]`。
   - 将 `PreviousForeground` 与 `ClipboardWatcher` 条件编译扩展为覆盖 macOS。
2. **macOS 启动器实现 (`src-tauri/src/launcher/macos.rs`)**：
   - 基于 `NSWorkspace` 实现 `current_foreground`（获取前台应用 PID）。
   - 基于 `NSRunningApplication` 实现 `activate`（切回原应用）。
   - 平台钩子 `suppress_alt_sysmenu`（no-op）与 `is_cursor_on_notification_chevron`（返回 `false`）。
3. **macOS 剪贴板实现 (`src-tauri/src/clipboard/macos.rs`)**：
   - 基于 `NSPasteboard.changeCount` 实现 `run_monitor` 与 `stop_monitor`。
   - 基于 `CGEventPost` 实现 `send_paste`（模拟 `Cmd+V`）及未授权优雅降级。
   - 复用通用跨平台 `backend::write`。
4. **生命周期装配与构建配置**：
   - 在 `src-tauri/src/lib.rs` 中装配 macOS 的 `setup_clipboard`、`setup_desktop` 与 `RunEvent::Exit`。
   - 在 `src-tauri/Cargo.toml` 中配置 `[target.'cfg(target_os = "macos")'.dependencies]`。

### 不包含 (Out of Scope)

- 启动器全局快捷键配置 UI（由全局设置任务单独处理）。
- 特殊的 macOS 原生菜单栏增强或原生自绘面板。

## 验收标准 (Acceptance Criteria)

- [ ] **AC-1 (macOS 剪贴板监听)**：在 macOS 环境下，复制任意文本、图片或文件，后台监听线程能在 300ms 内捕获并触发 `record()` 入库，前端能收到 `clipboard://changed` 广播。
- [ ] **AC-2 (macOS 原应用激活与自动粘贴)**：在 macOS 环境下，从剪贴板历史面板点击或按回车粘贴条目，启动器能成功将焦点交还给原前台应用，并在具备权限时模拟 `Cmd+V` 完成粘贴。
- [ ] **AC-3 (macOS 权限优雅降级)**：在未授予辅助功能权限时执行粘贴命令，条目正常写回剪贴板、原应用正常激活并收起面板，返回成功，不发生 panic 或悬挂。
- [ ] **AC-4 (窗口与托盘)**：在 macOS 下启动器面板按当前工作区水平居中、顶边 1/4 定位；失焦时自动隐藏；托盘左键点击可正常切换显隐。
- [ ] **AC-5 (进程生命周期)**：应用在 macOS 托盘退出或关闭时，后台剪贴板监听线程能够平稳退出，无孤儿线程或资源泄漏。
- [x] **AC-6 (跨平台回归无损)**：Windows 与 Linux 平台既有代码、单测与门禁不受影响（`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test` 全通过）。
- [x] **AC-7 (代码质量门禁)**：符合项目规范，注释全中文，unsafe 均有前置条件安全说明，无未处理警告。
