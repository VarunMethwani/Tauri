# Agent Harness

Agent Harness is a local-first Rust and Tauri foundation for a coding agent. The first milestone contains a provider-neutral event stream, conservative permission rules, SQLite sessions, workspace file/Git operations, a CLI, and a React desktop shell. The only provider is a deterministic offline demo; live model authentication and inference are future work.

## What is implemented

- The Rust core defines a provider interface, bounded event stream, cancellation, and plan/ask/auto edit/autonomous permissions.
- SQLite saves sessions and message transcripts. It does not store provider credentials.
- Workspace tools list files, search literal text with ripgrep, read/write existing text files inside the selected folder, and show Git status and diff. Saves reject stale editor content. The native Tauri commands call these tools.
- The UI opens a folder in Tauri, shows files and Git changes, edits text files with approval in Ask mode, and resumes sessions. A browser preview uses a small in-memory demo workspace and never reads local files.
- `harness-cli doctor` reports local prerequisites; `harness-cli inspect <path>` lists a workspace.

The source is a reviewable foundation. Native and frontend builds remain unverified because this environment lacks Rust, Linux WebKit development libraries, and working package registry access. See [Verification](docs/verification.md).

## Run it

Install the [Tauri 2 platform prerequisites](https://v2.tauri.app/start/prerequisites/), a stable Rust toolchain, and Node.js 20.19 or newer. From the repository root, resolve frontend dependencies and create a lockfile:

```bash
npm --prefix desktop install
```

Once the lockfile exists, use `npm --prefix desktop ci` for reproducible installs. Run the browser preview with `npm --prefix desktop run dev` or the native desktop app with `npm --prefix desktop run tauri dev`.

Run the Rust foundation tests and CLI with:

```bash
cargo test --workspace --exclude harness-desktop
cargo run -p harness-cli -- doctor
cargo run -p harness-cli -- inspect /path/to/workspace
cargo run -p harness-cli -- search /path/to/workspace 'literal phrase'
```

Run `npm --prefix desktop run build` to type check and build the frontend. Run `scripts/verify.sh` for the clean dependency install, audits, frontend build, Rust tests, and native compile check. This initial GitHub bootstrap snapshot is unverified; the dependency safety gate in [Verification](docs/verification.md) must pass before treating the dependencies as validated or shipping the app.

## Next work

The next milestone is a verified native build, a checked-in frontend lockfile, and end-to-end checks for workspace, approval, session, and event flows. Provider authentication, real model adapters, tool requests, shell sandboxing, MCP, skills, and plugins follow the boundaries in [Architecture](docs/architecture.md) and the sequence in [Roadmap](docs/roadmap.md). The distinction between model APIs and managed agent engines is in [Provider boundaries](docs/provider-boundaries.md); current credential constraints are in [Authentication](docs/authentication.md).
