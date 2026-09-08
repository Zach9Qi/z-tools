# 技术设计:设计令牌体系对齐

## 1. 分层模型

保持 `index.css` 与 `styling-guidelines.md` 现有的三层叫法不变:`:root` 原始层(唯一写具体值)→ `@theme inline` 语义层(只映射)→ `@layer base` 基础层(全局行为)→ 组件消费层。本任务只在各层内**增加内容**,不改层次结构与命名。

## 2. 目标令牌表(zinc 中性色 + red 危险色)

| 令牌 | light | dark | 用途 | 禁止 |
|---|---|---|---|---|
| `background` | `white` | `zinc-950`(由 900 调深) | 页面底 | 卡片、弹层 |
| `foreground` | `zinc-800` | `zinc-100` | 主文字 | — |
| `card` / `card-foreground` | `white` / `=foreground` | `zinc-900` / `=foreground` | 卡片、面板、侧栏等**静态表面** | 悬浮层 |
| `popover` / `popover-foreground` | `white` / `=foreground` | `zinc-900` / `=foreground` | dropdown、tooltip、dialog 等**悬浮表面**,配 `border` + `shadow-*` | 页内静态容器 |
| `primary` / `primary-foreground` | `zinc-900` / `zinc-50` | `zinc-50` / `zinc-900` | 主 CTA、选中态强调、链接 | 每屏超过一个主 CTA |
| `secondary` / `secondary-foreground` | `zinc-100` / `zinc-900` | `zinc-800` / `zinc-50` | 次级实底按钮、标签 | 作为 hover 态 |
| `muted` / `muted-foreground` | 现值 | 现值 | 静态次级底 / 低对比文字 | 可点击元素的底 |
| `accent` / `accent-foreground` | 现值(6% 叠加) | 现值(10% 叠加) | hover / 选中 / ghost 按钮的**叠加底** | 主按钮、静态容器 |
| `destructive` / `destructive-foreground` | `red-600` / `white` | `red-400` / `zinc-950` | 错误文案、删除按钮、危险边框 | 警告(非破坏性)提示 |
| `border` | 现值 | 现值 | 分隔线、卡片描边 | 表单控件描边 |
| `input` | `zinc-300` | `--alpha(white / 15%)` | 表单控件描边(比 `border` 更强) | 非控件 |
| `ring` | 现值 | 现值 | 焦点环、`outline-color` 兜底 | — |

取舍:

- **dark `background` 900 → 950**:为 `card`/`popover` 腾出「更亮一档」的空间;shadcn v4 zinc 主题同样是 950/900。副作用:现有页面深色底变深,可接受。
- `card` 与 `popover` 取值相同:两者区别靠 `popover` 必带 `border` + `shadow-md`;分成两个令牌是为将来换肤时可独立调整(与 shadcn 一致)。
- `secondary` 与 `muted` 不合并:`secondary` 是实底可交互(按钮),`muted` 是静态底;shadcn 中两者取值相同但语义不同,本仓库 `muted` 带透明度,保持现值。
- `destructive-foreground` 保留成对(shadcn v4 删掉了它改用 `text-white`),遵守本仓库「成对必须同时加」规则。
- 不加 `success/warning/info`:当前无使用场景;规范写明命名(`success/success-foreground`)与取色(`green-600/400`、`amber-600/400`、`blue-600/400`)以便后续按流程添加。

## 3. z-index 档位

```css
:root {
  --z-dropdown: 10;   /* 下拉、自动补全,依附触发器 */
  --z-sticky: 20;     /* 吸顶栏、固定工具栏 */
  --z-overlay: 30;    /* 模态遮罩 */
  --z-modal: 40;      /* 对话框、抽屉 */
  --z-popover: 50;    /* 弹层 / tooltip,可出现在 modal 之上 */
  --z-toast: 60;      /* 全局通知,永远最上 */
}
```

不走 `@theme`(Tailwind 4 无 z-index 命名空间),组件用 `z-(--z-modal)` 引用变量;禁止 `z-50` 裸数字与 `z-[999]`。

## 4. `@layer base` 增量

```css
@layer base {
  html {
    /* 现有 font-size / color-scheme 不变 */
    /* Windows WebView2 默认滚动条粗且不随主题;细滚动条 + 令牌着色 */
    scrollbar-width: thin;
    scrollbar-color: --alpha(var(--color-muted-foreground) / 40%) transparent;
  }
  body {
    /* 现有底色 / 文字 / 字体 不变 */
    /* 桌面应用不需要 overscroll 回弹 */
    overscroll-behavior: none;
  }
  * {
    border-color: var(--color-border);
    /* 焦点兜底:组件忘写 focus-visible:ring 时仍有可见轮廓;写了 ring 的组件通常配 outline-hidden */
    outline-color: --alpha(var(--color-ring) / 50%);
  }
  ::selection {
    background-color: --alpha(var(--color-primary) / 20%);
  }
  /* 系统开启「减少动态效果」时停用动画与过渡。
   * 这是本仓库唯一允许 !important 的地方:必须压过所有工具类,不能依赖层叠顺序 */
  @media (prefers-reduced-motion: reduce) {
    *, ::before, ::after {
      animation-duration: 0.01ms !important;
      animation-iteration-count: 1 !important;
      transition-duration: 0.01ms !important;
      scroll-behavior: auto !important;
    }
  }
}
```

`--alpha()` 与 `light-dark()` 组合已在现有代码中验证可用(Lightning CSS polyfill 路径见 `index.css` 注释),新增用法不引入新构建风险。

## 5. 交互 / 焦点范式(写进规范,组件按此写)

| 状态 | 实底按钮(primary/secondary/destructive) | ghost / 图标按钮 | 表单控件 |
|---|---|---|---|
| 默认 | `bg-primary text-primary-foreground` | `text-foreground` | `border-input bg-background` |
| hover | `hover:bg-primary/90` | `hover:bg-accent hover:text-accent-foreground` | — |
| active | `active:bg-primary/80`(可选) | —(accent 半透明,再降透明会比 hover 更淡) | — |
| focus | `outline-hidden focus-visible:ring-3 focus-visible:ring-ring/50` | 同左 | `outline-hidden focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50` |
| disabled | `disabled:pointer-events-none disabled:opacity-50` | 同左 | 同左 |
| 过渡 | `transition-colors`(默认 150ms / ease) | 同左 | 同左 |

`outline-hidden` 而非 `outline-none`:后者在 forced-colors 下连同被清除的 box-shadow 一起让焦点消失,前者是透明 outline,高对比模式下会被系统色替换而可见(审查 P1-1 决策)。`ring-3` 在 Tailwind 4 是合法动态值(3px);焦点环用 `ring/50` 半透明是 shadcn v4 与 Tailwind Plus 的通行写法,比 2px 实色环在深色下更柔和。

## 6. 层级(海拔)策略

- 浅色:`card` 与 `background` 同为白,层级靠 `border`(+ 可选 `shadow-sm`)。
- 深色:`card`/`popover` 比 `background` 亮一档,`border` 弱化;阴影几乎不可见,不依赖它。
- 悬浮层(`popover`)统一 `border shadow-md`;不新增阴影令牌,`shadow-*` 用 Tailwind 默认。

## 7. 组件变体写法(规范约定,不引依赖)

```ts
const variants = {
  default: "bg-primary text-primary-foreground hover:bg-primary/90",
  secondary: "bg-secondary text-secondary-foreground hover:bg-secondary/80",
  destructive: "bg-destructive text-destructive-foreground hover:bg-destructive/90",
  ghost: "hover:bg-accent hover:text-accent-foreground",
} as const satisfies Record<string, string>;
const sizes = { sm: "h-8 px-3 text-xs", md: "h-9 px-4 text-sm", lg: "h-10 px-6" } as const;
```

- `variant` / `size` 为 props,默认值解构;类名合并用模板字符串或数组 `join(" ")`,不引入 `clsx`/`cva`。
- 为让 prettier-plugin-tailwindcss 排序生效,对象里的类名字符串保持完整字面量,不拼接变量。

## 8. `HelloWorld.vue` 改动点

| 位置 | 现 | 改 |
|---|---|---|
| `<section>` | `rounded-2xl border p-8` | `+ bg-card text-card-foreground` |
| `<input>` | `border ... outline-hidden focus-visible:ring-2 focus-visible:ring-ring` | `border-input ... outline-hidden focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50` |
| `<button>` | `bg-accent text-accent-foreground hover:opacity-80 disabled:cursor-not-allowed disabled:opacity-50` | `bg-primary text-primary-foreground transition-colors hover:bg-primary/90 disabled:pointer-events-none disabled:opacity-50` |
| 错误 `<p>` | `text-foreground` | `text-destructive` |

`disabled:cursor-not-allowed` → `disabled:pointer-events-none`:后者同时挡掉 hover 变色,是 shadcn 与 Tailwind Plus 的通行写法;`cursor-not-allowed` 在 `pointer-events-none` 下本就不会显示。

## 9. 兼容性 / 回滚

- 令牌只增不删,现有类名全部有效;唯一取值变化是 dark `background`。
- 回滚点:单次提交,`git revert` 即可;spec 与代码在同一提交,避免文档与实现脱节。
- 构建验证:`bun run build` 会触发 Lightning CSS 对 `light-dark()` 的 polyfill,新令牌若写错会在这里暴露(计算为非法值时页面整体失色)。
