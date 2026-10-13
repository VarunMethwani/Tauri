use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

/// Controls the default permission response for workspace mutations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    Plan,
    Ask,
    AutoEdit,
    Autonomous,
}

impl PermissionMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Plan => "plan",
            Self::Ask => "ask",
            Self::AutoEdit => "auto_edit",
            Self::Autonomous => "autonomous",
        }
    }
}

impl fmt::Display for PermissionMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for PermissionMode {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "plan" => Ok(Self::Plan),
            "ask" => Ok(Self::Ask),
            "auto_edit" => Ok(Self::AutoEdit),
            "autonomous" => Ok(Self::Autonomous),
            other => Err(other.to_owned()),
        }
    }
}

/// Events emitted by a provider-backed turn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    TextDelta { text: String },
    ToolStarted { name: String },
    ToolFinished { name: String, output: String },
    PermissionRequired { operation: String },
    Finished,
    Cancelled,
    Error { message: String },
}

impl AgentEvent {
    /// Returns whether this event ends a turn.
    ///
    /// Providers should only emit progress events. The runtime owns terminal
    /// events so callers can reliably stop consuming a stream at the first
    /// terminal event.
    pub const fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Finished | Self::Cancelled | Self::Error { .. }
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub workspace: String,
    pub mode: PermissionMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipc_shapes_are_stable() {
        assert_eq!(
            serde_json::to_string(&PermissionMode::AutoEdit).unwrap(),
            r#""auto_edit""#
        );
        assert_eq!(
            serde_json::to_value(AgentEvent::TextDelta {
                text: "hello".into()
            })
            .unwrap(),
            serde_json::json!({"type": "text_delta", "text": "hello"})
        );
        assert_eq!(
            serde_json::to_value(AgentEvent::Finished).unwrap(),
            serde_json::json!({"type": "finished"})
        );
    }

    #[test]
    fn terminal_event_classification_is_explicit() {
        assert!(!AgentEvent::TextDelta { text: "hello".into() }.is_terminal());
        assert!(!AgentEvent::ToolStarted { name: "read".into() }.is_terminal());
        assert!(AgentEvent::Finished.is_terminal());
        assert!(AgentEvent::Cancelled.is_terminal());
        assert!(AgentEvent::Error { message: "failed".into() }.is_terminal());
    }
}

