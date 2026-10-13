# harness-core

`harness-core` is the provider-neutral foundation of the Agent Harness. It
contains:

- a bounded asynchronous event stream and cooperative cancellation;
- a `Provider` trait for client-supplied model integrations;
- a deterministic offline demo provider that performs no network or model
  calls and never presents its output as AI-generated;
- conservative typed permission defaults for plan, ask, auto-edit, and
  autonomous modes; and
- local SQLite persistence for sessions and messages.

The SQLite schema intentionally has no credential or provider-auth tables.
Applications should integrate with the operating system credential store and
pass short-lived provider access to their own `Provider` implementation.

```rust,no_run
use harness_core::{start_offline_turn, AgentEvent, PermissionMode};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let (mut events, cancellation) =
        start_offline_turn("Explain the local scaffold".into(), PermissionMode::Ask);

    while let Some(event) = events.recv().await {
        println!("{event:?}");
        if matches!(event, AgentEvent::Finished | AgentEvent::Cancelled) {
            break;
        }
    }

    // Safe to call more than once, including after the turn has ended.
    cancellation.cancel();
}
```
