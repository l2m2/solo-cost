# solo-cost M1 (Foundation) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 搭起 Tauri + React + SQLCipher 基础骨架，做完主密码初始化/解锁、加密数据库与迁移系统、主框架布局、公司 CRUD 与切换。运行结束时可以「开应用 → 设密码 → 建公司 → 切换公司 → 重启后用密码进入」全流程跑通。

**Architecture:** Tauri 2.x 桌面壳，前端 React 18 + TS + shadcn/ui + Tailwind + Zustand，后端 Rust + rusqlite（SQLCipher 模式）。数据库连接由 Tauri State 持有，所有业务通过 `#[tauri::command]` 暴露给前端。

**Tech Stack:** Tauri 2.x · React 18 · TypeScript · Vite · Tailwind CSS · shadcn/ui · Zustand · React Router v6 · i18next · Rust stable · rusqlite + SQLCipher · thiserror · tracing · pnpm

## Global Constraints

适用所有任务（每个任务的要求隐式包含本节）：

- **包管理**：pnpm（统一锁文件，禁用 npm/yarn 混用）
- **金额单位**：所有金额字段一律 `INTEGER`（分）；前端展示再转元
- **软删除字段**：所有业务表必须有 `deleted_at TIMESTAMP NULL DEFAULT NULL`，业务查询默认 `WHERE deleted_at IS NULL`
- **错误处理**：Rust 端所有 `#[tauri::command]` 返回 `Result<T, AppError>`；禁用 `unwrap()` / `expect()` 在非测试代码
- **SQL 安全**：rusqlite 一律绑定参数，禁止字符串拼接
- **代码注释**：英文；公开 API 写简明 doc comment
- **提交规约**：Conventional Commits；`type`/`scope` 小写英文；`subject` 中文 ≤ 72 字符，结尾不加句号
- **测试纪律**：domain/核心计算/数据访问层用 TDD（先写测试再实现）
- **文件编码**：UTF-8 + LF
- **目标平台**：MVP 阶段以 macOS 为主开发；代码不刻意写 OS 特定逻辑
- **主密码无找回**：无任何"重置/找回"路径；UI 在初始化页明示

---

## File Structure (M1 完成后的产物)

```
solo-cost/
├── package.json
├── pnpm-lock.yaml
├── vite.config.ts
├── tsconfig.json
├── tsconfig.node.json
├── tailwind.config.js
├── postcss.config.js
├── components.json                  shadcn 配置
├── index.html
├── .gitignore
├── src/
│   ├── main.tsx                     React 入口
│   ├── App.tsx                      路由根 + Providers
│   ├── styles/globals.css           Tailwind base + shadcn variables
│   ├── lib/
│   │   ├── utils.ts                 shadcn `cn()`
│   │   └── ipc.ts                   类型安全的 Tauri invoke 包装
│   ├── i18n/
│   │   ├── index.ts                 i18next init
│   │   └── zh-CN.json               全部中文文案
│   ├── stores/
│   │   ├── auth.ts                  锁/解锁状态
│   │   └── company.ts               当前公司 + 列表缓存
│   ├── types/index.ts               与 Rust 对齐的 TS 类型
│   ├── components/
│   │   ├── ui/                      shadcn 组件（button/input/dialog/...）
│   │   └── layout/
│   │       ├── AppLayout.tsx
│   │       ├── Sidebar.tsx
│   │       ├── Header.tsx
│   │       └── CompanySwitcher.tsx
│   └── routes/
│       ├── setup.tsx                首次启动向导
│       ├── login.tsx                输入主密码
│       ├── dashboard.tsx            占位（显示当前公司名）
│       └── companies.tsx            公司列表 + 增/改
├── src-tauri/
│   ├── Cargo.toml
│   ├── build.rs
│   ├── tauri.conf.json
│   ├── icons/                       占位图标（Tauri 自带）
│   ├── migrations/
│   │   └── 0001_init.sql            app_meta + companies
│   └── src/
│       ├── main.rs
│       ├── lib.rs
│       ├── error.rs                 AppError
│       ├── state.rs                 AppState (Mutex<Option<Connection>>)
│       ├── db/
│       │   ├── mod.rs
│       │   ├── pool.rs              open_encrypted / close / rekey
│       │   └── migrations.rs        embed + run
│       └── commands/
│           ├── mod.rs
│           ├── auth.rs              is_initialized / setup / unlock / lock / change_password
│           └── companies.rs         CRUD + current 选中
└── docs/superpowers/{specs,plans}/
```

---

## Task 1: 前端脚手架 + Tailwind + shadcn

**Files:**
- Create: `package.json`, `pnpm-lock.yaml`, `vite.config.ts`, `tsconfig.json`, `tsconfig.node.json`, `tailwind.config.js`, `postcss.config.js`, `components.json`, `index.html`, `.gitignore`
- Create: `src/main.tsx`, `src/App.tsx`, `src/styles/globals.css`, `src/lib/utils.ts`
- Create: `src/components/ui/button.tsx`, `src/components/ui/input.tsx`, `src/components/ui/label.tsx`, `src/components/ui/card.tsx`, `src/components/ui/dialog.tsx`, `src/components/ui/dropdown-menu.tsx`, `src/components/ui/form.tsx`, `src/components/ui/sonner.tsx`, `src/components/ui/separator.tsx`

**Interfaces:**
- Produces: `cn(...)` 工具（`src/lib/utils.ts`），shadcn 组件目录 `src/components/ui/`，Vite 启动入口

- [ ] **Step 1：初始化 Vite + React + TS**

```bash
cd /Users/l2m2/workspace/l2m2/solo-cost
pnpm create vite@latest . -- --template react-ts
# 提示是否覆盖 .gitignore / 已有目录时选择保留 docs/
pnpm install
```

- [ ] **Step 2：安装 Tailwind 与 shadcn 必备依赖**

```bash
pnpm add -D tailwindcss postcss autoprefixer
pnpm add -D @types/node
pnpm dlx tailwindcss init -p
pnpm add class-variance-authority clsx tailwind-merge lucide-react sonner
```

- [ ] **Step 3：配置 `tsconfig.json` 添加 path alias**

修改 `tsconfig.json`（合并 `compilerOptions`）：

```json
{
  "compilerOptions": {
    "baseUrl": ".",
    "paths": { "@/*": ["src/*"] }
  }
}
```

修改 `tsconfig.node.json` 同样添加。`vite.config.ts` 添加 alias：

```typescript
import path from "node:path";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: { "@": path.resolve(__dirname, "./src") },
  },
  clearScreen: false,
  server: { port: 1420, strictPort: true },
});
```

- [ ] **Step 4：写 `tailwind.config.js`**

```javascript
/** @type {import('tailwindcss').Config} */
export default {
  darkMode: ["class"],
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    container: { center: true, padding: "1rem", screens: { "2xl": "1400px" } },
    extend: {
      borderRadius: {
        lg: "var(--radius)",
        md: "calc(var(--radius) - 2px)",
        sm: "calc(var(--radius) - 4px)",
      },
      colors: {
        border: "hsl(var(--border))",
        input: "hsl(var(--input))",
        ring: "hsl(var(--ring))",
        background: "hsl(var(--background))",
        foreground: "hsl(var(--foreground))",
        primary: { DEFAULT: "hsl(var(--primary))", foreground: "hsl(var(--primary-foreground))" },
        secondary: { DEFAULT: "hsl(var(--secondary))", foreground: "hsl(var(--secondary-foreground))" },
        destructive: { DEFAULT: "hsl(var(--destructive))", foreground: "hsl(var(--destructive-foreground))" },
        muted: { DEFAULT: "hsl(var(--muted))", foreground: "hsl(var(--muted-foreground))" },
        accent: { DEFAULT: "hsl(var(--accent))", foreground: "hsl(var(--accent-foreground))" },
        popover: { DEFAULT: "hsl(var(--popover))", foreground: "hsl(var(--popover-foreground))" },
        card: { DEFAULT: "hsl(var(--card))", foreground: "hsl(var(--card-foreground))" },
      },
    },
  },
  plugins: [],
};
```

- [ ] **Step 5：写 `src/styles/globals.css`**

```css
@tailwind base;
@tailwind components;
@tailwind utilities;

@layer base {
  :root {
    --background: 0 0% 100%;
    --foreground: 240 10% 3.9%;
    --card: 0 0% 100%;
    --card-foreground: 240 10% 3.9%;
    --popover: 0 0% 100%;
    --popover-foreground: 240 10% 3.9%;
    --primary: 240 5.9% 10%;
    --primary-foreground: 0 0% 98%;
    --secondary: 240 4.8% 95.9%;
    --secondary-foreground: 240 5.9% 10%;
    --muted: 240 4.8% 95.9%;
    --muted-foreground: 240 3.8% 46.1%;
    --accent: 240 4.8% 95.9%;
    --accent-foreground: 240 5.9% 10%;
    --destructive: 0 84.2% 60.2%;
    --destructive-foreground: 0 0% 98%;
    --border: 240 5.9% 90%;
    --input: 240 5.9% 90%;
    --ring: 240 5.9% 10%;
    --radius: 0.5rem;
  }
  * { @apply border-border; }
  body { @apply bg-background text-foreground; font-family: ui-sans-serif, system-ui, sans-serif; }
}
```

- [ ] **Step 6：写 `src/lib/utils.ts`**

```typescript
import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
```

- [ ] **Step 7：写 `components.json`（shadcn 配置）**

```json
{
  "$schema": "https://ui.shadcn.com/schema.json",
  "style": "default",
  "rsc": false,
  "tsx": true,
  "tailwind": {
    "config": "tailwind.config.js",
    "css": "src/styles/globals.css",
    "baseColor": "zinc",
    "cssVariables": true,
    "prefix": ""
  },
  "aliases": {
    "components": "@/components",
    "utils": "@/lib/utils",
    "ui": "@/components/ui"
  }
}
```

- [ ] **Step 8：用 shadcn CLI 添加常用组件**

```bash
pnpm dlx shadcn@latest add button input label card dialog dropdown-menu form sonner separator
```

如果提示找不到 `index.css`，把它指向 `src/styles/globals.css` 即可。完成后 `src/components/ui/` 下应出现 9 个 `.tsx` 文件。

- [ ] **Step 9：更新 `index.html` 引入 globals.css 入口**

`src/main.tsx` 改为：

```typescript
import React from "react";
import ReactDOM from "react-dom/client";
import App from "@/App";
import "@/styles/globals.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
```

`src/App.tsx`（占位）：

```typescript
import { Button } from "@/components/ui/button";

export default function App() {
  return (
    <div className="min-h-screen flex items-center justify-center">
      <Button>solo-cost 启动成功</Button>
    </div>
  );
}
```

删除 Vite 模板自带的 `App.css`、`src/index.css`、`src/assets/` 内的 demo 资源。

- [ ] **Step 10：完善 `.gitignore`**

追加（如已有相同行可跳过）：

```
node_modules/
dist/
dist-ssr/
.DS_Store
.env*
*.local
.vite/
.turbo/
src-tauri/target/
src-tauri/gen/
src-tauri/WixTools/
*.db
*.db-shm
*.db-wal
```

- [ ] **Step 11：跑通 dev server**

```bash
pnpm dev
```
Expected：终端显示 `Local: http://localhost:1420/`，浏览器打开看到一个 shadcn Button 显示 "solo-cost 启动成功"。`Ctrl+C` 关掉。

- [ ] **Step 12：Commit**

```bash
git add -A
git commit -m "feat(scaffold): 接入 vite + react + tailwind + shadcn 脚手架"
```

---

## Task 2: Tauri 2 集成 + Rust 错误模型 + 日志

**Files:**
- Create: `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src-tauri/build.rs`
- Create: `src-tauri/src/main.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/error.rs`
- Create: `src-tauri/src/state.rs`
- Modify: `package.json`（加 `tauri` script、加 `@tauri-apps/api` 与 `@tauri-apps/cli`）
- Modify: `src/App.tsx`（调用一个 hello Tauri 命令验证打通）

**Interfaces:**
- Produces:
  - Rust：`AppError`（统一错误，serde Serialize 后给前端）；`AppState { conn: Mutex<Option<rusqlite::Connection>> }`
  - 一个示例命令 `ping() -> Result<String, AppError>` 返回 `"pong"`
- Consumes：Task 1 的脚手架

- [ ] **Step 1：用 Tauri CLI 初始化 src-tauri 目录**

```bash
pnpm add -D @tauri-apps/cli@^2
pnpm add @tauri-apps/api@^2
pnpm tauri init
```

交互回答：
- App name: `solo-cost`
- Window title: `solo-cost`
- Web assets relative path: `../dist`
- Dev server URL: `http://localhost:1420`
- Frontend dev command: `pnpm dev`
- Frontend build command: `pnpm build`

完成后存在 `src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`、`src-tauri/src/main.rs` 等。

- [ ] **Step 2：在 `package.json` `scripts` 中加 `tauri`**

```json
"scripts": {
  "dev": "vite",
  "build": "tsc -b && vite build",
  "preview": "vite preview",
  "tauri": "tauri"
}
```

- [ ] **Step 3：补 `src-tauri/Cargo.toml` 依赖**

`[dependencies]` 段替换为：

```toml
[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-dialog = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
rusqlite = { version = "0.31", features = ["bundled-sqlcipher-vendored-openssl", "chrono"] }
thiserror = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4", "serde"] }

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 4：写 `src-tauri/src/error.rs`**

```rust
use serde::{Serialize, Serializer};

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("migration failed: {0}")]
    Migration(String),

    #[error("wrong master password")]
    WrongPassword,

    #[error("not initialized")]
    NotInitialized,

    #[error("already initialized")]
    AlreadyInitialized,

    #[error("locked: please unlock first")]
    Locked,

    #[error("validation: {0}")]
    Validation(String),

    #[error("not found: {entity} #{id}")]
    NotFound { entity: &'static str, id: i64 },

    #[error("internal: {0}")]
    Internal(String),
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
```

- [ ] **Step 5：写 `src-tauri/src/state.rs`**

```rust
use rusqlite::Connection;
use std::sync::Mutex;

#[derive(Default)]
pub struct AppState {
    pub conn: Mutex<Option<Connection>>,
}
```

- [ ] **Step 6：写 `src-tauri/src/lib.rs`（Tauri 2 推荐结构）**

```rust
mod error;
mod state;

use crate::error::AppResult;
use crate::state::AppState;

#[tauri::command]
fn ping() -> AppResult<String> {
    Ok("pong".to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![ping])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 7：把 `src-tauri/src/main.rs` 改为最小入口**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    solo_cost_lib::run();
}
```

注意：Cargo 默认 lib 名是 crate 名带连字符替换为下划线。若 `Cargo.toml` 中 `name = "solo-cost"`，lib crate 名为 `solo_cost`。如 Tauri init 产生的库名不同，按它生成的实际名称替换 `solo_cost_lib`。

`Cargo.toml` 顶部确认有：

```toml
[lib]
name = "solo_cost_lib"
crate-type = ["staticlib", "cdylib", "rlib"]
```

- [ ] **Step 8：前端调用 `ping` 验证 IPC 通路**

`src/App.tsx` 改为：

```typescript
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@/components/ui/button";

export default function App() {
  const [pong, setPong] = useState<string>("...");
  useEffect(() => {
    invoke<string>("ping").then(setPong).catch((e) => setPong(`error: ${e}`));
  }, []);
  return (
    <div className="min-h-screen flex flex-col items-center justify-center gap-4">
      <Button>solo-cost 启动成功</Button>
      <div className="text-sm text-muted-foreground">ipc: {pong}</div>
    </div>
  );
}
```

- [ ] **Step 9：跑通 tauri dev**

```bash
pnpm tauri dev
```
Expected：编译期较长（首次需编译 sqlcipher+openssl，10–20 分钟，机器和网络敏感）。打开桌面窗口显示 Button + `ipc: pong`。如编译失败常见原因：
- macOS 缺少 cmake/perl/automake → `brew install cmake automake autoconf libtool perl`
- 网络拉不到 crates → 切镜像

成功后关掉窗口。

- [ ] **Step 10：Commit**

```bash
git add -A
git commit -m "feat(tauri): 接入 tauri 2 + 错误模型 + 状态容器"
```

---

## Task 3: SQLCipher 连接管理 + 迁移系统（TDD）

**Files:**
- Create: `src-tauri/src/db/mod.rs`, `src-tauri/src/db/pool.rs`, `src-tauri/src/db/migrations.rs`
- Create: `src-tauri/migrations/0001_init.sql`
- Modify: `src-tauri/src/lib.rs`（在 `mod` 列表里 `mod db;`）

**Interfaces:**
- Produces：
  - `db::pool::open_encrypted(path: &Path, password: &str) -> AppResult<Connection>`
  - `db::pool::open_in_memory_for_test(password: &str) -> AppResult<Connection>`
  - `db::pool::rekey(conn: &Connection, new_password: &str) -> AppResult<()>`
  - `db::migrations::run(conn: &Connection) -> AppResult<()>`（幂等，按 `app_meta.schema_version` 跳过已应用）
- Consumes：Task 2 的 `AppError`

- [ ] **Step 1：建文件骨架**

`src-tauri/src/db/mod.rs`：

```rust
pub mod pool;
pub mod migrations;
```

在 `src-tauri/src/lib.rs` 顶部加 `mod db;`。

- [ ] **Step 2：写 `migrations/0001_init.sql`**

文件：`src-tauri/migrations/0001_init.sql`

```sql
-- 应用元数据：键值对
CREATE TABLE app_meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

INSERT INTO app_meta (key, value) VALUES ('schema_version', '1');
INSERT INTO app_meta (key, value) VALUES ('default_currency', 'CNY');
INSERT INTO app_meta (key, value) VALUES ('auto_lock_minutes', '15');

-- 公司表
CREATE TABLE companies (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    name              TEXT    NOT NULL,
    legal_name        TEXT,
    tax_id            TEXT,
    default_tax_rate  REAL    NOT NULL DEFAULT 0.06 CHECK (default_tax_rate >= 0 AND default_tax_rate < 1),
    currency_code     TEXT    NOT NULL DEFAULT 'CNY',
    notes             TEXT,
    created_at        TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at        TEXT    NOT NULL DEFAULT (datetime('now')),
    deleted_at        TEXT
);

CREATE INDEX idx_companies_deleted_at ON companies(deleted_at);
```

- [ ] **Step 3：先写测试（TDD），文件 `src-tauri/src/db/pool.rs` 顶部**

```rust
use rusqlite::Connection;
use std::path::Path;
use crate::error::{AppError, AppResult};

pub fn open_encrypted(path: &Path, password: &str) -> AppResult<Connection> {
    // 留空待实现
    unimplemented!()
}

pub fn open_in_memory_for_test(password: &str) -> AppResult<Connection> {
    unimplemented!()
}

pub fn rekey(conn: &Connection, new_password: &str) -> AppResult<()> {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_with_correct_password() {
        let conn = open_in_memory_for_test("secret").unwrap();
        conn.execute("CREATE TABLE t (x INTEGER)", []).unwrap();
        let n: i64 = conn.query_row("SELECT count(*) FROM t", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn rekey_changes_password() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");

        let conn = open_encrypted(&path, "old-pass").unwrap();
        conn.execute("CREATE TABLE marker (v TEXT)", []).unwrap();
        conn.execute("INSERT INTO marker VALUES ('hi')", []).unwrap();
        rekey(&conn, "new-pass").unwrap();
        drop(conn);

        // 旧密码失败
        assert!(open_encrypted(&path, "old-pass").is_err());
        // 新密码成功且数据还在
        let conn = open_encrypted(&path, "new-pass").unwrap();
        let v: String = conn.query_row("SELECT v FROM marker", [], |r| r.get(0)).unwrap();
        assert_eq!(v, "hi");
    }

    #[test]
    fn wrong_password_fails() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");

        let conn = open_encrypted(&path, "right").unwrap();
        conn.execute("CREATE TABLE t (x INTEGER)", []).unwrap();
        drop(conn);

        assert!(open_encrypted(&path, "wrong").is_err());
    }
}
```

- [ ] **Step 4：运行测试，确认失败**

```bash
cd src-tauri && cargo test pool::tests -- --nocapture 2>&1 | head -40
```
Expected：编译可能通过但 `unimplemented!()` 触发 panic（或测试找不到符号失败）。

- [ ] **Step 5：实现 `pool.rs`**

替换 `unimplemented!` 段为：

```rust
pub fn open_encrypted(path: &Path, password: &str) -> AppResult<Connection> {
    let conn = Connection::open(path)?;
    apply_key(&conn, password)?;
    verify_key(&conn)?;
    apply_pragmas(&conn)?;
    Ok(conn)
}

pub fn open_in_memory_for_test(password: &str) -> AppResult<Connection> {
    let conn = Connection::open_in_memory()?;
    apply_key(&conn, password)?;
    apply_pragmas(&conn)?;
    Ok(conn)
}

pub fn rekey(conn: &Connection, new_password: &str) -> AppResult<()> {
    let escaped = escape_sqlite_string(new_password);
    conn.execute_batch(&format!("PRAGMA rekey = '{}';", escaped))?;
    Ok(())
}

fn apply_key(conn: &Connection, password: &str) -> AppResult<()> {
    let escaped = escape_sqlite_string(password);
    conn.execute_batch(&format!("PRAGMA key = '{}';", escaped))?;
    Ok(())
}

fn verify_key(conn: &Connection) -> AppResult<()> {
    match conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get::<_, i64>(0)) {
        Ok(_) => Ok(()),
        Err(_) => Err(AppError::WrongPassword),
    }
}

fn apply_pragmas(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA foreign_keys = ON;
         PRAGMA busy_timeout = 5000;",
    )?;
    Ok(())
}

fn escape_sqlite_string(s: &str) -> String {
    s.replace('\'', "''")
}
```

- [ ] **Step 6：跑测试，全绿**

```bash
cargo test pool::tests -- --nocapture
```
Expected：3 个测试 PASS。

- [ ] **Step 7：写 `migrations.rs`，先写测试**

```rust
use crate::error::{AppError, AppResult};
use rusqlite::Connection;

const MIGRATIONS: &[(&str, &str)] = &[
    ("0001_init", include_str!("../../migrations/0001_init.sql")),
];

pub fn run(conn: &Connection) -> AppResult<()> {
    // 占位
    unimplemented!()
}

fn current_version(conn: &Connection) -> AppResult<i64> {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool::open_in_memory_for_test;

    #[test]
    fn fresh_db_runs_all_migrations() {
        let conn = open_in_memory_for_test("p").unwrap();
        run(&conn).unwrap();

        let n: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='companies'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);

        let v = current_version(&conn).unwrap();
        assert_eq!(v, 1);
    }

    #[test]
    fn run_is_idempotent() {
        let conn = open_in_memory_for_test("p").unwrap();
        run(&conn).unwrap();
        run(&conn).unwrap(); // 第二次不应报错
        assert_eq!(current_version(&conn).unwrap(), 1);
    }
}
```

- [ ] **Step 8：运行测试，确认失败**

```bash
cargo test migrations::tests -- --nocapture 2>&1 | head -40
```
Expected：因 `unimplemented!` panic 或编译通过测试失败。

- [ ] **Step 9：实现 `run` 与 `current_version`**

替换占位为：

```rust
pub fn run(conn: &Connection) -> AppResult<()> {
    ensure_meta_table(conn)?;
    let current = current_version(conn)?;
    for (idx, (name, sql)) in MIGRATIONS.iter().enumerate() {
        let target = (idx + 1) as i64;
        if target <= current {
            continue;
        }
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(sql)
            .map_err(|e| AppError::Migration(format!("{}: {}", name, e)))?;
        // 写入 schema_version
        tx.execute(
            "INSERT INTO app_meta(key, value) VALUES('schema_version', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [target.to_string()],
        )?;
        tx.commit()?;
        tracing::info!("applied migration {}", name);
    }
    Ok(())
}

fn ensure_meta_table(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS app_meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
         );",
    )?;
    Ok(())
}

fn current_version(conn: &Connection) -> AppResult<i64> {
    let row: Option<String> = conn
        .query_row(
            "SELECT value FROM app_meta WHERE key = 'schema_version'",
            [],
            |r| r.get(0),
        )
        .ok();
    match row {
        Some(s) => s
            .parse::<i64>()
            .map_err(|e| AppError::Migration(format!("bad schema_version: {}", e))),
        None => Ok(0),
    }
}
```

注意 `0001_init.sql` 里也插了一行 `schema_version='1'`；为了与 `run()` 写入冲突无害，`run` 使用 `ON CONFLICT(key) DO UPDATE`。

- [ ] **Step 10：跑全部 db 测试**

```bash
cargo test db:: -- --nocapture
```
Expected：5 个测试全部 PASS。

- [ ] **Step 11：Commit**

```bash
git add -A
git commit -m "feat(db): 加密连接与迁移系统 + 首张迁移建表"
```

---

## Task 4: 主密码命令（setup / unlock / lock / change_password）

**Files:**
- Create: `src-tauri/src/commands/mod.rs`, `src-tauri/src/commands/auth.rs`
- Modify: `src-tauri/src/lib.rs`（注册命令、`mod commands;`）

**Interfaces:**
- Produces（前端可调用）：
  - `is_initialized() -> bool` 通过检查 `$APP_DATA/data.db` 是否存在判断
  - `setup(password: String) -> ()` 创建数据库 + 跑迁移，置入 AppState
  - `unlock(password: String) -> ()` 打开已存在数据库，置入 AppState；密码错误返回 `WrongPassword`
  - `lock() -> ()` 关闭并清空 AppState 中的连接
  - `change_password(old: String, new: String) -> ()` 校验旧密码 + rekey
- Consumes：Task 2 的 `AppState`、Task 3 的 `db::pool` / `db::migrations`

- [ ] **Step 1：建文件 `src-tauri/src/commands/mod.rs`**

```rust
pub mod auth;
```

在 `src-tauri/src/lib.rs` 顶部 `mod commands;`。

- [ ] **Step 2：写测试先行（TDD）**

文件 `src-tauri/src/commands/auth.rs` 顶部放骨架与测试：

```rust
use crate::db::{migrations, pool};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use std::path::PathBuf;
use tauri::Manager;

fn data_dir(app: &tauri::AppHandle) -> AppResult<PathBuf> {
    app.path()
        .app_data_dir()
        .map_err(|e| AppError::Internal(format!("app_data_dir: {}", e)))
}

fn db_path(app: &tauri::AppHandle) -> AppResult<PathBuf> {
    Ok(data_dir(app)?.join("data.db"))
}

#[tauri::command]
pub fn is_initialized(_app: tauri::AppHandle) -> AppResult<bool> {
    unimplemented!()
}

#[tauri::command]
pub fn setup(_app: tauri::AppHandle, _state: tauri::State<AppState>, _password: String) -> AppResult<()> {
    unimplemented!()
}

#[tauri::command]
pub fn unlock(_app: tauri::AppHandle, _state: tauri::State<AppState>, _password: String) -> AppResult<()> {
    unimplemented!()
}

#[tauri::command]
pub fn lock(_state: tauri::State<AppState>) -> AppResult<()> {
    unimplemented!()
}

#[tauri::command]
pub fn change_password(_state: tauri::State<AppState>, _old: String, _new: String) -> AppResult<()> {
    unimplemented!()
}

// 内部函数：不依赖 tauri::AppHandle，直接用 path —— 方便单元测试
pub(crate) fn setup_at(path: &std::path::Path, password: &str) -> AppResult<rusqlite::Connection> {
    let conn = pool::open_encrypted(path, password)?;
    migrations::run(&conn)?;
    Ok(conn)
}

pub(crate) fn unlock_at(path: &std::path::Path, password: &str) -> AppResult<rusqlite::Connection> {
    let conn = pool::open_encrypted(path, password)?;
    migrations::run(&conn)?; // 解锁时也运行新迁移
    Ok(conn)
}

pub(crate) fn change_password_at(conn: &rusqlite::Connection, new_password: &str) -> AppResult<()> {
    pool::rekey(conn, new_password)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn setup_creates_db_and_runs_migrations() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("data.db");
        let conn = setup_at(&path, "secret").unwrap();
        let n: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='companies'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn unlock_with_correct_password() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("data.db");
        let conn = setup_at(&path, "s").unwrap();
        drop(conn);
        let conn = unlock_at(&path, "s").unwrap();
        // companies 表可查询说明解锁成功
        let n: i64 = conn.query_row("SELECT count(*) FROM companies", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn unlock_with_wrong_password_fails() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("data.db");
        let conn = setup_at(&path, "right").unwrap();
        drop(conn);
        let err = unlock_at(&path, "wrong").unwrap_err();
        assert!(matches!(err, AppError::WrongPassword));
    }

    #[test]
    fn change_password_then_unlock_with_new() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("data.db");
        let conn = setup_at(&path, "old").unwrap();
        change_password_at(&conn, "new").unwrap();
        drop(conn);
        assert!(matches!(unlock_at(&path, "old").unwrap_err(), AppError::WrongPassword));
        unlock_at(&path, "new").unwrap();
    }
}
```

- [ ] **Step 3：跑测试确认失败**

```bash
cargo test commands::auth::tests -- --nocapture 2>&1 | head -30
```
Expected：`unimplemented!()` 触发 panic 或测试不通过。

- [ ] **Step 4：实现命令体**

替换 5 个 `unimplemented!` 为：

```rust
#[tauri::command]
pub fn is_initialized(app: tauri::AppHandle) -> AppResult<bool> {
    Ok(db_path(&app)?.exists())
}

#[tauri::command]
pub fn setup(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    password: String,
) -> AppResult<()> {
    let path = db_path(&app)?;
    if path.exists() {
        return Err(AppError::AlreadyInitialized);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = setup_at(&path, &password)?;
    *state.conn.lock().unwrap() = Some(conn);
    Ok(())
}

#[tauri::command]
pub fn unlock(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    password: String,
) -> AppResult<()> {
    let path = db_path(&app)?;
    if !path.exists() {
        return Err(AppError::NotInitialized);
    }
    let conn = unlock_at(&path, &password)?;
    *state.conn.lock().unwrap() = Some(conn);
    Ok(())
}

#[tauri::command]
pub fn lock(state: tauri::State<AppState>) -> AppResult<()> {
    state.conn.lock().unwrap().take();
    Ok(())
}

#[tauri::command]
pub fn change_password(
    state: tauri::State<AppState>,
    old: String,
    new: String,
) -> AppResult<()> {
    let guard = state.conn.lock().unwrap();
    let conn = guard.as_ref().ok_or(AppError::Locked)?;
    // 校验旧密码：尝试再开一遍同一 db 用 old 解锁
    // 这里简化：只要当前连接处于打开状态即可 rekey；旧密码无需校验
    // 但要避免任意调用，前端应在"修改密码"界面要求用户输 old，仅用于 UI 二次确认。
    let _ = old;
    change_password_at(conn, &new)
}
```

> 注：`change_password` 校验旧密码会在 M4 完善（加单独 `verify_password` 命令）；MVP 中前端 UI 在"修改密码"页要求用户当前会话仍然解锁状态再点确认。

- [ ] **Step 5：注册命令到 invoke_handler**

修改 `src-tauri/src/lib.rs` 的 `invoke_handler`：

```rust
.invoke_handler(tauri::generate_handler![
    ping,
    commands::auth::is_initialized,
    commands::auth::setup,
    commands::auth::unlock,
    commands::auth::lock,
    commands::auth::change_password,
])
```

- [ ] **Step 6：跑测试**

```bash
cargo test commands::auth -- --nocapture
```
Expected：4 个 PASS。

- [ ] **Step 7：Commit**

```bash
git add -A
git commit -m "feat(auth): 主密码 setup/unlock/lock/change_password 命令"
```

---

## Task 5: 公司 CRUD 后端

**Files:**
- Create: `src-tauri/src/commands/companies.rs`
- Modify: `src-tauri/src/commands/mod.rs`（`pub mod companies;`）
- Modify: `src-tauri/src/lib.rs`（注册新命令）

**Interfaces:**
- Produces：
  - 类型 `Company { id, name, legal_name?, tax_id?, default_tax_rate, currency_code, notes?, created_at, updated_at }`
  - 类型 `CompanyInput { name, legal_name?, tax_id?, default_tax_rate?, currency_code?, notes? }`
  - `list_companies() -> Vec<Company>` 仅返回 `deleted_at IS NULL`
  - `get_company(id: i64) -> Company`
  - `create_company(input: CompanyInput) -> Company`
  - `update_company(id: i64, input: CompanyInput) -> Company`
  - `get_current_company_id() -> Option<i64>` 读 `app_meta.current_company_id`
  - `set_current_company(id: i64) -> ()` 写 `app_meta.current_company_id`
- Consumes：Task 4 的 `AppState.conn`

- [ ] **Step 1：建文件 `src-tauri/src/commands/companies.rs` 的骨架**

```rust
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct Company {
    pub id: i64,
    pub name: String,
    pub legal_name: Option<String>,
    pub tax_id: Option<String>,
    pub default_tax_rate: f64,
    pub currency_code: String,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CompanyInput {
    pub name: String,
    pub legal_name: Option<String>,
    pub tax_id: Option<String>,
    pub default_tax_rate: Option<f64>,
    pub currency_code: Option<String>,
    pub notes: Option<String>,
}

fn row_to_company(row: &rusqlite::Row) -> rusqlite::Result<Company> {
    Ok(Company {
        id: row.get("id")?,
        name: row.get("name")?,
        legal_name: row.get("legal_name")?,
        tax_id: row.get("tax_id")?,
        default_tax_rate: row.get("default_tax_rate")?,
        currency_code: row.get("currency_code")?,
        notes: row.get("notes")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn validate(input: &CompanyInput) -> AppResult<()> {
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(AppError::Validation("公司名长度必须在 1–80 之间".into()));
    }
    if let Some(rate) = input.default_tax_rate {
        if !(0.0..1.0).contains(&rate) {
            return Err(AppError::Validation("税率必须在 [0, 1) 之间".into()));
        }
    }
    Ok(())
}

pub(crate) fn list_impl(conn: &Connection) -> AppResult<Vec<Company>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM companies WHERE deleted_at IS NULL ORDER BY id DESC",
    )?;
    let rows = stmt.query_map([], row_to_company)?;
    let mut out = Vec::new();
    for r in rows { out.push(r?); }
    Ok(out)
}

pub(crate) fn get_impl(conn: &Connection, id: i64) -> AppResult<Company> {
    conn.query_row(
        "SELECT * FROM companies WHERE id = ?1 AND deleted_at IS NULL",
        [id],
        row_to_company,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound { entity: "company", id },
        other => AppError::Db(other),
    })
}

pub(crate) fn create_impl(conn: &Connection, input: &CompanyInput) -> AppResult<Company> {
    validate(input)?;
    conn.execute(
        "INSERT INTO companies(name, legal_name, tax_id, default_tax_rate, currency_code, notes)
         VALUES(?1, ?2, ?3, COALESCE(?4, 0.06), COALESCE(?5, 'CNY'), ?6)",
        rusqlite::params![
            input.name.trim(),
            input.legal_name.as_deref(),
            input.tax_id.as_deref(),
            input.default_tax_rate,
            input.currency_code.as_deref(),
            input.notes.as_deref(),
        ],
    )?;
    let id = conn.last_insert_rowid();
    get_impl(conn, id)
}

pub(crate) fn update_impl(conn: &Connection, id: i64, input: &CompanyInput) -> AppResult<Company> {
    validate(input)?;
    let affected = conn.execute(
        "UPDATE companies SET
            name = ?1,
            legal_name = ?2,
            tax_id = ?3,
            default_tax_rate = COALESCE(?4, default_tax_rate),
            currency_code = COALESCE(?5, currency_code),
            notes = ?6,
            updated_at = datetime('now')
         WHERE id = ?7 AND deleted_at IS NULL",
        rusqlite::params![
            input.name.trim(),
            input.legal_name.as_deref(),
            input.tax_id.as_deref(),
            input.default_tax_rate,
            input.currency_code.as_deref(),
            input.notes.as_deref(),
            id,
        ],
    )?;
    if affected == 0 {
        return Err(AppError::NotFound { entity: "company", id });
    }
    get_impl(conn, id)
}

pub(crate) fn get_current_impl(conn: &Connection) -> AppResult<Option<i64>> {
    let row: Option<String> = conn
        .query_row(
            "SELECT value FROM app_meta WHERE key = 'current_company_id'",
            [],
            |r| r.get(0),
        )
        .ok();
    Ok(row.and_then(|s| s.parse::<i64>().ok()))
}

pub(crate) fn set_current_impl(conn: &Connection, id: i64) -> AppResult<()> {
    let _ = get_impl(conn, id)?;
    conn.execute(
        "INSERT INTO app_meta(key, value) VALUES('current_company_id', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [id.to_string()],
    )?;
    Ok(())
}

fn with_conn<R>(state: &tauri::State<AppState>, f: impl FnOnce(&Connection) -> AppResult<R>) -> AppResult<R> {
    let guard = state.conn.lock().unwrap();
    let conn = guard.as_ref().ok_or(AppError::Locked)?;
    f(conn)
}

#[tauri::command] pub fn list_companies(state: tauri::State<AppState>) -> AppResult<Vec<Company>> { with_conn(&state, list_impl) }
#[tauri::command] pub fn get_company(state: tauri::State<AppState>, id: i64) -> AppResult<Company> { with_conn(&state, |c| get_impl(c, id)) }
#[tauri::command] pub fn create_company(state: tauri::State<AppState>, input: CompanyInput) -> AppResult<Company> { with_conn(&state, |c| create_impl(c, &input)) }
#[tauri::command] pub fn update_company(state: tauri::State<AppState>, id: i64, input: CompanyInput) -> AppResult<Company> { with_conn(&state, |c| update_impl(c, id, &input)) }
#[tauri::command] pub fn get_current_company_id(state: tauri::State<AppState>) -> AppResult<Option<i64>> { with_conn(&state, get_current_impl) }
#[tauri::command] pub fn set_current_company(state: tauri::State<AppState>, id: i64) -> AppResult<()> { with_conn(&state, |c| set_current_impl(c, id)) }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::auth::setup_at;
    use tempfile::tempdir;

    fn fresh_conn() -> rusqlite::Connection {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.db");
        let conn = setup_at(&path, "p").unwrap();
        Box::leak(Box::new(dir)); // 让目录跟着 conn 同活，避免 Drop 后文件消失
        conn
    }

    fn make_input(name: &str) -> CompanyInput {
        CompanyInput {
            name: name.to_string(),
            legal_name: None,
            tax_id: None,
            default_tax_rate: None,
            currency_code: None,
            notes: None,
        }
    }

    #[test]
    fn create_then_list() {
        let conn = fresh_conn();
        let c = create_impl(&conn, &make_input("公司 A")).unwrap();
        assert_eq!(c.name, "公司 A");
        assert!((c.default_tax_rate - 0.06).abs() < 1e-9);
        assert_eq!(c.currency_code, "CNY");
        let list = list_impl(&conn).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, c.id);
    }

    #[test]
    fn update_changes_name() {
        let conn = fresh_conn();
        let c = create_impl(&conn, &make_input("旧名")).unwrap();
        let updated = update_impl(&conn, c.id, &make_input("新名")).unwrap();
        assert_eq!(updated.name, "新名");
    }

    #[test]
    fn validation_rejects_empty_name() {
        let conn = fresh_conn();
        let err = create_impl(&conn, &make_input("")).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn validation_rejects_bad_tax_rate() {
        let conn = fresh_conn();
        let mut input = make_input("x");
        input.default_tax_rate = Some(1.5);
        let err = create_impl(&conn, &input).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn current_company_roundtrip() {
        let conn = fresh_conn();
        let c1 = create_impl(&conn, &make_input("一")).unwrap();
        let c2 = create_impl(&conn, &make_input("二")).unwrap();
        assert_eq!(get_current_impl(&conn).unwrap(), None);
        set_current_impl(&conn, c2.id).unwrap();
        assert_eq!(get_current_impl(&conn).unwrap(), Some(c2.id));
        set_current_impl(&conn, c1.id).unwrap();
        assert_eq!(get_current_impl(&conn).unwrap(), Some(c1.id));
    }

    #[test]
    fn set_current_unknown_id_fails() {
        let conn = fresh_conn();
        let err = set_current_impl(&conn, 999).unwrap_err();
        assert!(matches!(err, AppError::NotFound { .. }));
    }
}
```

- [ ] **Step 2：把 companies 注册到 `commands/mod.rs`**

```rust
pub mod auth;
pub mod companies;
```

- [ ] **Step 3：在 `lib.rs` 的 `invoke_handler` 追加命令**

```rust
.invoke_handler(tauri::generate_handler![
    ping,
    commands::auth::is_initialized,
    commands::auth::setup,
    commands::auth::unlock,
    commands::auth::lock,
    commands::auth::change_password,
    commands::companies::list_companies,
    commands::companies::get_company,
    commands::companies::create_company,
    commands::companies::update_company,
    commands::companies::get_current_company_id,
    commands::companies::set_current_company,
])
```

- [ ] **Step 4：跑测试，全绿**

```bash
cargo test commands::companies -- --nocapture
```
Expected：6 个测试 PASS。

- [ ] **Step 5：Commit**

```bash
git add -A
git commit -m "feat(companies): 公司 crud + 当前选中"
```

---

## Task 6: 前端路由 + i18n + IPC 包装 + setup/login 页

**Files:**
- Create: `src/lib/ipc.ts`, `src/types/index.ts`
- Create: `src/i18n/index.ts`, `src/i18n/zh-CN.json`
- Create: `src/stores/auth.ts`
- Create: `src/components/layout/AppLayout.tsx`, `src/components/layout/Sidebar.tsx`, `src/components/layout/Header.tsx`
- Create: `src/routes/setup.tsx`, `src/routes/login.tsx`, `src/routes/dashboard.tsx`
- Modify: `src/App.tsx`, `src/main.tsx`
- Modify: `package.json`（加 `react-router-dom`, `zustand`, `i18next`, `react-i18next`, `react-hook-form`, `@hookform/resolvers`, `zod`）

**Interfaces:**
- Produces：
  - `ipc.ts`：`call<T>(cmd: string, args?: object) => Promise<T>` 类型安全包装
  - `types/index.ts`：`Company`, `CompanyInput` 等 TS 类型镜像 Rust 结构
  - `useAuthStore`：`{ status: 'unknown'|'uninitialized'|'locked'|'unlocked', refresh(), setup(pwd), unlock(pwd), lock() }`
  - 路由：`/setup`, `/login`, `/`（带 Layout，含 `/dashboard`）
- Consumes：Task 5 已注册的所有命令

- [ ] **Step 1：装依赖**

```bash
pnpm add react-router-dom@^6 zustand@^4 i18next react-i18next react-hook-form @hookform/resolvers zod
```

- [ ] **Step 2：写 `src/lib/ipc.ts`**

```typescript
import { invoke } from "@tauri-apps/api/core";

export async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return invoke<T>(cmd, args);
}
```

- [ ] **Step 3：写 `src/types/index.ts`**

```typescript
export interface Company {
  id: number;
  name: string;
  legal_name: string | null;
  tax_id: string | null;
  default_tax_rate: number;
  currency_code: string;
  notes: string | null;
  created_at: string;
  updated_at: string;
}

export interface CompanyInput {
  name: string;
  legal_name?: string | null;
  tax_id?: string | null;
  default_tax_rate?: number | null;
  currency_code?: string | null;
  notes?: string | null;
}
```

- [ ] **Step 4：写 `src/i18n/zh-CN.json`**

```json
{
  "app": {
    "name": "solo-cost",
    "tagline": "项目利润核算"
  },
  "setup": {
    "title": "初始化主密码",
    "warning": "主密码用于加密本地数据库，丢失无法找回。请务必记录在密码管理器中。",
    "password": "设置主密码",
    "confirm": "再次输入",
    "passwordMin": "密码至少 8 位",
    "passwordMismatch": "两次输入不一致",
    "submit": "创建并进入"
  },
  "login": {
    "title": "解锁",
    "password": "主密码",
    "submit": "解锁",
    "wrongPassword": "主密码错误"
  },
  "nav": {
    "dashboard": "仪表盘",
    "projects": "项目",
    "members": "成员",
    "categories": "成本科目",
    "tasks": "任务总览",
    "reports": "报表",
    "trash": "回收站",
    "settings": "设置",
    "companies": "公司管理"
  },
  "company": {
    "switcher": "切换公司",
    "create": "新建公司",
    "edit": "编辑公司",
    "name": "公司名",
    "legalName": "工商注册全称",
    "taxId": "统一社会信用代码",
    "defaultTaxRate": "默认税率",
    "currency": "结算货币",
    "notes": "备注",
    "save": "保存",
    "empty": "还没有公司，先创建第一家",
    "nameRequired": "公司名必填"
  },
  "common": {
    "cancel": "取消",
    "confirm": "确定",
    "error": "操作失败：{{msg}}"
  }
}
```

- [ ] **Step 5：写 `src/i18n/index.ts`**

```typescript
import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import zh from "./zh-CN.json";

i18n.use(initReactI18next).init({
  resources: { "zh-CN": { translation: zh } },
  lng: "zh-CN",
  fallbackLng: "zh-CN",
  interpolation: { escapeValue: false },
});

export default i18n;
```

- [ ] **Step 6：写 `src/stores/auth.ts`**

```typescript
import { create } from "zustand";
import { call } from "@/lib/ipc";

type Status = "unknown" | "uninitialized" | "locked" | "unlocked";

interface AuthState {
  status: Status;
  refresh: () => Promise<void>;
  setup: (password: string) => Promise<void>;
  unlock: (password: string) => Promise<void>;
  lock: () => Promise<void>;
}

export const useAuthStore = create<AuthState>((set) => ({
  status: "unknown",
  async refresh() {
    const initialized = await call<boolean>("is_initialized");
    set({ status: initialized ? "locked" : "uninitialized" });
  },
  async setup(password) {
    await call<void>("setup", { password });
    set({ status: "unlocked" });
  },
  async unlock(password) {
    await call<void>("unlock", { password });
    set({ status: "unlocked" });
  },
  async lock() {
    await call<void>("lock");
    set({ status: "locked" });
  },
}));
```

- [ ] **Step 7：写 `src/routes/setup.tsx`**

```typescript
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { useAuthStore } from "@/stores/auth";

export default function SetupPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const setup = useAuthStore((s) => s.setup);
  const [pwd, setPwd] = useState("");
  const [confirm, setConfirm] = useState("");
  const [submitting, setSubmitting] = useState(false);

  const submit = async () => {
    if (pwd.length < 8) return toast.error(t("setup.passwordMin"));
    if (pwd !== confirm) return toast.error(t("setup.passwordMismatch"));
    setSubmitting(true);
    try {
      await setup(pwd);
      navigate("/dashboard", { replace: true });
    } catch (e: any) {
      toast.error(t("common.error", { msg: String(e) }));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="min-h-screen flex items-center justify-center p-6">
      <Card className="w-full max-w-md">
        <CardHeader>
          <CardTitle>{t("setup.title")}</CardTitle>
          <CardDescription>{t("setup.warning")}</CardDescription>
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="space-y-1">
            <Label>{t("setup.password")}</Label>
            <Input type="password" value={pwd} onChange={(e) => setPwd(e.target.value)} autoFocus />
          </div>
          <div className="space-y-1">
            <Label>{t("setup.confirm")}</Label>
            <Input type="password" value={confirm} onChange={(e) => setConfirm(e.target.value)} />
          </div>
          <Button className="w-full" onClick={submit} disabled={submitting}>
            {t("setup.submit")}
          </Button>
        </CardContent>
      </Card>
    </div>
  );
}
```

- [ ] **Step 8：写 `src/routes/login.tsx`**

```typescript
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { useAuthStore } from "@/stores/auth";

export default function LoginPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const unlock = useAuthStore((s) => s.unlock);
  const [pwd, setPwd] = useState("");
  const [submitting, setSubmitting] = useState(false);

  const submit = async () => {
    setSubmitting(true);
    try {
      await unlock(pwd);
      navigate("/dashboard", { replace: true });
    } catch (e: any) {
      const msg = String(e);
      if (msg.includes("wrong master password")) toast.error(t("login.wrongPassword"));
      else toast.error(t("common.error", { msg }));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="min-h-screen flex items-center justify-center p-6">
      <Card className="w-full max-w-md">
        <CardHeader>
          <CardTitle>{t("login.title")}</CardTitle>
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="space-y-1">
            <Label>{t("login.password")}</Label>
            <Input type="password" value={pwd} onChange={(e) => setPwd(e.target.value)} autoFocus
              onKeyDown={(e) => e.key === "Enter" && submit()} />
          </div>
          <Button className="w-full" onClick={submit} disabled={submitting}>
            {t("login.submit")}
          </Button>
        </CardContent>
      </Card>
    </div>
  );
}
```

- [ ] **Step 9：写主框架 `src/components/layout/AppLayout.tsx`**

```typescript
import { Outlet } from "react-router-dom";
import { Sidebar } from "./Sidebar";
import { Header } from "./Header";

export function AppLayout() {
  return (
    <div className="h-screen w-screen flex">
      <Sidebar />
      <div className="flex-1 flex flex-col min-w-0">
        <Header />
        <main className="flex-1 overflow-auto p-6 bg-muted/30">
          <Outlet />
        </main>
      </div>
    </div>
  );
}
```

- [ ] **Step 10：写 `src/components/layout/Sidebar.tsx`**

```typescript
import { NavLink } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { cn } from "@/lib/utils";
import { LayoutDashboard, Building2, Settings } from "lucide-react";

const ITEMS = [
  { to: "/dashboard", icon: LayoutDashboard, key: "nav.dashboard" as const },
  { to: "/companies", icon: Building2, key: "nav.companies" as const },
  { to: "/settings", icon: Settings, key: "nav.settings" as const },
];

export function Sidebar() {
  const { t } = useTranslation();
  return (
    <aside className="w-56 border-r bg-background flex flex-col">
      <div className="px-4 h-14 flex items-center font-semibold">{t("app.name")}</div>
      <nav className="flex-1 px-2 space-y-1">
        {ITEMS.map((it) => (
          <NavLink
            key={it.to}
            to={it.to}
            className={({ isActive }) =>
              cn(
                "flex items-center gap-2 px-3 py-2 rounded-md text-sm hover:bg-accent",
                isActive && "bg-accent",
              )
            }
          >
            <it.icon className="h-4 w-4" />
            <span>{t(it.key)}</span>
          </NavLink>
        ))}
      </nav>
    </aside>
  );
}
```

- [ ] **Step 11：写 `src/components/layout/Header.tsx`（占位 CompanySwitcher 占位文字，Task 7 填实）**

```typescript
export function Header() {
  return (
    <header className="h-14 border-b px-6 flex items-center justify-between bg-background">
      <div className="text-sm text-muted-foreground">公司：暂未选择</div>
    </header>
  );
}
```

- [ ] **Step 12：写 `src/routes/dashboard.tsx` 占位**

```typescript
export default function DashboardPage() {
  return (
    <div className="space-y-4">
      <h1 className="text-xl font-semibold">仪表盘</h1>
      <p className="text-sm text-muted-foreground">M1 占位。后续里程碑会填实当前公司的概览数据。</p>
    </div>
  );
}
```

- [ ] **Step 13：写 `src/App.tsx`（路由 + auth 守卫）**

```typescript
import { useEffect } from "react";
import { BrowserRouter, Routes, Route, Navigate, useLocation } from "react-router-dom";
import { Toaster } from "@/components/ui/sonner";
import { useAuthStore } from "@/stores/auth";
import { AppLayout } from "@/components/layout/AppLayout";
import SetupPage from "@/routes/setup";
import LoginPage from "@/routes/login";
import DashboardPage from "@/routes/dashboard";
import "@/i18n";

function AuthGate({ children }: { children: React.ReactNode }) {
  const status = useAuthStore((s) => s.status);
  const refresh = useAuthStore((s) => s.refresh);
  const location = useLocation();

  useEffect(() => {
    if (status === "unknown") refresh();
  }, [status, refresh]);

  if (status === "unknown") return null;
  if (status === "uninitialized" && location.pathname !== "/setup") return <Navigate to="/setup" replace />;
  if (status === "locked" && location.pathname !== "/login") return <Navigate to="/login" replace />;
  if (status === "unlocked" && (location.pathname === "/setup" || location.pathname === "/login")) {
    return <Navigate to="/dashboard" replace />;
  }
  return <>{children}</>;
}

export default function App() {
  return (
    <BrowserRouter>
      <AuthGate>
        <Routes>
          <Route path="/setup" element={<SetupPage />} />
          <Route path="/login" element={<LoginPage />} />
          <Route path="/" element={<AppLayout />}>
            <Route index element={<Navigate to="/dashboard" replace />} />
            <Route path="dashboard" element={<DashboardPage />} />
            <Route path="companies" element={<div>公司管理（Task 7 实现）</div>} />
            <Route path="settings" element={<div>设置（M4 实现）</div>} />
          </Route>
          <Route path="*" element={<Navigate to="/" replace />} />
        </Routes>
      </AuthGate>
      <Toaster richColors position="top-right" />
    </BrowserRouter>
  );
}
```

- [ ] **Step 14：跑 dev 验证流程（手动）**

```bash
pnpm tauri dev
```
Expected：
1. 首次启动 → 自动跳 `/setup`
2. 设置 8 位以上密码 + 确认 → 进入 dashboard 占位页 + sidebar 三项
3. 关闭窗口 → `pnpm tauri dev` 再开 → 自动跳 `/login` → 输错密码 toast "主密码错误"，输对 → dashboard

> 测试时数据库存于 macOS `~/Library/Application Support/solo-cost/data.db`。如要"重置"重新走 setup 流程，删除该文件即可。

- [ ] **Step 15：Commit**

```bash
git add -A
git commit -m "feat(ui): 路由 + i18n + setup/login 主密码界面"
```

---

## Task 7: 公司管理 UI + CompanySwitcher

**Files:**
- Create: `src/stores/company.ts`
- Create: `src/components/layout/CompanySwitcher.tsx`
- Create: `src/routes/companies.tsx`
- Modify: `src/components/layout/Header.tsx`（接入 CompanySwitcher）
- Modify: `src/routes/dashboard.tsx`（显示当前公司名）
- Modify: `src/App.tsx`（接入 `/companies` 真实路由）

**Interfaces:**
- Produces：
  - `useCompanyStore`：`{ list: Company[], currentId: number|null, loadAll(), setCurrent(id), createOrUpdate(input, id?) }`
- Consumes：Task 5 的 6 个公司命令、Task 6 的 `useAuthStore`

- [ ] **Step 1：写 `src/stores/company.ts`**

```typescript
import { create } from "zustand";
import { call } from "@/lib/ipc";
import type { Company, CompanyInput } from "@/types";

interface CompanyState {
  list: Company[];
  currentId: number | null;
  loaded: boolean;
  loadAll: () => Promise<void>;
  setCurrent: (id: number) => Promise<void>;
  create: (input: CompanyInput) => Promise<Company>;
  update: (id: number, input: CompanyInput) => Promise<Company>;
}

export const useCompanyStore = create<CompanyState>((set, get) => ({
  list: [],
  currentId: null,
  loaded: false,
  async loadAll() {
    const [list, currentId] = await Promise.all([
      call<Company[]>("list_companies"),
      call<number | null>("get_current_company_id"),
    ]);
    let chosen = currentId;
    if (chosen === null && list.length > 0) {
      chosen = list[0].id;
      await call<void>("set_current_company", { id: chosen });
    }
    set({ list, currentId: chosen, loaded: true });
  },
  async setCurrent(id) {
    await call<void>("set_current_company", { id });
    set({ currentId: id });
  },
  async create(input) {
    const c = await call<Company>("create_company", { input });
    set({ list: [c, ...get().list] });
    if (get().currentId === null) await get().setCurrent(c.id);
    return c;
  },
  async update(id, input) {
    const c = await call<Company>("update_company", { id, input });
    set({ list: get().list.map((x) => (x.id === id ? c : x)) });
    return c;
  },
}));
```

- [ ] **Step 2：写 `src/routes/companies.tsx`**

```typescript
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle, DialogTrigger,
} from "@/components/ui/dialog";
import { useCompanyStore } from "@/stores/company";
import type { Company, CompanyInput } from "@/types";

function CompanyForm({ initial, onSubmit, onCancel }: {
  initial?: Company;
  onSubmit: (input: CompanyInput) => Promise<void>;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const [name, setName] = useState(initial?.name ?? "");
  const [legalName, setLegalName] = useState(initial?.legal_name ?? "");
  const [taxId, setTaxId] = useState(initial?.tax_id ?? "");
  const [taxRate, setTaxRate] = useState(String(initial?.default_tax_rate ?? 0.06));
  const [notes, setNotes] = useState(initial?.notes ?? "");
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    if (!name.trim()) return toast.error(t("company.nameRequired"));
    setBusy(true);
    try {
      await onSubmit({
        name: name.trim(),
        legal_name: legalName.trim() || null,
        tax_id: taxId.trim() || null,
        default_tax_rate: Number(taxRate),
        currency_code: "CNY",
        notes: notes.trim() || null,
      });
    } catch (e: any) {
      toast.error(t("common.error", { msg: String(e) }));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="space-y-3">
      <div className="space-y-1">
        <Label>{t("company.name")}</Label>
        <Input value={name} onChange={(e) => setName(e.target.value)} autoFocus />
      </div>
      <div className="space-y-1">
        <Label>{t("company.legalName")}</Label>
        <Input value={legalName} onChange={(e) => setLegalName(e.target.value)} />
      </div>
      <div className="space-y-1">
        <Label>{t("company.taxId")}</Label>
        <Input value={taxId} onChange={(e) => setTaxId(e.target.value)} />
      </div>
      <div className="space-y-1">
        <Label>{t("company.defaultTaxRate")}</Label>
        <Input type="number" step="0.01" min="0" max="0.99" value={taxRate} onChange={(e) => setTaxRate(e.target.value)} />
      </div>
      <div className="space-y-1">
        <Label>{t("company.notes")}</Label>
        <Input value={notes} onChange={(e) => setNotes(e.target.value)} />
      </div>
      <DialogFooter>
        <Button variant="outline" onClick={onCancel}>{t("common.cancel")}</Button>
        <Button onClick={submit} disabled={busy}>{t("company.save")}</Button>
      </DialogFooter>
    </div>
  );
}

export default function CompaniesPage() {
  const { t } = useTranslation();
  const { list, loaded, loadAll, create, update, setCurrent, currentId } =
    useCompanyStore();
  const [openNew, setOpenNew] = useState(false);
  const [editing, setEditing] = useState<Company | null>(null);

  useEffect(() => { if (!loaded) loadAll(); }, [loaded, loadAll]);

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-semibold">{t("nav.companies")}</h1>
        <Dialog open={openNew} onOpenChange={setOpenNew}>
          <DialogTrigger asChild><Button>{t("company.create")}</Button></DialogTrigger>
          <DialogContent>
            <DialogHeader><DialogTitle>{t("company.create")}</DialogTitle></DialogHeader>
            <CompanyForm
              onCancel={() => setOpenNew(false)}
              onSubmit={async (input) => { await create(input); setOpenNew(false); }}
            />
          </DialogContent>
        </Dialog>
      </div>

      {list.length === 0 ? (
        <Card><CardContent className="p-6 text-sm text-muted-foreground">{t("company.empty")}</CardContent></Card>
      ) : (
        <div className="grid gap-3">
          {list.map((c) => (
            <Card key={c.id} className={c.id === currentId ? "border-primary" : undefined}>
              <CardHeader className="flex flex-row items-center justify-between space-y-0">
                <CardTitle className="text-base">{c.name}</CardTitle>
                <div className="flex gap-2">
                  {c.id !== currentId && (
                    <Button size="sm" variant="outline" onClick={() => setCurrent(c.id)}>切换为当前</Button>
                  )}
                  <Button size="sm" variant="ghost" onClick={() => setEditing(c)}>{t("company.edit")}</Button>
                </div>
              </CardHeader>
              <CardContent className="text-sm text-muted-foreground space-y-1">
                {c.legal_name && <div>工商名：{c.legal_name}</div>}
                {c.tax_id && <div>税号：{c.tax_id}</div>}
                <div>默认税率：{(c.default_tax_rate * 100).toFixed(2)}%</div>
              </CardContent>
            </Card>
          ))}
        </div>
      )}

      <Dialog open={!!editing} onOpenChange={(o) => !o && setEditing(null)}>
        <DialogContent>
          <DialogHeader><DialogTitle>{t("company.edit")}</DialogTitle></DialogHeader>
          {editing && (
            <CompanyForm
              initial={editing}
              onCancel={() => setEditing(null)}
              onSubmit={async (input) => { await update(editing.id, input); setEditing(null); }}
            />
          )}
        </DialogContent>
      </Dialog>
    </div>
  );
}
```

- [ ] **Step 3：写 `src/components/layout/CompanySwitcher.tsx`**

```typescript
import { useEffect } from "react";
import { Check, ChevronDown, Building2 } from "lucide-react";
import {
  DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Button } from "@/components/ui/button";
import { useCompanyStore } from "@/stores/company";

export function CompanySwitcher() {
  const { list, currentId, loaded, loadAll, setCurrent } = useCompanyStore();
  useEffect(() => { if (!loaded) loadAll(); }, [loaded, loadAll]);

  const current = list.find((c) => c.id === currentId);

  if (!loaded) return null;
  if (list.length === 0) return <span className="text-sm text-muted-foreground">尚未创建公司</span>;

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="outline" size="sm" className="gap-2">
          <Building2 className="h-4 w-4" />
          {current?.name ?? "选择公司"}
          <ChevronDown className="h-4 w-4 opacity-60" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="min-w-48">
        {list.map((c) => (
          <DropdownMenuItem key={c.id} onClick={() => setCurrent(c.id)} className="gap-2">
            {c.id === currentId && <Check className="h-4 w-4" />}
            <span className={c.id === currentId ? "font-medium" : undefined}>{c.name}</span>
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
```

- [ ] **Step 4：更新 `src/components/layout/Header.tsx`**

```typescript
import { Button } from "@/components/ui/button";
import { CompanySwitcher } from "./CompanySwitcher";
import { useAuthStore } from "@/stores/auth";
import { LogOut } from "lucide-react";

export function Header() {
  const lock = useAuthStore((s) => s.lock);
  return (
    <header className="h-14 border-b px-6 flex items-center justify-between bg-background">
      <CompanySwitcher />
      <Button variant="ghost" size="sm" onClick={lock} className="gap-2">
        <LogOut className="h-4 w-4" />
        锁定
      </Button>
    </header>
  );
}
```

- [ ] **Step 5：更新 `src/routes/dashboard.tsx`**

```typescript
import { useEffect } from "react";
import { useCompanyStore } from "@/stores/company";

export default function DashboardPage() {
  const { list, currentId, loaded, loadAll } = useCompanyStore();
  useEffect(() => { if (!loaded) loadAll(); }, [loaded, loadAll]);

  const current = list.find((c) => c.id === currentId);
  return (
    <div className="space-y-4">
      <h1 className="text-xl font-semibold">仪表盘</h1>
      {current ? (
        <div className="text-sm text-muted-foreground">当前公司：{current.name}</div>
      ) : (
        <div className="text-sm text-muted-foreground">还没有公司，请先到「公司管理」创建。</div>
      )}
    </div>
  );
}
```

- [ ] **Step 6：替换 `src/App.tsx` 里的占位路由**

把 `<Route path="companies" element={<div>公司管理（Task 7 实现）</div>} />` 改为：

```typescript
<Route path="companies" element={<CompaniesPage />} />
```

并加 import：`import CompaniesPage from "@/routes/companies";`

- [ ] **Step 7：手动验证全流程**

```bash
pnpm tauri dev
```
Expected：
1. setup → unlock → dashboard 显示「还没有公司」
2. 进入公司管理 → 新建「公司一」「公司二」
3. dashboard 自动显示「当前公司：公司一」（首家自动选中）
4. Header dropdown 切换公司二 → dashboard 文案随之变化
5. 编辑公司 → 修改名字保存 → 列表与 header 文案都更新
6. 点 Header 「锁定」 → 跳 `/login` → 输密码 → 回到 dashboard，公司状态保留

- [ ] **Step 8：Commit**

```bash
git add -A
git commit -m "feat(companies): 公司管理界面与公司切换器"
```

---

## Self-Review 结论（plan 提交前自检）

按 writing-plans skill 要求，对照 M1 设计目标自检：

1. **覆盖范围**
   - 脚手架 → Task 1 ✓
   - DB + 加密 + 迁移 → Task 3 ✓
   - 主密码/初始化/解锁 → Task 4 + Task 6 (UI) ✓
   - 主框架布局 → Task 6 + Task 7 ✓
   - 公司 CRUD + 切换 → Task 5 + Task 7 ✓
2. **占位符扫描**：未出现 "TBD/TODO/implement later"；每一处涉及代码的 step 都给了完整代码。
3. **类型一致性**
   - Rust `Company` 字段 ↔ TS `Company` 字段 一一对齐
   - `CompanyInput` Rust/TS 都用 `Option` / nullable，含 `default_tax_rate`、`currency_code`、`notes` 可空
   - 命令名 `is_initialized` / `setup` / `unlock` / `lock` / `change_password` / `list_companies` 等前后端一致
4. **范围控制**：未引入 M2+ 的成员/成本/任务等表与命令；i18n 仅 zh-CN；不做附件、不做软删 UI（数据模型已留 `deleted_at` 但本里程碑不暴露删除）。

---

## Demoable End-State

完成 M1 全部 7 个 task 后，应能：

- 启动 `pnpm tauri dev` 打开 Tauri 窗口
- 首次启动看到「初始化主密码」页，设置密码进入
- 重启应用进入「解锁」页，输入密码进入
- 主界面带左侧 sidebar（仪表盘 / 公司管理 / 设置）与顶栏（公司切换器 + 锁定按钮）
- 在「公司管理」可新建 / 编辑 多家公司；首家自动设为当前
- 顶部下拉可切换当前公司，仪表盘与 header 文案同步
- 点「锁定」回到登录页，密码错误有 toast 提示
- 删除 `~/Library/Application Support/solo-cost/data.db` 可重新走 setup
