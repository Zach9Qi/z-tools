# 设计令牌体系对齐业界标准并重写样式规范

## Goal

把 `src/index.css` 的设计令牌体系从「8 个颜色令牌 + 圆角 + 字体」扩充为覆盖业界共识的完整语义集与全局行为策略,同步修正示例组件的语义误用,并把 `.trellis/spec/frontend/styling-guidelines.md` 重写为能指导后续所有 UI 开发的完整设计规范。

## Background

现状(见 `src/index.css` 与 `styling-guidelines.md`):

- 已有:`background/foreground`、`muted/muted-foreground`、`accent/accent-foreground`、`border`、`ring`;`--radius` 单基准派生;`light-dark()` + `color-scheme` 原生深浅色;`@theme inline` 映射。这一机制与 shadcn/ui v4 令牌模型一致,**保留不变**。
- 缺口:
  - 无 `primary`、`destructive`、`card`、`popover`、`input`、`secondary` 语义;`HelloWorld.vue` 主按钮误用 `accent`(规范自身定义为 hover/选中弱交互底),错误文案用 `text-foreground` 与正常文本无区分。
  - 无 `prefers-reduced-motion` 处理;焦点只靠组件自觉写 `focus-visible:ring`,无 `outline` 兜底;hover 范式 `hover:opacity-80` 会让文字/图标一起变淡。
  - 无 z-index 档位、无表面/层级(海拔)约定、无桌面端(Tauri WebView)约定(滚动条、`::selection`、overscroll)。
  - 规范文档只覆盖颜色/圆角/字体三类,缺交互状态、焦点、层级、排版、动效、可访问性、桌面端、组件变体等章节。

对齐目标:**令牌词汇表**采用 shadcn/ui v4 命名(Tailwind 生态事实标准,便于未来接 shadcn-vue / Reka UI 组件),**实现机制**保留本仓库 `light-dark()` 方案不照搬 `.dark` 类;跨生态项(reduced-motion、焦点兜底、z-index、对比度)依据 WCAG / DTCG / Material 3 共识补齐。

## Requirements

### R1 语义令牌扩充(`src/index.css`)

- R1.1 新增成对令牌:`primary/primary-foreground`、`secondary/secondary-foreground`、`destructive/destructive-foreground`、`card/card-foreground`、`popover/popover-foreground`;新增单值令牌 `input`。
- R1.2 所有新令牌遵守现有约束:仅在 `:root` 出现具体值,色相只引用 Tailwind 内置 `var(--color-*)`,透明用 `--alpha()`,深浅用 `light-dark()`,不写字面色值;在 `@theme inline` 映射为 `--color-*`。
- R1.3 深色模式下 `card` / `popover` 必须比 `background` 更亮(表面提亮表达层级),允许为此调整现有 `background` 深色取值。
- R1.4 每个令牌在 `:root` 有一行中文注释说明用途与禁止场景(如 `accent` 仅用于 hover/选中,不用于主按钮)。
- R1.5 `success` / `warning` / `info` 本次不加;规范中写明新增流程与命名约定,需要时按流程加。

### R2 全局行为策略(`src/index.css` `@layer base`)

- R2.1 焦点兜底:所有元素默认 `outline-color` 取自 `ring` 令牌(半透明),即使组件忘写 `focus-visible:ring` 也有可见焦点。
- R2.2 `prefers-reduced-motion: reduce` 时全局停用动画与过渡(允许使用 `!important`,需写注释说明为何是例外)。
- R2.3 `::selection` 使用语义令牌着色。
- R2.4 桌面端:细滚动条并用语义令牌着色;`body` 关闭 overscroll 回弹。
- R2.5 z-index 档位以 CSS 变量定义在 `:root`(`--z-*`),组件通过 `z-(--z-*)` 消费,禁止裸数字 `z-50` / 任意值 `z-[999]`。
- R2.6 不新增阴影 / 动效令牌,直接使用 Tailwind 默认 `shadow-*` / `ease-*` / `duration-*`;规范中写明层级 = 表面令牌 + `border` + 可选 `shadow-*` 的组合策略。

### R3 示例组件修正(`src/components/HelloWorld.vue`)

- R3.1 主按钮改用 `bg-primary text-primary-foreground`,hover 改为实底变色(`hover:bg-primary/90`)而非整体降透明;补 `transition-colors`。
- R3.2 错误文案改用 `text-destructive`。
- R3.3 输入框描边用 `border-input`,焦点样式采用规范新范式。
- R3.4 卡片容器使用 `bg-card text-card-foreground`。
- R3.5 `App.vue` 不改(布局容器不写底色的约定不变)。

### R4 规范文档重写(`.trellis/spec/frontend/styling-guidelines.md`)

- R4.1 分层模型保持现有「原始层 `:root` / 语义层 `@theme inline` / 基础层 `@layer base` / 消费层组件」叫法与表格不变,只在表格中补充新令牌与新 base 规则。
- R4.2 完整令牌表:每个令牌的用途、禁止场景、成对关系。
- R4.3 新增章节:交互状态策略(hover/active/disabled)、焦点策略、表面与层级、排版与密度、动效与 reduced-motion、z-index 档位、可访问性(对比度 AA、`forced-colors` 不炸)、桌面端约定(滚动条、选区、拖拽区 `data-tauri-drag-region`、chrome 区 `select-none`)、组件变体写法(`variant` / `size` props 对象映射)。
- R4.4 删除 / 替换过时范式:`hover:opacity-80` 不再作为推荐写法。
- R4.5 全文基于本仓库真实实现,不出现占位文本。

### R5 关联 spec 同步

- R5.1 `frontend/index.md`:质量检查清单补充新禁止项(主按钮不用 `accent`、无 `hover:opacity-*` 做实底按钮 hover、无裸 `z-*` 数字);关键决策记录新增「令牌词汇表对齐 shadcn v4 命名,机制保留 light-dark()」。
- R5.2 `frontend/quality-guidelines.md` §5 可访问性:焦点写法更新为新范式;补 reduced-motion 与对比度要求。
- R5.3 `frontend/component-guidelines.md` §6 增加指向组件变体章节的引用。

## Constraints

- 不引入新依赖(不装 shadcn-vue、cva、tailwind-variants)。
- 不改深浅色机制:仍是 `light-dark()` + `html { color-scheme }`,不出现 `dark:` 与 `.dark` 类。
- 不新建第二个全局 CSS 文件。
- 令牌命名与 shadcn/ui v4 一致,以便未来零成本接入生态组件。
- 所有注释、文案用中文。

## Acceptance Criteria

- [ ] `src/index.css` `:root` 含 R1.1 全部令牌,每个带中文注释;`@theme inline` 含对应 `--color-*` 映射;`grep -E "#[0-9a-fA-F]{3,8}|rgb\(|hsl\(|oklch\(" src/index.css` 无匹配。
- [ ] 深色模式下 `--card` / `--popover` 的取值比 `--background` 更亮(取值档位数字更小)。
- [ ] `@layer base` 含:`outline-color` 兜底、`prefers-reduced-motion` 媒体查询、`::selection`、滚动条样式、`overscroll-behavior`。
- [ ] `:root` 含 `--z-*` 档位变量;规范中写明消费方式为 `z-(--z-*)`。
- [ ] `HelloWorld.vue` 中不再出现 `bg-accent` 作为提交按钮底色、不再出现 `hover:opacity-80`、错误文案使用 `text-destructive`;`grep -rn "opacity-80" src/` 无匹配。
- [ ] `styling-guidelines.md` 含 R4.3 列出的全部章节标题,且不含 `hover:opacity-80` 推荐写法。
- [ ] R5 列出的三个 spec 文件均已同步。
- [ ] `bun run format && bun run format:check && bun run lint && bun run test && bun run build` 全部通过。
- [ ] `bun run tauri dev`(或 `bun run dev`)下浅色 / 深色系统主题切换时:页面底、卡片底、主按钮、错误文案四者均可区分,主按钮 hover 有可见变化,Tab 到输入框 / 按钮有可见焦点环。
