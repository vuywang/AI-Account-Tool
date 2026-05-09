# AGENTS.md

This file gives coding agents the local rules and project context needed to work safely in this repository.

## Project Snapshot

AI Account Tool is a Tauri 2 desktop app for Windows-focused Codex account management. The frontend is built with React, Vite, TypeScript, and lucide-react. The backend is Rust code exposed through Tauri commands.

Primary capabilities:

- Import existing Codex `auth.json` accounts.
- Add OAuth token accounts and API key accounts.
- Switch the active Codex account by writing the selected account into a `CODEX_HOME`.
- Save and switch API relay channels backed by `api-channels.json`.
- Create and launch multiple Codex instances with separate `CODEX_HOME` directories.
- Refresh and display OAuth quota information.

## Repository Layout

- `src/` - React frontend.
- `src/App.tsx` - Main UI and Tauri command calls.
- `src/App.css` - App styling.
- `public/` - Frontend static assets.
- `src-tauri/` - Tauri/Rust backend.
- `src-tauri/src/commands.rs` - Tauri command handlers and app orchestration.
- `src-tauri/src/codex.rs` - Codex auth parsing, account writing, CLI discovery, launch helpers.
- `src-tauri/src/api_channels.rs` - API relay channel storage and safe view models.
- `src-tauri/src/quota.rs` - ChatGPT/Codex usage quota fetching and parsing.
- `src-tauri/src/storage.rs` - Local data directory, store loading/saving, utility helpers.
- `src-tauri/src/models.rs` - Shared Rust data models.
- `src-tauri/tauri.conf.json` - Tauri app configuration.

## Common Commands

Run commands from the repository root unless noted.

```powershell
npm install
npm run dev
npm run build
npm run tauri dev
npm run tauri build
cargo check --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml
```

Useful focused checks:

```powershell
npx tsc --noEmit
cargo check --manifest-path src-tauri/Cargo.toml
```

## Working Guidelines

- Check `git status --short` before editing. The worktree may contain user changes; do not revert or overwrite them unless explicitly asked.
- Prefer small, scoped changes that match the existing React/Tauri structure.
- Keep frontend command names and Rust command handlers synchronized. Tauri commands are registered in `src-tauri/src/lib.rs`.
- Keep TypeScript view types aligned with Rust serialized models in `src-tauri/src/models.rs`.
- Do not commit or print real tokens, API keys, `auth.json` contents, `store.json`, or user-specific `CODEX_HOME` paths unless the user explicitly asks.
- Treat account data as sensitive. If adding logs, avoid secrets and truncate any identifiers that could reveal credentials.
- For Windows process or terminal behavior, check `src-tauri/src/codex.rs` before changing launch semantics.
- When changing storage behavior, preserve backward compatibility for existing `store.json` files where practical.
- Use structured parsing for JSON/TOML instead of ad hoc string editing.

## Data Locations

Runtime data is stored outside the repository:

- App data directory: `%LOCALAPPDATA%\AI Account Tool`
- Main store file: `%LOCALAPPDATA%\AI Account Tool\store.json`
- API relay channel file: `%LOCALAPPDATA%\AI Account Tool\api-channels.json`
- Default Codex home: `CODEX_HOME` if set, otherwise `%USERPROFILE%\.codex`
- Generated instance homes: `%LOCALAPPDATA%\AI Account Tool\codex-instances`

Do not add these generated files to the repository.

## Verification Checklist

For documentation-only changes, a diff review is usually enough.

For frontend changes:

- Run `npm run build`.
- Open the app with `npm run tauri dev` when UI behavior changes.
- Check that text does not overflow at the configured minimum window size.

For Rust/backend changes:

- Run `cargo fmt --manifest-path src-tauri/Cargo.toml`.
- Run `cargo check --manifest-path src-tauri/Cargo.toml`.
- If changing Tauri command payloads, update frontend types and call sites in the same change.
