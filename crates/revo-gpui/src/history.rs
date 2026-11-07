//! Workspace-side undo/redo history (Svelte parity: the `recordHistory`
//! closure stack). Entries carry the inverse and forward backend command
//! lists computed at dispatch time from the live snapshot, so undo/redo
//! re-dispatch typed commands instead of relying on engine-side state.

use revo_core::service::BackendCommand;

/// Bounded history: the Svelte shell kept 30 entries.
pub const HISTORY_LIMIT: usize = 30;

#[derive(Clone, Debug, PartialEq)]
pub struct HistoryEntry {
    pub label: String,
    pub undo: Vec<BackendCommand>,
    pub redo: Vec<BackendCommand>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct HistoryLog {
    pub undo_stack: Vec<HistoryEntry>,
    pub redo_stack: Vec<HistoryEntry>,
}

impl HistoryLog {
    /// Records a completed change; a new change drops the redo branch.
    pub fn record(&mut self, entry: HistoryEntry) {
        self.undo_stack.push(entry);
        if self.undo_stack.len() > HISTORY_LIMIT {
            let excess = self.undo_stack.len() - HISTORY_LIMIT;
            self.undo_stack.drain(0..excess);
        }
        self.redo_stack.clear();
    }

    /// Pops the newest undo entry and returns its inverse commands; the
    /// entry moves to the redo stack.
    pub fn undo(&mut self) -> Option<(String, Vec<BackendCommand>)> {
        let entry = self.undo_stack.pop()?;
        self.redo_stack.push(entry.clone());
        Some((entry.label, entry.undo))
    }

    /// Pops the newest redo entry and returns its forward commands; the
    /// entry moves back onto the undo stack.
    pub fn redo(&mut self) -> Option<(String, Vec<BackendCommand>)> {
        let entry = self.redo_stack.pop()?;
        self.undo_stack.push(entry.clone());
        Some((entry.label, entry.redo))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use revo_core::service::MoveDirection;

    fn entry(label: &str, undo: Vec<BackendCommand>, redo: Vec<BackendCommand>) -> HistoryEntry {
        HistoryEntry {
            label: label.into(),
            undo,
            redo,
        }
    }

    #[test]
    fn record_pushes_and_clears_the_redo_branch() {
        let mut log = HistoryLog::default();
        log.record(entry(
            "add scene",
            vec![BackendCommand::RemoveScene("s".into())],
            vec![BackendCommand::AddScene("s".into())],
        ));
        log.record(entry(
            "rename",
            vec![BackendCommand::RenameScene {
                from: "b".into(),
                to: "a".into(),
            }],
            vec![BackendCommand::RenameScene {
                from: "a".into(),
                to: "b".into(),
            }],
        ));
        assert_eq!(log.undo_stack.len(), 2);
        assert!(log.redo_stack.is_empty());
        let (label, commands) = log.undo().expect("undo pops");
        assert_eq!(label, "rename");
        assert_eq!(
            commands,
            vec![BackendCommand::RenameScene {
                from: "b".into(),
                to: "a".into()
            }]
        );
        // Re-recording drops the redo branch.
        log.record(entry("move", vec![], vec![]));
        assert!(log.redo_stack.is_empty());
        assert_eq!(log.undo_stack.len(), 2);
    }

    #[test]
    fn undo_redo_round_trip_restores_the_forward_commands() {
        let mut log = HistoryLog::default();
        log.record(entry(
            "move source",
            vec![BackendCommand::MoveSource {
                id: "cam".into(),
                direction: MoveDirection::Down,
            }],
            vec![BackendCommand::MoveSource {
                id: "cam".into(),
                direction: MoveDirection::Up,
            }],
        ));
        log.undo().expect("undo");
        let (label, commands) = log.redo().expect("redo");
        assert_eq!(label, "move source");
        assert_eq!(
            commands,
            vec![BackendCommand::MoveSource {
                id: "cam".into(),
                direction: MoveDirection::Up
            }]
        );
        assert_eq!(log.undo_stack.len(), 1);
        assert!(log.redo_stack.is_empty());
    }

    #[test]
    fn the_stack_is_bounded() {
        let mut log = HistoryLog::default();
        for index in 0..(HISTORY_LIMIT + 10) {
            log.record(entry(&format!("e{index}"), vec![], vec![]));
        }
        assert_eq!(log.undo_stack.len(), HISTORY_LIMIT);
        assert_eq!(
            log.undo_stack.last().unwrap().label,
            format!("e{}", HISTORY_LIMIT + 9)
        );
    }

    #[test]
    fn empty_stacks_report_nothing() {
        let mut log = HistoryLog::default();
        assert!(log.undo().is_none());
        assert!(log.redo().is_none());
    }
}
