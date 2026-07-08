# 仪表盘视觉精致化打磨（方案 B）

日期：2026-07-08

## 背景与目标

仪表盘功能完整，但视觉偏"默认模板感"：标准 shadcn zinc 中性主题、KPI 卡片是灰底
小标签 + `text-lg` 数字、进度条与表格样式朴素。目标是在**保持简洁专业**的前提下
做精致化打磨，去除模板感，覆盖总览 / 分布&排行 / 应收三个 Tab。

## 原则

- 不改数据、接口、Tab 结构，不引入图表库。
- 不改全局 zinc 基调，只把已在用的语义色（emerald / red / amber / sky）用得更一致。
- 纯前端展示层改动，集中在 `src/routes/dashboard.tsx`。

## 打磨项

### 1. KPI 卡片（centerpiece）

当前 `Kpi` 组件：灰底、`text-xs` 标签、`text-lg` 数字。改为：

- 左上角一个柔和色底的图标 chip（lucide 图标），作为视觉锚点。
- 数字放大到 `text-2xl font-semibold tabular-nums`，货币对齐。
- 新增可选 `accent` 属性：到手类指标（潜在到手 / 已收到手）用 emerald 前景色强调，
  其余保持中性。
- hover 态：`shadow-sm` + 边框微亮（`transition`）。
- 每个 KPI 传入对应图标与 accent，由调用处决定。

分区标题（合同口径 / 已收口径）：统一为 `text-xs font-medium uppercase tracking-wide
text-muted-foreground`，与卡片拉开层次。

### 2. 进度条（年度到手 / 状态分布）

统一为一套圆角进度条样式：

- 顶部小色点图例：年度卡片「已收=slate、到手=emerald」；状态分布沿用 sky。
- 右侧显示百分比（占最大值比例），数字 `tabular-nums`。
- 条形圆角、统一高度与背景。

### 3. 表格（排行 / 待办 / 应收）

- 表头统一：`text-xs uppercase tracking-wide text-muted-foreground`。
- 行 hover 底色（`hover:bg-muted/50`）。
- 货币 / 数值列 `tabular-nums` 右对齐。
- 排行表左侧加名次序号 `1 / 2 / 3`，前三名用微强调（例如加重或小圆底）。

### 4. 页头

`仪表盘` 标题与刷新按钮一行；标题下方可加一行 `text-sm text-muted-foreground` 副标题
（如"公司经营一览"），增强层次。保持刷新按钮现状。

## 组件与复用

- `Kpi`：增加 `icon`、`accent` 两个可选属性；不破坏现有调用（默认中性无图标）。
- 新增小型展示辅助：`SectionTitle`（分区标题）、`Legend`（色点图例）、`Bar`（统一进度条），
  就近定义在 `dashboard.tsx`，各自单一职责、便于复用。
- `RankCard`：增加名次序号列。
- 表头样式抽为一个 `className` 常量，避免重复。

## 范围之外（YAGNI）

- 不做数据可视化图表（趋势线 / 环形图）。
- 不做 Tab 数量角标、不做英雄指标重排（方案 C 的内容）。
- 不改后端、不改其它页面。

## 测试

- 无逻辑改动，无新增单测。
- 手动验证：三个 Tab 渲染正常、`pnpm tsc` 与 `oxlint` 通过、深浅数值对齐、
  hover 与空态显示正常。
