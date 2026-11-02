//! Host-only restart reconciliation for Lu's task and effect journals.
//!
//! Call only after exclusive profile ownership and prior worker shutdown have
//! been established by the desktop host. Reconciliation never resumes tasks,
//! executes tools, or assumes an interrupted side effect was rolled back.

use crate::{
    journal::{EffectJournal, JournalError},
    tasks::{TaskStore, TaskStoreError},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecoverySummary {
    pub interrupted_tasks: usize,
    pub uncertain_effects: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryError {
    Tasks(TaskStoreError),
    Effects(JournalError),
}

/// Requires exclusive ownership of both SQLite files and confirmation that
/// the previous Lu process and its workers have stopped. A failed step is
/// returned to the host; the operation is safe to repeat after fixing the
/// failure because both transitions are idempotent.
pub fn reconcile_native_startup(
    tasks: &TaskStore,
    effects: &EffectJournal,
) -> Result<RecoverySummary, RecoveryError> {
    // Mark effect uncertainty first; failure must not permit automatic task
    // resumption on the strength of a successfully updated task store.
    let uncertain_effects = effects
        .reconcile_interrupted()
        .map_err(RecoveryError::Effects)?;
    let interrupted_tasks = tasks
        .reconcile_interrupted()
        .map_err(RecoveryError::Tasks)?;
    Ok(RecoverySummary {
        interrupted_tasks,
        uncertain_effects,
    })
}
