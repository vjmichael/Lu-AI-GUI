//! Durable task progress and checkpoint store.
//!
//! Heartbeats are observations, not timeout-triggered cancellation. A power
//! outage marks RUNNING tasks INTERRUPTED on next exclusive startup;
//! checkpoint/result recovery and external effects need explicit orchestration.

use std::{path::Path, sync::Mutex};

use rusqlite::{params, Connection, OptionalExtension};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Pending,
    Running,
    Paused,
    Completed,
    Failed,
    Interrupted,
}
impl TaskState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Running => "RUNNING",
            Self::Paused => "PAUSED",
            Self::Completed => "COMPLETED",
            Self::Failed => "FAILED",
            Self::Interrupted => "INTERRUPTED",
        }
    }
    fn parse(state: &str) -> Option<Self> {
        match state {
            "PENDING" => Some(Self::Pending),
            "RUNNING" => Some(Self::Running),
            "PAUSED" => Some(Self::Paused),
            "COMPLETED" => Some(Self::Completed),
            "FAILED" => Some(Self::Failed),
            "INTERRUPTED" => Some(Self::Interrupted),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskStoreError {
    Database(String),
    InvalidTaskId,
    InvalidProgress,
    OversizedCheckpoint,
    DuplicateTaskId,
    InvalidTransition,
    InvalidStoredState,
    Poisoned,
}
impl From<rusqlite::Error> for TaskStoreError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Database(e.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRecord {
    pub task_id: String,
    pub workspace_id: Option<String>,
    pub state: TaskState,
    pub progress: u8,
    pub detail: String,
    pub output_folder: Option<String>,
    pub checkpoint: Vec<u8>,
    pub last_heartbeat: Option<String>,
}

pub struct TaskStore {
    connection: Mutex<Connection>,
}

impl TaskStore {
    pub fn open(path: &Path) -> Result<Self, TaskStoreError> {
        let connection = Connection::open(path)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "FULL")?;
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS lu_tasks (
                task_id TEXT PRIMARY KEY NOT NULL,
                workspace_id TEXT,
                state TEXT NOT NULL CHECK(state IN
                    ('PENDING','RUNNING','PAUSED','COMPLETED','FAILED','INTERRUPTED')),
                progress INTEGER NOT NULL CHECK(progress BETWEEN 0 AND 100),
                detail TEXT NOT NULL,
                output_folder TEXT,
                checkpoint BLOB NOT NULL,
                last_heartbeat TEXT,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
             );
             CREATE TABLE IF NOT EXISTS lu_task_events (
                sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id TEXT NOT NULL,
                event TEXT NOT NULL,
                progress INTEGER NOT NULL,
                detail TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
             );
             CREATE INDEX IF NOT EXISTS lu_task_events_task
             ON lu_task_events(task_id, sequence);",
        )?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn create(
        &self,
        task_id: &str,
        workspace_id: Option<&str>,
        output_folder: Option<&str>,
    ) -> Result<(), TaskStoreError> {
        if task_id.trim().is_empty() || task_id.len() > 128 {
            return Err(TaskStoreError::InvalidTaskId);
        }
        let connection = self
            .connection
            .lock()
            .map_err(|_| TaskStoreError::Poisoned)?;
        let inserted = connection.execute(
            "INSERT OR IGNORE INTO lu_tasks
             (task_id,workspace_id,state,progress,detail,output_folder,checkpoint)
             VALUES (?1,?2,'PENDING',0,'',?3,x'')",
            params![task_id, workspace_id, output_folder],
        )?;
        if inserted == 0 {
            return Err(TaskStoreError::DuplicateTaskId);
        }
        Ok(())
    }

    pub fn start(&self, task_id: &str) -> Result<(), TaskStoreError> {
        self.transition(task_id, "PENDING", TaskState::Running)
    }

    pub fn pause(&self, task_id: &str) -> Result<(), TaskStoreError> {
        self.transition(task_id, "RUNNING", TaskState::Paused)
    }

    pub fn resume(&self, task_id: &str) -> Result<(), TaskStoreError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| TaskStoreError::Poisoned)?;
        let changed = connection.execute(
            "UPDATE lu_tasks SET state='RUNNING', updated_at=CURRENT_TIMESTAMP
             WHERE task_id=?1 AND state IN ('PAUSED','INTERRUPTED')",
            params![task_id],
        )?;
        if changed == 0 {
            return Err(TaskStoreError::InvalidTransition);
        }
        Ok(())
    }

    pub fn complete(&self, task_id: &str) -> Result<(), TaskStoreError> {
        self.transition(task_id, "RUNNING", TaskState::Completed)
    }

    pub fn fail(&self, task_id: &str) -> Result<(), TaskStoreError> {
        self.transition(task_id, "RUNNING", TaskState::Failed)
    }

    fn transition(&self, task_id: &str, from: &str, to: TaskState) -> Result<(), TaskStoreError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| TaskStoreError::Poisoned)?;
        let changed = connection.execute(
            "UPDATE lu_tasks SET state=?1, updated_at=CURRENT_TIMESTAMP
             WHERE task_id=?2 AND state=?3",
            params![to.as_str(), task_id, from],
        )?;
        if changed == 0 {
            return Err(TaskStoreError::InvalidTransition);
        }
        Ok(())
    }

    /// Persist the checkpoint and append a progress event in one transaction.
    /// Task progress is monotonic and cannot exceed 100%.
    pub fn checkpoint(
        &self,
        task_id: &str,
        progress: u8,
        detail: &str,
        checkpoint: &[u8],
    ) -> Result<(), TaskStoreError> {
        if progress > 100 {
            return Err(TaskStoreError::InvalidProgress);
        }
        if checkpoint.len() > 4 * 1024 * 1024 {
            return Err(TaskStoreError::OversizedCheckpoint);
        }
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| TaskStoreError::Poisoned)?;
        let tx = connection.transaction()?;
        let changed = tx.execute(
            "UPDATE lu_tasks
             SET progress=?1, detail=?2, checkpoint=?3,
                 updated_at=CURRENT_TIMESTAMP, last_heartbeat=CURRENT_TIMESTAMP
             WHERE task_id=?4 AND state='RUNNING' AND progress<=?1",
            params![progress, detail, checkpoint, task_id],
        )?;
        if changed != 1 {
            return Err(TaskStoreError::InvalidTransition);
        }
        tx.execute(
            "INSERT INTO lu_task_events(task_id,event,progress,detail)
             VALUES(?1,'CHECKPOINT',?2,?3)",
            params![task_id, progress, detail],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Updates liveness without changing the task or forcing a timeout. If
    /// the process stops, the next startup reconciliation handles that.
    pub fn heartbeat(&self, task_id: &str) -> Result<(), TaskStoreError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| TaskStoreError::Poisoned)?;
        let changed = connection.execute(
            "UPDATE lu_tasks SET last_heartbeat=CURRENT_TIMESTAMP
             WHERE task_id=?1 AND state='RUNNING'",
            params![task_id],
        )?;
        if changed != 1 {
            return Err(TaskStoreError::InvalidTransition);
        }
        Ok(())
    }

    /// Requires exclusive ownership of the database/process at restart.
    pub fn reconcile_interrupted(&self) -> Result<usize, TaskStoreError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| TaskStoreError::Poisoned)?;
        Ok(connection.execute(
            "UPDATE lu_tasks SET state='INTERRUPTED',updated_at=CURRENT_TIMESTAMP
             WHERE state='RUNNING'",
            [],
        )?)
    }

    pub fn get(&self, task_id: &str) -> Result<Option<TaskRecord>, TaskStoreError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| TaskStoreError::Poisoned)?;
        let row = connection
            .query_row(
                "SELECT task_id, workspace_id, state, progress, detail,
                    output_folder, checkpoint, last_heartbeat
             FROM lu_tasks WHERE task_id=?1",
                params![task_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, u8>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, Vec<u8>>(6)?,
                        row.get::<_, Option<String>>(7)?,
                    ))
                },
            )
            .optional()?;
        match row {
            Some((
                task_id,
                workspace_id,
                state,
                progress,
                detail,
                output_folder,
                checkpoint,
                last_heartbeat,
            )) => Ok(Some(TaskRecord {
                task_id,
                workspace_id,
                state: TaskState::parse(&state).ok_or(TaskStoreError::InvalidStoredState)?,
                progress,
                detail,
                output_folder,
                checkpoint,
                last_heartbeat,
            })),
            None => Ok(None),
        }
    }
}
