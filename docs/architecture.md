# Architecture

Agent Harness is local-first and provider-neutral. The first milestone has source code for permission evaluation, SQLite session/message persistence, a deterministic offline provider, workspace file/Git tools, and a React/Vite/Tauri shell. Native integration still needs build and runtime validation.

## Runtime shape

```text
React/Vite shell (Tauri or browser preview)
                    |
             native command boundary
                    |
             Rust runtime / sessions
              +-----+----------+
              |                |
        provider adapter   permission engine
        (offline today)          |
              |            workspace tools
              |             (filesystem / Git)
              |                |
              +---------- local workspace

             Rust runtime ---- SQLite
                       (sessions / messages)
```

The browser preview demonstrates UI state and does not grant a web page access to the host filesystem. The Tauri IPC commands are wired in source, pending native build verification.

## Boundaries

1. **Provider to runtime.** Providers emit typed events through a bounded channel. The current offline provider emits text only. Typed tool proposals are planned for the agent loop; providers never receive raw filesystem, shell, Git, database, or Tauri handles.
2. **Runtime to tools.** The permission engine runs before an effect. The tool implementation independently normalizes paths and rejects a path outside the selected workspace.
3. **Workspace to Git.** The current Git operations run bounded `git status` and `git diff` in the selected workspace and return text. General shell execution and a PTY are future work.
4. **Runtime to SQLite.** SQLite stores session metadata and message transcripts. It has no credential tables and must not receive API keys, access tokens, refresh tokens, cookies, or provider session material.
5. **Credential boundary.** Live providers may use an OS-backed credential service in a later milestone. No credential integration exists today.

## Current and deferred work

The offline provider is deterministic and local so permission, persistence, event, and cancellation behavior can be tested without accounts, network access, or model output. A live adapter must preserve the same host-owned tool boundary and add provider-specific error and streaming handling.

Raw model APIs and managed agent engines need distinct adapters. Codex app-server and Claude Code can run their own tool loops; they require explicit mapping of their approval and sandbox controls. See [Provider boundaries](provider-boundaries.md).

Workspace tools and command wiring are implemented in source but unverified by a native build here. Live provider authentication, agent-loop recovery, terminal execution, extensions, packaging, and updates are deferred until those boundaries have integration tests. See [Roadmap](roadmap.md) and [Authentication](authentication.md).
