# 前端设计规范（DESIGN.md）

本文件是前端页面的设计规范：人与 agent 共同遵守。规则必须可观察、可执行——不做"布局要美观"这类主观描述。

分层原则（每条修正落在最窄的能执行它的地方）：

- **判断类规则** → 写入本文件
- **可复用机制** → 做成组件或 CSS（见"通用组件"与 token 层）
- **机械可查的** → Biome / tsc / 组件测试

每次人工纠正页面实现后，应把该纠正沉淀为本文件的条目，而不是每次口头重说。

## 1. Token 层（唯一事实源）

所有颜色、圆角、阴影必须引用 token，**禁止硬编码 hex 色值或任意间距**。

- 语义色：`web/src/index.css` 的 CSS 变量（shadcn 语义对 `--background`/`--foreground`、`--card`、`--muted`、`--destructive` 等），经 `tailwind.config.ts` 映射为 `bg-background`、`text-muted-foreground` 等工具类。
- 扩展语义色：`success` / `warning` / `info`（各自带 `-foreground`）。
- 圆角：`--radius: 1rem` 为基础，组件内用 `rounded-lg`/`rounded-2xl`/`rounded-3xl` 阶梯，不要自造值。
- 暗色模式：`class` 策略（`.dark` 选择器覆盖同一组变量），组件里**不写** `dark:` 下的具体色值来"对抗"token——先看语义色是否够用。
- 自定义工具类：`content-surface`（卡片玻璃拟态表面）、`page-enter`（页面进入动画）、`app-header`（吸顶头栏，由 `sticky-header.css` 的 scroll-state 驱动）。新页面直接复用，不要重写表面样式。

## 2. 页面布局模式

新页面（`src/pages/*.tsx`）遵守以下结构，参照 `notes.tsx` / `settings.tsx`：

- **顶层容器**：页面组件返回 `<div className="space-y-6">`（不是裸 fragment）。layout 的 `page-enter` 容器没有间距布局，页面内部元素间距全靠这个容器。
- **页头**：`<PageHeader icon={...} title={t("...")}>`，页面级操作按钮（如"新建"）作为 children 放右侧。
- **数据表格**：用 `<Card className="overflow-x-auto">` 包裹 `<Table>`，与 settings/notes 页一致；不要用 `DataTableToolbar` 包表格（它是搜索/筛选工具条的 flex 容器）。
- **加载态**：整页用 `PageHeaderSkeleton` + 内容骨架；表格用 `<TableSkeleton columns={n} />`。
- **错误态**：`<ErrorState onRetry={...} />`。
- **空态**：`<EmptyState icon title description action />`。
- **删除确认**：一律走 `<ConfirmDialog>`，不自己写 AlertDialog。
- **时间显示**：相对时间用 `<RelativeTime date={...} />`；需要绝对时间时 `toLocaleString(i18n.language)`。

## 3. 国际化（i18n）

- **所有用户可见文案必须走 `t("namespace.key")`**，包括表单校验消息、toast、aria-label。硬编码文案是缺陷。
- zh-CN（`locales/zh-CN.ts`）是源；`locales/en.ts` 用 `Translation` 类型对齐，缺键/多键 tsc 直接报错——**用到的 key 不存在时不会构建失败，会直接渲染 key 原文上线**，因此新增 key 必须两份 locale 同步。
- 日期格式化跟随界面语言（`i18n.language`），不用浏览器语言；日志类时间戳（机器可读）可用固定 `YYYY-MM-DD HH:mm:ss` 格式。
- 文案 key 按域组织（`notes.*`、`cronJobs.*`、`common.*`）；通用操作（确认/取消/编辑/删除）用 `common.*`，不重复造域内同义 key。
- 品牌字符串统一为 `selfhosted-template`；浏览器存储 key 统一 `selfhosted-template-*` 前缀（与 `index.html` 防闪烁脚本、zustand persist name 三处保持同步）。

## 4. 主题切换

- `useTheme` 三态（light/dark/system），持久化于 zustand persist。
- `setTheme` 在**解析后的主题不变**时（如手动暗色 → 跟随系统且系统为暗色）只更新偏好，不重放切换动画、不重写 DOM。
- 切换动画是 View Transition 圆形揭示（`use-theme.ts` 注入动态 style 烘焙圆心坐标）；用户偏好减少动效时跳过。

## 5. 反模式清单（命名）

以下模式已在评审中出现过，看到即修，不要新增：

- **裸 fragment 页面**：页面顶层直接 `<>`，页头与内容零间距。→ 顶层 `space-y-6` 容器。
- **裸表格**：`<Table>` 无 `Card` 包裹，与其他页面卡片风格断裂。→ `Card className="overflow-x-auto"`。
- **硬编码文案**：组件里出现非 i18n 的用户可见字符串。→ `t()` + 两份 locale 补 key。
- **浏览器 locale 日期**：`toLocaleString()` 不带语言参数。→ `toLocaleString(i18n.language)` 或 `RelativeTime`。
- **missing key 上线**：locale 缺 key 导致界面渲染出 `common.xxx` 字样。→ 合并前 grep 用到的 key 与 locale 比对（或依赖 Translation 类型对齐）。
- **token 逃逸**：className 里出现 hex 色值/任意间距值。→ 换语义 token 或既有阶梯。
