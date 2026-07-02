# 项目模块 设计文档

- **创建日期**：2026-07-02
- **作者**：l2m2
- **状态**：设计已定稿，待实现
- **关联主设计**：[2026-06-29-solo-cost-design.md](2026-06-29-solo-cost-design.md)
- **依赖 / 被依赖**：本 feature 是 [2026-07-02-zentao-import-notes.md](2026-07-02-zentao-import-notes.md) 描述的「禅道 CSV 导入」的前置依赖；导入功能会消费本 spec 建立的 `tasks.module_id` 与「模块管理」UI

---

## 1. 背景与目标

### 1.1 现状

- `tasks` 表只有 `title, description, assignee_id, status, estimated_hours, due_date`，没有二级归类
- 项目详情的「任务+工时」tab 展示所有任务，没有分组或过滤维度（除状态外）
- 财务面板只有整个项目维度的人力成本合计，看不到某一类工作（前端 / 后端 / 现场实施 / 硬件调试等）各花了多少

### 1.2 目标

- 给每个项目引入**扁平**的「模块」（module）列表，作为任务的可选归类
- 项目详情「任务+工时」tab 支持按模块过滤任务、按模块看人力成本
- 为后续「禅道 CSV 导入」铺路——ZenTao 数据里带模块信息，本 feature 落地后导入功能可以直接读入

### 1.3 非目标（本 spec 明确不做）

- 模块嵌套 / 树形层级（禅道有 `/(#0)/前端/表单` 这类，本次扁平化处理；导入时若源为嵌套，压平为叶子名或全路径字符串——由 Feature 2 决定）
- 模块级预估工时 / 截止日期
- 跨项目共享模块目录（明确项目级）
- 按模块的通用成本 (`cost_entries`) 汇总——`cost_entries` 不挂模块
- 模块级 Kanban / 图表 / 燃尽图
- 拖拽排序（本次做 ↑↓ 按钮，DnD 留后续增强）

## 2. 用户诉求

来源：l2m2 的实际项目管理口径，2026-07-02 对齐。

- 大项目里会分几个功能块（如 `前端 / 后端 / 硬件 / 现场实施`），需要按块统计各投了多少工时
- 项目可以完全不用模块——所有任务归到「未分类」，行为等同现状
- 从禅道导入时应能保留模块归属信息（Feature 2 会消费）

## 3. 领域模型

### 3.1 数据模型

- 新表 `modules`：项目级、扁平（无 `parent_id`）
- 现表 `tasks`：新增 `module_id INTEGER NULL REFERENCES modules(id)`
- 模块支持软删除（`deleted_at`），沿用现有 soft-delete 惯例
- 允许模块名重复吗？**不做唯一约束**——同一项目下两个「后端」允许存在（避免用户误录后无法保存），排序 + 前端提示由用户自理

### 3.2 删除策略

**强拒绝** 如果模块下存在**未软删**的任务：

- 后端 `soft_delete_module` 先查 `SELECT 1 FROM tasks WHERE module_id = ?1 AND deleted_at IS NULL LIMIT 1`，命中即返回 `AppError::DeleteBlocked("模块下还有任务，请先删除或转移")`
- 前端 toast 提示并保留对话框状态

### 3.3 任务与模块的关系

- `task.module_id` **可选**（Nullable）
  - 老任务迁移后 = NULL（自动进「未分类」桶）
  - 新任务表单里默认「未分类」
  - 用户可切回「未分类」——`UPDATE tasks SET module_id = NULL`
- **跨项目校验**：`create_task` / `update_task` 时若 `module_id` 非空，后端 `SELECT project_id FROM modules WHERE id = ?1 AND deleted_at IS NULL`——命中 project_id 与任务的 project_id 不一致或未命中 → `AppError::Validation("模块不属于当前项目")`。防止前端 bug 或 IPC 手工调用把 A 项目模块挂到 B 项目任务

### 3.4 排序

- `sort_order INTEGER NOT NULL DEFAULT 0`
- `create_module` 自动 `sort_order = MAX(sort_order) + 1`
- 用户在管理弹窗按「上移 / 下移」→ 前端计算相邻两行 sort_order 互换 → 各 UPDATE 一次（两次 IPC）
- 列表查询 `ORDER BY sort_order ASC, id ASC`（相同 sort_order 用 id 兜底稳定）

## 4. 数据库迁移

新增文件：`src-tauri/migrations/0005_modules.sql`

```sql
-- Project modules + tasks.module_id
BEGIN;

CREATE TABLE modules (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  project_id   INTEGER NOT NULL REFERENCES projects(id),
  name         TEXT    NOT NULL,
  sort_order   INTEGER NOT NULL DEFAULT 0,
  created_at   TEXT    NOT NULL DEFAULT (datetime('now')),
  updated_at   TEXT    NOT NULL DEFAULT (datetime('now')),
  deleted_at   TEXT
);
CREATE INDEX idx_modules_project ON modules(project_id, deleted_at);

ALTER TABLE tasks ADD COLUMN module_id INTEGER REFERENCES modules(id);
CREATE INDEX idx_tasks_module ON tasks(module_id) WHERE module_id IS NOT NULL;

COMMIT;
```

**兼容性**：
- 老 `tasks` 行 `module_id` 自动为 NULL，视为「未分类」，全部现有行为不变
- 不加 CHECK 约束——校验放 Rust 层

## 5. 后端变更

### 5.1 新文件 `src-tauri/src/commands/modules.rs`

结构体：

```rust
pub struct Module {
    pub id: i64,
    pub project_id: i64,
    pub name: String,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

pub struct ModuleInput {
    pub name: String,
    pub sort_order: Option<i64>,
}
```

校验：

```rust
fn validate(input: &ModuleInput) -> AppResult<()> {
    let n = input.name.trim();
    if n.is_empty() || n.chars().count() > 40 {
        return Err(AppError::Validation("模块名长度必须在 1–40 之间".into()));
    }
    Ok(())
}
```

IPC 命令：

| 命令 | 语义 |
|---|---|
| `list_modules(project_id) → Vec<Module>` | `WHERE project_id = ?1 AND deleted_at IS NULL ORDER BY sort_order ASC, id ASC` |
| `create_module(project_id, input) → Module` | 校验；`sort_order = COALESCE(input.sort_order, MAX(existing)+1)` |
| `update_module(id, input) → Module` | 校验；单独或同时改 name / sort_order；使用 COALESCE 支持部分更新 |
| `delete_module(id) → ()` | 强拒绝路径（见 §3.2）；否则 `deleted_at = datetime('now')` |

### 5.2 修改 `src-tauri/src/commands/tasks.rs`

- `Task` 与 `TaskInput` 各加：
  ```rust
  pub module_id: Option<i64>,  // Task: 可为 None，TaskInput: 同名 Option
  ```
- `row_to_task`：`row.get("module_id")?`
- 跨项目校验做成独立小函数，`create_impl` 与 `update_impl` 各自组织 `task_project_id` 后再调用：
  ```rust
  fn validate_module_belongs_to_project(
      conn: &Connection,
      module_id: Option<i64>,
      task_project_id: i64,
  ) -> AppResult<()> {
      let Some(mid) = module_id else { return Ok(()); };
      let pid: Option<i64> = conn.query_row(
          "SELECT project_id FROM modules WHERE id = ?1 AND deleted_at IS NULL",
          [mid], |r| r.get(0)).optional()?;
      match pid {
          Some(p) if p == task_project_id => Ok(()),
          _ => Err(AppError::Validation("模块不属于当前项目".into())),
      }
  }
  ```
  - `create_impl(conn, project_id, input)`：直接把外部传入的 `project_id` 作为 `task_project_id`
  - `update_impl(conn, task_id, input)`：先 `SELECT project_id FROM tasks WHERE id = ?1 AND deleted_at IS NULL`（一次多读，成本可接受），再传入该函数
- `create_impl` INSERT 加 `module_id`；`update_impl` UPDATE 用 `module_id = ?N`（直接覆盖，允许显式设为 NULL——从"某模块"切回「未分类」的路径）

### 5.3 新文件 `src-tauri/src/domain/module_stats.rs`

```rust
#[derive(Serialize)]
pub struct ModuleLaborStat {
    pub module_id: Option<i64>,      // None = 未分类
    pub module_name: Option<String>, // None = 未分类
    pub hours: f64,
    pub cost_cents: i64,
}

pub fn labor_by_module(
    conn: &Connection,
    project_id: i64,
) -> AppResult<Vec<ModuleLaborStat>>
```

SQL：

```sql
SELECT t.module_id,
       m.name AS module_name,
       COALESCE(SUM(tl.hours), 0.0) AS hours,
       COALESCE(SUM(ROUND(tl.hours / 8.0 * tl.daily_cost_snapshot_cents)), 0) AS cost
FROM tasks t
LEFT JOIN modules m
       ON m.id = t.module_id AND m.deleted_at IS NULL
LEFT JOIN time_logs tl
       ON tl.task_id = t.id AND tl.deleted_at IS NULL
WHERE t.project_id = ?1 AND t.deleted_at IS NULL
GROUP BY t.module_id, m.name
HAVING hours > 0
ORDER BY m.sort_order ASC NULLS LAST, m.id ASC
```

- 未分类桶 `module_id=NULL, module_name=NULL`，前端渲染时替换为「未分类」文本
- 仅返回 hours > 0 的行，避免"新建但未记工时"的模块出现噪音
- 前端拿到 `cost_cents` 直接展示（不做浮点二次运算，精度安全）

### 5.4 注册

- `src-tauri/src/commands/mod.rs`：`pub mod modules;`
- `src-tauri/src/domain/mod.rs`：`pub mod module_stats;`
- `src-tauri/src/lib.rs`：`invoke_handler!` 追加：
  - `commands::modules::{list_modules, create_module, update_module, delete_module}`
  - 一个新 IPC `get_module_labor_stats(project_id)` 包装 `domain::module_stats::labor_by_module`
- `src-tauri/src/db/migrations.rs`：`MIGRATIONS` 数组追加 `("0005_modules", include_str!(...))`，`current_version` 断言 4→5

### 5.5 单元测试（`cargo test`）

`commands::modules::tests`：

1. `create_defaults_sort_order_to_max_plus_one` — 连续 3 次 create，sort_order = 0/1/2
2. `create_persists_name_and_project`
3. `update_can_rename` — 单改 name，sort_order 保留
4. `update_can_reorder` — 单改 sort_order，name 保留
5. `list_orders_by_sort_order_then_id`
6. `list_excludes_soft_deleted`
7. `delete_blocks_when_task_attached` — 挂 task 的模块删除 → `DeleteBlocked`
8. `delete_succeeds_when_no_tasks` — 空模块可删
9. `validate_rejects_empty_name` / `validate_rejects_too_long_name`

`commands::tasks::tests`（追加 3 个）：

10. `create_task_with_module_persists_module_id`
11. `create_task_rejects_module_from_other_project`
12. `update_task_can_clear_module_to_null`

`domain::module_stats::tests`：

13. `labor_by_module_empty_project_returns_empty`
14. `labor_by_module_unassigned_bucket_only` — 仅 module_id=NULL 的 tasks，返回 1 行 `module_id=None`
15. `labor_by_module_mixes_named_and_unassigned` — 前端 20h + 后端 30h + 未分类 8h → 3 行、排序正确、cost 值正确
16. `labor_by_module_excludes_soft_deleted_tasks_and_logs`
17. `labor_by_module_uses_snapshot_daily_cost` — 成员改日薪不影响历史 timelog 成本

## 6. 前端变更

### 6.1 类型 `src/types/index.ts`

```ts
export interface Module {
  id: number;
  project_id: number;
  name: string;
  sort_order: number;
  created_at: string;
  updated_at: string;
}
export interface ModuleInput {
  name: string;
  sort_order?: number | null;
}
export interface ModuleLaborStat {
  module_id: number | null;
  module_name: string | null;
  hours: number;
  cost_cents: number;
}
// Task / TaskInput 各加：module_id: number | null; / module_id?: number | null;
```

### 6.2 新 store `src/stores/modules.ts`

```ts
interface S {
  byProject: Record<number, Module[]>;
  loadedForProject: Record<number, boolean>;
  loadFor(projectId: number): Promise<void>;
  create(projectId: number, input: ModuleInput): Promise<Module>;
  update(id: number, input: ModuleInput, projectId: number): Promise<Module>;
  moveUp(id: number, projectId: number): Promise<void>;   // swap sort_order with above
  moveDown(id: number, projectId: number): Promise<void>; // swap with below
  softDelete(id: number, projectId: number): Promise<void>;
}
```

### 6.3 新 store `src/stores/moduleStats.ts`

```ts
interface S {
  byProject: Record<number, ModuleLaborStat[]>;
  refresh(projectId: number): Promise<void>;
}
```

任务/工时的 create/update/delete/softDelete 时调用 `refresh`（同 `useFinancialStore.refresh` 联动模式）。

### 6.4 UI 改动 `src/routes/projects/detail.tsx::TasksPanel`

从上到下新布局：

```
┌ 顶部工具栏 ─────────────────────────────────┐
│  [状态 ▾]  [模块 ▾]  [管理模块]   ─── [新建任务] │
└──────────────────────────────────────────────┘

┌ 按模块统计人力成本 ────────────────────────┐  ← 仅当 stats.length > 0 时渲染
│  前端        12h    ¥1,200.00                │
│  后端        24h    ¥2,400.00                │
│  未分类       8h    ¥  800.00                │
└──────────────────────────────────────────────┘

┌ 任务列表 ───────────────────────────────────┐
│  按 状态 × 模块 双重过滤后                    │
└──────────────────────────────────────────────┘
```

**模块 Select**：值域 `__all` / `__unassigned` / `<module_id>`；前端 filter（tasks 已全量加载）。

**管理模块 Dialog**：
- 列出当前项目所有模块（`sort_order` 顺序）
- 每行：模块名 inline 编辑框、↑ / ↓ 按钮、删除按钮
- 底部：新增模块输入框 + 「新增」按钮
- 删除返回 `DeleteBlocked` → toast 显示错误消息，弹窗保留

**TaskForm**：在「负责人」旁加一列「模块」Select（`__none` / `<module_id>`），默认 `initial?.module_id ?? __none`；submit 时 `__none` → `null`。

### 6.5 i18n keys（`src/i18n/zh-CN.json`）

| key | zh |
|---|---|
| `module.title` | 模块 |
| `module.manage` | 管理模块 |
| `module.new` | 新增模块 |
| `module.rename` | 重命名 |
| `module.delete` | 删除 |
| `module.deleteConfirm` | 确认删除模块「{{name}}」？ |
| `module.deleteBlocked` | 该模块下还有任务，请先删除或转移 |
| `module.moveUp` | 上移 |
| `module.moveDown` | 下移 |
| `module.filterByModule` | 按模块筛选 |
| `module.allModules` | 全部模块 |
| `module.unassigned` | 未分类 |
| `module.nameRequired` | 模块名必填 |
| `module.nameTooLong` | 模块名不能超过 40 字符 |
| `task.module` | 模块 |
| `financial.laborByModule` | 按模块统计人力成本 |

### 6.6 手工验收清单

- 老项目打开：`labor_by_module` 空 → 统计卡不渲染；模块 Select 为空占位，TaskForm 无模块选项（只显示"未分类"）
- 新建一个模块「前端」→ 保存 → TaskForm 里出现「前端」选项 → 新建任务挂上 → 记 8h 工时 → 统计卡出现「前端 · 8h · ¥800」
- 再建「后端」→ ↓ 按钮排序 → 保存 → 列表顺序更新
- 尝试删除「前端」（有 task）→ toast「该模块下还有任务，请先删除或转移」，弹窗仍开
- 把该 task 转到「后端」→ 再删「前端」→ 成功
- 模块 Select 切换到「后端」/「未分类」/「全部模块」→ 任务列表相应过滤

## 7. 验收标准

- [ ] 老数据（迁移前）迁移后 tasks.module_id 全 NULL；`labor_by_module` 若无 timelog 返回空
- [ ] 建模块 → 建挂模块的任务 → 记工时 → 统计卡出现该模块行、hours 与 cost_cents 正确
- [ ] 任务从「未分类」→「某模块」保存后，`labor_by_module` 从"未分类"桶迁移到该模块桶
- [ ] 任务 module_id 切回 null → 走回「未分类」桶
- [ ] 跨项目模块挂载被后端拒绝（前端不出错，后端 `Validation`）
- [ ] 模块删除强拒绝路径命中时前端 toast 展示明确原因
- [ ] Rust `cargo test` 全绿（含新增 17 个测试）
- [ ] `pnpm exec tsc -b` 通过

## 8. 影响面清单

| 层 | 文件 | 新建 / 修改 |
|---|---|---|
| DB | `src-tauri/migrations/0005_modules.sql` | 新建 |
| Rust | `src-tauri/src/commands/modules.rs` | 新建 |
| Rust | `src-tauri/src/commands/mod.rs` | 加 `pub mod modules;` |
| Rust | `src-tauri/src/commands/tasks.rs` | Task/TaskInput 扩 `module_id`、CRUD SQL、跨项目校验、3 测试 |
| Rust | `src-tauri/src/domain/module_stats.rs` | 新建 |
| Rust | `src-tauri/src/domain/mod.rs` | 加 `pub mod module_stats;` |
| Rust | `src-tauri/src/lib.rs` | 4 modules IPC + 1 labor stats IPC 注入 handler |
| Rust | `src-tauri/src/db/migrations.rs` | MIGRATIONS 追加 + version 4→5 |
| TS | `src/types/index.ts` | Module / ModuleInput / ModuleLaborStat + Task 扩 module_id |
| TS | `src/stores/modules.ts` | 新建 |
| TS | `src/stores/moduleStats.ts` | 新建 |
| TSX | `src/routes/projects/detail.tsx` | TasksPanel 顶部工具栏 + 统计卡 + 管理弹窗 + TaskForm 加 module 选择 |
| i18n | `src/i18n/zh-CN.json` | ~15 keys |
| Docs | `CHANGELOG.md` | Added 段追加 |

## 9. 与 Feature 2「禅道 CSV 导入」的接口约定

本 spec 落地后，Feature 2 会：
- 在导入前弹「模块映射」对话框（与成员映射并列）：CSV「所属模块」列的所有出现值 → 目标项目已有模块 or 选「新建」（当场调 `create_module`）or「未分类」
- 建 task 时带 `module_id`
- 「按模块统计」卡自然反映导入的历史工时数据
- 若 CSV 里模块是嵌套路径（`/前端/表单`），Feature 2 决定压平策略（叶子名 or 全路径，本 spec 不约束）
