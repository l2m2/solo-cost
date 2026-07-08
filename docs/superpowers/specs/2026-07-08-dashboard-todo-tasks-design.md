# 仪表盘「待办任务」快捷操作卡片

日期：2026-07-08

## 背景与目标

仪表盘是公司级视图，目前只有财务相关内容（总览 / 分布&排行 / 应收三个 Tab），
没有任何任务信息。用户希望在仪表盘上直接看到待办任务，并能**直接开始 / 完成**，
或**跳转**到对应项目，而不必先进入具体项目再操作。

## 范围

在总览 Tab 底部新增一张「待办任务」卡片：

- 展示当前公司下所有**未关闭**（`status != 'closed'`）的任务，全部列出，不截断。
- 每行是一个可操作的快捷方式：跳转项目、开始、完成。
- 展示预估工时、实际工时（汇总自未软删工时）、实际开始、实际完成时间。
- 开始 / 完成复用项目详情页现有的 `StatusTransitionDialog`（录入时间、工时、描述），
  操作完成后刷新仪表盘数据。

### 明确不做（YAGNI）

- 不改任务本身的增删改逻辑。
- 不加筛选 / 分页。
- 跳转只到项目详情页（`/projects/{id}`），不深链到具体任务。

## 交互与展示

卡片标题：`待办任务 (N)`，N 为全部 `todo`+`in_progress` 任务总数（不受展示条数限制）。

紧凑表格列：

| 列   | 内容 |
|------|------|
| 项目 | 项目名，点击跳转 `/projects/{project_id}` |
| 标题 | 任务标题，点击跳转 `/projects/{project_id}` |
| 负责人 | 成员名，无则显示 `—` |
| 状态 | 徽章：`待办`=slate、`进行中`=amber、`已完成`=emerald，标签复用 `taskStatus.*` |
| 截止日 | 逾期显示红色 `text-red-600`，无截止日显示 `—` |
| 预估 | `estimated_hours`，`{h}h` 或 `—` |
| 实际 | `actual_hours`（>0 时 `{h}h`，否则 `—`）|
| 实际开始 | `started_at` 或 `—` |
| 实际完成 | `completed_at` 或 `—` |
| 操作 | ▶️ 开始（仅 `todo`）、✅ 完成（`todo`/`in_progress`，`done` 不显示）|

- 排序：`due_date` 升序，无截止日排最后。
- 全部列出，不截断。
- 空态：「暂无待办任务」。

## 后端设计（`src-tauri/src/domain/dashboard.rs`）

新增结构：

```rust
#[derive(Debug, Clone, Serialize)]
pub struct DashTaskRow {
    pub task_id: i64,
    pub project_id: i64,
    pub project_name: String,
    pub title: String,
    pub assignee_name: Option<String>,
    pub status: String,
    pub due_date: Option<String>,
    pub overdue: bool,
    pub estimated_hours: Option<f64>,
    pub actual_hours: f64,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}
```

`DashboardSummary` 增加字段：`pub todo_tasks: Vec<DashTaskRow>`。

查询（公司范围、未软删、状态过滤）：

```sql
SELECT t.id, t.project_id, p.name, t.title, m.name, t.status, t.due_date,
       t.estimated_hours, t.started_at, t.completed_at,
       COALESCE((SELECT SUM(hours) FROM time_logs
                 WHERE task_id = t.id AND deleted_at IS NULL), 0.0) AS actual_hours
FROM tasks t
JOIN projects p ON p.id = t.project_id
LEFT JOIN members m ON m.id = t.assignee_id
WHERE p.company_id = ?1 AND p.deleted_at IS NULL
  AND t.deleted_at IS NULL
  AND t.status != 'closed'
ORDER BY (t.due_date IS NULL), t.due_date ASC, t.id ASC
```

- `overdue`：`status != 'done'` 且 `due_date` 存在且 `due_date < today`
  （`today` 已由 `company_dashboard` 传入；已完成任务不标记逾期）。

开始/完成弹框需要完整 Task，由前端在点击操作时调用 `get_task(id)` 按需获取。

## 前端设计

### 1. 提取共享组件

把 `StatusTransitionDialog`（含其内部逻辑，原样搬迁，不改行为）从
`src/routes/projects/detail.tsx` 移到 `src/components/tasks/StatusTransitionDialog.tsx`，
`detail.tsx` 改为 import。这是本次复用的必要拆分，非机会性重构。

### 2. `src/routes/dashboard.tsx`

- 在总览 `TabsContent` 末尾新增待办任务 `<Card>`，复用现有紧凑表格样式。
- 加局部 `TASK_STATUS_CLASS` 映射（`todo`→slate，`in_progress`→amber）。
- 点击开始 / 完成：先 `get_task(task_id)` 取完整 Task → 打开共享
  `StatusTransitionDialog`。
- 提交：
  - 开始：`useTasksStore().update(id, { ...input, status: "in_progress" }, project_id)`
  - 完成：`useTasksStore().update(id, { ...input, status: "done" }, project_id)`，
    若填了本次工时且有负责人，`useTimelogsStore().create(...)`。
  - 逻辑与 `detail.tsx` 现有开始/完成完全一致，用行自带的 `project_id`。
- 成功后 `useDashboardStore().loadFor(currentId)` 刷新仪表盘。
- 跳转用 `react-router-dom` 的 `useNavigate()` → `/projects/{project_id}`。

### 3. `src/types.ts`

- 新增 `DashTaskRow` 类型（对应后端结构，含工时与起止时间字段）。
- `DashboardSummary` 增加 `todo_tasks: DashTaskRow[]`。

### 4. i18n（`src/i18n/zh-CN.json`）

`dashboard` 下新增：`todoTasks`、`taskTitle`、`assignee`、`taskDue`、
`estimated`、`actual`、`startedAt`、`completedAt`、`noTodoTasks`。
项目 / 状态列头复用已有键，任务状态标签复用 `taskStatus.*`。

## 测试

- 后端：在 `domain/dashboard.rs` 现有测试模块中新增用例，构造公司 + 项目 + 任务
  （含不同状态、有/无 due_date、逾期/未逾期、工时与起止时间，及跨公司/软删/关闭），
  断言 `todo_tasks` 的过滤（非关闭）、排序、`overdue` 计算与工时字段正确。
- 前端：手动验证跳转、开始、完成后卡片刷新。
