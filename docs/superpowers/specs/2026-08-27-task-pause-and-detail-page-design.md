# 任务暂停与任务详情页 — 设计

日期：2026-08-27
状态：待评审

## 背景

任务目前只有 `todo / in_progress / done / closed` 四个状态。实际使用中任务会被外部因素卡住（等客户确认、等第三方接口、等素材），既不能算进行中，也不能算完成或关闭。没有第五个状态，这类任务只能挂在「进行中」里，Dashboard 待办卡片会持续把它们当活跃任务提醒，且列表上无法筛选出「卡住的任务」。

同时，任务的历史信息无处存放。`tasks.description` 是一个可覆盖的静态文本字段，写进去的上一段内容会被下一段冲掉。「这个任务为什么停了三周」这种信息在当前模型里留不下痕迹，而它恰好是解释「实际工时为什么超出预估」的关键——那是本产品的核心问题。

## 目标

1. 新增 `paused` 任务状态，暂停时强制记录原因。
2. 新增追加式的任务时间线：手写备注与状态流转按时间穿插，工时记录合并展示。
3. 任务详情从对话框改为独立页面 `/projects/:projectId/tasks/:taskId`。
4. 老任务（含禅道导入）在迁移时回填历史事件，时间线不为空。

## 非目标

明确不做，防止实施时范围蔓延：

- 备注内容不进全局搜索索引。
- 不做历史累计暂停时长统计，详情页只显示「当前已暂停 N 天」。
- 不做字段级 diff 日志（谁把预估工时从 8h 改成 16h）。本产品是单人成本核算工具，通用审计日志的价值不成立。
- 不做项目详情页的 tab 路由化。现有 tab 用 `useState` 且不能深链，这是真问题，但与本次改动无关，属于独立改动。
- 不合并 `tasks.description` 与备注流。`description` 继续承担「任务是什么」的静态说明，备注流承担「发生了什么」。

## 关键设计决策

### 为什么是详情页而不是对话框

对话框的心智模型是「做完一件事就关掉」，有一个隐含的完成动作。而「持续追加备注」和「翻历史」是停留式浏览：没有终点，长度不可预测，用户会来回滚动比对时间。两种模型冲突。

支撑证据：

- 任务已经有 3 个互不连通的对话框（编辑 `TaskForm`、工时 `TimeLogsSection`、开始/完成 `StatusTransitionDialog`）。再加备注和时间线会到 5 个，唯一的出路是在对话框里套 tab。
- 深链已存在但半残。`CommandPalette.tsx:71` 跳的是 `/projects/:id?task=<id>`，落地效果只是列表里某一行黄底高亮。用户搜到一个任务，想看的是这个任务。
- `detail.tsx` 已 1634 行，`TasksPanel` 占约一半。

### 为什么是一张 `task_events` 表而不是备注表 + 日志表

备注和状态流转在阅读时是同一条叙事：「开始 → 记 4h → 暂停（等客户素材）→ 恢复 → 记 2h → 完成」。拆成两张表、两个 UI 区块，用户要自己对照时间戳把它拼回去。

合并的直接收益：暂停原因就是那条 `status_change` 事件自带的 `body`，不需要在 `tasks` 表上加 `pause_reason` 字段。

### 工时为什么不写进 `task_events`

`time_logs` 带 `daily_cost_snapshot_cents`，是财务数据，有自己的增删改路径。双写事件必然出现不同步（工时改了、删了，事件怎么办）。因此 `time_logs` 保持独立表，只在**渲染层**按时间合并进同一条时间线。

## 数据层

### 迁移 `0009_task_events.sql`

`tasks` 表第三次重建，CHECK 约束加入 `paused`。同一迁移内建立事件表。

```sql
CREATE TABLE task_events (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id      INTEGER NOT NULL REFERENCES tasks(id),
    kind         TEXT    NOT NULL CHECK (kind IN ('note','status_change')),
    from_status  TEXT,
    to_status    TEXT,
    body         TEXT,
    occurred_at  TEXT    NOT NULL,
    created_at   TEXT    NOT NULL DEFAULT (datetime('now')),
    deleted_at   TEXT
);
CREATE INDEX idx_task_events_task ON task_events(task_id, deleted_at, occurred_at);
```

两个字段的理由：

- `occurred_at` 与 `created_at` 分离。现有 `StatusTransitionDialog` 已允许补填过去的开始/完成时间——事实发生时间与记录时间不是一回事。时间线按 `occurred_at` 排序。
- `deleted_at` 与全库软删机制一致，见下文级联。

`tasks` 表重建照 `0008_tasks_status_dates.sql` 的模式（建 `tasks_new` → 拷数据 → drop → rename → 重建全部索引）。迁移运行器在整个迁移过程关闭了 `foreign_keys`，drop 被 `time_logs` 引用的 `tasks` 不会触发约束错误。

`migrations.rs` 的 `MIGRATIONS` 数组追加条目，测试里 `assert_eq!(current_version(&conn).unwrap(), 8)` 改为 9。

### 历史回填

同一迁移内，对每个 `deleted_at IS NULL` 的任务生成事件，`body` 留空：

| 条件 | 生成事件 | `occurred_at` |
|---|---|---|
| 总是 | `to_status='todo'` | `tasks.created_at` |
| `started_at IS NOT NULL` | `to_status='in_progress'`，`from_status='todo'` | `started_at` |
| `completed_at IS NOT NULL` | `to_status` 取当前 `status`（`done` 或 `closed`） | `completed_at` |

禅道导入的任务一并覆盖（它们的 `created_at` 是源系统时间，回填结果反而更准确）。

回填后 `task_events` 成为流转历史的唯一来源，前端不需要写「老数据兼容」分支。

### 约束校验

以下规则在 Rust 侧校验而非 SQLite CHECK，因为错误信息需要中文化：

- `kind='note'` 时 `body` 去空后非空。
- `kind='status_change'` 且 `to_status='paused'` 时 `body` 去空后非空，错误信息「暂停任务必须填写原因」。
- `body` 长度上限 2000 字符。

## 后端

### 状态机

```
todo        → in_progress | closed
in_progress → paused | done | closed
paused      → in_progress | done | closed
done        → closed
closed      → ✗
```

同状态到同状态视为 no-op：不报错，也不写事件。

`create_task` 不走转换校验（创建时的 `status` 是初始值，禅道导入依赖这一点），只走 `ALLOWED_STATUSES` 值合法性校验。`ALLOWED_STATUSES` 加入 `"paused"`。

### 命令签名调整

现状：`update_impl` 通过 `status = COALESCE(?4, status)` 顺带改状态，`StatusTransitionDialog` 靠这个把「填完成时间 + 改描述 + 状态改 done」塞进一次调用。拆成两次 IPC 会失去原子性。

```rust
#[derive(Deserialize)]
pub struct StatusChange {
    pub to: String,
    pub occurred_at: Option<String>,  // None → datetime('now')
    pub body: Option<String>,
}

update_task(id: i64, input: TaskInput, change: Option<StatusChange>) -> Task
set_task_status(id: i64, change: StatusChange) -> Task
```

两者共用内部 `apply_transition(tx, id, from_status, &change)`，整体包在 `conn.unchecked_transaction()` 内（与 `soft_delete.rs` 的模式一致）。

**行为变化（需要点名）**：`TaskInput.status` 在 update 路径上不再生效，`update_impl` 中的 `status = COALESCE(?4, status)` 改为不再更新 status 列。`TaskInput.status` 字段保留，仅供 `create_task` 与禅道导入使用。

理由：若保留两条改状态的路径而其中一条不写事件，日志就不可信了——那是这类功能失效的典型成因。

由于 `TaskForm` 中状态是只读的（`const [status] = useState(...)`，渲染为 disabled input），状态只能经专用动作变更，此改动不影响任何现有前端流程。

### `occurred_at` 取值

| 动作 | `occurred_at` |
|---|---|
| 开始 | 弹窗中选择的开始时间 |
| 完成 | 弹窗中选择的完成时间 |
| 暂停 / 恢复 | 弹窗中选择的时间 |
| 列表上一键关闭 | `datetime('now')` |

这保证时间线的排序与列表上显示的 `started_at` / `completed_at` 始终一致。

### 新命令 `src-tauri/src/commands/task_events.rs`

- `list_task_events(task_id) -> Vec<TaskEvent>`，按 `occurred_at` 升序返回，前端负责倒序渲染。
- `create_task_note(task_id, body, occurred_at: Option<String>)`
- `update_task_note(id, body)`
- `delete_task_note(id)` — 软删

备注可改可删；`kind='status_change'` 的事件不可改不可删，它是记录而非内容。`update_task_note` / `delete_task_note` 需校验目标事件 `kind='note'`，否则返回 `AppError::Validation`。

在 `commands/mod.rs` 注册模块，在 `lib.rs` 的 `invoke_handler` 注册四个命令。

### 软删级联

`domain/soft_delete.rs`：

- `soft_delete_task` / `restore_task`：带上 `task_events`，按 `task_id = ?` 匹配。
- `soft_delete_project` / `restore_project`：按 `task_id IN (SELECT id FROM tasks WHERE project_id = ?)` 匹配。

还原用现有的 `deleted_at = ?ts` 配对模式，照抄 `time_logs` 的两段即可。

### 回收站

`commands/trash.rs` 的硬删（`:147-170`）加上 `task_events`：purge 项目时按 `task_id IN (SELECT id FROM tasks WHERE project_id = ?1)`，purge 任务时按 `task_id = ?1`。

**备注不出现在回收站列表中**。它有 `deleted_at` 的唯一理由是：删除任务后再还原时，备注必须跟着回来。回收站列表面向有财务后果的实体（项目 / 成本 / 任务 / 收款 / 工时），单条备注进去是噪音。代价是手动删除的备注不可恢复，因此删除需二次确认（复用 `lib/confirm.ts` 的 `confirmDialog`）。这是有意的取舍。

### 备份

`domain/backup.rs` 为文件级复制，不枚举表，新表无需注册。

## 前端

### 路由

`App.tsx` 在 `AppLayout` 下新增：

```tsx
<Route path="projects/:projectId/tasks/:taskId" element={<TaskDetailPage />} />
```

采用独立顶层路由而非嵌套在项目详情 tab 内。嵌套方案需要先把 tab 从 `useState` 改成路由驱动，连带改动 `?task=` 补丁与 `CommandPalette`，属于扩大范围（见非目标）。面包屑一次点击即可返回列表，体验损失可接受。

### 文件拆分

```
src/routes/projects/detail.tsx          保留项目 shell + 财务/成本/收款面板
src/routes/projects/tasks/panel.tsx     TasksPanel + TaskForm（从 detail.tsx 迁出）
src/routes/projects/tasks/detail.tsx    任务详情页（新建）
src/components/tasks/Timeline.tsx       时间线：事件 + 工时合并渲染
src/components/tasks/NoteComposer.tsx   备注输入
src/components/tasks/TimeLogForm.tsx    工时表单（从 detail.tsx 迁出）
src/stores/taskEvents.ts                事件 store
```

迁出的代码只搬不改，唯一的行为改动是 `TasksPanel` 中点击任务标题从 `setEditing(tk)` 改为 `navigate()`。

`TimeLogsSection`（`detail.tsx:1304`）不整体迁移：它由「提示语 + 添加按钮 + 工时表格 + 编辑/删除」四部分组成，在详情页上表格部分由时间线接管，只有 `TimeLogForm` 及其增删改回调被复用。`TimeLogsSection` 本身随对话框一起下线。

### 详情页布局

单栏，自上而下：

1. **面包屑**：`项目名 / 任务`，带返回。返回目标为 `/projects/:pid?task=<id>`，复用列表页现有的行定位逻辑。
2. **标题行**：任务标题、状态徽章、右侧动作按钮（开始 / 暂停 / 恢复 / 完成 / 关闭，按状态机显隐）。删除任务的入口随编辑对话框一并迁到这里（现位于 `TaskForm` 的 footer），删除成功后 `navigate` 回列表。
3. **暂停横幅**：仅当前状态为 `paused` 时显示「已暂停 N 天 · 原因」。N 自最近一条 `to_status='paused'` 事件的 `occurred_at` 算起，原因取该事件的 `body`。
4. **基本信息卡**：负责人、模块、预估/实际工时、截止日期、开始/完成时间。就地编辑，复用 `TaskForm` 的字段布局，去掉 Dialog 外壳。因此 `TaskForm` 需去除对 `useFormDialog()` 的硬依赖——把 `markDirty` 改为可选（`useContext` 返回 null 时降级为 no-op），而不是抛错。
5. **备注输入框**：常驻，不需要点「添加」再展开。右侧放「记录工时」按钮，点开 `TimeLogForm` 对话框。
6. **时间线**：`task_events` 与 `time_logs` 按时间倒序合并。工时条目渲染为「记录工时 4h · 张三」，事件条目渲染为状态变化或备注正文。

工时与备注条目在时间线上 hover 出现编辑 / 删除按钮，分别调用 `timelogs` store 与 `taskEvents` store。状态流转条目无操作按钮。时间线因此是详情页上工时的唯一列表——不再另设独立的工时区块，否则同一批数据会在页面上出现两次。

### 列表页

两个对话框下线：编辑任务（`detail.tsx:1028-1061`）与工时清单（`:1063-1070`），连同 `editing` / `openLogs` 两个 state。点击标题与点击工时图标都改为跳转详情页。开始 / 完成 / 暂停 / 恢复 / 关闭保留在列表上就地操作——扫一眼列表快速打点是本工具的高频动作，强制先进详情页是倒退。

`detail.tsx:959-997` 的图标按钮列新增暂停 / 恢复：`in_progress` 显示暂停，`paused` 显示恢复。

- 暂停：必弹窗（原因必填）。
- 恢复：轻量弹窗（时间 + 选填备注）。

`StatusTransitionDialog` 扩展一个 `paused` 分支承载这两个动作，不新建组件——列表页与 Dashboard 卡片共用它正是为了避免行为分叉。

### 配色

`TASK_STATUS_BADGE_CLASS` 加 `paused: "bg-rose-100 text-rose-700"`（避开 `in_progress` 的 amber）。走 shadcn 中性色，不用 `lib/brand.ts` 的品牌色——后者明确只服务于 shell 与 dashboard。

### Dashboard

`domain/dashboard.rs` 三处：

- `:426` `:439` 的 `t.status != 'closed'` 不改。`paused` 自动进入待办列表是正确的——暂停的任务仍然是待办。
- `:445` 逾期判定 `status != "done"` **改为同时排除 `paused`**。任务停着是因为在等外部依赖，标成逾期是归因错误。
- `:440` 排序 `ORDER BY (t.status = 'done'), ...` 插入 `(t.status = 'paused')` 作为第二键，顺序变为 活跃 → 暂停 → 已完成。

`components/dashboard/` 的待办卡片同步加暂停 / 恢复按钮，并渲染 `paused` 徽章。

### 全局搜索

`commands/search.rs` 不改。`CommandPalette.tsx:71` 的任务跳转改为 `/projects/${hit.project_id}/tasks/${hit.id}`。

`TasksPanel` 中的 phase1/phase2 行定位逻辑（`detail.tsx:763-800`）保持不变：它失去了搜索这个调用方，但获得了「从详情页返回」这个调用方。

### i18n

`src/i18n/zh-CN.json` 新增：`taskStatus.paused`、`task.pause`、`task.resume`、`task.pauseReason`、`task.pauseReasonRequired`、`task.noteRequired`、`task.noteDeleteConfirm`、`timeline.*`（空状态、工时条目、状态变化文案）。

## 验证

项目无前端测试框架（`package.json` 无 vitest / jest / playwright），因此：

**Rust 单测**，照 `tasks.rs` 现有 `TestDb` 模式：

- 合法转换通过，非法转换（如 `todo → done`、`closed → in_progress`）被拒。
- 同状态到同状态为 no-op，不产生事件。
- `to_status='paused'` 缺 `body` 被拒。
- 每次成功流转恰好写入一条 `task_events`，`from_status` / `occurred_at` 正确。
- `update_task` 传入 `input.status` 不再改变状态。
- `soft_delete_task` / `restore_task` / `soft_delete_project` / `restore_project` 级联到 `task_events`。
- purge 任务 / 项目时 `task_events` 被硬删。
- `update_task_note` / `delete_task_note` 对 `kind='status_change'` 的事件返回 Validation。
- 迁移回填：有 `started_at` 与 `completed_at` 的老任务生成三条事件，顺序与时间正确。

**构建与静态检查**：`pnpm build`（含 `tsc -b`）、`pnpm lint`。

**手工验证**：

- 迁移后打开历史任务，时间线非空且时间正确。
- 暂停 → 恢复 → 完成 全链路，事件依次出现。
- 暂停时不填原因被拦截。
- 从详情页返回列表，筛选、分页、滚动位置正确恢复。
- 全局搜索命中任务直达详情页。
- Dashboard：暂停的逾期任务不再标红，排序位于活跃任务之后。

## 发布

四块作为一个版本一起发布。它们互相依赖较重（暂停原因存在 `task_events` 中，时间线只在详情页上），拆分发布反而需要编写丢弃代码。

版本号按 SemVer 0.x 规则，MINOR 递增至 `0.6.0`。
