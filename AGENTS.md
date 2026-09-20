# Project Instructions

## Project Overview

`solo-cost` is a desktop cost and time management application for individuals and small teams.

- Frontend: React 19, Vite, TypeScript, Tailwind CSS, and Zustand
- Desktop/backend: Tauri v2 and Rust
- Storage: SQLite encrypted through SQLCipher
- Package manager: pnpm
- User-facing language: Simplified Chinese

## Repository Layout

- `src/`: React application, routes, components, stores, types, and localization
- `src-tauri/src/commands/`: Tauri command handlers exposed to the frontend
- `src-tauri/src/domain/`: reusable Rust domain logic
- `src-tauri/src/db/`: database initialization, pooling, and migration support
- `src-tauri/migrations/`: ordered database migrations
- `docs/superpowers/`: feature designs and implementation plans
- `public/`: static frontend assets

Keep frontend IPC wrappers in `src/lib/ipc.ts` and backend command registration consistent when adding or changing Tauri commands.

## Development Rules

- Use `pnpm`; do not introduce npm or yarn lockfiles.
- Do not install or update dependencies without explicit approval.
- Keep changes focused on the requested task. Do not perform opportunistic refactors.
- Preserve public APIs unless the task explicitly requires an incompatible change.
- Use English for code comments. User-facing copy belongs in `src/i18n/zh-CN.json` unless an existing feature follows another established pattern.
- Handle errors explicitly. Do not silently discard exceptions or Rust `Result` values.
- Do not modify CI/CD configuration unless explicitly requested.
- Do not modify generated output such as `dist/`, `src-tauri/target/`, installers, or disk images.
- This project does not use unit tests. Do not add or maintain unit-test code or test-only dependencies.

## Database Changes

- Treat committed migrations as immutable because existing encrypted databases may already have applied them.
- Add a new sequentially numbered migration for schema changes.
- Keep migration execution compatible with existing databases and update the Rust migration registry when required by the current implementation.
- Never commit database files, WAL files, credentials, master passwords, or decrypted user data.

## Verification

Run checks relevant to the changed area before reporting completion:

```bash
pnpm lint
pnpm build
cargo check --manifest-path src-tauri/Cargo.toml
```

- Frontend-only changes require at least `pnpm lint` and `pnpm build`.
- Rust or database changes require `cargo check --manifest-path src-tauri/Cargo.toml`; run the frontend checks too when IPC types or calls change.
- For documentation-only or agent-instruction changes, inspect the rendered content and Git diff; full builds are unnecessary unless the documentation changes executable examples or build configuration.
- Report every failed or skipped verification command honestly.

## Git, Changelog, and Releases

- Follow Conventional Commits: `<type>(<scope>): <中文主题>`.
- Keep the first commit line at most 72 characters and do not end it with punctuation.
- Explain the reason, impact, and solution in the commit body when context is not obvious.
- After committing user-visible behavior, compatibility, or security changes, use the `changelog` skill to update `CHANGELOG.md`. Do not add changelog entries for internal-only refactors, tests, comments, formatting, or agent instructions.
- Follow Semantic Versioning. During the `0.x` phase, minor releases may contain breaking changes.
- Do not initiate a release unless the user explicitly asks. When asked, use the `release` skill.
