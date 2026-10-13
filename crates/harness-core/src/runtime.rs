use crate::{AgentEvent, PermissionMode};
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    time::Duration,
};
use thiserror::Error;
use tokio::sync::{mpsc, watch};

pub const DEFAULT_EVENT_CAPACITY: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnRequest {
    pub prompt: String,
    pub mode: PermissionMode,
}

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("{0}")]
    Message(String),
}

pub type ProviderFuture =
    Pin<Box<dyn Future<Output = std::result::Result<(), ProviderError>> + Send + 'static>>;

/// The provider-neutral streaming boundary.
///
/// Implementations emit non-terminal events through `events`. [`AgentRuntime`]
/// owns terminal `Finished`, `Cancelled`, and `Error` events so every provider
/// has the same lifecycle semantics.
pub trait Provider: Send + Sync + 'static {
    fn stream(
        &self,
        request: TurnRequest,
        events: mpsc::Sender<AgentEvent>,
        cancellation: CancellationHandle,
    ) -> ProviderFuture;
}

/// Cloneable cooperative cancellation handle for a running turn.
#[derive(Debug, Clone)]
pub struct CancellationHandle {
    cancelled: watch::Sender<bool>,
}

impl Default for CancellationHandle {
    fn default() -> Self {
        let (cancelled, _receiver) = watch::channel(false);
        Self { cancelled }
    }
}

impl CancellationHandle {
    pub fn cancel(&self) {
        self.cancelled.send_replace(true);
    }

    pub fn is_cancelled(&self) -> bool {
        *self.cancelled.borrow()
    }

    /// Resolves after cancellation is requested. It is safe for multiple
    /// provider tasks to await this concurrently.
    pub async fn cancelled(&self) {
        let mut cancellation = self.cancelled.subscribe();
        if *cancellation.borrow() {
            return;
        }
        while cancellation.changed().await.is_ok() {
            if *cancellation.borrow() {
                return;
            }
        }
    }
}

/// Runs one provider and exposes its events through a bounded channel.
#[derive(Clone)]
pub struct AgentRuntime {
    provider: Arc<dyn Provider>,
    event_capacity: usize,
}

impl AgentRuntime {
    pub fn new(provider: impl Provider) -> Self {
        Self {
            provider: Arc::new(provider),
            event_capacity: DEFAULT_EVENT_CAPACITY,
        }
    }

    pub fn with_event_capacity(provider: impl Provider, event_capacity: usize) -> Self {
        Self {
            provider: Arc::new(provider),
            event_capacity: event_capacity.max(1),
        }
    }

    /// Returns the maximum number of events buffered for each turn.
    pub const fn event_capacity(&self) -> usize {
        self.event_capacity
    }

    pub fn start_turn(
        &self,
        prompt: String,
        mode: PermissionMode,
    ) -> (mpsc::Receiver<AgentEvent>, CancellationHandle) {
        let (events, receiver) = mpsc::channel(self.event_capacity);
        let cancellation = CancellationHandle::default();
        let task_cancellation = cancellation.clone();
        let provider = Arc::clone(&self.provider);

        tokio::spawn(async move {
            let request = TurnRequest { prompt, mode };
            let provider_future = provider.stream(
                request,
                events.clone(),
                task_cancellation.clone(),
            );

            let terminal = tokio::select! {
                _ = task_cancellation.cancelled() => AgentEvent::Cancelled,
                // Dropping the receiver is a normal way for a caller to stop
                // observing a turn. Stop polling the provider in that case so
                // a provider cannot outlive the stream it feeds.
                _ = events.closed() => {
                    task_cancellation.cancel();
                    return;
                },
                result = provider_future => {
                    if task_cancellation.is_cancelled() {
                        AgentEvent::Cancelled
                    } else {
                        match result {
                            Ok(()) => AgentEvent::Finished,
                            Err(error) => AgentEvent::Error { message: error.to_string() },
                        }
                    }
                }
            };
            let _ = events.send(terminal).await;
        });

        (receiver, cancellation)
    }
}

/// A deterministic, local-only provider used to exercise the runtime.
#[derive(Debug, Clone, Copy, Default)]
pub struct OfflineProvider;

impl Provider for OfflineProvider {
    fn stream(
        &self,
        request: TurnRequest,
        events: mpsc::Sender<AgentEvent>,
        cancellation: CancellationHandle,
    ) -> ProviderFuture {
        Box::pin(async move {
            let prompt = prompt_excerpt(&request.prompt);
            let fragments = [
                "Offline demo mode: no model or network provider is connected.\n".to_owned(),
                format!("Received this prompt locally: \"{prompt}\"\n"),
                format!("Permission mode: {}.\n", request.mode),
                "Implement the Provider trait in the client to connect a real model; session storage, bounded streaming, permissions, and cancellation are ready to use."
                    .to_owned(),
            ];

            for text in fragments {
                tokio::select! {
                    _ = cancellation.cancelled() => return Ok(()),
                    _ = tokio::time::sleep(Duration::from_millis(10)) => {}
                }
                tokio::select! {
                    _ = cancellation.cancelled() => return Ok(()),
                    result = events.send(AgentEvent::TextDelta { text }) => {
                        if result.is_err() {
                            return Ok(());
                        }
                    }
                }
            }
            Ok(())
        })
    }
}

/// Starts a turn using the deterministic local-only demo provider.
pub fn start_offline_turn(
    prompt: String,
    mode: PermissionMode,
) -> (mpsc::Receiver<AgentEvent>, CancellationHandle) {
    AgentRuntime::new(OfflineProvider).start_turn(prompt, mode)
}

fn prompt_excerpt(prompt: &str) -> String {
    const LIMIT: usize = 240;
    let mut excerpt: String = prompt
        .trim()
        .chars()
        .take(LIMIT)
        .flat_map(char::escape_default)
        .collect();
    if prompt.trim().chars().count() > LIMIT {
        excerpt.push('…');
    }
    excerpt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn offline_stream_is_honest_and_finishes() {
        let (mut events, _cancellation) =
            start_offline_turn("make a useful scaffold".into(), PermissionMode::Plan);
        let mut transcript = String::new();
        let mut terminal = None;

        while let Some(event) = events.recv().await {
            match event {
                AgentEvent::TextDelta { text } => transcript.push_str(&text),
                event @ (AgentEvent::Finished
                | AgentEvent::Cancelled
                | AgentEvent::Error { .. }) => terminal = Some(event),
                event => panic!("offline provider emitted a fake action: {event:?}"),
            }
        }

        assert!(transcript.contains("Offline demo mode"));
        assert!(transcript.contains("no model or network provider is connected"));
        assert!(transcript.contains("make a useful scaffold"));
        assert_eq!(terminal, Some(AgentEvent::Finished));
    }

    #[tokio::test]
    async fn cancellation_ends_stream_with_cancelled_event() {
        let runtime = AgentRuntime::with_event_capacity(OfflineProvider, 1);
        let (mut events, cancellation) =
            runtime.start_turn("a turn that will be cancelled".into(), PermissionMode::Ask);

        assert!(matches!(
            events.recv().await,
            Some(AgentEvent::TextDelta { .. })
        ));
        cancellation.cancel();

        let mut saw_cancelled = false;
        while let Some(event) = events.recv().await {
            match event {
                AgentEvent::Cancelled => saw_cancelled = true,
                AgentEvent::Finished => panic!("cancelled turn finished normally"),
                _ => {}
            }
        }
        assert!(saw_cancelled);
        assert!(cancellation.is_cancelled());
    }

    #[derive(Debug, Clone)]
    struct BlockingProvider {
        stopped: std::sync::Arc<std::sync::atomic::AtomicBool>,
    }

    impl Provider for BlockingProvider {
        fn stream(
            &self,
            _request: TurnRequest,
            _events: mpsc::Sender<AgentEvent>,
            _cancellation: CancellationHandle,
        ) -> ProviderFuture {
            struct PendingProvider(std::sync::Arc<std::sync::atomic::AtomicBool>);

            impl Future for PendingProvider {
                fn poll(
                    self: Pin<&mut Self>,
                    _context: &mut std::task::Context<'_>,
                ) -> std::task::Poll<std::result::Result<(), ProviderError>> {
                    std::task::Poll::Pending
                }
            }

            impl Drop for PendingProvider {
                fn drop(&mut self) {
                    self.0.store(true, std::sync::atomic::Ordering::SeqCst);
                }
            }

            Box::pin(PendingProvider(std::sync::Arc::clone(&self.stopped)))
        }
    }

    #[tokio::test]
    async fn dropping_receiver_stops_a_blocked_provider() {
        let stopped = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let runtime = AgentRuntime::new(BlockingProvider {
            stopped: std::sync::Arc::clone(&stopped),
        });
        let (events, _cancellation) = runtime.start_turn("drop me".into(), PermissionMode::Plan);
        drop(events);

        // The runtime task should observe the closed channel and return. Wait
        // for the provider's drop signal without depending on one scheduler
        // tick or a fixed sleep duration.
        tokio::time::timeout(Duration::from_secs(1), async {
            while !stopped.load(std::sync::atomic::Ordering::SeqCst) {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .expect("runtime did not stop the provider after receiver drop");
    }
}

