# 禅道 CSV 导入 设计文档

- **创建日期**：2026-07-02
- **作者**：l2m2
- **状态**：设计已定稿，待实现
- **关联主设计**：[2026-06-29-solo-cost-design.md](2026-06-29-solo-cost-design.md)
- **依赖**：[2026-07-02-project-modules-design.md](2026-07-02-project-modules-design.md)（已交付）
- **中断笔记来源**：[2026-07-02-zentao-import-notes.md](2026-07-02-zentao-import-notes.md)

---

## 1. 背景与目标

### 1.1 现状

- solo-cost 目前没有任何导入能力（禅道、CSV、JSON 都不支持），全靠手工录入
- 用户历史数据都在禅道，`tasks/timelogs` 都是空的

### 1.2 目标

- 把禅道后台导出的 **任务** CSV 导入到 solo-cost 的指定项目下
- **一次性**把任务 + 工时（可选）转换过来，避免手工重录
- 支持幂等：同一份 CSV 二次导入不产生重复
- 参考样本：`~/Downloads/a005-2-全部任务.csv`（5 行数据）

### 1.3 非目标

- 增量同步 / 双向同步 / 禅道 REST API 集成
- 导入历史落库（一次性总结 Dialog 就够）
- Bug、需求、缺陷、故事等非任务类型
- 附件导入
- 已导入后重新导（幂等-更新）——命中 external_ref 则**整行跳过**
- 用户自定义状态映射 / 成员映射持久化
- 禅道自定义字段
- 关闭原因非中文（英文/其他语言版本禅道）
- 全路径嵌套模块（本次只取叶子名）
- 用户自定义分隔符（禅道导出用逗号）

## 2. 用户诉求

来源：l2m2 的真实业务口径，2026-07-02 对齐。样本 CSV 5 行数据，全属禅道 `a005-2(#25)` 项目，状态都是 `已关闭` + 关闭原因 `已完成`，指派给显示 `Closed`（禅道关闭态的 sentinel），实际执行者是 `李黎明`。目标项目 `a012` 会带有真实模块（本 CSV `所属模块` 都是根 `/(#0)`，看不到）。

## 3. 决策速览（brainstorm 结论）

| 主题 | 决策 |
|---|---|
| 入口 | 项目详情 →「任务+工时」tab 顶部工具栏加「从禅道 CSV 导入」按钮，绑定当前项目 |
| CSV 编码 | UTF-8 + GBK（`encoding_rs` 探测；UTF-8 优先） |
| CSV 分隔符 | 逗号（禅道默认，不做用户选项） |
| 上传方式 | 文件选择器（`@tauri-apps/plugin-dialog`），不支持粘贴文本 |
| 幂等 | `tasks` 加列 `external_ref TEXT`（例 `zentao:368`），命中已有整行跳过（含 timelog） |
| 成员映射 | 本次导入弹对话框；CSV 里出现的名字 → 下拉挑对应 solo-cost member 或「未指派」/「跳过含此人的行」。**不持久化** |
| 名字取哪一列 | 优先 `由谁完成`；空则用 `指派给`（`"Closed"` 视为 sentinel 空值）；再空用 `由谁创建` |
| 模块映射 | 本次导入弹对话框；CSV 里出现的模块叶子名 → 下拉挑目标项目已有模块 or「新建『<该名>』」or「未分类」。**不持久化** |
| 模块路径 | 嵌套压平为**叶子名**（`/前端/表单(#8)` → `表单`）；根 `/(#0)` → 未分类 |
| 状态映射 | 见 §5 |
| 工时导入 | 任务 + 对应 timelog 一起导；`hours=0` 或 `member 未映射` → 只导任务，不生成 timelog |
| 结束展示 | 一次性总结 Dialog，不落库审计 |

## 4. 数据模型与迁移

新增迁移 `src-tauri/migrations/0006_tasks_external_ref.sql`：

```sql
ALTER TABLE tasks ADD COLUMN external_ref TEXT;
CREATE UNIQUE INDEX idx_tasks_external_ref
    ON tasks(project_id, external_ref)
    WHERE external_ref IS NOT NULL AND deleted_at IS NULL;
```

**兼容性**：
- 老 `tasks` 行 `external_ref = NULL`，行为不变
- UI 手工新建的任务 `external_ref` 始终 NULL
- 唯一索引仅对 `external_ref IS NOT NULL AND deleted_at IS NULL` 生效——同一 solo-cost 项目下 `zentao:368` 全局唯一，但软删的行不占索引位（软删+恢复流程也不阻塞重导）

**结构体扩展**：
- `Task` 与 `TaskInput` 各加 `external_ref: Option<String>`
- `commands/tasks.rs::create_impl` INSERT 语句带上该列；正常 UI 路径传 `None`，导入路径传 `Some("zentao:<编号>")`

## 5. 状态映射规则

| 禅道 `任务状态` | 附加条件 | solo-cost `status` |
|---|---|---|
| 已关闭 | `关闭原因 = 已完成` | `done` |
| 已完成 | — | `done` |
| 进行中 | — | `in_progress` |
| 已激活 | — | `in_progress` |
| 已暂停 | — | `todo` |
| 未开始 | — | `todo` |
| 已取消 | — | **跳过整行**（`ParsedRow.status = None`） |
| 已关闭 | `关闭原因 ≠ 已完成`（如"已取消"、"重复问题"） | **跳过整行** |
| 其他未识别 | — | 视为 `todo`（防御性兜底，避免整批失败） |

跳过的行计入 `ImportReport.skipped.cancelled`。

## 6. 后端

### 6.1 新增 crate

`src-tauri/Cargo.toml`：
- `csv = "1"`（成熟 CSV 解析）
- `encoding_rs = "0.8"`（UTF-8 / GBK 探测）

### 6.2 IPC 命令

```rust
#[tauri::command]
pub fn preview_zentao_csv(
    state: tauri::State<AppState>,
    project_id: i64,
    file_path: String,
) -> AppResult<ImportPreview>;

#[tauri::command]
pub fn execute_zentao_import(
    state: tauri::State<AppState>,
    project_id: i64,
    file_path: String,
    member_mapping: std::collections::HashMap<String, MemberChoice>,
    module_mapping: std::collections::HashMap<String, ModuleChoice>,
) -> AppResult<ImportReport>;
```

### 6.3 共用结构

```rust
#[derive(Debug, Clone, Serialize)]
pub struct ImportPreview {
    pub total_rows: u32,
    pub member_names: Vec<String>,   // 去重、按出现次数降序
    pub module_names: Vec<String>,   // 去重、按出现次数降序，跳过根 (#0)
    pub pre_skip: PreSkipStats,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreSkipStats {
    pub cancelled: u32,
    pub already_imported: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MemberChoice {
    UseMember { member_id: i64 },
    Unassigned,
    SkipRow,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ModuleChoice {
    UseModule { module_id: i64 },
    CreateWithName { name: String },
    Unassigned,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportReport {
    pub imported_tasks: u32,
    pub imported_timelogs: u32,
    pub skipped: SkipCounts,
    pub failed: Vec<FailedRow>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkipCounts {
    pub cancelled: u32,
    pub already_imported: u32,
    pub member_skipped: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct FailedRow {
    pub row_no: u32,
    pub zentao_id: String,      // "zentao:368"
    pub error: String,
}
```

### 6.4 内部解析流程

`fn parse_all(file_path: &str) -> AppResult<Vec<ParsedRow>>`

1. **读文件**：`std::fs::read` → 字节流
2. **编码探测**：优先 UTF-8（`std::str::from_utf8`）；失败尝试 `encoding_rs::GBK.decode`。都失败 → `AppError::Validation("不支持的编码，请另存为 UTF-8")`
3. **CSV 头解析**：`csv::ReaderBuilder::new().has_headers(true).from_reader(...)`；必要列：`编号 / 任务名称 / 任务状态`。缺任一 → `AppError::Validation("CSV 缺少必要列: <列名>")`
4. **可选列位置**：`任务描述 / 关闭原因 / 最初预计 / 总计消耗 / 实际开始 / 实际完成 / 创建日期 / 由谁完成 / 指派给 / 由谁创建 / 所属模块 / 截止日期`。缺列时按空处理，不报错
5. **逐行规范化**为 `ParsedRow`：

```rust
struct ParsedRow {
    row_no: u32,
    zentao_id: String,              // "zentao:368"
    title: String,
    description: Option<String>,
    status: Option<String>,         // None → 跳过整行（取消）
    assignee_name: Option<String>,
    module_name: Option<String>,
    estimated_hours: Option<f64>,
    consumed_hours: f64,             // 兜底 0.0
    work_date: Option<String>,       // "YYYY-MM-DD"
    due_date: Option<String>,
}
```

规范化细则：

- `zentao_id = format!("zentao:{}", 编号.trim())`
- `title = 任务名称.trim().to_string()`
- `description = 任务描述.trim().is_empty() ? None : Some(...)`
- `status`：按 §5 映射，异常/未识别 → `Some("todo")`；取消 → `None`
- `assignee_name`：
  - 若 `由谁完成` 非空 → `Some(该值.trim())`
  - 否则若 `指派给` 非空且 ≠ `"Closed"` → `Some(该值.trim())`
  - 否则若 `由谁创建` 非空 → `Some(该值.trim())`
  - 否则 `None`
- `module_name`：从 `所属模块` 提取叶子。字符串形如 `/前端/表单(#8)` → 剥去 `(#\d+)` 后取最后一段 → `"表单"`；等于 `/` 或 `/(#0)` → `None`
- `estimated_hours`：`最初预计` 剥去尾部 `h` 后 `parse::<f64>().ok()`
- `consumed_hours`：同理，`parse::<f64>().unwrap_or(0.0)`
- `work_date`：优先 `实际开始[0..10]`；空则 `实际完成`（若已是 `YYYY-MM-DD` 直接用）；再空则 `创建日期[0..10]`。都空 → `None`
- `due_date = 截止日期` 或 None

### 6.5 `preview_zentao_csv` 具体做

- `parse_all(file_path)` → `rows`
- 收集：
  - `total_rows = rows.len()`
  - `member_names`：所有 `Some(assignee)` 去重，按出现次数降序
  - `module_names`：所有 `Some(module)` 去重，按出现次数降序
  - `pre_skip.cancelled = rows.iter().filter(|r| r.status.is_none()).count()`
  - `pre_skip.already_imported`：对所有 `zentao_id` 查 `SELECT 1 FROM tasks WHERE project_id = ?1 AND external_ref = ?2 AND deleted_at IS NULL`，命中数
- 返回 `ImportPreview`

### 6.6 `execute_zentao_import` 具体做

再次 `parse_all` 拿 fresh 数据（避免 preview / execute 之间用户改文件的边界）。

对每行按顺序处理：

1. `row.status.is_none()` → `skipped.cancelled += 1`，continue
2. `SELECT 1 FROM tasks WHERE project_id AND external_ref` 命中 → `skipped.already_imported += 1`，continue
3. `assignee_id_opt = match member_mapping.get(&row.assignee_name.unwrap_or_default())`：
   - `Some(SkipRow)` → `skipped.member_skipped += 1`，continue
   - `Some(UseMember { member_id })` → `Some(member_id)`
   - `Some(Unassigned)` 或 `None`（该名字在预览时不存在，可能是新加入）→ `None`
4. `module_id_opt = match module_mapping.get(&row.module_name.unwrap_or_default())`：
   - `Some(UseModule { module_id })` → `Some(module_id)`
   - `Some(CreateWithName { name })` → 若本次执行已缓存 → 复用；否则调 `commands::modules::create_impl(conn, project_id, &ModuleInput { name, sort_order: None })`，缓存 id
   - `Some(Unassigned)` 或 `None` → `None`
5. 每行开一个 `conn.unchecked_transaction()`；出错 rollback + 记入 `failed`，下一行继续。事务内：
   - 组装 `TaskInput { title, description, module_id, assignee_id, status, estimated_hours, due_date, external_ref: Some(zentao_id) }`
   - 调 `commands::tasks::create_impl(conn, project_id, &input)` → 拿 `task_id`
   - 若 `row.consumed_hours > 0.0 && assignee_id.is_some() && row.work_date.is_some()`：
     - 调 `commands::timelogs::create_impl(conn, ..., &TimeLogInput { task_id, member_id, work_date, hours: consumed_hours, notes: None })`
     - 成功 → `imported_timelogs += 1`
     - 失败（如 hours > 24）→ 事务照样提交（任务已建）、记入 `failed[{error: "timelog: ..."}]`——但这需要子事务或事务外补建，SQLite 不支持嵌套事务
   - 简化策略：**任务和 timelog 在同一 tx 内**，任一失败 → 整行 rollback + `failed`。用户在错误里看到"hours > 24"就知道要修 CSV 再重导

### 6.7 事务与错误策略

- **每行独立事务**：一行失败不影响其他
- 任务和 timelog 同事务：数据完整性优先，用户体验略降（一行只要有 timelog 问题任务也不建）
- `failed` 列表限制最多显示 100 条，超过则显示 `"... 及另 N 条"`（避免报告 Dialog 撑爆）

### 6.8 注册

- `src-tauri/src/commands/mod.rs`：`pub mod zentao_import;`
- `src-tauri/src/lib.rs`：`invoke_handler!` 追加 `preview_zentao_csv` 与 `execute_zentao_import`
- `src-tauri/src/db/migrations.rs`：`MIGRATIONS` 追加 `("0006_tasks_external_ref", ...)`；两个 `#[test]` 版本断言 5→6

## 7. 前端

### 7.1 入口

`src/routes/projects/detail.tsx::TasksPanel` 顶部工具栏（已有 状态 Select / 模块 Select / 管理模块）再加一个 outline 按钮：

```tsx
<Button variant="outline" onClick={() => setOpenImport(true)}>
  {t("zentaoImport.title")}
</Button>
```

配套 state：`const [openImport, setOpenImport] = useState(false)`；在 `TasksPanel` 底部与其他 Dialog 并列渲染 `<ZentaoImportDialog projectId={projectId} open={openImport} onOpenChange={setOpenImport} />`。

### 7.2 新组件

`src/components/zentao-import/ZentaoImportDialog.tsx`。

Props：`{ projectId: number; open: boolean; onOpenChange: (b: boolean) => void }`。

内部 state（全部 `useState`）：
- `step: 1 | 2 | 3 | 4 | 5`
- `filePath: string | null`
- `preview: ImportPreview | null`
- `memberMapping: Record<string, MemberChoice>`（key = CSV 名字）
- `moduleMapping: Record<string, ModuleChoice>`（key = CSV 模块名）
- `report: ImportReport | null`
- `busy: boolean`

打开时（`useEffect(() => { if (open) { setStep(1); ...重置... } })`）默认 step=1。关闭时 state 保留（供再次打开），但生产可视觉体验上无所谓。

### 7.3 Wizard 5 步

用 shadcn `<Dialog>` `className="max-w-3xl"`，`DialogContent` 内先渲染一个 breadcrumb（`选文件 → 成员 → 模块 → 确认 → 报告`）。

| Step | 内容 |
|---|---|
| **1. 选文件** | 大按钮「选择 CSV 文件」→ `@tauri-apps/plugin-dialog::open({filters:[{name:'CSV',extensions:['csv']}]})` 拿 path → 调 `preview_zentao_csv` → 显示 `total_rows / pre_skip` 计数摘要 → 「下一步」到 Step 2。选文件失败或 preview 报错 → toast 显示原因、留在 Step 1 |
| **2. 成员映射** | 若 `preview.member_names` 为空，直接跳 Step 3。否则渲染 `<Table compact>`：`CSV 名字 | 映射到`。右列 Select 选项：solo-cost active members 列表 + `<SelectItem value="__unassigned">未指派</SelectItem>` + `<SelectItem value="__skip">跳过含此人的行</SelectItem>`。默认按同名匹配（solo-cost 里存在同名 active member 时自动选中）；否则默认「未指派」。底部「上一步」/「下一步」 |
| **3. 模块映射** | 若 `preview.module_names` 为空，直接跳 Step 4。否则同结构：`CSV 模块名 | 映射到`。选项：目标项目现有模块 + `<SelectItem value="__create">新建「{name}」</SelectItem>` + `<SelectItem value="__unassigned">未分类</SelectItem>`。默认按同名匹配已有模块；否则默认新建 |
| **4. 确认** | 卡片摘要：「将导入 X 个任务（其中 Y 个带工时），跳过 Z 个（cancelled=A / already imported=B / member skipped=C）」。X 由前端推算：`total_rows - pre_skip.cancelled - pre_skip.already_imported - (member_mapping 里 SkipRow 命中的行数)`。「上一步」/「开始导入」按钮 → 后者调 `execute_zentao_import`，显示 loading 遮罩 → 完成后 setReport(res)、setStep(5) |
| **5. 报告** | 上半：绿色勾 + `已导入 X 任务、Y 工时；跳过 Z (A/B/C)`。下半（`report.failed.length > 0` 时）：红字列表，每条 `[row {row_no}] {zentao_id}: {error}`。「完成」按钮 → 关闭 Dialog，同时 `useTasksStore.loadFor(projectId, null)` + `useModulesStore.loadFor(projectId)` + `useModuleStatsStore.refresh(projectId)`（financial 也一并 refresh） |

### 7.4 前后 back 支持

Step 2/3/4 底部允许「上一步」；Step 5 只能「完成」（执行不可回滚）。

### 7.5 types

`src/types/index.ts` 追加：

```ts
export interface ImportPreview {
  total_rows: number;
  member_names: string[];
  module_names: string[];
  pre_skip: { cancelled: number; already_imported: number };
}

export type MemberChoice =
  | { kind: "use_member"; member_id: number }
  | { kind: "unassigned" }
  | { kind: "skip_row" };

export type ModuleChoice =
  | { kind: "use_module"; module_id: number }
  | { kind: "create_with_name"; name: string }
  | { kind: "unassigned" };

export interface ImportReport {
  imported_tasks: number;
  imported_timelogs: number;
  skipped: { cancelled: number; already_imported: number; member_skipped: number };
  failed: { row_no: number; zentao_id: string; error: string }[];
}
```

`Task` 加 `external_ref: string | null`；`TaskInput` 加 `external_ref?: string | null`。

### 7.6 i18n keys（`zh-CN.json`）

```json
"zentaoImport": {
  "title": "从禅道 CSV 导入",
  "chooseFile": "选择 CSV 文件",
  "reselect": "重选文件",
  "step": { "file": "选文件", "members": "成员", "modules": "模块", "confirm": "确认", "report": "报告" },
  "preview": {
    "summary": "共 {{total}} 行，其中已在库 {{already}} 行 / 已取消 {{cancelled}} 行",
    "willImport": "将导入 {{n}} 个任务（其中 {{logs}} 个带工时）",
    "willSkip": "跳过 {{n}} 个（已存在 {{already}} / 已取消 {{cancelled}} / 成员选择跳过 {{member}}）"
  },
  "member": { "column": "CSV 名字", "mapTo": "映射到", "unassigned": "未指派", "skipRow": "跳过含此人的行" },
  "module": { "column": "CSV 模块名", "mapTo": "映射到", "createWith": "新建「{{name}}」", "unassigned": "未分类" },
  "report": {
    "title": "导入完成",
    "imported": "已导入 {{tasks}} 个任务、{{logs}} 条工时",
    "skipped": "跳过 {{n}} 个（已存在 {{already}} / 已取消 {{cancelled}} / 成员跳过 {{member}}）",
    "failedTitle": "失败 {{n}} 条：",
    "failedItem": "[第 {{row}} 行] {{ref}}: {{err}}"
  },
  "action": { "start": "开始导入", "importing": "导入中…" },
  "error": { "parseFailed": "CSV 解析失败：{{msg}}", "executeFailed": "导入执行失败：{{msg}}" }
}
```

`common` 若缺 `next / back / done`，补齐（「下一步」/「上一步」/「完成」）。

## 8. 错误与边界

**Preview 阶段**：

| 场景 | 处理 |
|---|---|
| 文件不存在 / 无读权限 | `AppError::Validation("无法读取文件")`，前端 toast |
| 编码探测失败 | `AppError::Validation("不支持的编码，请另存为 UTF-8")` |
| CSV 首行缺关键列（`编号 / 任务名称 / 任务状态`） | `AppError::Validation("CSV 缺少必要列: {列名}")` |
| 空行 / 完全空白 | 静默跳过，不计入总数 |
| 单行字段解析异常（如"最初预计"值非 `\d+h`） | 兜底为 None，不阻塞 preview |

**Execute 阶段**：

| 场景 | 处理 |
|---|---|
| 后端 `tasks::create_impl` 拒绝 | 整行事务 rollback，记入 `failed`，继续 |
| `timelogs::create_impl` 拒绝 | 同上（任务和 timelog 在同事务） |
| CreateWithName 模块 > 40 字符 | `modules::create_impl` 校验拒绝 → 记入 failed |
| 用户选择的 member_id 已被软删 | `tasks::create_impl` 的跨公司校验会拒绝 → 记入 failed |
| `failed` 超过 100 条 | 报告 Dialog 显示前 100 + "… 及另 N 条" |

**幂等重放**：同一份 CSV 二次导入 → 所有行走「已存在」分支跳过，`imported_tasks=0, imported_timelogs=0, skipped.already_imported=total`。

## 9. 测试

### 9.1 Rust（`cargo test`）

`commands::zentao_import::tests`（26 个用例）：

1. `detects_utf8` — UTF-8 样本 → 行数正确
2. `detects_gbk` — GBK 样本 → 中文正确解码
3. `rejects_missing_required_columns` — 缺 `编号` → Validation
4. `parse_status_closed_done_maps_to_done`
5. `parse_status_done_maps_to_done`
6. `parse_status_in_progress_maps_to_in_progress`
7. `parse_status_paused_maps_to_todo`
8. `parse_status_wait_maps_to_todo`
9. `parse_status_cancelled_yields_none`
10. `parse_status_closed_non_done_yields_none`
11. `parse_assignee_completer_first`
12. `parse_assignee_falls_back_to_assigned_when_completer_empty`
13. `parse_assignee_treats_closed_sentinel_as_empty`
14. `parse_module_leaf_from_nested_path` — `/前端/表单(#8)` → `表单`
15. `parse_module_root_yields_none` — `/(#0)` → `None`
16. `parse_workdate_falls_back_from_start_to_end_to_created`
17. `execute_creates_task_with_external_ref` — external_ref = `zentao:368`
18. `execute_creates_timelog_when_hours_and_member`
19. `execute_skips_timelog_when_hours_zero` — 任务建、timelog 不建
20. `execute_skips_timelog_when_member_unassigned` — 同上
21. `execute_skips_row_when_member_skip_row` — 整行不建
22. `execute_skips_row_when_status_cancelled` — 整行不建
23. `execute_skips_row_when_external_ref_already_imported`
24. `execute_creates_module_on_the_fly` — CreateWithName → 项目下新建
25. `execute_reuses_created_module_across_rows` — 两行同 CreateWithName → 只 create 一次
26. `execute_records_failure_and_continues` — 中间一行失败，前后行正常

（把 6 种状态拆成各自一条 + 未识别 fallback 用例，共同保证映射矩阵可回归。）

`commands::tasks::tests`（增加 1 条）：

- `create_task_persists_external_ref` — 传 `Some("zentao:1")` → 读回一致

`db::migrations::tests`：两个断言 5→6。

### 9.2 前端

TS 类型检查（`pnpm exec tsc -b` EXIT=0）+ 手工验收（Task 6 手工验收步骤）：

- 用样本 CSV 走一遍：选文件 → preview 显示 5 行、cancelled=0、already=0 → 成员映射「李黎明」→ 模块映射（应为空）→ 确认摘要 X=5 Y=5 → 「开始导入」→ 报告 imported=5 tasks、5 timelogs
- 再选同一份 CSV 二次导入 → imported=0、already_imported=5
- 故意用一份 GBK 编码文件、故意删掉 `编号` 列

## 10. 验收标准

- [ ] 老数据（迁移前）迁移后 `tasks.external_ref` 全 NULL；`tasks::create_impl` 手工路径行为不变
- [ ] 样本 CSV（`~/Downloads/a005-2-全部任务.csv`）第一次导入：5 任务 + 5 工时；第二次导入：0 imported、5 already_imported
- [ ] `执行`/`in_progress`/`todo`/`done` 各状态样本均能正确映射；`已取消` 与 `已关闭+非已完成` 均整行跳过
- [ ] 成员映射对话框：选「跳过含此人的行」→ 该人所在行不导；选「未指派」→ 任务建但无 timelog
- [ ] 模块映射对话框：选「新建」→ 项目下自动生成对应 module；多行映射到同名 CreateWithName 仅生成一次
- [ ] 报告 Dialog 显示准确计数；点「完成」→ 任务列表刷新、模块统计卡刷新、financial 面板刷新
- [ ] 26 个后端 zentao_import 测试全绿；`cargo test` 全库 PASS；`pnpm exec tsc -b` EXIT=0

## 11. 影响面清单

| 层 | 文件 | 新建 / 修改 |
|---|---|---|
| DB | `src-tauri/migrations/0006_tasks_external_ref.sql` | 新建 |
| Rust | `src-tauri/Cargo.toml` | 加 `csv` + `encoding_rs` |
| Rust | `src-tauri/src/commands/zentao_import.rs` | 新建（parse + preview + execute + 测试） |
| Rust | `src-tauri/src/commands/mod.rs` | `pub mod zentao_import;` |
| Rust | `src-tauri/src/commands/tasks.rs` | `Task/TaskInput` 加 `external_ref`；`row_to_task`；`create_impl` INSERT 带上；1 持久化测试 |
| Rust | `src-tauri/src/lib.rs` | 注入 2 个新 IPC handler |
| Rust | `src-tauri/src/db/migrations.rs` | MIGRATIONS 追加 + version 5→6 |
| TS | `src/types/index.ts` | Task 加 `external_ref`；4 个新接口 |
| TSX | `src/components/zentao-import/ZentaoImportDialog.tsx` | 新建（wizard 5 步） |
| TSX | `src/routes/projects/detail.tsx` | TasksPanel 顶部加「从禅道 CSV 导入」按钮 + 挂 Dialog |
| i18n | `src/i18n/zh-CN.json` | 一整块 `zentaoImport.*` + `common.{next,back,done}` 若缺 |
| Docs | `CHANGELOG.md` | Added 段追加 |

## 12. 与既有 feature 的交互

- **modules**（0005）：`ModuleChoice::CreateWithName` 直接调 `modules::create_impl`，自动挂到目标项目下、sort_order 追加。若命名重复不阻塞（本 spec §3.1 明确模块名允许重复）
- **soft-delete**：老任务被软删（在回收站里）不占用 `external_ref` 唯一索引；重导可再建同 `zentao:id` 的新行
- **financial**：导入完成后前端触发 `useFinancialStore.refresh`，人力成本立即反映
- **回收站**：导入的任务与手工任务无区别，进入回收站流程一致
