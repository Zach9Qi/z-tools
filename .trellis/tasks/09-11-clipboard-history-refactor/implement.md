# 执行计划

前置:读 `.trellis/spec/frontend/composable-guidelines.md`、`component-guidelines.md`、`quality-guidelines.md`、`.trellis/spec/backend/state-events-async.md`、`persistence.md`、`quality-guidelines.md`;设计见 `design.md`。

## 步骤

- [x] 1. Rust:`src-tauri/src/clipboard/store.rs`(design §0)
  - `upsert` → `RETURNING *` + `query_as::<_, ItemRow>` + `fetch_one`,返回 `ItemRow`
  - `record_captured` → 返回 `ClipboardItem`(`to_item` 在 `trim` 前)
  - 测试:`pause_at` / `assert_waiting` 泛型化;新增 `upsert_returns_row_with_same_id_and_bumped_copied_at`、`record_captured_returns_item_matching_list_head`
- [x] 2. Rust:`src-tauri/src/clipboard.rs` `record()` emit `&item`;`CLIPBOARD_CHANGED` 注释与模块文档「无 payload」措辞改掉
  - 校验:`cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
- [x] 3. 前端:`src/lib/events.ts` payload 类型改 `ClipboardItem`,注释同步
- [x] 4. 重写 `src/tools/clipboard/composables/useClipboardHistory.ts`(design §1–§6)
  - 文件头注释改写:「统一入口 + generation」「selectedId」「事件本地合并 / 搜索词退化重拉」「唤起 sync」「单防抖」
  - 每个 `ref` 一行 `/** */`;常量 `PAGE_SIZE`(导出)、`REFRESH_DEBOUNCE_MS`、`KIND_CYCLE`
  - 校验:`grep -nE "loadState|canLoadMore|hasMore|clampSelection|requestSeq|select\(" src/tools/clipboard/composables/useClipboardHistory.ts` 无输出
- [x] 5. 改 `src/tools/clipboard/ClipboardPage.vue`(design §7)
  - 去哨兵 / IO / `watch(canLoadMore)`;加 `@scroll` + `watch(resetTick)`
  - 头注释同步:「底部哨兵」改为「滚动阈值」
  - 校验:`grep -nE "IntersectionObserver|sentinel|canLoadMore" src/tools/clipboard/ClipboardPage.vue` 无输出
- [x] 6. 重写 `src/tools/clipboard/composables/useClipboardHistory.test.ts`(design §8 十一组用例)
  - 保留自定义渲染器与 `requests` 队列手法;`useTauriEvent` mock 按事件名分别收集 handler
- [x] 7. 前端门禁:`bun run format && bun run format:check && bun run lint && bun run test && bun run build`
- [~] 8. 手工验证(浏览器预览已过:切 Tab / 收藏筛选 / ↓ 选中;Tauri 下 N1 / N2 / R9 待用户验证)(`bun run dev` 浏览器假数据 + `bun run tauri dev`),对照 PRD Acceptance 第 3 条逐项勾
  - R9:把假数据 `listClipboardItems` 临时改为翻页 reject,滚到底只报一次错,再滚才重试;验证完还原
  - N1:在剪贴板页停留,去别的应用 Ctrl+C,唤起后新条目在顶部且滚动 / 选中未变;Enter 粘贴一条中间条目,再唤起它已在顶部且只出现一次
  - N2:隐藏期间 kill 掉事件(断点 / 临时注释 emit)再唤起,列表应整表重置
- [ ] 9. 评审门:与用户过一遍 diff,确认 R1–R11、N1–N3 无回退

## Phase 3

- [x] 3.3 spec 更新(`trellis-update-spec`):
  - `frontend/composable-guidelines.md` §2.1 表格:「递增序号丢过期响应」→「统一入口 + generation 只在整表重拉递增」;「选中下标修正集中在一处」→「selectedId 派生下标,`dropItem` 就近改选」;「`hasMore` 判据唯一」→「`exhausted` 只在响应到达时写」;「筛选变化与外部事件才 refresh」→ 事件本地合并 + 搜索词退化;「定时器清理」→ 单定时器;§4 哨兵例子改 `@scroll`
  - `frontend/index.md` 决策表「列表本地变更 vs 重拉」「列表分页」两行
  - `backend/state-events-async.md` §2:`clipboard://changed` 改为带 payload 的现例,原「不带新条目」理由改写为「kind / favorite 前端可判,LIKE 不可判则退化重拉」
  - `guides/ipc-contract.md` §1 事件行、§2「谁发事件」
  - `backend/index.md` / `frontend/ipc-guidelines.md` 若提到「无 payload」同步
- [ ] 3.4 提交:`refactor(clipboard): 参考 zach-tools 重写历史列表状态编排,事件携带条目本地合并` + `docs(spec): …` 两段
- [ ] 3.5 归档任务

## 回滚点

步骤 1–7 未过门禁前不提交;任一步失败 `git checkout -- src src-tauri/src` 回到基线。
