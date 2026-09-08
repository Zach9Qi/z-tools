# IPC 契约(前后端共同遵守)

> 前端 `src/lib/api.ts` ↔ Rust `src-tauri/src/commands/*.rs` 之间的规则汇总。两侧细节分别见 `../frontend/ipc-guidelines.md` 与 `../backend/command-guidelines.md`;本文件只写「两边必须一致」的部分与变更清单。

---

## 1. 命名对照

| 概念 | Rust 侧 | 前端侧 | 例子 |
|---|---|---|---|
| 命令名 | snake_case 函数名 | `invoke("同名")`;封装函数名为其 camelCase | `get_settings` ↔ `invoke("get_settings")` / `getSettings()` |
| 参数 | snake_case 形参 | camelCase key,Tauri 自动映射 | `fn f(user_name: String)` ↔ `{ userName }` |
| 结构体字段 | snake_case + `#[serde(rename_all = "camelCase")]` | camelCase interface 字段 | `created_at` ↔ `createdAt` |
| 枚举 | `#[serde(rename_all = "camelCase")] enum Kind { Text }` | `type Kind = "text"` | — |
| 带数据的枚举 | `#[serde(tag = "type")]` | 判别联合 `{ type: "…" }` | `{ type: "progress"; percent: number } \| { type: "done" }` |
| `Option<T>` | `None` | `null`(不是 `undefined`) | — |
| 事件名 | `pub const X: &str = "domain://action"` | `EVENTS.X = "domain://action"` | `settings://updated` |
| 错误 | `AppError` → 中文字符串 | `catch (error)` → `String(error)` | `"参数错误: 名字不能为空"` |

## 2. 责任边界

- **信任边界在 Rust 命令层**:前端输入不可信,校验放命令入口;Rust 返回的数据前端不二次校验。
- **Rust 是数据真相**:持久化、系统交互、业务规则都在 Rust;前端 store 只镜像与缓存。
- **文案由 Rust 产出**:错误文案是完整中文句子,前端不拼前缀。
- **降级由前端负责**:每个 `api.ts` 封装函数处理非 Tauri 环境,Rust 不知道浏览器预览的存在。
- **类型镜像手写**:`src/types/<domain>.ts` 头部注明对应 Rust 路径;Rust 改字段,同一个 PR 改 TS。

## 3. 新增 / 改名一个命令

| # | 位置 | 动作 |
|---|---|---|
| 1 | `src-tauri/src/commands/<domain>.rs` | `#[tauri::command] pub fn xxx(...) -> Result<T, AppError>`,`///` 文档 |
| 2 | `src-tauri/src/commands.rs` | 新领域时 `pub mod <domain>;` |
| 3 | `src-tauri/src/lib.rs` | `generate_handler![..., commands::<domain>::xxx]` |
| 4 | `src-tauri/src/error.rs` | 需要新失败类别时加变体 + 文案测试 |
| 5 | `src/lib/api.ts` | 同名 camelCase 封装,`invoke<T>` 泛型,JSDoc |
| 6 | `src/lib/api.ts` | 非 Tauri 分支(假数据 / no-op / reject) |
| 7 | `src/types/<domain>.ts` | 返回结构体的 TS 镜像(如有) |
| 8 | 测试 | Rust `#[cfg(test)]` 校验路径;TS `lib/` 纯逻辑测试(如有) |
| 9 | 改名时 | `grep -rn "旧名" src src-tauri`,确认无残留 |

## 4. 新增一个事件(Rust → 前端)

| # | 位置 | 动作 |
|---|---|---|
| 1 | Rust 领域模块 | `pub const XXX: &str = "domain://action";`,注释指向前端常量 |
| 2 | 同处 | payload 结构体 `#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]` |
| 3 | emit 点 | `use tauri::Emitter;` 后 `if let Err(e) = app.emit(XXX, payload) { log::warn!(...) }` |
| 4 | `src/lib/events.ts` | `EVENTS.XXX` 常量 + `EventPayloads[...]` 类型 |
| 5 | `src/composables/useXxx.ts` | 用事件 composable 监听,`onUnmounted` 清理 |
| 6 | 评估 | payload 是否够小?是否应改用 `Channel`(流式 / 高频)? |

## 5. 版本兼容

- 入参结构体 `#[serde(default)]`,新增字段给默认值,老前端不崩。
- 删除或改名字段视为破坏性变更,两侧同一 PR 完成并在 PR 描述里列出。
- 大 payload 可带 `schemaVersion` 字段,前端校验不匹配时报错而不是静默渲染;当前项目无此需要,payload 长大再加。

## 6. 快速自检

```bash
# 在 Git Bash / POSIX shell 下执行(仓库 CI 也统一用 bash)
# 前端只有 src/lib/api.ts 或 src/lib/api/**/*.ts 引用 invoke
grep -rn "@tauri-apps/api/core" src --include=*.ts --include=*.vue
# 对照命令定义与注册列表
grep -rn -A2 "tauri::command" src-tauri/src/commands/ | grep "pub .*fn"
grep -n -A20 "generate_handler" src-tauri/src/lib.rs
# 没有 Result<_, String>
grep -rn "Result<.*, String>" src-tauri/src
```
