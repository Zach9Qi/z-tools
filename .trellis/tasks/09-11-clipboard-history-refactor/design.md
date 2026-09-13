# 技术设计:useClipboardHistory 重写

对照见 `research/reference-comparison.md`。改动范围:

| 层 | 文件 | 改动 |
|---|---|---|
| Rust | `src-tauri/src/clipboard/store.rs` | `upsert` 改 `RETURNING *` 返回 `ItemRow`;`record_captured` 返回 `ClipboardItem`;测试泛型化 + 新增 2 个用例 |
| Rust | `src-tauri/src/clipboard.rs` | `record()` 把返回的条目作为 payload emit;常量与模块文档注释同步 |
| 前端 | `src/lib/events.ts` | `EventPayloads[CLIPBOARD_CHANGED]: ClipboardItem` |
| 前端 | `src/tools/clipboard/composables/useClipboardHistory.ts` | 重写 |
| 前端 | `src/tools/clipboard/composables/useClipboardHistory.test.ts` | 重写 |
| 前端 | `src/tools/clipboard/ClipboardPage.vue` | 去哨兵改 `@scroll`;`watch(resetTick)` |

命令、`lib/api/clipboard.ts`、`types/clipboard.ts`、子组件不动。

## 0. Rust:事件携带条目

```rust
// store.rs
/// 写入一条快照并返回落库后的整行:同 hash 已存在则只上浮 copied_at(不新增、不改收藏),否则插入。
async fn upsert(&self, captured: &Captured, now: i64) -> Result<ItemRow, AppError> {
    // … 绑定同前 …
    sqlx::query_as::<_, ItemRow>("INSERT … ON CONFLICT(hash) DO UPDATE SET copied_at = excluded.copied_at RETURNING *")
        .bind(…).fetch_one(&self.pool).await.map_err(Into::into)
}

/// 完整录入 … 返回录入 / 上浮后的条目(列表 DTO),供 `record()` 作为事件 payload。
pub(super) async fn record_captured(&self, captured: &Captured) -> Result<ClipboardItem, AppError> {
    // … 落盘同前 …
    let row = self.upsert(captured, now_ms()).await?;
    let item = self.to_item(row)?;
    let evicted = self.trim().await?;
    self.remove_image_files(&evicted);
    Ok(item)
}
```

- SQLite 的 `INSERT … ON CONFLICT DO UPDATE … RETURNING *` 无论走插入还是更新分支都返回该行(SQLite ≥ 3.35;sqlx 内置的 libsqlite3 远高于此)。
- `to_item` 在 `trim` 之前调用:刚写入的行 `copied_at = now` 最新,不会被淘汰;先转 DTO 也避免持锁期间多一次 `fetch_row`。
- 测试:`pause_at<T: Debug, F: Future<Output = Result<T, AppError>>>` / `assert_waiting` 同样泛型化;新增
  `upsert_returns_row_with_same_id_and_bumped_copied_at`、`record_captured_returns_item_matching_list_head`。

```rust
// clipboard.rs
/// 监听器录入新内容或上浮旧内容后广播,payload 为该条目(列表 DTO `ClipboardItem`);
/// 前端 `src/lib/events.ts` 的 `EVENTS.CLIPBOARD_CHANGED` 与此一一对应。
pub const CLIPBOARD_CHANGED: &str = "clipboard://changed";

pub async fn record<R: Runtime>(app: &AppHandle<R>, store: &ClipboardStore, captured: Captured) {
    let item = match store.record_captured(&captured).await {
        Ok(item) => item,
        Err(e) => { log::warn!("记录剪贴板内容失败: {e}"); return; }
    };
    if let Err(e) = app.emit(CLIPBOARD_CHANGED, &item) {
        log::warn!("发送 {CLIPBOARD_CHANGED} 事件失败: {e}");
    }
}
```

前端 `src/lib/events.ts`:`[EVENTS.CLIPBOARD_CHANGED]: ClipboardItem`(`import type { ClipboardItem } from "@/types/clipboard"`),注释改为「payload 为刚录入 / 上浮的条目;重复复制同一内容时以同 id、新 copiedAt 重发,消费方按 id 去重」。

## 1. 状态

```ts
const items = ref<ClipboardItem[]>([]);          // 与后端 (copiedAt DESC, id DESC) 全序一致
const kind = ref<ClipboardKind | null>(null);
const favoriteOnly = ref(false);
const selectedId = ref<number | null>(null);     // 以 id 持有,插入 / 删除不漂移
const expandedId = ref<number | null>(null);     // 与 selectedId 独立;整表重置时清空
const loading = ref(false);                      // 有 list 请求在途(首屏 / 重拉 / 翻页)
const exhausted = ref(false);                    // 上一页不满 PAGE_SIZE;只在响应到达时写
const error = ref("");                           // 最近一次失败文案;下一次成功清空
const resetTick = ref(0);                        // 整表重置计数,页面据此滚回顶部

const selectedIndex = computed(() => items.value.findIndex((i) => i.id === selectedId.value));
const selected = computed<ClipboardItem | undefined>(() => items.value[selectedIndex.value]);
let generation = 0;                              // 整表重拉版本号
```

删掉:`loadState`、`hasMore`、`canLoadMore`、`requestSeq`、`clampSelection`、`changedTimer` / `queryTimer` 二选一。

## 2. 请求编排

```ts
async function requestList(options: {
  /** true = 整表重拉:版本号 +1,在途请求回来后作废 */
  invalidate?: boolean;
  /** 翻页游标;缺省拉首页 */
  before?: ListCursor;
  /** 把结果合入列表;发出后又被整表重拉过则不调用 */
  apply: (page: ClipboardItem[]) => void;
}): Promise<void> {
  const gen = options.invalidate ? ++generation : generation;
  loading.value = true;
  try {
    const page = await listClipboardItems({ kind: kind.value ?? undefined, favoriteOnly: favoriteOnly.value, query: toValue(query), before: options.before, limit: PAGE_SIZE });
    if (gen !== generation) return;
    error.value = "";
    options.apply(page);
  } catch (e) {
    if (gen !== generation) return;
    console.error("拉取剪贴板历史失败:", e);
    error.value = String(e);
  } finally {
    if (gen === generation) loading.value = false;
  }
}
```

- `refreshList(mode: "reset" | "sync")`:`clearTimeout(refreshTimer)` → `requestList({ invalidate: true, apply })`;`apply` 在 `mode === "reset"` 或 `page[0]?.id !== items[0]?.id` 时调 `applyReset(page)`,否则什么都不做(滚动 / 选中 / 已加载分页全部保留)。
- `refresh()` = `refreshList("reset")`,给筛选切换 / 搜索词 / 页面使用。
- `applyReset(page)`:`items = page`、`exhausted = page.length < PAGE_SIZE`、`selectedId = page[0]?.id ?? null`、`expandedId = null`、`resetTick++`。
- `loadMore()`:`const last = items.at(-1)`;`if (loading || exhausted || !last) return`;`requestList({ before: { copiedAt: last.copiedAt, id: last.id }, apply })`,`apply` 写 `exhausted` 并按 id 去重后 `push`(在途期间事件重拉已被 `generation` 挡掉,这里的去重是对「同一游标被两次滚动触发」的兜底,zach-tools 同款)。
- `scheduleRefresh()`:单定时器 `REFRESH_DEBOUNCE_MS = 150`,`clearTimeout` 后 `setTimeout(() => void refresh())`。搜索词 `watch` 与「有搜索词时收到事件」共用。

## 2.1 事件合并(`clipboard://changed` 带条目)

```ts
/** 条目是否属于当前 kind / favoriteOnly 视图;关键字语义留给后端 */
function matchesFilters(item: ClipboardItem): boolean {
  if (kind.value !== null && item.kind !== kind.value) return false;
  if (favoriteOnly.value && !item.favorite) return false;
  return true;
}

/** 已在列表中则先移除旧位置,再置顶;镜像后端 (copiedAt DESC, id DESC) 排序 */
function placeOnTop(item: ClipboardItem): void {
  const index = items.value.findIndex((i) => i.id === item.id);
  if (index !== -1) items.value.splice(index, 1);
  items.value.unshift(item);
}

/** 监听器录入 / 上浮:不属于当前视图忽略;有搜索词只当失效信号防抖重拉;否则本地置顶 */
function handleRecorded(item: ClipboardItem): void {
  if (!matchesFilters(item)) return;
  if (toValue(query).trim() !== "") { scheduleRefresh(); return; }
  placeOnTop(item);
}
```

- `selectedId` / `expandedId` 都是 id,置顶后自动跟随条目;`exhausted` 不变(尾部没动)。
- 与在途请求的关系:整表重拉(`generation` 已递增)回来会用快照覆盖本地 `unshift`——快照要么已含该条(DB 已提交)要么不含(极小窗口,下次事件 / 唤起 sync 兜底);翻页在途时置顶不影响游标(keyset 锚在末条),翻页响应按 id 去重防止「快照里仍在旧位置」的同一条被追加。
- 粘贴:后端写回 → 监听器回捕 → 同 id 新 `copiedAt` 的条目经此路径置顶,不需要 zach-tools 的 `mirrorTouch`。

## 2.2 唤起 sync

```ts
useTauriEvent(EVENTS.LAUNCHER_OPENED, () => void refreshList("sync"));
```

隐藏期间列表由事件维护;唤起只比对首条 id,对得上不重置。`EVENTS.LAUNCHER_OPENED` 只在工具页挂载时才会被这里收到(用户在剪贴板页内隐藏再唤起)。

**过期语义**:翻页请求不递增 `generation`,因此「翻页在途 → 用户切 Tab」时 refresh 递增版本,翻页响应回来 `gen !== generation` 整体忽略(包括 `finally` 不关 loading,由 refresh 自己收尾);「翻页在途 → 再次滚动」被 `loading` 守卫挡住。这两条覆盖了旧测试里全部「过期成功 / 过期失败」用例。

## 3. 本地变更

```ts
function dropItem(id: number): void {
  const index = items.value.findIndex((i) => i.id === id);
  if (index === -1) return;
  items.value.splice(index, 1);
  if (selectedId.value === id) selectedId.value = items.value[Math.min(index, items.value.length - 1)]?.id ?? null;
  if (expandedId.value === id) expandedId.value = null;
}
```

- `remove(id)`:`await deleteClipboardItem(id)` 成功 → `error = ""` → `dropItem(id)`。
- `toggleFavorite(id)`:成功 → 翻转 `item.favorite`;`favoriteOnly && !next` → `dropItem(id)`。
- `paste(id)`:只调命令与写 `error`,不动列表(不变)。
- 命令失败:`console.error("中文前缀:", e); error.value = String(e)`(不变)。

## 4. 筛选与选中

- `setKind(next)`:相同返回;写 `kind` → `refresh()`。
- `cycleKind(delta)`:`KIND_CYCLE` 循环(不变)。
- `toggleFavoriteOnly()`:翻转 → `refresh()`。
- 选中归零不再散落:`applyReset` 统一选中首项。
- `moveSelection(delta)`:`current = selectedIndex`(-1 视为 0 起点);`next = current + delta`;`next >= length` → `if (!exhausted) void loadMore()`,不动;`next < 0` → 不动;否则 `selectedId = items[next].id`。
- `toggleExpanded(id)`(不变)。
- 删除 `select(index)`:页面已不再有 hover 选中(commit 2d0aa81),没有调用方。

## 5. 生命周期

```ts
useTauriEvent(EVENTS.CLIPBOARD_CHANGED, handleRecorded);
useTauriEvent(EVENTS.LAUNCHER_OPENED, () => void refreshList("sync"));
watch(() => toValue(query), scheduleRefresh);
onMounted(() => void refresh());
onUnmounted(() => { clearTimeout(refreshTimer); generation++; });
```

`generation++` 让卸载后到达的响应全部作废(含 `finally`),取代旧的 `requestSeq++; loadState = "idle"`。

## 6. 返回值

```ts
return { items, kind, favoriteOnly, selectedIndex, selected, expandedId, loading, exhausted, error, resetTick,
         refresh, loadMore, paste, remove, toggleFavorite, toggleFavoriteOnly, setKind, cycleKind, toggleExpanded, moveSelection };
```

移除:`hasMore`、`canLoadMore`、`select`。新增:`exhausted`、`resetTick`。

## 7. 页面 `ClipboardPage.vue`

- 删除 `sentinelRef` / `observer` / `sentinelInView` / `onMounted` / `onUnmounted` / `watch(canLoadMore)` 与模板里的哨兵 `<div>`。
- 列表容器加 `@scroll="handleScroll"`:`scrollTop + clientHeight >= scrollHeight - LOAD_MORE_THRESHOLD_PX(200)` → `void loadMore()`。
- `watch(resetTick, () => nextTick(() => listRef.value?.scrollTo({ top: 0 })))`。
- 空态、错误条、键位登记不变;`selected` 仍可能为 `undefined`,`onPress` 判空写法不变。

## 8. 测试策略

沿用现有自定义渲染器挂载真实生命周期的手法,mock `@/lib/api/clipboard` 与 `@/composables/useTauriEvent`。用例(每个对应一条 PRD 需求):

1. 挂载拉首屏,满页 `exhausted=false`,短页 `exhausted=true`;选中首项(R2 / S4)。
2. `loadMore` 用末条 `{ copiedAt, id }` 作游标;在途 / 到底 / 空列表不发请求;响应按 id 去重(R2)。
3. 翻页在途时 `setKind` → 翻页响应被丢弃,`loading` 由 refresh 收尾,`items` 为新筛选结果(R5 / S2)。
4. 搜索词连续变化只发一次请求;事件与搜索词共用防抖;切分类清掉待发防抖(R1 / R4 / S5)。
5. 删除选中项 → 选中同位置下一条;删末条 → 选中新末条;删展开项 → 收起;`exhausted` 不变(R3 / R7)。
6. `favoriteOnly` 下取消收藏 → 本地移除;普通视图翻转星标不移除(R3)。
7. 列表失败保留旧 `items`,`error` 有文案;下次成功清空(R8)。
8. 卸载后:防抖不再发请求,在途响应不落地(R5)。
9. `moveSelection`:顶底停住;末条按 ↓ 且未到底触发 `loadMore`(R6)。
10. 事件:新条目置顶且选中 / 展开 / `exhausted` 不变;同 id 重发 → 移到顶不重复;kind / favoriteOnly 不匹配 → 忽略;有搜索词 → 防抖后整表重拉且与搜索词防抖合并为一次请求(N1)。
11. 唤起 sync:首条 id 相同 → 不重置(`items` / `selectedId` / `resetTick` 不变);不同 → 整表重置(N2)。

## 9. 兼容与回滚

- 事件契约两侧同一提交内改(`ipc-contract.md` §5);浏览器预览没有事件总线,`useTauriEvent` 不订阅,行为与现在一致。
- 对外只有 `ClipboardPage.vue` 一个消费方,返回值变更同一提交内消化。
- 回滚点:Rust + 前端一次提交,`git revert` 即可;spec 更新单独提交。

## 10. 明确不做(与 PRD Non-Goals 一致)

自写事件后端抑制 + `mirrorTouch`、↑↓ 回绕、composable 内登记键位、`copy` 命令。
