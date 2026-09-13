# 参考对照:zach-tools `useClipboardPage` vs z-tools `useClipboardHistory`

来源:`C:/Users/chenziqi/Desktop/codes/rust/zach-tools/src/tools/clipboard/composables/useClipboardPage.ts`(415 行)、`components/ClipboardPage.vue`、`.trellis/spec/frontend/composable-guidelines.md`「异步请求编排」一节。

## 逐项对照

| 关注点 | zach-tools | z-tools 现状 | 取舍 |
|---|---|---|---|
| 请求入口 | 单一 `requestList({ invalidate, cursor, apply })`,`loading` / `try-catch` / 过期判断 / `finally` 收在一处 | `load(before)` 一个函数里同时做「替换 or 追加」「写 hasMore」「clamp 选中」「写 loadState」 | **采用** zach-tools:调用方只声明差异 |
| 过期丢弃 | `generation` 只在整表重拉时 `++`;翻页不递增,靠 `loading` 守卫串行 | `requestSeq` 每次请求都 `++`,翻页也会作废前一次翻页;`finally` 语义靠 `if (seq !== requestSeq) return` 散在 try / catch 两处 | **采用** zach-tools |
| 选中 | `selectedId` + 派生 `selectedIndex` / `selected`;删除时 `dropFromList(index)` 取同位置下一条 | `selectedIndex` 下标;`removeLocal` 里 `index < selectedIndex → --`,再 `clampSelection()` | **采用** `selectedId`(zach-tools spec「常见错误」明确点名下标持久保存会漂移) |
| 分页可用性 | `loading || exhausted || !last` 三个守卫 | `loadState === "success" && hasMore` 派生 `canLoadMore`,四态机为「失败后暂停」而生 | **采用** `exhausted`;「暂停」的需求随触发方式改变而消失(见下) |
| 分页触发 | 列表 `@scroll`,距底 200px 调 `loadMore()` | 底部哨兵 `IntersectionObserver`,IO 只在进出时回调 → 需要 `watch(canLoadMore) + nextTick + sentinelInView()` 补查 → 失败会补查成循环 → 引入 `loadState` | **采用** `@scroll`:scroll 只在用户滚动时触发,失败不会自转,再滚一次即重试 |
| 整表重置 | `applyReset`:替换 items、写 exhausted、选中首项、`refreshTick++` 让页面滚回顶部 | `refresh()` 只收起展开项;选中归零散在 `setKind` / `toggleFavoriteOnly` / query watch 三处,事件重拉不归零 | **采用** `applyReset` 集中处理;事件重拉也归零(事件几乎只在面板隐藏时到达) |
| 防抖 | 一个 `refreshTimer`,搜索词与事件共用;筛选切换 `clearTimeout` 后立即重拉 | 两个定时器 `changedTimer` / `queryTimer`,常量 150 / 200 ms | **采用** 单定时器 |
| 事件契约 | `clipboard-new-item` 携带条目;`handleNewItem` 本地 `placeOnTop`,有搜索词时退化为重拉;自写事件由后端抑制 | `clipboard://changed` 无 payload;粘贴写回后监听器回捕并 emit(设计上「期望行为」) | **采用**(用户评审时要求):Rust `record_captured` 返回条目、`upsert` 用 `RETURNING *`;前端 `handleRecorded` 同 zach-tools。**不采用**后端抑制自写 + `mirrorTouch`:回捕事件按 id 去重置顶即得到同样效果 |
| 唤起同步 | `onLauncherOpen → refreshList("sync")` 比首条 id | 无;工具页隐藏期间仍挂载并持续收事件 | **采用**(用户评审时要求):作为 emit 失败 / 事件竞态的兜底,首条 id 相同不重置 |
| 键位登记 | 在 composable 内 `useKeymap` | 在 `ClipboardPage.vue` | **保持** z-tools 约定 |
| 错误展示 | 只 `console.error` | `error` 文案渲染成错误条(spec 错误契约) | **保持** z-tools |
| ↑/↓ 回绕 | 回绕 | 顶底停住(commit 18b9c44 有意为之) | **保持** z-tools |
| 卸载 | `clearTimeout` + 逐个 `unlisten` | `clearTimeout` ×2 + `requestSeq++` + `loadState = "idle"` | 改为 `clearTimeout` + `generation++`(`useTauriEvent` 自带 unlisten) |

## 现状「混乱」的根因链

```
IntersectionObserver 只在进出视口时回调
  → 一页加载完哨兵仍在视口内不会再触发 → 页面 watch(loading) 补查
    → 失败后补查成死循环(哨兵一直在视口内) → 引入 loadState 四态 + canLoadMore
      → 页面 watch(canLoadMore) + nextTick 复查 → 372 行测试专门断言 canLoadMore 各态
```

把触发方式换成 `@scroll`,整条链都不需要了。

## zach-tools 的 `handleScroll`

```ts
function handleScroll() {
  const el = listEl.value;
  if (el && el.scrollTop + el.clientHeight >= el.scrollHeight - 200) {
    void loadMore();
  }
}
```

`loadMore` 内部 `loading || exhausted || !last` 守卫,滚动事件高频触发也只会发一次请求。
