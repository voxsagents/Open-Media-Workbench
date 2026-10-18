//! Undo and redo editing history manager for dubbing projects.

use std::collections::VecDeque;

use super::models::DubbingProject;

/// Maximum number of undo states retained in memory.
pub const DUBBING_MAX_HISTORY_SNAPSHOTS: usize = 50;

/// In-memory undo/redo stack managing project state snapshots.
#[derive(Debug, Clone, Default)]
pub struct DubbingHistory {
    undo_stack: VecDeque<DubbingProject>,
    redo_stack: VecDeque<DubbingProject>,
}

impl DubbingHistory {
    pub fn new() -> Self {
        Self {
            undo_stack: VecDeque::with_capacity(DUBBING_MAX_HISTORY_SNAPSHOTS),
            redo_stack: VecDeque::new(),
        }
    }

    /// Records a new snapshot before an operation is executed, clearing redo history.
    pub fn record_snapshot(&mut self, project: &DubbingProject) {
        if self.undo_stack.len() >= DUBBING_MAX_HISTORY_SNAPSHOTS {
            self.undo_stack.pop_front();
        }
        self.undo_stack.push_back(project.clone());
        self.redo_stack.clear();
    }

    /// Reverts to the previous snapshot, placing the current state onto the redo stack.
    pub fn undo(&mut self, current: DubbingProject) -> Option<DubbingProject> {
        let previous = self.undo_stack.pop_back()?;
        if self.redo_stack.len() >= DUBBING_MAX_HISTORY_SNAPSHOTS {
            self.redo_stack.pop_front();
        }
        self.redo_stack.push_back(current);
        Some(previous)
    }

    /// Re-applies the next undone snapshot, placing the current state onto the undo stack.
    pub fn redo(&mut self, current: DubbingProject) -> Option<DubbingProject> {
        let next = self.redo_stack.pop_back()?;
        if self.undo_stack.len() >= DUBBING_MAX_HISTORY_SNAPSHOTS {
            self.undo_stack.pop_front();
        }
        self.undo_stack.push_back(current);
        Some(next)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}
