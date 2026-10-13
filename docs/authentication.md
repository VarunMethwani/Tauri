# Authentication and credentials

The first milestone has no provider authentication and no live model provider. It uses a deterministic offline provider. SQLite stores sessions and message transcripts only; it is not a credential store.

The links below are official provider documentation. Eligibility, product terms, preview programs, and supported flows can change, so they must be checked again before implementation. This document does not claim that a third-party application may use a user's ChatGPT subscription through OAuth.

## OpenAI and Codex

OpenAI's [Codex authentication documentation][openai-auth] describes Sign in with ChatGPT and API-key authentication for Codex products. OpenAI's [Codex app-server auth endpoints][app-server] document account inspection and login modes including `apiKey`, `chatgpt`, and `chatgptDeviceCode`.

Those pages support a future investigation of a pinned, local Codex app-server integration. They do not make Agent Harness an approved Codex client, grant an entitlement, or establish that a third-party app may offer ChatGPT subscription OAuth. Any implementation would need current OpenAI eligibility, account, distribution, token-handling, and terms requirements verified first.

OpenAI also publishes separate [Sign in with ChatGPT token-sharing documentation][siwc-overview], including [registration and sign-in][siwc-signin], [Codex app-server guidance][siwc-app-server], and [preview limitations][siwc-limits]. These pages describe a specific preview with its own eligibility and restrictions. The repository does not request a client ID, define a redirect URI, handle ChatGPT tokens, or claim access to that preview.

The proposed first OpenAI spike is therefore only a design direction: use the documented app-server account boundary, pin the version, keep approvals and tools in the host, and store no provider material in SQLite. It is not implemented or promised support.

## Anthropic and Claude

Anthropic's [Claude Agent SDK overview][agent-sdk] and [quickstart][agent-quickstart] describe embedding the Claude Code agent loop with an Anthropic API key or supported cloud-provider credentials. Anthropic's [Claude Code legal and compliance guidance][claude-legal] must be reviewed before integrating the Claude Code client or any subscription-backed flow. The [Claude API overview][anthropic-api] is the reference for a separate API adapter.

The project currently collects no Claude credentials and makes no claim that a Claude subscription supplies general API access. A future adapter must use a credential and product path that Anthropic's current documentation permits; local Claude Code authentication, if evaluated, must remain inside the supported client flow.

## Ollama

No Ollama adapter exists. The planned policy is an explicit design assumption: default to a local endpoint, require a deliberate choice before using a remote endpoint, and disclose when workspace content will be sent. The [Ollama API documentation][ollama-api] is the source to consult when this adapter is implemented.

## Credential boundary

The following are planned design constraints, not implemented behavior:

| Concern | Current state | Planned constraint |
| --- | --- | --- |
| Provider credentials | None; offline provider only | Keep named secrets in an OS credential service where the provider path requires them. |
| SQLite | Sessions and messages only | Never add API keys, access/refresh tokens, cookies, or provider session files. |
| Child processes | No authenticated child process | Pass only the credential needed by the selected provider and redact diagnostics. |
| OAuth state | Not implemented | Follow the selected provider's documented flow; generate and bind request state in the credential component. |
| Logs and UI | No live secret values | Redact headers, environment values, query parameters, token-shaped strings, and raw provider errors. |
| Logout/export | Not implemented | Clear provider material through the provider-supported path and exclude secrets from exports by default. |

If secure OS storage is unavailable, the future provider setup should fail clearly rather than silently falling back to SQLite or plaintext. This is a security design requirement for a later milestone, not a capability of the current code.

[openai-auth]: https://developers.openai.com/codex/auth
[app-server]: https://developers.openai.com/codex/app-server#auth-endpoints
[siwc-overview]: https://developers.openai.com/siwc/token-sharing-open-source
[siwc-signin]: https://developers.openai.com/siwc/token-sharing-open-source/sign-in
[siwc-app-server]: https://developers.openai.com/siwc/token-sharing-open-source/codex-app-server
[siwc-limits]: https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations
[agent-sdk]: https://code.claude.com/docs/en/agent-sdk/overview
[agent-quickstart]: https://code.claude.com/docs/en/agent-sdk/quickstart
[claude-legal]: https://code.claude.com/docs/en/legal-and-compliance
[anthropic-api]: https://platform.claude.com/docs/en/api/overview
[ollama-api]: https://docs.ollama.com/api
