# Roadmap

Work proceeds from the local permission boundary outward. Status describes the current first milestone; proposed provider integrations do not imply that authentication or live inference is available.

## A. Core runtime — foundation present

- Permission modes and conservative decisions.
- SQLite sessions and message transcripts, with no credential tables and a v0-to-v1 schema migration.
- Deterministic offline provider with bounded events and cancellation.

Schema migrations, redaction, and broader integration coverage remain future work.

## B. Workspace and desktop shell — source present, build pending

- File listing, literal text search, text read/write of existing workspace files, and Git status/diff are wired through Tauri commands. Saves detect stale editor content.
- React/Vite shell and CLI diagnostics have initial source implementations.
- Resolve dependencies, create a frontend lockfile, run Rust/React tests and native build, then perform manual end-to-end checks.
- Add native packaging and cross-platform checks after the foundation passes.

The browser surface is a preview. It does not provide native filesystem access and does not represent a live model connection.

## C. Provider boundary and authentication — not started

1. Define separate adapter contracts for raw model APIs and managed agent engines, with shared event normalization and redaction.
2. Add OS-backed credential storage with an explicit unavailable-storage failure.
3. Evaluate a pinned Codex app-server integration through its documented account APIs.
4. Add an Ollama adapter with explicit endpoint and remote-network policy.
5. Add an Anthropic API adapter using credentials owned by the user or supported cloud provider.
6. Re-check provider terms and eligibility before considering any local Claude Code or OpenAI preview flow.

No third-party ChatGPT subscription OAuth, provider API key, or live model is supported by the current milestone. See [Authentication](authentication.md).

## D. Agent loop and approvals — planned

Add durable streamed events, explicit allow/ask/deny decisions, cancellation and restart recovery, bounded process execution, context budgeting, and deterministic replay fixtures.

## E. Product hardening and extensions — planned

Add accessibility and keyboard navigation, signed platform packaging, migration recovery, redacted diagnostics, performance checks, and a versioned extension manifest whose capabilities use the same permission engine. Isolation and update rules require a separate threat model.
