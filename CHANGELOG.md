# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## Unreleased

### Added
- 集成 Tauri 2，支持将前端打包为原生桌面应用
- 统一错误类型 `AppError`，可序列化传递给前端展示
- 应用状态容器，持有加密数据库连接
- `ping` IPC 命令，前端可验证与 Rust 后端的通信通路
- 前端首页展示 IPC 响应（`ipc: pong`），确认打通前后端
- 主密码命令：`setup`（初始化加密数据库）、`unlock`（密码验证并解锁）、`lock`（锁定）、`is_initialized`（检查是否已初始化）
- 公司管理页面：支持创建、编辑公司信息（工商名、税号、默认税率等），并可切换当前公司
- Header 顶部公司切换器：通过下拉菜单快速切换当前公司，支持锁定操作
- 仪表盘显示当前所选公司名称，无公司时引导用户前往创建
- 项目、成本科目、成本录入三张表（schema_version 2）
- `AppError::DeleteBlocked` 变体，用于拒绝删除语义
- `domain::soft_delete` 模块：项目与成本录入的软删除/恢复，支持同一时间戳级联删除及按时间戳整组恢复（恢复项目时不影响独立删除的成本条目）；恢复成本条目时若关联项目已删则阻断并提示先恢复项目
- 成本科目管理后端：支持创建、重命名、软删除、列表查询
- 首次访问公司时自动幂等种子 9 个系统预设科目（外包成本、硬件采购、服务器与SaaS 等），预设科目不可删除
- 删除系统预设科目或仍被成本录入引用的科目时返回 `DeleteBlocked` 错误
- 项目管理后端：支持创建、查询、编辑、按状态筛选（洽谈中/待启动/进行中/已交付/已结算/已归档）、状态切换及软删除
- 删除项目时级联软删除其下所有成本录入
- 新建项目时自动为所属公司幂等种入 9 个预设科目，确保成本录入始终有科目可选
