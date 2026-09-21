# solo-cost

面向个人/小团队的成本与工时管理桌面应用。前端 React + Vite + TypeScript，后端 Tauri v2 + Rust，数据库使用 SQLCipher 加密存储。

## 技术栈

- **前端**：React 19、Vite 8、TypeScript、Tailwind CSS、shadcn/radix、zustand、react-hook-form + zod、i18next
- **后端**：Tauri v2、Rust、rusqlite（`bundled-sqlcipher-vendored-openssl`）
- **包管理**：pnpm

## 环境要求

- Node.js ≥ 20（推荐 22.x）
- pnpm ≥ 10
- Rust 稳定版 ≥ 1.77.2（安装后自带 `cargo`、`rustc`）
- 平台原生依赖：
  - **macOS**：Xcode Command Line Tools（`xcode-select --install`）
  - **Linux**：`libwebkit2gtk-4.1-dev`、`build-essential`、`libssl-dev`、`libayatana-appindicator3-dev`、`librsvg2-dev`
  - **Windows**：WebView2 Runtime（Win11 已预装）

## 启动程序

## ChatGPT 决策报表

在“设置 → ChatGPT 决策报表”中选择公司和日期范围，即可生成 Markdown 报表。报表包含收入、回款、销售分成、一般成本、人工收入、到手收入、剩余利润、项目排行、月度趋势和应收明细，可直接上传到 ChatGPT Project Chat 分析。生成过程不会调用 OpenAI API，也不会自动上传数据。

### 1. 安装依赖

```bash
pnpm install
```

### 2. 日常使用与开发调试

日常使用请直接打开已安装的发行版（macOS 为 `/Applications/沃工本.app`），不要通过开发模式启动。

仅在确实需要修改、调试代码时运行：

```bash
pnpm tauri dev
```

启动流程：

1. Vite 在 `http://localhost:1420` 提供前端热更新
2. Cargo 编译 Rust 后端（首次编译含 SQLCipher / OpenSSL，耗时较长；之后走缓存）
3. 打开桌面窗口（标题 `solo-cost`），进入锁定页，使用主密码解锁

前端改动即时热更新；`src-tauri/` 下的 Rust 改动会触发自动重编译并重启窗口。

> `pnpm tauri dev` 会生成体积较大的 Rust debug 构建缓存。调试结束后可运行
> `cargo clean --manifest-path src-tauri/Cargo.toml` 清理；该命令不会删除源码、数据库或已安装应用。

### 3. 打包发行版

项目默认只保留 release 构建，不长期保留 debug 构建缓存。macOS 发布时构建 Intel 与 Apple Silicon 通用包：

```bash
pnpm tauri build --target universal-apple-darwin
```

macOS 产物位于 `src-tauri/target/universal-apple-darwin/release/bundle/`。其他平台需要构建时运行 `pnpm tauri build`，产物位于对应的 `release/bundle/` 目录：

- macOS：`.app` 与 `.dmg`
- Windows：`.msi` / `.exe`
- Linux：`.AppImage` / `.deb`

### 其他脚本

| 命令 | 用途 |
|---|---|
| `pnpm dev` | 仅启动 Vite 前端开发服务器（不含 Tauri 后端） |
| `pnpm preview` | 预览 `pnpm build` 产物 |
| `pnpm build` | TypeScript 类型检查 + Vite 生产构建 |
| `pnpm lint` | 使用 oxlint 做静态检查 |

## 目录结构

```
solo-cost/
├── src/              # React 前端源码
├── src-tauri/        # Tauri + Rust 后端
│   ├── src/          # Rust 命令与业务逻辑
│   ├── migrations/   # SQLCipher 数据库迁移脚本
│   ├── capabilities/ # Tauri 权限声明
│   └── tauri.conf.json
├── docs/             # 里程碑计划与设计文档
├── public/           # 静态资源
└── CHANGELOG.md      # 版本变更记录
```
