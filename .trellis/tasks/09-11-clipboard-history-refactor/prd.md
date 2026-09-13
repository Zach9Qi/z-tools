# 参考 zach-tools 重构剪贴板历史状态编排

## Goal

`src/tools/clipboard/composables/useClipboardHistory.ts` 经过几轮修 bug(暂停分页、丢过期响应、顶底停住、去 hover 选中)后,状态与判据散落:`loadState` 四态机 + `loading` / `hasMore` / `canLoadMore` 三个派生量、`requestSeq` 每次请求都递增、`selectedIndex` 下标需要 `clampSelection` / `removeLocal` 手工修正、两个防抖定时器、`onUnmounted` 里靠 `requestSeq++` 兜底,页面侧还要用 `watch(canLoadMore) + nextTick + sentinelInView()` 补查哨兵。对照 `zach-tools/src/tools/clipboard/composables/useClipboardPage.ts` 的编排方式,重写这一层,让「谁触发重拉、谁本地改、谁丢弃过期响应」一眼可读,同时行为不回退。

## Requirements

### 必须保留的行为(回归底线)

- R1 三维筛选 `kind × favoriteOnly × query` 叠加;切分类 / 切收藏立即重拉,搜索词防抖后重拉;相同分类不重复拉。
- R2 keyset 分页:游标从当前末条现取 `{ copiedAt, id }`;不用 offset;`PAGE_SIZE` 仍为 100。
- R3 自己发起的删除 / 收藏成功后只改本地列表,不重拉、不重置分页与滚动;「只看收藏」下取消收藏的条目本地移除。
- R4 粘贴本身不动列表:后端写回 → 监听器回捕 → `clipboard://changed` 上浮同一条(同 id、新 copiedAt),前端按 id 去重后置顶。
- R5 快速切换筛选 / 连续输入产生的多个在途请求,只有最新一次的结果可以落地;卸载后的响应不写任何状态。
- R6 ↑/↓ 顶底停住不回绕;在末条按 ↓ 且还有下一页时追加下一页并停在原位。
- R7 删除选中项后选中就近的下一条(末条则上一条);删除展开项则收起;从头重拉后展开项一律收起。
- R8 列表请求失败与命令失败都把 Rust 的中文文案写进 `error` 供页面展示,下一次成功清空;失败不清空已加载列表。
- R9 分页请求失败后不能出现「自动反复重试」的循环(commit 5a9e36a 修的问题不能回归);用户再次滚动可以重试。
- R10 空态判定不变:`items` 为空且不在加载中才显示空态。
- R11 所有键位仍由 `ClipboardPage.vue` 通过 `useKeymap` 登记(z-tools 约定:composable 不登记按键),键位与 label 不变。

### 新增行为(用户在评审时明确要求,对齐 zach-tools)

- N1 `clipboard://changed` 事件**携带刚录入 / 上浮的条目**(列表 DTO `ClipboardItem` 形态)。前端收到后:
  - 条目不满足当前 `kind` / `favoriteOnly` 筛选 → 忽略(切筛选时会整表重拉);
  - 当前有搜索词 → 不在前端复刻后端 `LIKE` 语义,只当失效信号,防抖后整表重拉;
  - 否则按 id 去重后置顶(已在列表中则先移除旧位置再 `unshift`)。已加载的分页、滚动位置、选中项、展开项都不受影响。
- N2 启动器唤起(`launcher://open`)时做一次 **sync 刷新**:拉首页,只比对首条 id;对得上不重置(滚动与选中留着),对不上(隐藏期间漏了事件 / emit 失败)才整表重置。
- N3 Rust 侧 `ClipboardStore::record_captured` 返回录入 / 上浮后的 `ClipboardItem`,`record()` 把它作为 payload emit;`upsert` 用 `RETURNING` 一次拿回整行,不额外查询。删除 / 收藏命令仍不 emit(前端自知结果)。

### 结构性要求

- S1 列表请求只有一个入口:`loading`、`try/catch`、过期判断、`error` 写入都收在这里;调用方只声明「是否作废在途请求」「拉哪一页」「结果如何合入」。
- S2 过期判断用 `generation` 版本号:只在整表重拉时递增,翻页不递增(翻页由 `loading` 守卫串行化)。
- S3 选中以 `selectedId` 持有,`selectedIndex` / `selected` 派生;不再有 `clampSelection`、`if (index < selectedIndex) selectedIndex--` 这类下标修正。
- S4 去掉 `loadState` / `canLoadMore` / `hasMore`,只留 `loading: Ref<boolean>` 与 `exhausted: Ref<boolean>`(上一页不满一页)。
- S5 防抖只有一个定时器(搜索词变化与「有搜索词时收到事件」共用),筛选切换直接重拉并清掉待发防抖。
- S6 页面分页触发由「哨兵 IntersectionObserver + `watch(canLoadMore)` 补查」改为「列表 `@scroll` 距底 ≤ 200px 时 `loadMore()`」;删除 `sentinelRef` / `observer` / `sentinelInView`。
- S7 整表重置后页面把列表滚回顶部并选中第一项(composable 暴露 `resetTick` 计数,页面 `watch` 它 `scrollTo({ top: 0 })`)。
- S8 `useClipboardHistory.test.ts` 重写为针对新结构的用例,不再逐条断言 `canLoadMore` 状态机;覆盖首屏 / 分页游标与去重 / 过期丢弃 / 筛选重拉 / 删除选中修正 / 收藏筛选下取消收藏 / 事件置顶与筛选忽略与搜索词退化重拉 / 唤起 sync / 卸载丢弃。
- S9 事件契约变更只涉及 `clipboard.rs::record` / `store.rs::record_captured, upsert` 与 `src/lib/events.ts` 的 payload 类型;命令、`lib/api/clipboard.ts`、`types/clipboard.ts` 不变;不改 `ClipboardItemRow` / `ClipboardTabs` / `ClipboardItemDetail` 的 props / emits。
- S10 Rust 侧 `store.rs` 测试:`upsert` 返回行(同内容二次 upsert 返回同 id、新 copied_at);`record_captured` 返回条目与 `list` 首条一致;现有生命周期串行测试的 `pause_at` / `assert_waiting` 泛型化以兼容新返回类型。

## Non-Goals

- 不做 zach-tools 的「自写事件由后端抑制 + 前端 `mirrorTouch`」:z-tools 设计上让粘贴写回被监听器回捕并 emit,前端按 id 去重置顶即得到同样效果,少一条特殊路径。
- 不加 `copy`(仅复制不粘贴)命令、不改快捷键。
- 不改 ↑/↓ 顶底停住的决策(zach-tools 是回绕)。

## Acceptance Criteria

- [ ] `useClipboardHistory.ts` 不再出现 `loadState`、`canLoadMore`、`hasMore`、`clampSelection`、`requestSeq`、`selectedIndex = ref(...)`;有且只有一个 `setTimeout` 防抖句柄。
- [ ] `ClipboardPage.vue` 不再出现 `IntersectionObserver`、`sentinelRef`、`sentinelInView`、`watch(canLoadMore)`;分页由 `@scroll` 触发。
- [ ] R1–R11、N1–N2 在浏览器预览(`bun run dev`,假数据)与 Tauri 运行下手工验证通过:切 Tab / 收藏 / 搜索、滚到底翻页、Delete 删选中项、Ctrl+P 在「只看收藏」下取消收藏、Enter 粘贴后原条目置顶且滚动位置不丢、在别的应用复制后再唤起列表顶部出现新条目、分页失败后滚动可重试且不自转。
- [ ] 重写后的 `useClipboardHistory.test.ts` 通过,并覆盖 S8 列出的场景;`store.rs` 新增 S10 的测试通过。
- [ ] 前端 `bun run format && bun run format:check && bun run lint && bun run test && bun run build` 全绿;后端 `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` 全绿。
- [ ] Phase 3.3 spec 回写:`frontend/composable-guidelines.md` §2.1、`frontend/index.md` 决策表(列表分页 / 本地变更 vs 重拉)、`backend/state-events-async.md` §2(`clipboard://changed` 带 payload 的理由)、`guides/ipc-contract.md` §1 / §2「谁发事件」、`backend/index.md` 若有事件表。
