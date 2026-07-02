# 禅道 CSV 导入 — brainstorm 中断笔记

- 状态：**pending Feature 1 (Modules) 完成**
- 保存日期：2026-07-02
- 下一步：Feature 1（项目模块）完整交付后，重启本 brainstorm，把这里的决策带入正式 spec

## 目的

把禅道后台导出的任务 CSV 导入到 solo-cost 的指定项目下，一次性把任务 + 工时（可选）转换过来，避免手工重录。参考样本：`~/Downloads/a005-2-全部任务.csv`（5 行数据）。

## 已确认的决策

| 主题 | 决策 |
|---|---|
| 入口 | 项目详情 →「任务+工时」tab 顶部加「从禅道 CSV 导入」按钮，绑定当前项目 |
| 成员映射 | 导入前弹映射对话框；CSV 里出现的名字 → 下拉挑对应 solo-cost member 或选「未指派」/「跳过含此人的行」。本次导入沿用，**不持久化**（不建 zentao_name → member_id 表） |
| 名字取哪一列 | 优先 `由谁完成`；空则用 `指派给`（"Closed" 视为 sentinel 空值）；再空用 `由谁创建` |
| 工时导入 | 任务 + 对应 timelog 一起导。`hours = 总计消耗剥 'h'`，`work_date = 实际开始日期部分（空则实际完成，再空则创建日期）`，`member_id = 映射结果`。总计消耗=0 或成员未映射 → timelog 跳过（任务照建）|
| 幂等 | 幂等-跳过：`tasks` 表加列 `external_ref TEXT`（例 `zentao:368`），命中已有整行跳过（含 timelog）。**不做**幂等-更新 |
| 模块 | Feature 1（模块）交付后，Feature 2 支持读「所属模块」列，导入前弹「模块映射」对话框，映射到目标 solo-cost 项目下的模块（或当场新建） |

## 未决点（Feature 2 重启时要问的）

- CSV 编码 / 分隔符：是否只支持 UTF-8 CSV？还是也支持 GBK（禅道旧版默认）？
- 状态映射规则：ZenTao `已关闭 + 关闭原因=已完成` → `done`；`进行中/已激活` → `in_progress`；`已暂停/未开始` → `todo`；`已取消` → 跳过 or 视为 done？
- 上传方式：本地文件选择器 vs 粘贴 CSV 文本 vs 两者都支持
- 导入报告：成功/跳过/失败的行数如何展示，是否落库为「导入历史」

## 相关代码位置（Feature 2 重启时的入口）

- Tasks 后端：`src-tauri/src/commands/tasks.rs`
- Timelogs 后端：`src-tauri/src/commands/timelogs.rs`
- 项目详情前端：`src/routes/projects/detail.tsx::TasksPanel`
- Migrations：`src-tauri/migrations/`（下一版应为 `0006_*` 或更晚，取决于 Feature 1 用几个）
- 类型定义：`src/types/index.ts`

## Feature 1 交付后需回读

Feature 1（模块）落地后：

- Feature 2 spec 里把「模块映射」对话框补上
- `tasks` INSERT 时新增 `module_id` 字段
- 导入报告新增「模块未映射被跳过」的分支
