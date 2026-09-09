# 剪切板工具（Windows，文本/图片/文件）

## Goal

移除 `src/tools/demo/` 占位工具，实现仿 uTools「超级剪贴板」的 Windows 剪贴板历史工具：应用常驻后台时持续监听系统剪贴板，记录文本 / 图片 / 文件三类内容并跨重启保留；用户在启动器中打开工具页即可分类浏览、搜索、收藏、删除，选中一条后一步完成「复制 + 收起面板 + 粘贴到原窗口」。

## Background / 已确认事实（仓库证据）

- 仓库目前**没有任何剪贴板代码**；所谓占位只有 `src/tools/demo/index.ts`（1 个 view 工具 `demo-view` + 12 个空 launch 工具）和 `src/tools/demo/DemoToolPage.vue`，文件头已注明「真实工具进来后整目录删除」。`src/tools/registry.ts:4,7` 是唯一引用点；`src/lib/launcher/search.test.ts:15` 的 `"demo"` 是自造 fixture，不依赖 demo 目录。
- 技术栈：Tauri 2 + Vue 3 + Pinia + Tailwind 4；Rust 单 crate（`src-tauri/`），edition 2024。无 tokio 直接依赖（Cargo.lock 已有 1.53 间接依赖）、无持久化层、无 clipboard 相关 crate / 插件。Cargo.lock 只锁定一份 `windows 0.61.3`。
- 已有 Windows 专用代码模式：`src-tauri/src/launcher/windows.rs` + `#[cfg(windows)] mod windows;`，依赖 `windows = "0.61"`（`[target.'cfg(windows)'.dependencies]`）。
- 工具契约：`src/types/tool.ts`（`ViewToolModule` 需 `page` 接收 `query: string`），登记 `src/tools/registry.ts`，图标 `src/tools/icons.ts`。工具页可用高度 = 600 − 64 − 40 px；`Escape` / 空输入 `Backspace` / `Tab` 被面板占用；`KeyBinding` 支持 `ctrl` 修饰（`src/stores/keymap.ts`）。
- IPC 约定：命令封装在 `src/lib/api/<domain>.ts`（含 `isTauriRuntime()` 回退）；事件名 `domain://action`，Rust `pub const` 与 `src/lib/events.ts` 镜像；高频数据不用事件。
- 后端约定：托管状态 `app.manage`，后台任务用 `tauri::async_runtime::spawn(_blocking)` 在 `setup_desktop` 里启动并把句柄存进托管状态；非 `Send` 系统句柄不跨 await；`block_on` 仅允许在 setup。
- 启动器 `show()`（`src-tauri/src/launcher.rs`）在 `set_focus()` 前没有记录前台窗口——「粘贴回原窗口」需在此处补记。
- Tauri 2 asset 协议只需 `tauri.conf.json5` 的 `app.security.assetProtocol.{enable,scope}`，无对应 capability 权限项（`src-tauri/gen/schemas/desktop-schema.json` 无 `core:asset*`）。
- 应用现有落盘项在 Windows 上已都位于 `%LOCALAPPDATA%\<identifier>\`：WebView2 数据 `EBWebView\`（Tauri 默认）、`tauri-plugin-log` 的 `LogDir` = `app_log_dir()` = `logs\`。仓库尚无其他持久化。

## Requirements

### R1 清理占位
- 删除 `src/tools/demo/` 整目录及 `registry.ts` 中的引用；仅剩剪贴板工具登记。仅 demo 使用的图标登记同步移除。

### R2 后台采集（Windows）
- 应用运行期间（含面板隐藏时）持续监听系统剪贴板变化，识别三类内容并入库：
  - **文本**（`CF_UNICODETEXT`）：trim 后为空不记录；超过 1 MiB 不记录。
  - **图片**（位图）：解码后 RGBA 超过 20 MiB 不记录；落盘为 PNG 原图 + 缩略图（最长边 256px）。
  - **文件**（`CF_HDROP`）：只记录路径列表，不复制文件本体。
- 同一内容重复复制：**上浮到顶部**而不新增（文本按内容、图片按解码像素、文件按路径列表哈希去重）。
- 非收藏条目最多保留 **500 条**，超出按最近复制时间淘汰（连同图片文件）；收藏不计入、不淘汰（收藏是保护标记，不是排序依据）。
- 上限等阈值为 Rust 侧常量，本版不做设置界面。

### R3 持久化
- Rust 侧持久化到 **`app_local_data_dir()/clipboard/`**（Windows：`%LOCALAPPDATA%\<identifier>\clipboard\`）：`history.db`（SQLite，**sqlx**，运行时查询而非编译期宏）+ `images/`。
- **项目级约定**：启动器所有落盘文件（数据、日志、WebView 缓存、后续设置）统一位于 `%LOCALAPPDATA%\<identifier>\`，不使用 `app_data_dir()`（Roaming），用户删该目录即可完整清理。此约定写入 `.trellis/spec/backend/config-and-permissions.md`。
- 跨重启保留；前端不持有第二份真相。

### R4 工具页（view 型，id `clipboard`）
- 筛选三个维度，**可任意叠加**：
  - 顶部分类 Tab：全部 / 文本 / 图片 / 文件。
  - **「收藏」切换按钮**（Tab 栏右侧，星标）：开启后只显示收藏条目，与当前 Tab、搜索词同时生效（如「图片 + 收藏」「文本 + 收藏 + 关键字」）。收藏是独立筛选选项，**不是置顶**。
  - 工具页搜索栏（`query` prop）实时过滤：文本按内容、文件**仅按文件名（含扩展名）**，不匹配所在目录；图片不参与文本搜索（有搜索词时「图片」Tab 为空、「全部」Tab 排除图片）。
- 列表项**收起态**显示：类型图标 + 摘要（文本前两行 / 图片缩略图与 `宽×高` / 首文件名 + 「等 N 项」）+ 相对时间 + **星标图标（每行常显、可点击）**：已收藏为实心，未收藏为空心；点击即切换收藏，不触发粘贴。文件已不存在的条目标灰但保留。
- 列表项**可展开**（行内手风琴，鼠标点击行右侧 chevron 展开 / 收起，**不设快捷键**）查看完整内容：
  - 文本：全文（保留换行与缩进，可滚动），头部显示字符数 / 字节数。全文在展开时按需向后端拉取，列表 DTO 只带摘要。
  - 图片：原图（等比缩放到展开区最大高度内，不裁切）+ `宽×高` 与 PNG 大小。
  - 文件：全部文件的完整路径逐行列出，已不存在的路径单独标灰并注明「不存在」。
  - 同时只有一项展开；展开另一项时前一项收起；切 Tab / 切换只看收藏 / 搜索词变化 / 重拉列表时全部收起。展开不改变选中项，`↑`/`↓`/`Enter`/`Delete`/`Ctrl+P` 行为不受影响。
  - 内容本身不超过一行（如短文本、单个文件）时不显示 chevron。
- 列表**仅**按最近复制时间倒序排列（同一毫秒并列时后录入者在前，顺序确定），收藏不影响位置；首屏 100 条，滚动到底部追加，追加期间新复制的内容不得造成重复或漏项。
- 交互：
  - 键盘 `↑`/`↓` 选择、`Enter` 粘贴、`Delete` 删除当前项、`Ctrl+P` 收藏/取消当前项（位置不变）、`Ctrl+F` 切换「只看收藏」、`Ctrl+←`/`Ctrl+→` 切换 Tab；均经 `useKeymap` 登记并出现在页脚。
  - 鼠标单击条目 = 粘贴；行内**不设**悬停「收藏」「删除」按钮，鼠标能做的只有点星标切换收藏、点 chevron 展开；删除只经键盘 `Delete`。
- 监听器录入新内容后后端广播 `clipboard://changed`（无 payload），前端重拉列表，工具页打开期间新复制的内容实时出现。用户自己的删除 / 收藏在命令成功后由前端直接更新本地列表，**不**重拉、不丢失已加载的分页与滚动位置。
- 浏览器预览（非 Tauri）显示带「(浏览器预览)」标识的假数据，页面可打开。

### R5 粘贴行为（A 方案 + B 兜底）
- 选中后：把该条内容写回系统剪贴板（文本 / 图片 / 文件列表）→ 激活启动器唤出前记录的前台窗口 → 收起面板 → 模拟 `Ctrl+V`。
- 兜底：未记录到前台窗口 / 该窗口已关闭 / 激活失败时，只复制并收起面板，命令仍成功，不报错。
- 写回后监听器会再次捕获同一内容并上浮，这是期望行为。

### R6 平台范围与跨平台预留
- 剪贴板**读写**用跨平台库 `arboard`（主流、1Password 维护），代码平台无关；**监听**与**粘贴模拟**按平台拆文件（`#[cfg(windows)]`），本版仅实现 Windows（`windows` crate 自建消息窗口监听 + `SendInput`）。后续做 macOS / Linux 时只需各补一份监听 / 粘贴实现，数据层、命令层、前端不动。
- 非 Windows 不启动监听器，`paste_clipboard_item` 返回明确的「当前平台暂不支持」错误，列表命令可用（空库）。不为其他平台写 stub 行为。

## Acceptance Criteria

- [ ] AC1（R1）：`src/tools/demo/` 不存在；`grep -rn "demo" src --include=*.ts --include=*.vue` 只剩 `search.test.ts` 的 fixture；主页网格只显示「剪贴板」一个磁贴。
- [ ] AC2（R2）：面板隐藏时在记事本复制文本、Win+Shift+S 截图、资源管理器复制 2 个文件，打开工具页三条记录依次位于顶部且类型 / 摘要正确；`%LOCALAPPDATA%\<identifier>\clipboard\images\` 出现 `<hash>.png` 与 `<hash>.thumb.png`；`%APPDATA%\<identifier>\`（Roaming）**不**被创建。
- [ ] AC3（R2）：连续复制同一段文本两次，列表只有一条且位于首位；仅空白字符的文本不入库。
- [ ] AC4（R2）：Rust 单测覆盖：upsert 去重上浮、超过 500 条淘汰且保留 favorite、排序不受 favorite 影响、同毫秒并列按 id 倒序稳定、游标分页在中途插入新条目后无重复无漏项、kind × favorite_only × query 三维度叠加过滤与 `%`/`_` 转义、文件搜索只命中文件名不命中目录名。
- [ ] AC5（R3）：重启应用后历史与收藏状态保留。
- [ ] AC6（R4）：四个 Tab 切换正确；输入关键字后文本 / 文件按内容 / 文件名过滤（搜所在文件夹名不命中），「图片」Tab 为空；`Ctrl+←/→` 切 Tab；页脚出现「选择 / 粘贴 / 删除 / 收藏 / 只看收藏 / 切换分类」提示。
- [ ] AC7（R4）：`Ctrl+P` 后条目**位置不变**、出现实心星标；开启「收藏」筛选后只剩收藏条目，再切到「图片」Tab 只剩收藏的图片，再输入关键字后在「文本」Tab 只剩匹配的收藏文本；`Delete` 删除当前项后选中移到相邻项；鼠标点击某行空心星标 → 变实心、位置不变、面板不收起（未触发粘贴），再点变回空心。
- [ ] AC8（R4）：工具页打开期间在其他应用复制新内容，列表在 1 秒内出现新条目而无需手动刷新。
- [ ] AC13（R4）：复制一段 10 行以上文本后，列表行只显前两行 + chevron；点击 chevron 后展开显示全文（换行保留、可滚动）与字符数，再点收起；点另一条的 chevron 时前一条自动收起；展开图片条目显示原图与尺寸；展开一条含已删除文件的文件条目，该路径标灰并注明「不存在」，其余路径正常；展开期间 `↑`/`↓` 仍正常移动选中，切换 Tab 后无任何展开项。短文本（单行）与单文件条目不显示 chevron。
- [ ] AC9（R5）：在记事本中唤出 → 选中文本条目 Enter → 面板收起且记事本光标处出现该文本；图片条目粘贴到「画图」成功；文件条目粘贴到另一文件夹成功。
- [ ] AC10（R5）：关闭记事本后再对某条目 Enter → 面板收起、系统剪贴板已是该内容、无错误提示。
- [ ] AC11（R4）：`bun run dev` 浏览器预览下工具页显示假数据且无控制台报错。
- [ ] AC12（全部）：`bun run format:check && bun run lint && bun run test && bun run build` 与 `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` 全部通过；`grep -rn "@tauri-apps/api/core" src` 仅命中 `src/lib/api*`。

## Out of Scope（后续迭代）

- 非 Windows 平台的剪贴板监听 / 写回 / 粘贴。
- 右侧大预览面板 / 图片放大；数字键 1-9 快速粘贴；多选合并粘贴；纯文本（去格式）粘贴；HTML / RTF 记录。
- 图片 OCR、内容规则过滤（如排除密码管理器）、暂停监听开关、容量 / 阈值设置界面。
- 直接唤出剪贴板工具的独立全局快捷键（需要启动器契约支持「直达某工具」，另开任务）。
- 已知限制不处理：`SendInput` 对以管理员权限运行的目标窗口无效（UIPI）。

## Technical Notes

技术方案、依赖选型（`arboard` 读写 + 自写平台监听；不选 `clipboard-rs`：小众且引入双份 `windows` crate）、数据模型、命令 / 事件契约与数据流见 `design.md`；执行顺序与验证命令见 `implement.md`。
