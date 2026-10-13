use crate::PermissionMode;
use serde::{Deserialize, Serialize};

/// A typed operation category evaluated before side effects are performed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    Read,
    Search,
    WorkspaceWrite,
    OutsideWorkspaceWrite,
    Shell,
    Network,
    Commit,
    Delete,
    Secrets,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionDecision {
    Allow,
    Ask,
    Deny,
}

/// Evaluates the conservative built-in policy for one permission mode.
#[derive(Debug, Clone, Copy)]
pub struct PermissionEvaluator {
    mode: PermissionMode,
}

impl PermissionEvaluator {
    pub const fn new(mode: PermissionMode) -> Self {
        Self { mode }
    }

    pub const fn mode(self) -> PermissionMode {
        self.mode
    }

    pub const fn evaluate(self, operation: Operation) -> PermissionDecision {
        use Operation::*;
        use PermissionDecision::*;

        match operation {
            Read | Search => Allow,
            Secrets => Deny,
            OutsideWorkspaceWrite | Shell | Commit | Delete if matches!(self.mode, PermissionMode::Plan) => Deny,
            OutsideWorkspaceWrite | Shell | Network | Commit | Delete => Ask,
            WorkspaceWrite => match self.mode {
                PermissionMode::Plan => Deny,
                PermissionMode::Ask => Ask,
                PermissionMode::AutoEdit | PermissionMode::Autonomous => Allow,
            },
        }
    }
}

pub const fn evaluate_permission(
    mode: PermissionMode,
    operation: Operation,
) -> PermissionDecision {
    PermissionEvaluator::new(mode).evaluate(operation)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_matrix_covers_every_mode_and_operation() {
        use Operation::*;
        use PermissionDecision::*;

        let modes = [
            PermissionMode::Plan,
            PermissionMode::Ask,
            PermissionMode::AutoEdit,
            PermissionMode::Autonomous,
        ];

        for mode in modes {
            let evaluator = PermissionEvaluator::new(mode);
            assert_eq!(evaluator.evaluate(Read), Allow);
            assert_eq!(evaluator.evaluate(Search), Allow);
            assert_eq!(evaluator.evaluate(Secrets), Deny);

            assert_eq!(evaluator.evaluate(Network), Ask);
            for guarded in [OutsideWorkspaceWrite, Shell, Commit, Delete] {
                let expected = if mode == PermissionMode::Plan { Deny } else { Ask };
                assert_eq!(evaluator.evaluate(guarded), expected, "{mode}: {guarded:?}");
            }
        }

        assert_eq!(evaluate_permission(PermissionMode::Plan, WorkspaceWrite), Deny);
        assert_eq!(evaluate_permission(PermissionMode::Ask, WorkspaceWrite), Ask);
        assert_eq!(
            evaluate_permission(PermissionMode::AutoEdit, WorkspaceWrite),
            Allow
        );
        assert_eq!(
            evaluate_permission(PermissionMode::Autonomous, WorkspaceWrite),
            Allow
        );
    }
}

