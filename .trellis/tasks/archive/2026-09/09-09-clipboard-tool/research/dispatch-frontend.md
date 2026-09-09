# 前端实现指令(implement.md 步骤 6 ~ 7)

> 本文件是主会话派发给前端实现子代理的指令;契约以 `design.md` 为准,本文件只是把步骤拆细。

## 边界

- 只允许改动:`src/**`(**不含** `src-tauri/`)。
- **不要碰 `src-tauri/**`**——另一个子代理正在并行实现后端,你按 design.md §3.1 / §3.3 / §3.4 / §4.3 / §5 独立镜像契约即可,不必等后端。
- 步骤 1(删 demo)已完成并提交:`src/tools/demo/` 已删,`src/tools/registry.ts` 的 `modules` 暂为 `[]`,`src/tools/icons.ts` 已登记 `clipboard`、`star`、`chevron-down`、`image`、`file-text`、`files`、`puzzle` 七个图标(不需要再改 icons.ts,如需其他图标再加)。

## 必读(编码前)

1. `.trellis/tasks/09-09-clipboard-tool/prd.md`、`design.md`、`implement.md`
2. 规范:`.trellis/spec/frontend/index.md` 及其下 `tool-module-guidelines.md`、`ipc-guidelines.md`、`composable-guidelines.md`、`component-guidelines.md`、`styling-guidelines.md`、`directory-structure.md`;`.trellis/spec/guides/ipc-contract.md`
3. 现有代码:`src/types/tool.ts`、`src/stores/keymap.ts`、`src/lib/api/*.ts`、`src/lib/events.ts`、`src/composables/`(useKeymap / useTauriEvent 用法)、`src/components/`(工具页容器、页脚)、`src/tools/registry.ts`

## 步骤 6:契约层

- `src/types/clipboard.ts`:镜像 Rust(文件头注明 `src-tauri/src/clipboard.rs`):`ClipboardKind = "text" | "image" | "files"`;`ClipboardItem` 是 `kind` 判别联合(camelCase 字段):Text `{kind:"text", id, favorite, copiedAt, size, preview, charCount, truncated}` / Image `{kind:"image", id, favorite, copiedAt, size, imagePath, thumbPath, width, height}` / Files `{kind:"files", id, favorite, copiedAt, files: ClipboardFile[]}`;`ClipboardFile {path, name, exists}`;`ListCursor {copiedAt, id}`;`ListQuery {kind?: ClipboardKind, favoriteOnly: boolean, query: string, before?: ListCursor, limit: number}`。
- `src/lib/api/clipboard.ts`:6 个封装 `listClipboardItems(query)`→`list_clipboard_items`、`getClipboardText(id)`→`get_clipboard_text`、`pasteClipboardItem(id)`→`paste_clipboard_item`、`deleteClipboardItem(id)`→`delete_clipboard_item`、`setClipboardItemFavorite(id, favorite)`→`set_clipboard_item_favorite`、`clearClipboardHistory()`→`clear_clipboard_history`;`toAssetUrl(path)`(**唯一** `convertFileSrc` 调用点,非 Tauri 时返回占位图 data URL)。非 Tauri(`isTauriRuntime()` 为 false)返回带「(浏览器预览)」标识的假数据:3 类各 1-2 条,含一条 `truncated: true` 的多行长文本、一条含 `exists: false` 路径的多文件条目、一条单文件条目、一条单行短文本;`getClipboardText` 假数据返回一段 20 行文本;假数据要能按 kind / favoriteOnly / query 做简单过滤并模拟分页(before 游标),便于预览。
- `src/lib/events.ts`:追加 `CLIPBOARD_CHANGED: "clipboard://changed"`,payload `null`,遵循现有事件表写法。

## 步骤 7:工具模块 `src/tools/clipboard/`

- `lib/format.ts` + `format.test.ts`:`formatRelativeTime(ms, now)`(刚刚 / N 分钟前 / N 小时前 / N 天前 / 日期)、`formatBytes`、`summarizeFiles(files)`(首文件名 + 「等 N 项」,文件名用 DTO 的 `name`,不解析路径)、`isExpandable(item)`(text: `truncated || preview.includes("\n")`;image: 恒 true;files: `files.length > 1`)。每个分支进单测。
- `composables/useClipboardHistory.ts`:状态 `items / kind(null=全部) / favoriteOnly / selectedIndex / expandedId / loading / hasMore / error`;`PAGE_SIZE = 100`;`refresh()`(before 空,响应替换 items,置 expandedId=null,递增序号丢弃过期响应)、`loadMore()`(游标从 `items.at(-1)` 现取 `{copiedAt, id}`,items 空则退化为 refresh;`hasMore && !loading` 才执行)、`hasMore` **只在响应到达时写**:`page.length < PAGE_SIZE ? false : true`;`paste(id)`(不动列表)、`remove(id)`(成功后 splice,selectedIndex 钳到相邻项;删的是 expandedId 则置 null)、`toggleFavorite(id)`(成功后本地翻转;favoriteOnly 开且变为未收藏则本地移除)、`toggleFavoriteOnly()`(翻转后 refresh)、`setKind(kind)` / `cycleKind(±1)`(refresh)、`toggleExpanded(id)`、`clear()`(成功后 `items = items.filter(favorite)`,hasMore 不动,展开项不在则置 null)、`moveSelection(±1)`(到末条且 hasMore 时调 loadMore)。订阅 `CLIPBOARD_CHANGED` 防抖 150ms → refresh;`query` 变化防抖 200ms → refresh。本地变更**不重拉、不碰 hasMore**。
- `components/ClipboardTabs.vue`:左侧 全部 / 文本 / 图片 / 文件 Tab;右侧「★ 收藏」切换按钮(独立筛选,与 Tab 叠加,激活态高亮)+「清空」按钮(内联二次确认「确认清空?」确认 / 取消两键)。所有按钮 `@mousedown.prevent` 不夺搜索框焦点。
- `components/ClipboardItemRow.vue`:收起态一行——类型图标 + 摘要(text: `line-clamp-2` 的 preview;image: 缩略图 `toAssetUrl(thumbPath)`、`loading="lazy"`、`object-contain`、高 56px + `W×H`;files: `summarizeFiles`,任一 `!exists` 整行 `text-muted-foreground`)+ 相对时间 + **常显星标按钮**(favorite 时 `fill-current text-primary`,否则空心 `text-muted-foreground`;`@click.stop` emit toggle-favorite,`@mousedown.prevent`)+ 右侧 chevron(仅 `isExpandable` 时渲染,否则等宽占位;展开时 `rotate-180 transition-transform`;`@click.stop` emit toggle-expanded,`@mousedown.prevent`)。行本身 `@click` emit paste。选中态 `bg-accent` 覆盖整行(含详情区)。展开时下方渲染 ClipboardItemDetail。**无**悬停删除按钮。
- `components/ClipboardItemDetail.vue`:text → `onMounted` 调 `getClipboardText(id)`,有 loading / error 态(错误只在详情区内显示),头部「{charCount} 字 · {formatBytes(size)}」+ `<pre class="whitespace-pre-wrap break-all font-mono text-xs">`;image → 原图 `<img :src="toAssetUrl(imagePath)" class="max-h-72 w-auto object-contain">` + 「W×H · 大小」;files → 逐行完整路径 `break-all`,`!exists` 行 `text-muted-foreground line-through` + 尾部「不存在」。容器 `max-h-72 overflow-auto`,`@click.stop` 不触发粘贴但允许选中文本。展开后 `scrollIntoView({ block: "nearest" })`。
- `ClipboardPage.vue`:props `query: string`;单根 `<section>`;Tabs + 列表容器(`min-h-0 flex-1 overflow-y-auto`)+ 底部哨兵(`IntersectionObserver` 触发 loadMore)+ 空态 / 加载 / 错误条;用 `useKeymap` 登记 6 组键位并出现在页脚:`ArrowUp`/`ArrowDown`「选择」、`Enter`「粘贴」、`Delete`「删除」、`Ctrl+P`「收藏」、`Ctrl+F`「只看收藏」、`Ctrl+ArrowLeft`/`Ctrl+ArrowRight`「切换分类」。不登记 Escape / Backspace / Tab。选中项变化时 `scrollIntoView({ block: "nearest" })`。
- `index.ts`:`ViewToolModule`,id `clipboard`、title `剪贴板`、icon `clipboard`、keywords `["clipboard","jtb","剪切板","粘贴","历史","paste"]`、placeholder `搜索剪贴板历史…`、page 指向 ClipboardPage(按 `src/types/tool.ts` 契约)。
- `src/tools/registry.ts`:`modules = [clipboardTool]`。

## 样式约束

遵守 styling-guidelines.md:语义 token(`bg-accent` 选中、`text-muted-foreground` 次要、`text-destructive` 错误、`text-primary` 强调),不写十六进制。

## 完成标准(必须全部通过后再汇报)

```
bun run format && bun run format:check && bun run lint && bun run test && bun run build
grep -rn "@tauri-apps/api/core" src   # 只能命中 src/lib/api*
```

汇报内容:新增/修改文件清单、测试通过数、任何偏离 design.md 的地方及理由。
