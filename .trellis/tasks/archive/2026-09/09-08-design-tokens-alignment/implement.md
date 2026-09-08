# 执行计划

前置:读 `design.md` §2–§8;读 `.trellis/spec/frontend/styling-guidelines.md`(现版,作为被替换对象)与 `quality-guidelines.md`。

## 步骤

### 1. `src/index.css` — 语义令牌
- [ ] `:root`:dark `--background` 改为 `zinc-950`。
- [ ] `:root` 新增(按 design §2 取值,每个一行中文注释含用途 + 禁止场景):`--card`、`--card-foreground`、`--popover`、`--popover-foreground`、`--primary`、`--primary-foreground`、`--secondary`、`--secondary-foreground`、`--destructive`、`--destructive-foreground`、`--input`。
- [ ] `:root` 新增 `--z-dropdown/sticky/overlay/modal/popover/toast`(design §3)。
- [ ] `@theme inline` 新增对应 `--color-*` 映射。
- [ ] 文件头注释补一句:新增令牌与 z-index 档位同样遵守「具体值只进 :root」。

### 2. `src/index.css` — `@layer base`
- [ ] `html` 加 `scrollbar-width` / `scrollbar-color`。
- [ ] `body` 加 `overscroll-behavior: none`;更新 body 注释(原注释提到 overscroll 回弹露底,现已关闭回弹,措辞调整)。
- [ ] `*` 加 `outline-color`。
- [ ] 新增 `::selection`。
- [ ] 新增 `prefers-reduced-motion` 媒体查询,带「唯一允许 !important」注释。

### 3. `src/components/HelloWorld.vue`
- [ ] 按 design §8 四处改动。
- [ ] 更新按钮上方注释(若提及 hover/disabled 写法)。

### 4. `.trellis/spec/frontend/styling-guidelines.md` 重写
章节顺序:
1. 三层令牌(保留现有表格,基础层一行补 outline / selection / 滚动条 / reduced-motion)
2. 令牌表(design §2 全表 + `--z-*`)+ 新增令牌流程 + 预留 `success/warning/info` 命名取色
3. 深浅色(保留现有内容,补 dark 表面提亮策略)
4. 交互状态与焦点(design §5 表)
5. 表面与层级(design §6)
6. 排版与密度(字号用 Tailwind 默认档位 `text-xs..2xl`;标题 `font-semibold`;正文 `text-sm`;`leading`/`tracking` 只用默认档位;间距 4px 网格 `gap-2/4/6`)
7. 动效(`transition-colors` + 默认 duration/ease;`animate-spin` 等;reduced-motion 由 base 层全局处理,组件无需再写 `motion-reduce:`)
8. z-index 档位(design §3;`z-(--z-*)` 写法)
9. 可访问性(文字对比 AA 4.5:1;`forced-colors` 下不写 `forced-color-adjust: none`;焦点不可隐藏)
10. 桌面端约定(滚动条 / 选区由 base 层处理;标题栏 `data-tauri-drag-region` + `select-none`;不在 body 全局 `user-select: none`)
11. 组件变体写法(design §7)
12. 组件内写法(保留现有 §3 内容,替换 `hover:opacity-80` 与 `disabled:cursor-not-allowed`)
13. 字体与图标(保留)
14. 禁止(保留 + 新增:主按钮用 `accent`、`hover:opacity-*` 做实底 hover、裸 `z-*` 数字 / `z-[...]`、组件内 `motion-reduce:` 双写)

### 5. 关联 spec 同步
- [ ] `frontend/index.md` 质量检查:新增三条(见 prd R5.1);关键决策记录新增一行。
- [ ] `frontend/quality-guidelines.md` §5:焦点写法改为 `outline-hidden focus-visible:ring-3 focus-visible:ring-ring/50`;新增 reduced-motion 由 base 层保证、对比度 AA 两条。
- [ ] `frontend/component-guidelines.md` §6 加一条「有 variant/size 的组件按 styling-guidelines.md「组件变体写法」」。

## 验证命令

```bash
grep -nE "#[0-9a-fA-F]{3,8}|rgb\(|hsl\(|oklch\(" src/index.css   # 期望无输出
grep -rn "opacity-80\|bg-accent" src/components/HelloWorld.vue        # 期望无输出
grep -rn "hover:opacity-80" .trellis/spec/frontend/                     # 期望无输出
bun run format && bun run format:check && bun run lint && bun run test && bun run build
```

手动验证(`bun run dev` 浏览器 / `bun run tauri dev`):切换系统深浅色,确认页面底 / 卡片底 / 主按钮 / 错误文案可区分;hover 主按钮变色;Tab 焦点环可见;`prefers-reduced-motion` 模拟下 loader 不旋转。

## 审查门禁

- 步骤 1–3 完成后:跑验证命令 + 手动验证,再进入步骤 4–5。
- 步骤 4–5 完成后:对照 prd 验收标准逐条勾选;`trellis-check` 做规范一致性复查(spec 中每个类名在 Tailwind 4 中真实存在、与 index.css 令牌一一对应)。

## 回滚点

- 单提交;任一验收项不过则 `git checkout -- src .trellis/spec` 回到起点。
