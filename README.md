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

### 1. 安装依赖

```bash
pnpm install
```

### 2. 开发模式（日常使用）

```bash
pnpm tauri dev
```

启动流程：

1. Vite 在 `http://localhost:1420` 提供前端热更新
2. Cargo 编译 Rust 后端（首次编译含 SQLCipher / OpenSSL，耗时较长；之后走缓存）
3. 打开桌面窗口（标题 `solo-cost`），进入锁定页，使用主密码解锁

前端改动即时热更新；`src-tauri/` 下的 Rust 改动会触发自动重编译并重启窗口。

### 3. 打包发行版

```bash
pnpm tauri build
```

产物位于 `src-tauri/target/release/bundle/`：

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
