# 全局搜索（Cmd+K 快速跳转）

日期：2026-08-26

## 背景与目标

应用目前没有任何文本搜索入口，只有四个下拉筛选（项目详情的任务状态 / 模块，
仪表盘的状态 / 负责人）。项目和任务多起来之后，找一个已知存在的东西只能靠翻页。

目标是一个**跳转器**，不是搜索结果页：按 Cmd+K 打开，输入关键字，实时出候选，
回车直接跳过去。重点是快和键盘流，不展示详情、不做高亮片段。

## 范围

搜两类实体：

| 类型 | 匹配字段 | 回车后 |
|------|----------|--------|
| 项目 | 项目名、所属客户名 | `/projects/{id}` |
| 任务 | 任务标题 | `/projects/{project_id}?task={id}` |

### 明确不做（YAGNI）

- 不搜客户、成员、模块、成本、工时。
- 不搜任务的 `description` 和 `external_ref`。备注类长文本会让"搜个词命中一堆"，
  而 `external_ref` 只有禅道导入的数据才有，等确有需要再加。
- 不做搜索结果页、不做命中片段高亮。
- 不做搜索历史、不做最近访问。
- 不做 FTS5 全文索引，理由见「被否决的方案」。

## 交互

- **打开**：`Cmd+K`（macOS）/ `Ctrl+K`（Windows、Linux）。
- **输入**：防抖 200ms 后调用后端。空串或全空白不发请求，直接清空候选。
- **候选**：项目组在前、任务组在后，每组最多 8 条，各自带分组标题。
  每条显示主标题与副标题（项目的副标题是客户名，任务的是所属项目名）。
- **键盘**：`↑` `↓` 在扁平化后的候选列表上移动（跨组连续），`Enter` 跳转，
  `Esc` 关闭。鼠标悬停同步选中项。
- **空态**：有输入但零结果时显示"没有匹配的项目或任务"。
- **载体**：复用现有 `Dialog` / `DialogContent`。
  **不使用 `FormDialogContent`** —— 那是为防止误关丢失录入内容而设计的，
  搜索框里没有可丢的东西，Esc 和点遮罩就应该直接关。

## 后端设计（`src-tauri/src/commands/search.rs`）

新增模块，在 `commands/mod.rs` 注册 `pub mod search;`，
在 `lib.rs` 的 `generate_handler!` 中加入 `commands::search::search`。

### 类型

```rust
#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    pub kind: String,          // "project" | "task"
    pub id: i64,               // 项目 id 或任务 id
    pub title: String,         // 项目名 或 任务标题
    pub subtitle: Option<String>, // 项目→客户名；任务→所属项目名
    pub project_id: i64,       // 项目命中时等于 id；任务命中时是所属项目
}
```

`project_id` 对两类都给出，前端据此拼路由，不必再判类型取字段。

### 命令签名

```rust
#[tauri::command]
pub fn search(
    state: tauri::State<AppState>,
    company_id: i64,
    query: String,
    limit: Option<u32>,
) -> AppResult<Vec<SearchHit>>
```

`limit` 缺省为 8，指**每类**上限，不是总数。

`company_id` **由前端显式传入**，与 `list_projects`、`list_members` 等现有命令
一致（`AppState` 只持有 `conn`，当前公司存在 `app_meta` 表的
`current_company_id` 键里，前端通过 company store 拿到后随调用传下来）。

命令体沿用本仓库范式：`#[tauri::command]` 包一层 `with_conn`，真实逻辑放在
`search_impl(conn, company_id, query, limit)` 里，测试直接测 `search_impl`。

返回的 `Vec` 按**先项目、后任务**拼接，各自内部保持 SQL 的排序。前端直接顺序渲染
即可得到「项目组在前」的分组效果，不需要再排一次；分组标题按 `kind` 变化处插入。

### 关键字处理

输入先 `trim()`。空串直接返回空 `Vec`，不查库。

**LIKE 通配符必须转义**：用户输入的 `%`、`_`、`\` 若原样进 LIKE，
打一个 `%` 就会匹配全部记录。转义后拼成 `%{escaped}%`，SQL 侧统一带 `ESCAPE '\'`：

```rust
fn like_pattern(raw: &str) -> String {
    let escaped = raw
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}
```

前缀匹配用的模式是 `format!("{escaped}%")`，转义规则相同。

### 查询

两条独立 SQL，不用 UNION —— 两类的排序规则和字段来源不同，分开写更直白，
也便于各自限流。

项目：

```sql
SELECT p.id, p.name, c.name AS client_name
FROM projects p
LEFT JOIN clients c ON c.id = p.client_id AND c.deleted_at IS NULL
WHERE p.company_id = ?1
  AND p.deleted_at IS NULL
  AND (p.name LIKE ?2 ESCAPE '\' OR c.name LIKE ?2 ESCAPE '\')
ORDER BY CASE WHEN p.name LIKE ?3 ESCAPE '\' THEN 0 ELSE 1 END,
         p.updated_at DESC
LIMIT ?4
```

任务：

```sql
SELECT t.id, t.title, t.project_id, p.name AS project_name
FROM tasks t
JOIN projects p ON p.id = t.project_id
WHERE p.company_id = ?1
  AND t.deleted_at IS NULL
  AND p.deleted_at IS NULL
  AND t.title LIKE ?2 ESCAPE '\'
ORDER BY CASE WHEN t.title LIKE ?3 ESCAPE '\' THEN 0 ELSE 1 END,
         t.updated_at DESC
LIMIT ?4
```

要点：

- `tasks` 表**没有 `company_id`**，隔离必须靠 JOIN `projects`。
- 任务查询要同时排除**任务自身**和**所属项目**的软删除，漏掉后者会搜出已删项目下的任务。
- `projects.client_name` 列在迁移 0007 中已被 DROP，客户名现在来自
  `client_id → clients.name`，所以是 JOIN 而非直接匹配列。
- `LEFT JOIN` 而非 `JOIN`：项目的 `client_id` 可为 NULL，用内连接会漏掉未绑定客户的项目。
- 排序按「前缀命中优先，其次最近更新」。

## 前端设计

### 1. `src/components/search/CommandPalette.tsx`（新增）

自持 `open` 状态，通过 `useEffect` 挂全局 `keydown` 监听（`(e.metaKey || e.ctrlKey) && e.key === 'k'`，
命中时 `preventDefault` 以免与浏览器默认行为冲突）。

内部状态：`query`、`hits`、`activeIndex`、`loading`。
`query` 变化后 200ms 防抖调用 `invoke("search", { query })`。
请求用递增序号标记，回包时丢弃非最新的结果，避免慢请求覆盖快请求。

选中项跳转后关闭面板并清空 `query`。

### 2. `src/components/layout/AppLayout.tsx`

挂载 `<CommandPalette />`。放在这里而非 `App.tsx`，因为它只应在已解锁的主界面可用，
`/setup` 和 `/login` 不需要。

### 3. `src/routes/projects/detail.tsx` — 任务深链

`TasksPanel` 读 `useSearchParams()` 取 `task` 参数。命中时按顺序：

1. **`statusFilter` 置 `__all`**。默认值是 `__active`，会过滤掉 `closed` 任务——
   不重置的话，搜到一条已关闭任务跳过去会看到空列表。`moduleFilter` 一并置 `__all`。
2. **翻到正确的页**。任务列表是前端分页，`PAGE_SIZE = 20`。在重置筛选后的
   `visibleTasks` 中定位该任务下标，算出页码并 `setCurrentPage`。
3. **滚动并高亮**。给该行加 `ref`，`scrollIntoView({ block: "center" })`，
   加一个 2 秒后自动移除的高亮类。
4. **清除 URL 参数**（`setSearchParams({}, { replace: true })`）。
   不清的话，用户在页面内改了筛选或翻了页之后，任何触发重渲染的操作都可能把他弹回去。

该 effect 依赖任务数据已加载完成，需要等 `byProject[projectId]` 有值后再执行；
若 `task` 参数对应的任务不存在（已删除或不属于该项目），静默忽略，只清参数。

### 4. `src/i18n/zh-CN.json`

新增 `search` 段：`placeholder`、`groupProjects`、`groupTasks`、`empty`、`hintNavigate`。

### 5. `src/types/index.ts`

新增 `SearchHit` 类型，与后端 struct 对应。

## 被否决的方案

**前端过滤已加载数据。** 不可行。任务是按项目分片加载的（store 里是
`byProject[projectId]`），全局搜任务只能搜到本次会话已打开过的项目下的任务。
这不是精度问题，是功能上错的。

**FTS5 全文索引。** 过早。需要新增迁移建虚拟表并写同步触发器维护，
且需先确认 `rusqlite` 的 `bundled-sqlcipher-vendored-openssl` 构建是否编入 FTS5
（**尚未验证**）。个人 / 小团队量级下 LIKE 足够，等实际变慢再议。

## 测试

后端进 Rust 单元测试，沿用现有套路（当前 21 个文件、174 个测试）：

- 关键字含 `%` / `_` / `\` 时被正确转义，不会匹配到不该匹配的记录。
- 只返回当前公司的数据，另一公司的同名项目 / 任务不出现。
- 已软删除的项目、任务不出现；**所属项目已软删除的任务**也不出现。
- 未绑定客户（`client_id IS NULL`）的项目仍能按项目名搜到。
- 按客户名能搜到该客户名下的项目。
- 前缀命中排在包含命中之前。
- 空串 / 纯空白返回空结果且不查库。
- `limit` 对每类分别生效。

前端无自动化测试。项目目前没有 vitest / testing-library，
Cmd+K 的交互、防抖、深链跳转只能手动验证。以下几条必须人工过一遍：

- 搜到一条**已关闭**的任务，回车后能看到它（验筛选重置）。
- 搜到一条落在**第 2 页之后**的任务，回车后能看到它（验翻页）。
- 跳转后 URL 上的 `?task=` 已被清掉，手动改筛选或翻页不会被弹回。
- Windows 上 `Ctrl+K` 生效（开发机是 macOS，此项无法本地验证）。

## 已知风险

- 高亮与滚动依赖任务行的 `ref`，而任务行目前在 `detail.tsx` 内联渲染，
  该文件已 1561 行。本次只做必要改动，不顺带拆分；拆分是独立的一件事。
- `Cmd+K` 在 Tauri webview 中是否会被系统或 WebView 抢占，未验证。
  若被抢占，退路是改用 `Cmd+P` 或在设置中可配。
