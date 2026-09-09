# 状态、事件与异步

> 现状(剪贴板工具落地后):
>
> | 类别 | 现例 | 位置 |
> |---|---|---|
> | 托管状态 | `ClipboardStore { pool: SqlitePool, images_dir: PathBuf }`;`#[cfg(windows)] ClipboardWatcher { hwnd: AtomicIsize }`;`#[cfg(windows)] PreviousForeground(AtomicIsize)` | `clipboard.rs` / `clipboard/windows.rs` / `launcher.rs`,均在 `lib.rs::setup_desktop` 里 `app.manage` |
> | 事件 | `launcher://open` / `launcher://close`(`launcher.rs`);`clipboard://changed`(`clipboard.rs::CLIPBOARD_CHANGED`) | 全部无 payload,emit `()`;前端 `src/lib/events.ts` 镜像 |
> | 后台任务 | `spawn_blocking(run_monitor)` 消息循环,`RunEvent::Exit` 时 `stop_monitor(hwnd)` | `lib.rs` / `clipboard/windows.rs` |
> | async 命令 | `commands/clipboard.rs` 6 个(`State<'_, ClipboardStore>` + sqlx) | 见 `command-guidelines.md` |
>
> 以下是引入新状态 / 事件 / 后台任务时的约定,现例作为样板。

---

## 1. 托管状态(`app.manage`)

- 需要跨命令共享的对象(配置、数据库连接池、后台任务句柄)在 `lib.rs` 的 `setup_desktop` / `setup_xxx` 里 `app.manage(XxxState::new(...))`,命令用 `State<'_, XxxState>` 或 `app.state::<XxxState>()` 取;不用全局 `static APP_HANDLE`。
- **一个职责一个状态类型**:`ClipboardStore`(连接池 + 图片目录)、`ClipboardWatcher`(监听窗口句柄)、`PreviousForeground`(唤出前的前台窗口)是三个类型,不合成一个 `AppData`。
- 状态类型放领域模块里(`ClipboardStore` 在 `clipboard.rs`,`PreviousForeground` 在 `launcher.rs`),不放 `lib.rs`。
- setup 里的初始化顺序有依赖时,用注释写明「A 必须在 B 之前」(`setup_desktop` 的 1~6 步编号注释;顺序理由见 `config-and-permissions.md` §8)。
- 状态要被后台任务持有时,让它 `Clone` 且只克隆 Arc 级句柄(`ClipboardStore` 派生 `Clone`:`SqlitePool` 本身是 Arc;`on_clipboard_update` 里 `store.inner().clone()` 后 move 进 `spawn`)。
- 在可能尚未托管的路径(监听线程、`RunEvent::Exit`)用 `app.try_state::<T>()` 而不是 `state()`:后者在未托管时 panic。
- 平台专属状态整个类型带 `#[cfg(windows)]`(`ClipboardWatcher` / `PreviousForeground`),`manage` 处同样 cfg;不为其他平台造空状态。
- 原生句柄(`HWND`)存进状态时转成 `isize` 放 `AtomicIsize`:`HWND` 是裸指针不 `Send`,而托管状态必须 `Send + Sync`;0 表示「未记录 / 已退出」。

### 锁选型

| 情况 | 用 | 原因 |
|---|---|---|
| 锁内只做短同步操作,不跨 `.await` | `std::sync::Mutex` / `RwLock` | 最简单,无额外依赖 |
| 必须跨 `.await` 持有(如异步连接池) | `tokio::sync::Mutex` | std 锁跨 await 会阻塞运行时;可用 `clippy::await_holding_lock = deny` 机制化禁止 |
| 只写一次、之后只读的全局 | `std::sync::OnceLock` / `LazyLock` | 标准库已提供,无需 `once_cell` / `lazy_static` |
| 单个整数 / 句柄 | `AtomicIsize` 等(现例 `ClipboardWatcher.hwnd`、`PreviousForeground.0`,`Ordering::SeqCst`) | 不用 Mutex 包整数 |

- 不引入 `parking_lot` 与 `lazy_static`。
- sqlx 的 `SqlitePool` 自带并发控制,不要再包一层 Mutex。

## 2. 事件(Rust → 前端)

```rust
use tauri::Emitter; // emit 来自 Emitter trait

// 现例(clipboard.rs):无 payload 事件
/// 监听器录入新内容或上浮旧内容后广播;前端 `src/lib/events.ts` 的 `EVENTS.CLIPBOARD_CHANGED` 与此一一对应,无 payload
pub const CLIPBOARD_CHANGED: &str = "clipboard://changed";

if let Err(e) = app.emit(CLIPBOARD_CHANGED, ()) {
    log::warn!("发送 {CLIPBOARD_CHANGED} 事件失败: {e}");
}

// 示意:将来带 payload 的事件
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsUpdated<'a> { pub key: &'a str }
```

- 事件名 `domain://action`,定义为 `pub const` 放在 **emit 点所在的领域模块**,注释指向前端镜像常量。不写字面量到 `emit` 里。现实样板:`launcher.rs` 的 `LAUNCHER_OPENED` / `LAUNCHER_CLOSED` 在 `show()` / `hide()` 内 emit;`clipboard.rs` 的 `CLIPBOARD_CHANGED` 在 `record()` 内 emit。
- **谁 emit:只有前端无法自知的变化才发事件,且只在领域层发。** `clipboard://changed` 只由 `record()`(监听器录入)发出;`delete_item` / `set_favorite` 与对应命令**不 emit**。理由:这两种变化的发起者就是前端自己,单窗口应用里发命令的前端就是唯一消费者,命令返回 `Ok` 它就知道结果并直接改本地列表;再 emit 会造成「本地已改 + 收到事件重拉」双重刷新,还会丢掉已加载的分页与滚动位置。反过来,监听器录入(含粘贴写回后的回捕上浮)是前端不可能自知的,所以发。新增事件前先问:前端是不是本来就知道?
- payload 是独立结构体:`#[derive(Debug, Clone, Serialize)]` + `rename_all = "camelCase"`;可借用字段避免 clone(如上例 `SettingsUpdated<'a>`)。有多种形态时用 `#[serde(tag = "type")]` 的枚举(struct 变体还要 `rename_all_fields = "camelCase"`,见 `../guides/ipc-contract.md`)。
- **无 payload 的事件 emit `()`**(序列化为 `null`),前端 `EventPayloads` 对应类型写 `null`;不为此造空结构体——空结构体除了多一个名字要维护,对前端没有任何信息增量。将来真需要带数据时再改成结构体,两侧同一 PR 改。
- payload 保持小:只发「什么变了」,前端需要完整数据时用现有命令再拉,避免 payload 长成第二份数据模型。`clipboard://changed` 甚至不带新条目:前端无法复现后端的 `LIKE` / kind / favorite 筛选来判断该不该插入,重拉首页是唯一不会错的做法。
- `emit` 失败只 `log::warn!`,不 `unwrap`、不让命令因此失败。
- 只发给某个窗口用 `emit_to("main", …)`;全局广播用 `emit`。
- **流式 / 高频 / 需要顺序**的数据(下载进度、日志尾随)用 `tauri::ipc::Channel<T>` 作为命令参数,不用事件(官方文档:事件系统不为高吞吐设计)。
- macOS 上从 runtime worker 线程 emit 可能与 WebKit 死锁;若在后台线程大量 emit,通过 `app.run_on_main_thread` 派发。

## 3. 异步与后台任务

- 运行时用 Tauri 内置的:`tauri::async_runtime::spawn` / `spawn_blocking`;不要混用 `tokio::spawn`。
- 直接依赖 `tokio` 时按需列 features,不用 `full`。当前只有 `[dev-dependencies] tokio = { features = ["macros", "rt"] }` 供 `#[tokio::test]`;运行时代码不直接依赖 tokio。
- `block_on` 只允许出现在 `setup` 里做一次性初始化(现例 `setup_clipboard` 里 `block_on(ClipboardStore::open(...))`);命令与领域层禁止。
- 阻塞操作(大文件读写、图片解码、同步系统 API)放 `spawn_blocking`;`std::thread::sleep` 换 `tokio::time::sleep`。**例外**:`spawn_blocking` 闭包内部本来就是阻塞线程,允许 `std::thread::sleep`(`paste()` 里等 50ms 再 `send_paste`、`backend::with_retry` 的 50ms 重试间隔)。
- **非 `Send` 的系统句柄在同步块内用完**,不要让它跨 await。现例 `arboard::Clipboard`:
  - 采集路径:`on_clipboard_update` 在监听线程同步调 `backend::read_snapshot()`(`Clipboard::new()` → 读 → drop 全在函数内),得到 `Captured`(纯数据,`Send`)后才 `spawn(async { record(...).await })`;
  - 写回路径:`paste()` 里 `spawn_blocking(move || backend::write(&captured)).await??`,`Clipboard` 只活在闭包内。
  - `backend.rs` 因此全是同步函数,模块文档写明「调用方放在 `spawn_blocking` 或监听线程里用完即丢」。
- **后台长任务的固定形态**(现例 `clipboard/windows.rs::run_monitor`):
  1. 在 `setup_xxx` 里 `app.manage(XxxWatcher::default())` 先托管句柄容器;
  2. `tauri::async_runtime::spawn_blocking(move || run_monitor(handle))` 启动(消息窗口必须在创建它的线程上跑阻塞的 `GetMessageW` 循环,所以是 `spawn_blocking` 而不是 `spawn`);
  3. 任务启动后把可用于停止它的句柄写回状态(`watcher.hwnd.store(...)`),退出时清零;
  4. 任务内部需要做异步工作(写库)时 `tauri::async_runtime::spawn` 回到运行时,**不在阻塞线程里 `block_on`**;
  5. 停止入口是一个 `pub fn stop_monitor(hwnd)`(`PostMessageW(WM_CLOSE)`,由任务线程自己清理),在 `lib.rs` 的 `.run(|app, event| if let RunEvent::Exit = event { … })` 里调用——`Builder` 因此用 `.build()?.run(...)` 而不是链式 `.run(ctx)`,才能覆盖托盘退出等所有退出路径;
  6. 任务启动失败(创建窗口 / 注册监听失败)`log::error!` 后线程直接返回,不 panic;功能降级为「历史不再录入」,其余命令照常。
- 不要用裸线程 `std::thread::spawn` 起后台循环;不要把句柄放在 `static`。

## 4. 前端 → Rust 的事件

前端 `emit` 到 Rust(`app.listen`)只用于窗口生命周期之类的通知;需要返回值或需要错误处理的一律用命令。不用事件模拟请求-响应。

## 5. 禁止

- `static APP_HANDLE`(用 `AppHandle` 参数 / 托管状态)。
- std `Mutex` 跨 `.await`。
- 事件名字面量散落在 emit 点;事件名与前端常量不一致。
- `emit(...).unwrap()`。
- 命令层为「前端自己发起的变更」emit 事件(前端已知结果,直接改本地)。
- 高频数据走事件。
- `tokio::spawn`、`tokio = { features = ["full"] }`。
- 在 `spawn_blocking` 线程或事件回调里 `block_on`。
- 让 `arboard::Clipboard` / `HWND` 等非 `Send` 值跨 `.await` 或存进托管状态(转 `isize` 存)。
