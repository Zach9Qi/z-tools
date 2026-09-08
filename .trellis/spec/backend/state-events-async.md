# 状态、事件与异步

> 本仓库目前没有托管状态、没有事件、没有异步命令;`lib.rs` 的 setup 注释已预留位置。以下是引入时的约定。

---

## 1. 托管状态(`app.manage`)

- 需要跨命令共享的对象(配置、数据库连接池、后台任务句柄)在 `lib.rs` 的 `setup` 里 `app.manage(XxxState::new(...))`,命令用 `State<'_, XxxState>` 或 `app.state::<XxxState>()` 取;不用全局 `static APP_HANDLE`。
- **一个职责一个状态类型**(如 `DatabaseState` / `SettingsStore` / `WindowStateStore`),不要一个大 `AppData` 装一切。
- 状态类型放领域模块里(`src/settings.rs` 里定义 `SettingsStore`),不放 `lib.rs`。
- setup 里的初始化顺序有依赖时,用注释写明「A 必须在 B 之前」。

### 锁选型

| 情况 | 用 | 原因 |
|---|---|---|
| 锁内只做短同步操作,不跨 `.await` | `std::sync::Mutex` / `RwLock` | 最简单,无额外依赖 |
| 必须跨 `.await` 持有(如异步连接池) | `tokio::sync::Mutex` | std 锁跨 await 会阻塞运行时;可用 `clippy::await_holding_lock = deny` 机制化禁止 |
| 只写一次、之后只读的全局 | `std::sync::OnceLock` / `LazyLock` | 标准库已提供,无需 `once_cell` / `lazy_static` |

- 不引入 `parking_lot` 与 `lazy_static`。
- 原子计数用 `AtomicXxx`,不用 Mutex 包整数。

## 2. 事件(Rust → 前端)

```rust
use tauri::Emitter; // emit 来自 Emitter trait

/// 设置更新后广播;前端 `src/lib/events.ts` 的 EVENTS.SETTINGS_UPDATED 与此一一对应
pub const SETTINGS_UPDATED: &str = "settings://updated";

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsUpdated<'a> {
    pub key: &'a str,
}

if let Err(e) = app.emit(SETTINGS_UPDATED, SettingsUpdated { key }) {
    log::warn!("发送 {SETTINGS_UPDATED} 事件失败: {e}");
}
```

- 事件名 `domain://action`,定义为 `pub const` 放在 **emit 点所在的领域模块**,注释指向前端镜像常量。不写字面量到 `emit` 里。
- payload 是独立结构体:`#[derive(Debug, Clone, Serialize)]` + `rename_all = "camelCase"`;可借用字段避免 clone(如上例 `SettingsUpdated<'a>`)。有多种形态时用 `#[serde(tag = "type")]` 的枚举。
- payload 保持小:只发「什么变了」,前端需要完整数据时用现有命令再拉,避免 payload 长成第二份数据模型。
- `emit` 失败只 `log::warn!`,不 `unwrap`、不让命令因此失败。
- 只发给某个窗口用 `emit_to("main", …)`;全局广播用 `emit`。
- **流式 / 高频 / 需要顺序**的数据(下载进度、日志尾随)用 `tauri::ipc::Channel<T>` 作为命令参数,不用事件(官方文档:事件系统不为高吞吐设计)。
- macOS 上从 runtime worker 线程 emit 可能与 WebKit 死锁;若在后台线程大量 emit,通过 `app.run_on_main_thread` 派发。

## 3. 异步

- 运行时用 Tauri 内置的:`tauri::async_runtime::spawn` / `spawn_blocking`;不要混用 `tokio::spawn`。
- 直接依赖 `tokio` 时按需列 features(`["sync", "time", "macros"]`),不用 `full`。
- `block_on` 只允许出现在 `setup` 里做一次性初始化;命令与领域层禁止。
- 阻塞操作(大文件读写、图片解码、同步系统 API)放 `spawn_blocking`;`std::thread::sleep` 换 `tokio::time::sleep`。
- 非 `Send` 的系统句柄(剪贴板、窗口原生指针)在同步块内用完再 `.await`,不要让它跨 await。
- 后台长任务(监听器、定时器)在 setup 里 spawn,句柄存进托管状态以便退出时停止;不要用裸线程 `std::thread::spawn` 起后台循环。

## 4. 前端 → Rust 的事件

前端 `emit` 到 Rust(`app.listen`)只用于窗口生命周期之类的通知;需要返回值或需要错误处理的一律用命令。不用事件模拟请求-响应。

## 5. 禁止

- `static APP_HANDLE`(用 `AppHandle` 参数 / 托管状态)。
- std `Mutex` 跨 `.await`。
- 事件名字面量散落在 emit 点;事件名与前端常量不一致。
- `emit(...).unwrap()`。
- 高频数据走事件。
- `tokio::spawn`、`tokio = { features = ["full"] }`。
