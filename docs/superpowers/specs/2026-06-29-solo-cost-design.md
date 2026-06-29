# solo-cost 设计文档

- **创建日期**：2026-06-29
- **作者**：l2m2
- **版本**：v1（MVP 设计）

---

## 1. 项目概述

### 1.1 定位

**solo-cost** 是一款面向小公司、个人开发者、OPC（一人公司）的**项目利润核算工具**，附带轻量任务/工时管理能力。核心目标：

- 完整记录公司所有项目的合同总价、税率、各类成本、人天投入、利润情况
- 通过任务+工时模块自动累计实际人力成本，让"预估利润"和"实际利润"都可见
- 本地优先，单机使用，数据加密保存，未来支持 Google Drive 同步

### 1.2 用户与场景

- **当前阶段**：作者自用（单机、单用户）
- **目标场景**：
  - OPC / 个人开发者：自己记账核算
  - 1–3 人小团队：管理者代填工时与成本

### 1.3 非目标（明确不做）

- 多用户登录与协作（v1.0 之前不考虑）
- 完整的项目管理（看板、冲刺、燃尽图等）
- 国地税申报、附加税自动计提（v1.0 之前不考虑）
- 多币种实时汇率换算（仅在数据模型中预留字段）
- 复杂权限/审计日志

### 1.4 迭代规划

| 版本 | 范围 |
|------|------|
| **MVP (v0.1)** | 公司/成员/项目/合同/收款节点/成本/任务+工时/附件/备份/加密/软删除 |
| **v0.2 报表** | 月年汇总、成员维度、现金流时间轴、Excel/CSV 导出、跨项目工时统计 |
| **v0.3 同步** | Google Drive 同步（数据库 + 附件） |
| **v0.4 增强** | CSV 导入、附件预览、客户/项目类型维度、任务优先级/子任务 |

---

## 2. 整体架构

### 2.1 技术栈

| 层 | 选型 |
|----|------|
| 桌面外壳 | Tauri 2.x |
| 前端 | React 18 + TypeScript + Vite |
| UI | shadcn/ui + Tailwind CSS（主题 `zinc`，radius `0.5rem`） |
| 状态 | Zustand |
| 路由 | React Router v6 |
| 图表 | Recharts（v0.2） |
| 国际化 | i18next（当前仅 zh-CN，预留扩展） |
| 后端 | Rust + Tauri 命令 |
| 数据库 | SQLCipher（`rusqlite` + `bundled-sqlcipher-vendored-openssl`） |
| 数据库驱动 | `rusqlite` 直接写 SQL + 薄封装（不引入大 ORM） |
| 日志 | `tracing` + 文件输出 |
| 错误处理 | `thiserror` |

### 2.2 数据存储位置

按 OS 规范的应用数据目录：

- macOS: `~/Library/Application Support/solo-cost/`
- Windows: `%APPDATA%\solo-cost\`
- Linux: `~/.local/share/solo-cost/`

目录结构：
```
$APP_DATA/
├── data.db                  # 加密数据库
├── attachments/             # 附件文件（UUID 命名）
├── backups/                 # 自动备份
└── logs/                    # 日志（保留 14 天）
```

### 2.3 启动流程

1. 应用启动 → 检查 `$APP_DATA/data.db` 是否存在
2. **不存在**：进入「初始化向导」
   - 设置主密码（强制提示记入密码管理器，无法找回）
   - 创建加密数据库 + 跑 migrations
   - 引导建第一家公司
3. **存在**：进入「输入主密码」窗口 → 验证 → 打开加密连接 → 进入主界面
4. 进入主界面前：执行启动自动备份（如距上次备份 > 24h）
5. 闲置 15 分钟（可配置）→ 关闭数据库连接 → 跳回 `/login`

---

## 3. 数据模型

### 3.1 设计原则

- **金额一律用分（INTEGER）**：避免浮点累计误差
- **所有业务表 `deleted_at TIMESTAMP NULL`**：软删除，回收站查非 NULL
- **time_logs 存日成本快照**：成员调薪后历史项目成本不变
- **附件用多态表**：一张 `attachments`，按 `entity_type/entity_id` 关联

### 3.2 表结构

```
app_meta
  键值表
  键示例: schema_version, current_company_id, last_backup_at, default_currency,
         auto_lock_minutes

companies
  id, name, legal_name, tax_id, default_tax_rate, currency_code (default 'CNY'),
  notes, created_at, updated_at, deleted_at

members
  id, company_id (FK), name, role, daily_cost_cents, effective_from,
  is_active, notes, created_at, updated_at, deleted_at

cost_categories
  id, company_id (FK), name, is_system (1=预设不可删), sort_order, deleted_at
  预设科目: 外包成本 / 硬件采购 / 服务器与SaaS / 差旅 / 办公耗材 /
           市场推广 / 税费与手续费 / 培训与资料 / 其它

projects
  id, company_id (FK), name, client_name, status,
  contract_amount_cents, contract_amount_is_tax_inclusive (1=含税),
  tax_rate, start_date, end_date, actual_delivered_at,
  notes, created_at, updated_at, deleted_at
  status enum: negotiating(商务洽谈) / pending(待启动) / in_progress(进行中) /
              delivered(已交付待结款) / settled(已结款) / archived(已归档)

contract_payments
  id, project_id (FK), name (例: "预付款"), expected_amount_cents,
  expected_date, actual_amount_cents, actual_received_at, sort_order,
  notes, deleted_at

cost_entries
  id, project_id (FK), category_id (FK), incurred_at, amount_cents,
  description, notes, created_at, deleted_at

tasks
  id, project_id (FK), title, description, assignee_id (FK members, nullable),
  status (todo / in_progress / done), estimated_hours, due_date,
  created_at, updated_at, deleted_at

time_logs
  id, task_id (FK), member_id (FK), work_date, hours,
  daily_cost_snapshot_cents,        -- 关键：成员该时点日成本快照
  notes, created_at, deleted_at

attachments
  id, entity_type (project / cost_entry / task), entity_id,
  file_name (原始名), relative_path (在 $APP_DATA/attachments/ 下),
  file_size_bytes, mime_type, created_at, deleted_at
```

### 3.3 关键计算公式

```
不含税收入  = is_tax_inclusive ? amount / (1 + tax_rate) : amount
含税收入    = is_tax_inclusive ? amount : amount × (1 + tax_rate)
税额        = 含税 - 不含税

人力成本    = Σ(time_log.hours / 8 × daily_cost_snapshot_cents)   -- 8 小时 = 1 人天
一般成本    = Σ(cost_entries.amount_cents)
总成本      = 人力成本 + 一般成本

毛利润     = 不含税收入 - 总成本
利润率     = 毛利润 / 不含税收入

实际收款汇总 = Σ(contract_payments WHERE actual_received_at IS NOT NULL)
回款率      = 实际收款 / 合同总额
```

> **注**：所有金额计算在 Rust 后端进行（前端只展示），避免 JS 浮点问题。

### 3.4 软删除与级联

- **级联软删（同一时间戳）**
  - 删项目 → 收款节点 / 成本 / 任务 / 工时 / 附件 全部级联软删
  - 删任务 → 工时级联软删
- **拒绝删除**
  - 成员若有 time_logs 引用，拒绝软删，提示「该成员有 N 条工时记录，请先归档/转移」
- **恢复**：按删除时间戳整组恢复
- **物理清理**：30 天后启动时清理 → 弹通知"已清理 N 条过期记录"，同步删 attachments 文件

### 3.5 Schema 迁移

- `src-tauri/migrations/0001_init.sql`、`0002_xxx.sql` 顺序文件
- 启动时读 `app_meta.schema_version`，循环跑未应用的迁移
- 每次迁移在事务内执行，失败整体回滚

---

## 4. 模块划分 & UI 结构

### 4.1 路由

```
/setup                          首次启动：设置主密码 + 建第一家公司
/login                          打开应用：输入主密码

进入主框架（左侧 sidebar + 顶部公司切换）：
/dashboard                      当前公司概览（进行中项目数、本月新增成本、待收款汇总）
/projects                       项目列表（按状态分组、可筛选）
/projects/:id                   项目详情（Tabs：概览/收款/成本/任务+工时/附件）
/members                        成员管理（含日成本编辑、调薪生效日）
/categories                     成本科目管理
/tasks                          任务总览（跨项目视图，只读列表）
/reports                        报表（v0.2 实装）
/trash                          回收站
/settings                       设置（通用/安全/备份/数据/关于）
```

> 工时录入只在「项目详情 → 任务+工时 Tab」里进行；跨项目工时统计放 v0.2 报表。

### 4.2 前端目录结构

```
src/
├── routes/                     页面组件
│   └── projects/detail/        概览/收款/成本/任务+工时/附件 5 个子页
├── components/
│   ├── ui/                     shadcn 组件（直接 copy 进项目）
│   ├── layout/                 Sidebar, Header, CompanySwitcher
│   ├── forms/                  共用表单组件（MoneyInput, DateInput 等）
│   └── charts/                 图表 wrappers（v0.2）
├── stores/                     Zustand：auth, company, ui
├── lib/
│   ├── ipc.ts                  Tauri 命令包装（类型安全 invoke<T>）
│   ├── money.ts                分 ↔ 元、formatCNY
│   ├── time.ts                 日期工具
│   └── calc.ts                 利润、税额前端公式（仅展示）
├── types/                      与 Rust 端 struct 对齐的 TS 类型
└── i18n/zh-CN.json
```

### 4.3 后端目录结构

```
src-tauri/src/
├── main.rs / lib.rs
├── commands/                   一个业务一个文件，统一注册到 invoke_handler
│   ├── auth.rs                 setup, unlock, change_password
│   ├── companies.rs / members.rs / categories.rs
│   ├── projects.rs / payments.rs / costs.rs
│   ├── tasks.rs / timelogs.rs
│   ├── attachments.rs          含文件保存/读取
│   ├── trash.rs                软删恢复、清空
│   ├── backup.rs               手动/自动备份、导出
│   └── settings.rs
├── db/
│   ├── pool.rs                 SQLCipher 连接管理
│   ├── migrations.rs           顺序执行 migrations/*.sql
│   └── models.rs               struct ↔ row mapping
├── domain/                     业务核心（脱离 IPC，方便单测）
│   ├── profit.rs               利润/税额/成本聚合
│   ├── soft_delete.rs          级联软删/恢复
│   └── ...
├── error.rs                    统一错误（thiserror）
└── migrations/0001_init.sql ...
```

### 4.4 关键 UI 决策

- **公司切换**：sidebar 顶部 dropdown，切换后右侧基于当前 company_id 重新 fetch
- **项目详情用 Tabs 而非多页面**：财务操作经常在收款/成本/任务之间跳，tab 体验比路由更顺
- **金额输入控件统一**：`<MoneyInput>` 对内传分（integer），对外显示元（带千位分隔符）
- **shadcn 主题**：`zinc` + `radius=0.5rem`（朴素商务感）

---

## 5. 安全、备份与数据保护

### 5.1 主密码与加密

- **加密方式**：SQLCipher
- **密钥派生**：用户主密码 → SQLCipher 内部 PBKDF2（默认 256000 轮）→ 256-bit 数据库密钥
- **校验**：打开数据库后立即 `SELECT count(*) FROM sqlite_master`，失败 = 密码错
- **修改主密码**：`PRAGMA rekey = 'new-password'`，事务保护
- **无找回机制**：丢了等于数据全丢。首次设置时向导文案强制提醒"必须记入密码管理器"

### 5.2 主密码内存管理

- 主密码字符串只在解锁时驻留几秒，验证通过后立刻清零
- SQLCipher 句柄持有派生密钥，不再持有原文
- 应用切到后台或闲置 15 分钟（可配置）→ 关闭数据库连接 → 跳回 `/login`

### 5.3 备份策略

| 类型 | 触发 | 位置 | 保留 |
|------|------|------|------|
| 启动自动备份 | 启动时距上次备份 >24h | `$APP_DATA/backups/auto_YYYYMMDD_HHmmss.db` | 最近 7 份 |
| 手动备份 | 用户点"立即备份" | 用户选位置 `.db` | 用户自管 |
| 导出明文快照 | 用户选"导出明文备份" | 用户选位置 `.db`（UI 醒目警告） | 用户自管 |

- 备份文件**仍是加密的 SQLCipher 文件**，要用同一主密码才能打开
- 附件目录单独打包：`backups/auto_YYYYMMDD_HHmmss_attachments.zip`
- 备份元数据写入 `app_meta.last_backup_at`

### 5.4 数据完整性

- 跨表写入用**事务**
- 数据库连接用 `WAL` 模式
- 启动时 `PRAGMA integrity_check`，失败弹「数据库损坏，请用最近备份恢复」对话框
- 关键写操作（删除/修改主密码/批量导入）前自动触发一次备份

### 5.5 攻击面

| 风险 | 措施 |
|------|------|
| 路径遍历 | 附件文件名服务端用 UUID 重命名存盘，原始名只在 DB 里展示 |
| SQL 注入 | rusqlite 全部用绑定参数，零字符串拼接 |
| CSV/Excel 公式注入 | CSV 字段以 `=`、`+`、`-`、`@` 开头时加前导单引号 |
| 附件撑爆磁盘 | 单文件 ≤ 50MB（可配置） |

---

## 6. 错误处理

### 6.1 Rust 端统一错误模型

```rust
// src-tauri/src/error.rs
#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("database error: {0}")]      Db(#[from] rusqlite::Error),
    #[error("io error: {0}")]             Io(#[from] std::io::Error),
    #[error("migration failed: {0}")]     Migration(String),
    #[error("wrong master password")]     WrongPassword,
    #[error("validation: {0}")]           Validation(String),
    #[error("not found: {entity} #{id}")] NotFound { entity: &'static str, id: i64 },
    #[error("cannot delete: {0}")]        DeleteBlocked(String),
    #[error("file too large: {0} bytes")] FileTooLarge(u64),
    #[error("backup failed: {0}")]        Backup(String),
}
```

- Tauri 命令统一返回 `Result<T, AppError>`
- `AppError` 实现 `Serialize` 给前端

### 6.2 前端错误处理分层

| 错误类型 | 处理 | 用户感知 |
|---------|------|---------|
| `WrongPassword` | 登录页 inline 提示 | "密码错误" |
| `Validation` | 表单字段红框 + 提示文案 | 字段级 |
| `NotFound` | 跳列表页 + Toast | "该记录已不存在" |
| `DeleteBlocked` | Modal 显示阻止原因 + 建议操作 | "成员有 12 条工时记录，请先归档" |
| `Db` / `Io` / `Backup` | 顶部红色 banner + 重试按钮，详细写日志 | "操作失败，请重试或查看日志" |
| `Migration` | 应用启动失败页 + "回滚到最近备份" 按钮 | 阻塞性 |

### 6.3 关键异常场景与策略

| 场景 | 策略 |
|------|------|
| 数据库迁移半途失败 | 事务回滚 → 错误页 → 提供"用最近备份替换 data.db"按钮 |
| integrity_check 报错 | 启动阻塞，列出可恢复备份让用户选 |
| 写文件失败（磁盘满/无权限） | 抛 `Io`，不写部分数据；事务里同步 DB 撤销 |
| 附件文件丢失但 DB 有记录 | 列表里标"⚠ 文件丢失"，不报致命错 |
| 导入 CSV 部分行格式错 | 报告"成功 N 条 / 失败 M 条 + 行号 + 原因"，已成功不回滚 |
| 应用崩溃 | WAL + Tauri panic hook 写日志，重启后自动恢复 |

### 6.4 校验层级

- **前端**：字段格式、必填、范围（即时反馈、减少 IPC 往返）
- **后端**：再做一次完整校验（永远不信前端） + 业务规则
- **数据库**：CHECK 约束（`amount_cents >= 0`、`hours BETWEEN 0 AND 24`）

### 6.5 日志

- `tracing` + `tracing-subscriber`，写 `$APP_DATA/logs/app-YYYY-MM-DD.log`
- 默认 INFO 级别，设置页可切 DEBUG
- 保留 14 天，启动时清理
- 设置页可点"打开日志目录"

---

## 7. 测试策略

### 7.1 分层

| 层级 | 工具 | 重点 |
|------|------|------|
| Rust 单元测试 | `cargo test` | `domain/` 业务计算（profit、税额、人天聚合、软删级联）—— 目标 100% |
| Rust 集成测试 | `cargo test` + 内存 SQLCipher | DB 命令端到端 |
| TS 单元测试 | Vitest | `lib/money.ts`、`lib/calc.ts`、formatters |
| TS 组件测试 | Vitest + Testing Library | 表单校验、关键展示组件 |
| E2E（v0.2） | Playwright + Tauri WebDriver | 关键流程 1-2 条 |

### 7.2 测试数据

- 不 mock 数据库；用真实 SQLCipher 内存模式
- `tests/fixtures/seed.sql` 准备标准数据集，多个测试共用
- 每个测试一个临时 db 实例，互不干扰

### 7.3 必须有的测试用例

**计算正确性**
- 含税合同总价 → 不含税收入、税额（边界税率 0 / 6% / 13%）
- 人力成本聚合：3 个人 × 不同日成本 × 不同工时
- 调薪后历史项目人力成本不变（验证 `daily_cost_snapshot_cents`）
- 利润 = 不含税收入 - 总成本
- 金额单位：1 元 → 100 分；99.99 元 → 9999 分

**软删级联**
- 删项目 → 5 类子表级联软删，时间戳一致
- 按时间戳整组恢复
- 30 天清理后物理消失，附件文件同步删
- 删成员有工时 → 返回 `DeleteBlocked`

**迁移**
- 空库依次跑完所有 migrations，schema 正确
- 中间迁移失败 → 事务回滚，schema_version 不前进

**安全/边界**
- 错密码打不开数据库
- rekey 后旧密码失效、新密码可用
- 附件超 50MB 被拒
- CSV 字段以 `=` 开头时被转义

### 7.4 TDD 节奏

- **domain 层用 TDD**：先写测试再实现
- **commands 层先写实现后补测试**：IPC 接口形状会迭代
- **UI 组件按需测**：表单校验、关键展示要测，纯展示组件不强求

### 7.5 CI（v0.2 起做）

- GitHub Actions：lint（`cargo clippy` + `eslint`）+ `cargo test` + `vitest`
- macOS / Windows 矩阵构建（CI 先只验 build 通过）

---

## 8. 待决事项 / 未来方向

- **v0.3 Google Drive 同步**：采用「整库 + 附件 zip 上传」的简单策略，单设备使用，多设备时以最后一次上传为准（不做合并）
- **v1.0 多用户协作**：届时再评估走 C/S 架构（Postgres + Web/Desktop 双端）还是混合（本地主端 + 轻量工时端）
- **应用图标 / 启动画面**：设计阶段未涉及，开发前补
- **打包签名**：macOS 公证、Windows 代码签名，发布前处理
