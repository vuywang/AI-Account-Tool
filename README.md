# AI Account Tool

AI Account Tool is a Windows-focused desktop helper for managing multiple Codex accounts and launching isolated Codex CLI instances. It is built with Tauri 2, React, TypeScript, and Rust.

## Features

- Import an existing Codex account from `auth.json`.
- Start a Codex login flow and import the account after login completes.
- Add API key accounts manually.
- Add OAuth token accounts manually.
- Switch the active Codex account by writing credentials into a selected `CODEX_HOME`.
- Create multiple Codex instances with separate `CODEX_HOME` directories.
- Bind instances to specific accounts and launch them in Windows Terminal or `cmd.exe`.
- Save API relay channels with Base URL and API Key, then switch Codex to them with one click.
- Refresh and display OAuth quota information for hourly, weekly, and code review limits.

## Tech Stack

- Tauri 2 for the desktop shell and native commands.
- React 19 and TypeScript for the UI.
- Vite for frontend development and production builds.
- Rust for account storage, Codex CLI integration, quota fetching, and process launch behavior.

## Requirements

- Windows.
- Node.js and npm.
- Rust toolchain.
- Codex CLI available as `codex.cmd` or `codex.exe`.
- Windows Terminal is optional; the app falls back to `cmd.exe`.

## Getting Started

Install dependencies:

```powershell
npm install
```

Run the frontend dev server:

```powershell
npm run dev
```

Run the Tauri app in development:

```powershell
npm run tauri dev
```

Build the frontend:

```powershell
npm run build
```

Build the desktop app:

```powershell
npm run tauri build
```

## Useful Scripts

- `npm run dev` - Start Vite on `127.0.0.1:1420`.
- `npm run build` - Type-check and build the frontend.
- `npm run preview` - Preview the built frontend.
- `npm run tauri` - Run the Tauri CLI.

Backend checks:

```powershell
cargo check --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml
```

## Project Structure

```text
.
|-- public/                 Static frontend assets
|-- src/                    React frontend
|   |-- App.tsx             Main application UI
|   |-- App.css             UI styles
|   `-- main.tsx            React entrypoint
|-- src-tauri/              Tauri and Rust backend
|   |-- src/
|   |   |-- codex.rs        Codex CLI, auth, and launch helpers
|   |   |-- commands.rs     Tauri command handlers
|   |   |-- lib.rs          Tauri command registration
|   |   |-- models.rs       Shared backend models
|   |   |-- quota.rs        Usage quota fetching
|   |   `-- storage.rs      Store and path utilities
|   |-- capabilities/       Tauri permissions
|   |-- icons/              App icons
|   |-- Cargo.toml
|   `-- tauri.conf.json
|-- package.json
|-- tsconfig.json
`-- vite.config.ts
```

## Runtime Data

The app stores account metadata and tokens outside the repository:

- App data directory: `%LOCALAPPDATA%\AI Account Tool`
- Store file: `%LOCALAPPDATA%\AI Account Tool\store.json`
- API relay channel file: `%LOCALAPPDATA%\AI Account Tool\api-channels.json`
- Default Codex home: `CODEX_HOME` if set, otherwise `%USERPROFILE%\.codex`
- Generated instance homes: `%LOCALAPPDATA%\AI Account Tool\codex-instances`

Account tokens and API keys are sensitive. Do not share `store.json`, `auth.json`, or generated `CODEX_HOME` directories.

## Development Notes

- Frontend calls into Rust through `@tauri-apps/api/core` `invoke(...)`.
- New backend commands must be registered in `src-tauri/src/lib.rs`.
- Keep frontend TypeScript types in `src/App.tsx` aligned with Rust structs in `src-tauri/src/models.rs`.
- Account switching writes `auth.json` and may update `config.toml` in the target `CODEX_HOME`.
- API relay switching writes an API key to `auth.json`, sets `model_provider = "openai"`, and manages `openai_base_url` in `config.toml`.
- Quota refresh works for OAuth accounts; API key usage should be checked through the OpenAI dashboard.
