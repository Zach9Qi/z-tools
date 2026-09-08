# 样式规范(Tailwind CSS 4 + 三层设计令牌)

> 唯一的全局样式文件是 `src/index.css`;组件只写工具类。本仓库选 Tailwind 4(CSS-first 配置、`light-dark()` 原生深浅色),令牌词汇表对齐 shadcn/ui v4 命名以便未来零成本接入 shadcn-vue / Reka UI 组件,实现机制保留 `light-dark()`。以下全部基于本仓库真实实现(`src/index.css`、`src/components/HelloWorld.vue`)。

---

## 1. 三层令牌(`src/index.css`)

| 层 | 位置 | 允许出现的东西 | 禁止 |
|---|---|---|---|
| 原始层 | `:root` | 唯一允许写具体值的地方:颜色用 `light-dark(var(--color-zinc-*), …)`、透明用 `--alpha(… / n%)`、圆角基准 `--radius`、根字号、字体栈、z-index 档位 `--z-*` | 字面色值(`#fff`、`rgb()`、`oklch()`) |
| 语义层 | `@theme inline` | 只做映射与派生:`--color-background: var(--background)`、`--radius-sm: calc(var(--radius) - 4px)` | 任何具体值 |
| 基础层 | `@layer base` | 全局行为策略:根字号、`html { color-scheme: light dark }`、细滚动条(`scrollbar-width` / `scrollbar-color`)、`body` 消费语义令牌设文档底色 / 文字色 / 字体族并关闭 overscroll 回弹、字体平滑、`button { cursor: pointer }`、默认 `border-color`、`outline-color` 焦点兜底、`::selection`、`prefers-reduced-motion` 全局停用动效 | 具体色值、原始色变量(`var(--color-zinc-*)`) |
| 消费层 | `.vue` 组件 | 语义工具类:`bg-card`、`text-muted-foreground`、`bg-primary text-primary-foreground`、`border-input`、`focus-visible:ring-ring/50`、`z-(--z-modal)` | 原始色工具类 `bg-zinc-900`、任意值 `bg-[#123]`、裸 `z-50` / `z-[999]` |

## 2. 令牌表

### 2.1 颜色令牌(zinc 中性色 + red 危险色)

取值是 `:root` 中 `light-dark(浅, 深)` 的两侧;成对令牌(`x` + `x-foreground`)必须一起用、一起加。

| 令牌 | 浅色 | 深色 | 用途 | 禁止 |
|---|---|---|---|---|
| `background` | `white` | `zinc-950` | 页面底(由 `body` 消费) | 卡片、弹层 |
| `foreground` | `zinc-800` | `zinc-100` | 主文字 | — |
| `card` / `card-foreground` | `white` / `=foreground` | `zinc-900` / `=foreground` | 卡片、面板、侧栏等**静态表面** | 悬浮层 |
| `popover` / `popover-foreground` | `white` / `=foreground` | `zinc-900` / `=foreground` | dropdown、tooltip、dialog 等**悬浮表面**,必配 `border shadow-md` | 页内静态容器 |
| `primary` / `primary-foreground` | `zinc-900` / `zinc-50` | `zinc-50` / `zinc-900` | 主 CTA、选中态强调、链接 | 每屏超过一个主 CTA |
| `secondary` / `secondary-foreground` | `zinc-100` / `zinc-900` | `zinc-800` / `zinc-50` | 次级实底按钮、标签 | 作为 hover 态 |
| `muted` / `muted-foreground` | `zinc-100/90%` / `zinc-500` | `zinc-800/75%` / `zinc-400` | 静态次级底(徽章、代码片段)/ 低对比文字(提示、占位) | 可点击元素的底 |
| `accent` / `accent-foreground` | `zinc-900/6%` / `zinc-900` | `white/10%` / `zinc-50` | hover / 选中 / ghost 按钮的**叠加底** | 主按钮、静态容器 |
| `destructive` / `destructive-foreground` | `red-600` / `white` | `red-400` / `zinc-950` | 错误文案、删除按钮、危险边框 | 非破坏性的警告提示 |
| `border` | `zinc-200/80%` | `zinc-700/60%` | 分隔线、卡片描边(`*` 默认 `border-color`) | 表单控件描边 |
| `input` | `zinc-300` | `white/15%` | 表单控件描边(比 `border` 更强) | 非控件 |
| `ring` | `zinc-400` | `zinc-500` | 焦点环、基础层 `outline-color` 兜底 | — |

`card` 与 `popover` 目前取值相同,区别在用法:`popover` 必带 `border shadow-md`;拆成两个令牌是为将来换肤时可独立调整。`secondary` 与 `muted` 不合并:前者是实底可交互,后者是静态底且带透明度。

### 2.2 非颜色令牌

| 令牌 | 值 | 消费 |
|---|---|---|
| `radius-sm` / `md` / `lg` / `xl` / `2xl` | 基准 `--radius: 0.5rem` 的 -4 / -2 / 0 / +4 / +8px | `rounded-sm` … `rounded-2xl` |
| `font-sans` / `font-mono` | 系统 UI 字体栈 / 等宽字体栈 | `body` 默认 `font-sans`;组件只在需要等宽时写 `font-mono` |
| `--font-size-base` | `100%`,根字号入口 | `html { font-size }`;改它后所有 rem 尺寸等比缩放,圆角偏移仍是 px |
| `--z-dropdown` / `sticky` / `overlay` / `modal` / `popover` / `toast` | 10 / 20 / 30 / 40 / 50 / 60 | `z-(--z-modal)`(见 §8) |

### 2.3 新增令牌流程

1. 在 `:root` 加原始变量:`light-dark(var(--color-xxx-600), var(--color-xxx-400))`,带一行中文注释写清**用途 + 禁止场景**。
2. 在 `@theme inline` 映射为 `--color-*`(圆角为 `--radius-*`)。
3. 组件用工具类消费。
4. 在本文件 §2.1 表格补一行。

**成对的前景 / 背景**(`x` + `x-foreground`)必须同时加,即使 `x-foreground` 只是 `var(--foreground)` 的别名(`card-foreground` 即如此)。

### 2.4 预留:状态色(本次未加,需要时按 §2.3 添加)

| 令牌 | 浅色 / 深色 | 用途 |
|---|---|---|
| `success` / `success-foreground` | `green-600` / `green-400` | 成功提示、完成态 |
| `warning` / `warning-foreground` | `amber-600` / `amber-400` | 非破坏性警告(与 `destructive` 区分) |
| `info` / `info-foreground` | `blue-600` / `blue-400` | 中性信息提示 |

对应的 `*-foreground` 沿用 `destructive-foreground` 的模式:浅色 `white` / 深色 `zinc-950`。

## 3. 深浅色

- 靠 `light-dark()` + `@layer base` 里的 `html { color-scheme: light dark }`,跟随系统,无 JS、无 `dark:` 变体。
- `color-scheme` **必须和 `:root` 上的 `light-dark()` 令牌声明在同一元素(html)上**,不要在组件里用 `scheme-*` 工具类充当主题根。原因见 `index.css` 注释:一是 Teleport 到 body 的弹层会脱离组件子树;二是构建时 Lightning CSS 会 polyfill `light-dark()`,开关变量不在 `:root` 上时所有颜色令牌都会算成非法值。
- 文档级底色 / 文字色 / 字体族由 `@layer base` 的 `body` 承担(`var(--color-background)` / `var(--color-foreground)` / `var(--font-sans)`),这样 Teleport 弹层等脱离布局容器的区域不会露出 webview 白底。布局组件(`App.vue` 的 `<main>`)不写 `bg-background text-foreground font-sans`;需要局部换底时才在容器上用 `bg-card` / `bg-muted` 等令牌。
- **深色表面提亮**:深色下 `background` 取 `zinc-950`,`card` / `popover` 取 `zinc-900`,表面比页面底亮一档来表达层级(与 shadcn v4 zinc 主题一致);浅色下两者同白,层级靠 `border`。详见 §5。
- 需要手动切换主题时,在 `html` 上覆盖 `color-scheme`(如加 `scheme-dark` 类),仍不写 `dark:`。
- 不使用 `dark:` 前缀写双份样式;要新颜色就加令牌。

## 4. 交互状态与焦点

| 状态 | 实底按钮(`primary` / `secondary` / `destructive`) | ghost / 图标按钮 | 表单控件 |
|---|---|---|---|
| 默认 | `bg-primary text-primary-foreground` | `text-foreground` | `border border-input bg-background` |
| hover | `hover:bg-primary/90` | `hover:bg-accent hover:text-accent-foreground` | — |
| active(可选) | `active:bg-primary/80` | —(`accent` 本身是半透明叠加,再降透明会比 hover 更淡,不写) | — |
| focus | `outline-hidden focus-visible:ring-3 focus-visible:ring-ring/50` | 同左 | `outline-hidden focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50` |
| disabled | `disabled:pointer-events-none disabled:opacity-50` | 同左 | 同左 |
| 过渡 | `transition-colors`(默认 150ms / ease) | 同左 | 同左 |

- **hover 用实底变色**(`hover:bg-primary/90`),不用 `hover:opacity-*`:后者会让文字、图标一起变淡。
- **disabled 用 `pointer-events-none`**,不用 `cursor-not-allowed`:前者顺带挡掉 hover 变色;`cursor-not-allowed` 在 `pointer-events-none` 下本就不会显示。
- **焦点用半透明 3px 环**(`ring-3 ring-ring/50`,Tailwind 4 中 `ring-3` 是合法动态值),深色下比 2px 实色环更柔和;表单控件同时把描边换成 `focus-visible:border-ring`。
- **焦点兜底**:基础层给 `*` 设了 `outline-color: --alpha(var(--color-ring) / 50%)`,组件即使漏写 `focus-visible:ring` 也有可见轮廓(WebKit 对 `outline-style: auto` 可能忽略颜色、回落到系统色,但仍可见);写了 ring 的组件配 `outline-hidden` 避免双环。**不允许** `outline-hidden` 后不补 `focus-visible:ring`。
- **用 `outline-hidden` 而不是 `outline-none`**:Tailwind 4 中 `outline-none` 是 `outline-style: none`,会连高对比模式一起关掉;`outline-hidden` 是 `outline: 2px solid transparent`,正常模式下不可见,`forced-colors` 下 `box-shadow`(即 `ring`)被系统清除时透明 outline 会被系统色替换,焦点仍可见。
- `accent` 只做 hover / 选中的叠加底,不做主按钮底色。

## 5. 表面与层级

层级 = 表面令牌 + `border` + 可选 `shadow-*`,不新增阴影令牌,`shadow-*` 用 Tailwind 默认档位。

| 表面 | 令牌 | 描边 / 阴影 | 例子 |
|---|---|---|---|
| 页面底 | `background`(由 `body` 消费,组件不写) | — | `App.vue` `<main>` |
| 静态表面 | `bg-card text-card-foreground` | `border`(+ 可选 `shadow-sm`) | `HelloWorld.vue` `<section>` |
| 悬浮表面 | `bg-popover text-popover-foreground` | `border shadow-md` | dropdown、tooltip、dialog |
| 次级静态底 | `bg-muted` | — | 徽章、`<code>` 片段 |

- 浅色:`card` 与 `background` 同白,层级靠 `border`(+ 可选 `shadow-sm`)。
- 深色:`card` / `popover` 比 `background` 亮一档,`border` 弱化;阴影在深色下几乎不可见,不依赖它表达层级。
- 悬浮层统一 `border shadow-md`;不要用 `shadow-2xl` 等重阴影堆层级。

## 6. 排版与密度

- 字号只用 Tailwind 默认档位 `text-xs` … `text-2xl`;正文 `text-sm`,标题 `font-semibold`(`HelloWorld.vue` 的 `h1` 为 `text-2xl font-semibold`),不写 `text-[13px]`。
- `leading-*` / `tracking-*` 只用默认档位,不写任意值。
- 间距走 4px 网格:`gap-2 / 4 / 6`、`p-2 / 4 / 6 / 8`。一次性控件用内边距定高(输入框 `px-3 py-2`、按钮 `px-4 py-2`,与 `HelloWorld.vue` 一致);带 `size` prop 的组件用固定 `h-*`(见 §11)保证同一行内对齐。
- 尺寸用 `size-*` / `h-*` / `w-*` 等 rem 工具类;不写 `px` 任意值。
- 低对比说明文字用 `text-sm text-muted-foreground`;等宽片段用 `font-mono`。

## 7. 动效

- 颜色过渡统一 `transition-colors`,用默认 `duration` / `ease`,不单独写 `duration-*` / `ease-*` 除非有明确理由。
- 加载态用 `animate-spin`(`HelloWorld.vue` 的 loader 图标)。
- **reduced-motion 由基础层全局处理**:`@media (prefers-reduced-motion: reduce)` 下把所有 `animation-duration` / `transition-duration` 压到 `0.01ms`、`animation-iteration-count: 1`、`scroll-behavior: auto`(`!important`,这是本仓库唯一允许 `!important` 的地方);组件**不需要**再写 `motion-reduce:` 变体。
- 不引入动效库;需要复杂动画时用 `<style scoped>` + `@keyframes`,并写注释说明为何工具类表达不了。

## 8. z-index 档位

`:root` 定义六档,组件用 `z-(--z-*)` 引用(Tailwind 4 的 CSS 变量简写语法):

| 变量 | 值 | 用途 |
|---|---|---|
| `--z-dropdown` | 10 | 下拉、自动补全,依附触发器 |
| `--z-sticky` | 20 | 吸顶栏、固定工具栏 |
| `--z-overlay` | 30 | 模态遮罩 |
| `--z-modal` | 40 | 对话框、抽屉 |
| `--z-popover` | 50 | 弹层 / tooltip,可出现在 modal 之上 |
| `--z-toast` | 60 | 全局通知,永远最上 |

- 写法:`class="fixed inset-0 z-(--z-overlay)"`、`class="z-(--z-modal)"`。
- z-index 不走 `@theme`(Tailwind 4 没有 z-index 命名空间),所以没有 `z-modal` 这样的类名。
- 禁止裸数字 `z-10` / `z-50` 与任意值 `z-[999]`;需要新档位先在 `:root` 加变量并更新本表。

## 9. 可访问性

- 文字与其底色的对比度满足 WCAG AA(4.5:1):`foreground` / `primary-foreground` / `destructive` 在对应底上已满足;`muted-foreground` 只用于辅助说明,不承载关键信息。
- 非文字对比(WCAG 1.4.11,3:1):`border-input` 与主按钮底满足;焦点环 `ring-ring/50` 对齐 shadcn v4,浅色下低于 3:1,这是有意取舍——表单控件靠 `focus-visible:border-ring` 补强,不要再为此单独调环色。
- 焦点不可隐藏:`outline-hidden` 必须与 `focus-visible:ring-*` 成对出现(§4);基础层 `outline-color` 兜底只是保险,不是免写 ring 的理由。
- `forced-colors`(Windows 高对比模式)下不写 `forced-color-adjust: none`,让系统替换颜色。该模式会清除 `box-shadow`,即 `ring` 焦点环不可见;控件轮廓靠 `border` 保证(颜色被替换但仍存在),焦点靠 `outline-hidden` 的透明 outline 被系统色替换后显示(§4)。
- 装饰图标 `aria-hidden="true"`,仅图标按钮要有 `aria-label`(见 `quality-guidelines.md` §5)。
- 深浅色都必须验证:页面底 / 卡片底 / 主按钮 / 错误文案四者可区分,Tab 到控件焦点环可见。

## 10. 桌面端约定(Tauri WebView)

- **滚动条**:基础层 `html { scrollbar-width: thin; scrollbar-color: --alpha(var(--color-muted-foreground) / 40%) transparent }`,细滚动条并随主题着色;组件不写 `::-webkit-scrollbar`。
- **选区**:基础层 `::selection { background-color: --alpha(var(--color-primary) / 20%) }`,组件不再单独设。
- **overscroll**:`body { overscroll-behavior: none }`,桌面应用不需要回弹。
- **自定义标题栏 / 拖拽区**:承载拖拽的元素加 `data-tauri-drag-region`,并给该 chrome 区域加 `select-none` 防止拖动时选中文字;拖拽区内的按钮不继承拖拽(Tauri 只对带属性的元素本身生效)。
- **不在 `body` 全局 `user-select: none`**:内容区文字必须可选中复制;`select-none` 只加在标题栏、工具栏等 chrome 区。

## 11. 组件变体写法

有 `variant` / `size` props 的组件,用 `as const` 对象映射类名,不引入 `clsx` / `cva` / `tailwind-variants`:

```ts
const variants = {
  default: "bg-primary text-primary-foreground hover:bg-primary/90",
  secondary: "bg-secondary text-secondary-foreground hover:bg-secondary/80",
  destructive: "bg-destructive text-destructive-foreground hover:bg-destructive/90",
  ghost: "hover:bg-accent hover:text-accent-foreground",
} as const satisfies Record<string, string>;
const sizes = { sm: "h-8 px-3 text-xs", md: "h-9 px-4 text-sm", lg: "h-10 px-6" } as const;

const { variant = "default", size = "md" } = defineProps<{
  variant?: keyof typeof variants;
  size?: keyof typeof sizes;
}>();
```

- `variant` / `size` 为 props,默认值用解构默认值(见 `component-guidelines.md` §2)。
- 类名合并用模板字符串或数组 `join(" ")`:`` :class="`${variants[variant]} ${sizes[size]}`" ``。
- 为让 `prettier-plugin-tailwindcss` 排序生效,对象里的类名字符串保持完整字面量,不拼接变量。
- 公共部分(`inline-flex items-center gap-1.5 rounded-md font-medium transition-colors outline-hidden focus-visible:ring-3 focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-50`)写在模板 `class` 上,变体对象只放差异。

## 12. 组件内写法

- 工具类顺序由 `prettier-plugin-tailwindcss` 排序(`.prettierrc` 已配置 `tailwindStylesheet: ./src/index.css`),提交前跑 `bun run format`,不手动排。
- 尺寸用 `size-*` / `gap-*` / `p-*` 等 rem 工具类;不写 `px` 任意值。
- 交互态按 §4 表格写:实底按钮 `transition-colors hover:bg-primary/90 disabled:pointer-events-none disabled:opacity-50`,焦点 `outline-hidden focus-visible:ring-3 focus-visible:ring-ring/50`,与 `HelloWorld.vue` 一致。
- 需要复用的一组类名:优先抽组件(带 `variant` / `size` props 的按钮组件,见 §11),其次才是 `@utility`;`@apply` 仅在组件无法抽取时使用(Tailwind 官方文档建议优先组件化而非 `@apply`)。
- `<style scoped>` 仅在工具类确实表达不了(复杂动画、第三方组件深层选择器 `:deep()`)时使用,并写注释说明为何不能用工具类。

## 13. 字体与图标

- 字体族只在 `:root` 定义一次(`--font-family-sans` / `--font-family-mono`);`body` 默认 `font-sans`,组件只在需要等宽时写 `font-mono`。
- 图标是 SVG 组件(`~icons/lucide/*`),尺寸 `size-4` 等,颜色随 `currentColor`;不设 `fill` / `stroke`。

## 14. 禁止

- 组件里出现字面色值、`bg-zinc-*` 等原始色工具类、`bg-[...]` 任意色值。
- `dark:` 变体。
- 主按钮用 `accent`(`accent` 只做 hover / 选中叠加底)。
- 实底按钮 hover 用 `hover:opacity-*`(用 `hover:bg-primary/90` 等实底变色)。
- 裸 `z-*` 数字(`z-10` / `z-50`)与任意值 `z-[...]`(用 `z-(--z-*)`)。
- 组件内 `motion-reduce:` 双写(reduced-motion 已由基础层全局处理)。
- `outline-hidden` 后不补 `focus-visible:ring-*`。
- 全局 `<style>`、在组件里 `@import` CSS。
- 新建第二个全局 CSS 文件(主题相关只改 `index.css` 的 `:root`)。
- `!important` / `!` 修饰符:唯一例外是基础层的 `prefers-reduced-motion`;其他场景需要它通常意味着令牌或层叠层设计有问题,先修根因;确实需要(覆盖第三方内联样式)时写注释。
