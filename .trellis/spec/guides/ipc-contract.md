# IPC 契约(前后端共同遵守)

> 前端 `src/lib/api/<domain>.ts`(由 `api/index.ts` 汇出)↔ Rust `src-tauri/src/commands/<domain>.rs` 之间的规则汇总。两侧细节分别见 `../frontend/ipc-guidelines.md` 与 `../backend/command-guidelines.md`;本文件只写「两边必须一致」的部分与变更清单。

---

## 1. 命名对照

| 概念 | Rust 侧 | 前端侧 | 例子 |
|---|---|---|---|
| 命令名 | snake_case 函数名 | `invoke("同名")`;封装函数名为其 camelCase | `hide_launcher` ↔ `hideLauncher()`(`commands/launcher.rs` ↔ `src/lib/api/launcher.ts`);`list_clipboard_items` ↔ `listClipboardItems()`(`commands/clipboard.rs` ↔ `src/lib/api/clipboard.ts`) |
| 参数 | snake_case 形参 | camelCase key,Tauri 自动映射 | `fn f(store, id: i64, favorite: bool)` ↔ `invoke("…", { id, favorite })`;`State` 参数前端不传 |
| 结构体字段 | snake_case + `#[serde(rename_all = "camelCase")]` | camelCase interface 字段 | `copied_at` ↔ `copiedAt`(`ListCursor`) |
| 入参结构体 | 附加 `#[serde(default)]`,`Option` 字段缺省 `None` | 可选字段写 `?:`,省略即可 | `ListQuery { kind: Option<ClipboardKind>, before: Option<ListCursor>, … }` ↔ `{ kind?: ClipboardKind; before?: ListCursor }` |
| 枚举 | `#[serde(rename_all = "camelCase")] enum Kind { Text }` | `type Kind = "text"` | `ClipboardKind { Text, Image, Files }` ↔ `"text" \| "image" \| "files"` |
| 带数据的枚举 | `#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]` | 判别联合 `{ kind: "…" }`,TS 侧拆成 `XxxTextItem \| XxxImageItem \| …` 并抽公共 `Base` | `ClipboardItem::Text { id, copied_at, preview, … }` ↔ `{ kind: "text"; id; copiedAt; preview; … }`(`src/types/clipboard.ts`) |
| `Option<T>` | `None` | `null`(不是 `undefined`) | — |
| 事件名 | `pub const X: &str = "domain://action"` | `EVENTS.X = "domain://action"` | `launcher://open`(`launcher.rs::LAUNCHER_OPENED`)、`clipboard://changed`(`clipboard.rs::CLIPBOARD_CHANGED` ↔ `EVENTS.CLIPBOARD_CHANGED`) |
| 无 payload 事件 | `app.emit(X, ())` | `EventPayloads[X]: null` | `launcher://open` / `launcher://close` / `clipboard://changed` |
| 错误 | `AppError` → 中文字符串 | `catch (error)` → `String(error)` | `"参数错误: 记录不存在"`、`"当前平台暂不支持: 剪贴板粘贴"` |
| 本地文件路径 | 返回绝对路径字符串(`image_path` / `thumb_path`) | `convertFileSrc(path)` → `asset://` URL,**只在 `src/lib/api/**`** | `toAssetUrl()`(`src/lib/api/clipboard.ts`);需 `tauri.conf.json5` `assetProtocol.scope` 覆盖该目录 |

> **Warning(本任务测试抓到)**:`#[serde(tag = "kind", rename_all = "camelCase")]` 在**枚举**上只改变体名(`Text` → `"text"`),**struct 变体的字段名不受影响**——`copied_at` 会原样输出,前端读 `copiedAt` 得到 `undefined` 且无报错。带字段的 tagged 枚举必须同时写 `rename_all_fields = "camelCase"`(`clipboard.rs::ClipboardItem` 已如此并有注释),并用一条序列化测试锁定完整 JSON 字符串(`item_serializes_with_kind_tag` 断言 `{"kind":"files","id":1,…,"copiedAt":2,…}`)。
>
> 普通 struct 上 `rename_all = "camelCase"` 就够;只有枚举需要两个属性。

## 2. 责任边界

- **信任边界在 Rust 命令层**:前端输入不可信,校验放命令入口;Rust 返回的数据前端不二次校验。
- **Rust 是数据真相**:持久化、系统交互、业务规则都在 Rust;前端 store 只镜像与缓存。
- **文案由 Rust 产出**:错误文案是完整中文句子,前端不拼前缀。
- **降级由前端负责**:每个 `src/lib/api/**` 封装函数处理非 Tauri 环境,Rust 不知道浏览器预览的存在。预览假数据要像真实 IPC 一样返回**全新对象**(`structuredClone`),不要把可变内存表的对象直接交出去(见 `../frontend/ipc-guidelines.md` §4)。
- **谁发事件**:前端自己发起的变更(删除 / 收藏)命令返回 `Ok` 就是结果,前端直接改本地,Rust 不 emit;只有前端无法自知的变化(监听器录入)才发事件,且不带数据,前端重拉(`clipboard://changed`)。
- **类型镜像手写**:`src/types/<domain>.ts` 头部注明对应 Rust 路径;Rust 改字段,同一个 PR 改 TS。

## 3. 新增 / 改名一个命令

| # | 位置 | 动作 |
|---|---|---|
| 1 | `src-tauri/src/commands/<domain>.rs` | `#[tauri::command] pub fn xxx(...) -> Result<T, AppError>`(不可失败则直接 `T`),`///` 文档 |
| 2 | `src-tauri/src/commands.rs` | 新领域时 `pub mod <domain>;` |
| 3 | `src-tauri/src/lib.rs` | `generate_handler![..., commands::<domain>::xxx]` |
| 4 | `src-tauri/src/error.rs` | 需要新失败类别时加变体 + 文案测试 |
| 5 | `src/lib/api/<domain>.ts` | 同名 camelCase 封装,`invoke<T>` 泛型,JSDoc;新领域时在 `src/lib/api/index.ts` 加 `export * from "@/lib/api/<domain>"` |
| 6 | `src/lib/api/<domain>.ts` | 非 Tauri 分支(假数据 / no-op / reject);假数据返回前 `structuredClone` |
| 7 | `src/types/<domain>.ts` | 返回结构体的 TS 镜像(如有);tagged 枚举确认 Rust 侧有 `rename_all_fields` |
| 8 | 测试 | Rust `#[cfg(test)]` 校验路径;TS `lib/` 纯逻辑测试(如有) |
| 9 | 改名时 | `grep -rn "旧名" src src-tauri`,确认无残留 |

## 4. 新增一个事件(Rust → 前端)

| # | 位置 | 动作 |
|---|---|---|
| 1 | Rust 领域模块 | `pub const XXX: &str = "domain://action";`,注释指向前端常量 |
| 2 | 同处 | payload 结构体 `#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]`;**无 payload 则 emit `()`**,不造空结构体 |
| 3 | emit 点 | `use tauri::Emitter;` 后 `if let Err(e) = app.emit(XXX, payload) { log::warn!(...) }` |
| 4 | `src/lib/events.ts` | `EVENTS.XXX` 常量 + `EventPayloads[...]` 类型(无 payload 写 `null`) |
| 5 | 消费方组件 / composable | `useTauriEvent(EVENTS.XXX, handler)`(`src/composables/useTauriEvent.ts`),不手写 `listen` |
| 6 | 评估 | payload 是否够小?是否应改用 `Channel`(流式 / 高频)? |

## 5. 版本兼容

- 入参结构体 `#[serde(default)]`,新增字段给默认值,老前端不崩。
- 删除或改名字段视为破坏性变更,两侧同一 PR 完成并在 PR 描述里列出。
- 大 payload 可带 `schemaVersion` 字段,前端校验不匹配时报错而不是静默渲染;当前项目无此需要,payload 长大再加。

## 6. 快速自检

```bash
# 在 Git Bash / POSIX shell 下执行(仓库 CI 也统一用 bash)
# 前端只有 src/lib/api/**/*.ts 引用 invoke / convertFileSrc
grep -rn "@tauri-apps/api/core" src --include=*.ts --include=*.vue
# 事件 API 只在 src/composables/useTauriEvent.ts;窗口 API 只在 src/lib/window.ts
grep -rn "@tauri-apps/api/event" src --include=*.ts --include=*.vue
grep -rn "@tauri-apps/api/window" src --include=*.ts --include=*.vue
# 事件名两侧一致:每条的字面量集合应相同(新领域事件再加一条)
grep -rn "launcher://" src/lib/events.ts src-tauri/src/launcher.rs
grep -rn "clipboard://" src/lib/events.ts src-tauri/src/clipboard.rs
# 带字段的 tagged 枚举都要有 rename_all_fields:逐个对照下面两条的命中文件
grep -rn -B3 'tag = "' src-tauri/src
grep -rn 'rename_all_fields' src-tauri/src
# 对照命令定义与注册列表
grep -rn -A2 "tauri::command" src-tauri/src/commands/ | grep "pub .*fn"
grep -n -A20 "generate_handler" src-tauri/src/lib.rs
# 没有 Result<_, String>
grep -rn "Result<.*, String>" src-tauri/src
```
