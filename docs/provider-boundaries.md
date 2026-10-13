# Provider and agent-engine boundaries

The offline provider in `harness-core` is a minimal event-streaming contract. Live integrations need two kinds of adapter:

| Adapter | Examples | Who runs the tool loop? | Host responsibility |
| --- | --- | --- | --- |
| Model provider | Anthropic API, Ollama, OpenAI-compatible API | Agent Harness | Validate model tool proposals, route typed tools through permissions, record events and diffs. |
| Managed agent engine | Codex app-server, Claude Code/Agent SDK | The external engine | Set the engine's own sandbox/approval policy, handle approval requests, normalize events, and observe workspace changes. |

A managed agent engine is not interchangeable with a raw model request at the tool-execution boundary. Calling Codex app-server a `ModelProvider` while letting it edit files would bypass Harness's current `harness-tools` checks. The engine adapter must either delegate all edits to host-owned tools or explicitly map and enforce the engine's approval/sandbox controls. A live adapter must not be enabled before that contract is tested.

## Local Codex protocol inspection

The installed `codex-cli 0.159.2` can generate its app-server JSON schemas with `codex app-server generate-json-schema --out <directory>`. The generated v2 schemas available in this environment include:

- `initialize` with required `clientInfo`;
- account read/login, model list, thread start, and turn start requests;
- agent message delta and turn completed notifications;
- server-to-client approval requests for command execution and file changes.

This is local protocol inspection, not an authentication entitlement or a production integration. The next Codex adapter should pin and check a supported CLI version, launch a local stdio child process, initialize it, inspect the account without reading token files, discover models, and translate notifications into `AgentEvent`. Approval requests need an explicit host decision and must default to denial when the UI is absent or a request is unknown. Cancellation, child restart, path scope, and diff reconciliation need integration tests against the pinned protocol.

The same distinction applies to Claude Code: it can run its own agent loop. A separate Anthropic API adapter can instead use the Harness-owned loop. See [Authentication](authentication.md) for credential and product constraints.
