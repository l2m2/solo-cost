# 本地 MCP 收入决策访问 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** 在桌面应用内提供只读本地 MCP 服务，让 ChatGPT Project Chat 查询收入、到手、销售分成、人工收入和剩余利润。

**Architecture:** Tauri 启动仅监听 127.0.0.1:47831 的 HTTP 服务，处理 MCP 初始化、工具发现和工具调用。工具通过共享 AppState 访问已解锁的 SQLCipher 连接，并调用集中式收入决策领域查询；应用锁定后立即拒绝新查询。

**Tech Stack:** Rust 1.77.2、Tauri v2、rusqlite/SQLCipher、serde/serde_json、tiny_http 0.12.0、React 19、TypeScript、i18next

**Spec:** docs/superpowers/specs/2026-09-20-local-mcp-income-access-design.md

## Global Constraints

- SQLCipher 本地数据库仍是唯一权威数据源，不添加云同步或远端数据库。
- 首版只读，不提供任意 SQL、写入、修改、删除、附件或备注全文访问。
- MCP 只监听 127.0.0.1:47831，数据库未解锁时返回 APP_LOCKED。
- 主密码、数据库路径、密钥、SQL 和财务结果正文不得写入 MCP 日志。
- 默认限定当前公司；跨公司必须显式传入 scope: all_companies。
- 金额使用整数分；合同/潜在与实收/已实现口径不得混用。
- 必须满足：到手 = 人工收入 + 剩余利润。
- 明细默认 50 条、最大 200 条，并返回分页元数据。
- 不新增单元测试或测试专用依赖；使用编译检查和 MCP 端到端手工验证。
- 添加 tiny_http 前必须获得用户明确的依赖安装批准。

## Review Focus

- 数据库在请求前或执行期间锁定：返回 APP_LOCKED，不得使用缓存数据。
- 未设置当前公司且未指定公司：返回 INVALID_ARGUMENT，不得隐式跨公司。
- 日期倒置、格式非法、limit 超过 200：返回稳定参数错误，不泄露 SQL。
- 固定分成未结算和比例分成部分回款：潜在与已实现口径按现有规则分别计算。
- 端口被占用：桌面应用继续可用，设置页显示 MCP 未运行及安全化错误。

---

### Task 1: 收入决策领域模型与查询

**Files:**
- Create: src-tauri/src/domain/income.rs
- Modify: src-tauri/src/domain/mod.rs
- Modify: src-tauri/src/domain/profit.rs

**Interfaces:**
- Consumes: rusqlite::Connection 和现有业务表。
- Produces: 六类查询输入输出，以及 get_income_overview、rank_income_sources、get_project_decision_summary、get_income_trend、list_payments_page、list_income_details。

- [ ] **Step 1: 定义输入输出类型**

创建 IncomeScope、DateRange、IncomeMetrics、IncomeOverview、IncomeRankRow、ProjectDecisionSummary、IncomeTrendRow、PaymentPage、IncomeDetailPage。核心字段如下：

~~~rust
pub struct IncomeMetrics {
    pub contract_exclusive_cents: i64,
    pub received_exclusive_cents: i64,
    pub commission_potential_cents: i64,
    pub commission_realized_cents: i64,
    pub general_cost_cents: i64,
    pub take_home_potential_cents: i64,
    pub take_home_realized_cents: i64,
    pub labor_income_cents: i64,
    pub residual_profit_potential_cents: i64,
    pub residual_profit_realized_cents: i64,
}
~~~

顶层响应包含 generated_at、实际公司列表和 basis，明确潜在与已实现来源。

- [ ] **Step 2: 实现统一输入校验**

~~~rust
fn resolve_company_ids(conn: &Connection, scope: &IncomeScope) -> AppResult<Vec<CompanyRef>>;
fn validate_date_range(range: &DateRange) -> AppResult<()>;
fn validate_page(offset: i64, limit: i64) -> AppResult<()>;
~~~

CurrentCompany 读取 app_meta.current_company_id；缺失时报错。日期仅接受 YYYY-MM-DD 且结束不早于开始；offset >= 0，1 <= limit <= 200。

- [ ] **Step 3: 集中计算指标**

用绑定参数查询非软删除项目、回款、一般成本和工时。比例分成按收入比例计算；固定分成只有结算后计入已实现。人工逐条按 round(hours / 8 × daily_cost_snapshot_cents) 后求和：

~~~rust
take_home_potential = contract_exclusive - commission_potential - general_cost;
take_home_realized = received_exclusive - commission_realized - general_cost;
residual_profit_potential = take_home_potential - labor_income;
residual_profit_realized = take_home_realized - labor_income;
~~~

日期筛选分别作用于回款日期、成本发生日期和工时日期；合同潜在口径按项目聚合，并在 basis 说明不是现金发生日期口径。

- [ ] **Step 4: 实现六类查询**

~~~rust
pub fn get_income_overview(conn: &Connection, scope: &IncomeScope, range: &DateRange) -> AppResult<IncomeOverview>;
pub fn rank_income_sources(conn: &Connection, input: &RankIncomeInput) -> AppResult<Vec<IncomeRankRow>>;
pub fn get_project_decision_summary(conn: &Connection, project_id: i64) -> AppResult<ProjectDecisionSummary>;
pub fn get_income_trend(conn: &Connection, input: &IncomeTrendInput) -> AppResult<Vec<IncomeTrendRow>>;
pub fn list_payments_page(conn: &Connection, input: &PaymentListInput) -> AppResult<PaymentPage>;
pub fn list_income_details(conn: &Connection, input: &IncomeDetailInput) -> AppResult<IncomeDetailPage>;
~~~

排行仅接受 client/project 与既定五种指标；趋势仅接受 month/year 并补零；明细仅接受 general_cost/commission/labor 且不返回 notes。

- [ ] **Step 5: 复用人工计算并检查**

让 profit.rs 调用 income::project_labor_income，保持原公开字段不变。

~~~bash
cargo check --manifest-path src-tauri/Cargo.toml
~~~

Expected: 编译通过；人工核对固定分成未结算、部分回款、空数据、负剩余利润和跨公司分组。

- [ ] **Step 6: 提交领域层**

~~~bash
git add src-tauri/src/domain/income.rs src-tauri/src/domain/mod.rs src-tauri/src/domain/profit.rs
git commit -m "feat(income): 增加收入决策查询模型"
~~~

### Task 2: MCP 协议与工具适配

**Files:**
- Create: src-tauri/src/mcp/protocol.rs
- Create: src-tauri/src/mcp/tools.rs
- Create: src-tauri/src/mcp/mod.rs
- Modify: src-tauri/Cargo.toml
- Modify: src-tauri/Cargo.lock

**Interfaces:**
- Consumes: Task 1 的六个查询和 AppState::with_conn。
- Produces: handle_json_rpc(body: Value, app: &AppHandle) -> JsonRpcReply、六个工具 schema 和安全错误映射。

- [ ] **Step 1: 经批准添加唯一新依赖**

~~~toml
tiny_http = "0.12.0"
~~~

Run: cargo update --manifest-path src-tauri/Cargo.toml -p tiny_http --precise 0.12.0

Expected: 锁文件只增加该包及必要传递依赖；不升级无关依赖。未获批准则停止。

- [ ] **Step 2: 实现 MCP JSON-RPC 协议**

支持 initialize、notifications/initialized、ping、tools/list、tools/call。初始化返回服务名 solo-cost、版本和 tools capability；未知方法返回 -32601，非法参数 -32602，内部错误 -32603。

- [ ] **Step 3: 声明六个精确 schema**

声明 get_income_overview、rank_income_sources、get_project_decision_summary、get_income_trend、list_payments、list_income_details。对范围、日期、维度、指标、粒度、明细类型和分页使用 enum/minimum/maximum。描述明确“到手包含人工收入，人工不是额外加项”。

- [ ] **Step 4: 映射工具调用**

所有调用通过 app.state::<AppState>().with_conn(...)。成功返回 MCP text content 和 structuredContent。Locked 映射 APP_LOCKED；NotFound、Validation、分页限制映射稳定错误码；内部错误只返回通用中文信息。

- [ ] **Step 5: 用真实 JSON 验证协议**

~~~json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"manual","version":"1"}}}
{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"get_income_overview","arguments":{"scope":"current_company"}}}
~~~

Expected: 初始化声明 tools；工具恰好六项；锁定时第三项返回 APP_LOCKED。

- [ ] **Step 6: 检查并提交**

~~~bash
cargo check --manifest-path src-tauri/Cargo.toml
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/mcp
git commit -m "feat(mcp): 增加只读收入查询工具"
~~~

### Task 3: 本地 HTTP 生命周期与状态

**Files:**
- Modify: src-tauri/src/state.rs
- Create: src-tauri/src/mcp/server.rs
- Create: src-tauri/src/commands/mcp.rs
- Modify: src-tauri/src/commands/mod.rs
- Modify: src-tauri/src/lib.rs

**Interfaces:**
- Consumes: Task 2 的协议分派。
- Produces: start_mcp_server(app: AppHandle)、get_mcp_status() -> McpStatus 和 http://127.0.0.1:47831/mcp。

- [ ] **Step 1: 统一数据库访问与运行状态**

在 AppState 增加 with_conn 和 McpRuntimeStatus { running, error }。Mutex 中毒映射 AppError::Internal，不新增 unwrap。查询持锁期间与 lock() 互斥。

- [ ] **Step 2: 实现回环 HTTP 服务**

使用 tiny_http::Server::http("127.0.0.1:47831")。仅接受 POST /mcp 和 GET /health；请求体上限 1 MiB；存在 Origin 时只允许本地来源；响应为 application/json；其他路径 404，错误方法 405。

- [ ] **Step 3: 隔离启动失败**

端口绑定失败时保存“本地端口 47831 不可用”并记录错误类别，桌面窗口和数据库仍正常工作。

- [ ] **Step 4: 注册启动和状态命令**

在 setup 前 manage AppState；setup 中启动服务；注册 get_mcp_status。响应包含 address、running、database_unlocked、error。

- [ ] **Step 5: 验证 HTTP 边界**

~~~bash
curl -sS http://127.0.0.1:47831/health
curl -sS -X POST http://127.0.0.1:47831/mcp -H 'Content-Type: application/json' --data '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}'
~~~

Expected: health 返回运行/锁定状态，工具列表六项。另验证超过 1 MiB 返回 413；占用端口后应用仍启动。

- [ ] **Step 6: 检查并提交**

~~~bash
cargo check --manifest-path src-tauri/Cargo.toml
git add src-tauri/src/state.rs src-tauri/src/mcp/server.rs src-tauri/src/commands/mcp.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat(mcp): 启动本地回环查询服务"
~~~

### Task 4: ChatGPT 连接设置界面

**Files:**
- Create: src/stores/mcp.ts
- Modify: src/routes/settings.tsx
- Modify: src/types/index.ts
- Modify: src/i18n/zh-CN.json

**Interfaces:**
- Consumes: Task 3 的 get_mcp_status。
- Produces: 设置页 ChatGPT 标签、地址、运行状态、锁定状态和 Tunnel 说明。

- [ ] **Step 1: 添加类型和 store**

定义 McpStatus { address, running, database_unlocked, error }；store 提供 status、loading、error、loadStatus()，调用失败必须可见。

- [ ] **Step 2: 增加设置标签**

显示固定地址、服务/数据库状态，以及“应用必须运行并解锁；OpenAI 密钥由 tunnel-client 管理”。提供刷新按钮，不输入或保存 API Key，所有文案放入 zh-CN.json。

- [ ] **Step 3: 检查四种状态**

验证服务正常且已解锁、服务正常但锁定、端口占用、状态命令失败。

- [ ] **Step 4: 检查并提交**

~~~bash
pnpm lint
pnpm build
git add src/stores/mcp.ts src/routes/settings.tsx src/types/index.ts src/i18n/zh-CN.json
git commit -m "feat(settings): 展示 ChatGPT 本地连接状态"
~~~

### Task 5: Tunnel 联调、文档与最终验证

**Files:**
- Create: docs/chatgpt-mcp.md
- Modify: README.md
- Modify: CHANGELOG.md

**Interfaces:**
- Consumes: 本地 MCP 地址和六个工具。
- Produces: 可执行的 Tunnel 配置和 Project Chat 使用说明。

- [ ] **Step 1: 编写连接说明**

覆盖创建 tunnel、指向本地 MCP、保持应用运行且解锁、Developer Mode 创建 Tunnel App、扫描工具、Project Chat 使用 @Solo Cost，以及换电脑后重新配置。API Key 仅使用环境变量占位符。

- [ ] **Step 2: 联调 Secure MCP Tunnel**

运行官方 tunnel-client doctor 和 run，在 ChatGPT 扫描六个工具并执行：

~~~text
@Solo Cost 总结当前公司今年的实收、已收到手、其中人工收入和剩余利润。
@Solo Cost 按已收到手排列今年的项目，并解释前三名的销售分成和一般成本。
~~~

Expected: 第一条满足恒等关系；第二条只在解释时下钻。无法取得外部凭据时如实标记跳过。

- [ ] **Step 3: 验证锁定和公司边界**

锁定后返回 APP_LOCKED，解锁后恢复；切换当前公司后默认结果改变；只有显式 all_companies 才跨公司且保留公司维度。

- [ ] **Step 4: 运行完整检查**

~~~bash
pnpm lint
pnpm build
cargo check --manifest-path src-tauri/Cargo.toml
git diff --check
~~~

Expected: 全部退出码为 0；不得用本地 curl 冒充远端 Tunnel 已连通。

- [ ] **Step 5: 更新 CHANGELOG 并提交**

使用 changelog skill 更新用户可感知的新能力：

~~~bash
git add docs/chatgpt-mcp.md README.md CHANGELOG.md
git commit -m "docs(mcp): 补充 ChatGPT 连接指南"
~~~

- [ ] **Step 6: 请求最终代码审查**

使用 superpowers:requesting-code-review 检查指标恒等关系、双口径、只读边界、锁定竞态、错误脱敏、回环限制和回归；修复确认问题后重跑完整检查。

