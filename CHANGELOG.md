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
