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
    Cancelled,
    TimedOut,
}
impl TaskState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Running => "RUNNING",
            Self::Paused => "PAUSED",
            Self::Completed => "COMPLETED",
            Self::Failed => "FAILED",
            Self::Interrupted => "INTERRUPTED",
            Self::Cancelled => "CANCELLED",
            Self::TimedOut => "TIMED_OUT",
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
            "CANCELLED" => Some(Self::Cancelled),
            "TIMED_OUT" => Some(Self::TimedOut),
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

/// Host-generated event. A model consumes a durable notification from Lu,
/// never polls the worker or decides if its heartbeat is healthy.
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct TaskEvent {
    pub sequence:i64,
    pub task_id:String,
    pub event:String,
    pub progress:u8,
    pub detail:String,
}
/// Durable, scoped terminal notice. An absent acknowledgement makes the
/// event re-deliverable after restarts; the sequence is its idempotency key.
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct TaskTerminalNotice {
    pub sequence:i64,
    pub task_id:String,
    pub workspace_id:String,
    pub state:TaskState,
    pub progress:u8,
    pub detail:String,
}
/// Sanitized native operator view. Never includes checkpoints, raw worker
/// output, files, or executable paths. Only Lu's trusted desktop host should
/// expose this to the local user; model-facing tools remain workspace scoped.
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct TaskDashboardEntry{
    pub task_id:String,
    pub workspace_id:Option<String>,
    pub state:TaskState,
    pub progress:u8,
    pub detail:String,
    pub last_heartbeat:Option<String>,
    pub updated_at:String,
}
pub struct TaskStore {
    connection: Mutex<Connection>,
}

impl TaskStore {
    pub fn open(path: &Path) -> Result<Self, TaskStoreError> {
        let mut connection = Connection::open(path)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "FULL")?;
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS lu_tasks (
                task_id TEXT PRIMARY KEY NOT NULL,
                workspace_id TEXT,
                state TEXT NOT NULL CHECK(state IN
                    ('PENDING','RUNNING','PAUSED','COMPLETED','FAILED','INTERRUPTED','CANCELLED','TIMED_OUT')),
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
             ON lu_task_events(task_id, sequence);
             CREATE TABLE IF NOT EXISTS lu_task_notice_ack(
               event_sequence INTEGER PRIMARY KEY NOT NULL,
               acknowledged_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
             );
             CREATE TABLE IF NOT EXISTS lu_task_auto_continue (
               workspace_id TEXT PRIMARY KEY NOT NULL,
               enabled INTEGER NOT NULL CHECK(enabled IN (0,1))
             );",
        )?;
        // Existing desktop profiles may have the older CHECK schema.
        // Rebuild atomically with every row preserved before enabling
        // cancellation; SQLite cannot ALTER an existing CHECK constraint.
        let schema:String=connection.query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='lu_tasks'",
            [],|row|row.get(0)
        )?;
        if !schema.contains("'TIMED_OUT'"){
            let tx=connection.transaction()?;
            tx.execute_batch(
                "CREATE TABLE lu_tasks_migrating (
                    task_id TEXT PRIMARY KEY NOT NULL,
                    workspace_id TEXT,
                    state TEXT NOT NULL CHECK(state IN
                       ('PENDING','RUNNING','PAUSED','COMPLETED','FAILED','INTERRUPTED','CANCELLED','TIMED_OUT')),
                    progress INTEGER NOT NULL CHECK(progress BETWEEN 0 AND 100),
                    detail TEXT NOT NULL,
                    output_folder TEXT,
                    checkpoint BLOB NOT NULL,
                    last_heartbeat TEXT,
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                 );
                 INSERT INTO lu_tasks_migrating
                 (task_id,workspace_id,state,progress,detail,output_folder,checkpoint,
                  last_heartbeat,updated_at)
                 SELECT task_id,workspace_id,state,progress,detail,output_folder,checkpoint,
                        last_heartbeat,updated_at FROM lu_tasks;
                 DROP TABLE lu_tasks;
                 ALTER TABLE lu_tasks_migrating RENAME TO lu_tasks;"
            )?;
            tx.commit()?;
        }
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
        let mut connection=self.connection.lock().map_err(|_|TaskStoreError::Poisoned)?;
        let tx=connection.transaction()?;
        let changed=tx.execute(
            "UPDATE lu_tasks SET state='RUNNING', updated_at=CURRENT_TIMESTAMP
             WHERE task_id=?1 AND state IN ('PAUSED','INTERRUPTED')",params![task_id]
        )?;
        if changed!=1{return Err(TaskStoreError::InvalidTransition);}
        tx.execute(
            "INSERT INTO lu_task_events(task_id,event,progress,detail)
             SELECT task_id,'RUNNING',progress,detail FROM lu_tasks WHERE task_id=?1",
            params![task_id],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn complete(&self, task_id: &str) -> Result<(), TaskStoreError> {
        self.transition(task_id, "RUNNING", TaskState::Completed)
    }

    pub fn fail(&self, task_id: &str) -> Result<(), TaskStoreError> {
        self.transition(task_id, "RUNNING", TaskState::Failed)
    }
    /// A supervisor may record verified cancellation only *after* its owned
    /// process has been terminated and reaped. This is not a UI-only flag.
    pub fn cancelled(&self,task_id:&str)->Result<(),TaskStoreError>{
        let mut connection=self.connection.lock().map_err(|_|TaskStoreError::Poisoned)?;
        let tx=connection.transaction()?;
        let changed=tx.execute(
            "UPDATE lu_tasks SET state='CANCELLED',updated_at=CURRENT_TIMESTAMP
             WHERE task_id=?1 AND state IN ('PENDING','RUNNING','PAUSED')",
            params![task_id],
        )?;
        if changed!=1{return Err(TaskStoreError::InvalidTransition);}
        tx.execute(
            "INSERT INTO lu_task_events(task_id,event,progress,detail)
             SELECT task_id,'CANCELLED',progress,detail FROM lu_tasks WHERE task_id=?1",
            params![task_id],
        )?;
        tx.commit()?;
        Ok(())
    }
    /// Host-enforced hard deadline: only after the owned worker was stopped
    /// and reaped. No heartbeat-based classification or inferred timeout.
    pub fn timed_out(&self,task_id:&str)->Result<(),TaskStoreError>{
        self.transition(task_id,"RUNNING",TaskState::TimedOut)
    }
    /// When launch never occurred, move a PENDING intent to FAILED rather
    /// than leaving a ghost job in the operator's task pane.
    pub fn fail_pending(&self,task_id:&str)->Result<(),TaskStoreError>{
        self.transition(task_id,"PENDING",TaskState::Failed)
    }

    fn transition(&self, task_id: &str, from: &str, to: TaskState) -> Result<(), TaskStoreError> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| TaskStoreError::Poisoned)?;
        let tx=connection.transaction()?;
        let changed=tx.execute(
            "UPDATE lu_tasks SET state=?1, updated_at=CURRENT_TIMESTAMP
             WHERE task_id=?2 AND state=?3",
            params![to.as_str(),task_id,from],
        )?;
        if changed!=1{return Err(TaskStoreError::InvalidTransition);}
        tx.execute(
            "INSERT INTO lu_task_events(task_id,event,progress,detail)
             SELECT task_id,?1,progress,detail FROM lu_tasks WHERE task_id=?2",
            params![to.as_str(),task_id],
        )?;
        tx.commit()?;
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
        let mut connection=self.connection.lock().map_err(|_|TaskStoreError::Poisoned)?;
        let tx=connection.transaction()?;
        // Insert events for precisely the RUNNING set while holding a
        // transaction, so after a crash every interrupted job is discoverable.
        tx.execute(
            "INSERT INTO lu_task_events(task_id,event,progress,detail)
             SELECT task_id,'INTERRUPTED',progress,detail FROM lu_tasks
             WHERE state='RUNNING'",[],
        )?;
        let changed=tx.execute(
            "UPDATE lu_tasks SET state='INTERRUPTED',updated_at=CURRENT_TIMESTAMP
             WHERE state='RUNNING'",[],
        )?;
        tx.commit()?;
        Ok(changed)
    }

    /// Cursor-based events are delivered by the native host. A caller must
    /// already have a trusted task/workspace binding: this method rejects
    /// cross-workspace reads and never leaks checkpoints or output paths.
    pub fn events_after(
        &self, task_id:&str, workspace_id:&str, after:i64, limit:usize
    )->Result<Vec<TaskEvent>,TaskStoreError>{
        if after<0||limit==0||limit>128||workspace_id.is_empty()
           ||workspace_id.len()>128{
            return Err(TaskStoreError::InvalidProgress);
        }
        let connection=self.connection.lock().map_err(|_|TaskStoreError::Poisoned)?;
        let owned:bool=connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM lu_tasks
             WHERE task_id=?1 AND workspace_id=?2)",
            params![task_id,workspace_id],|row|row.get(0)
        )?;
        if !owned{return Ok(Vec::new());}
        let mut statement=connection.prepare(
            "SELECT sequence,task_id,event,progress,detail FROM lu_task_events
             WHERE task_id=?1 AND sequence>?2 ORDER BY sequence ASC LIMIT ?3"
        )?;
        let rows=statement.query_map(params![task_id,after,limit as i64],|row|{
            Ok(TaskEvent{sequence:row.get(0)?,task_id:row.get(1)?,
                event:row.get(2)?,progress:row.get(3)?,
                detail:row.get::<_,String>(4)?.chars().take(2048).collect()})
        })?;
        let mut events=Vec::new();
        for row in rows{events.push(row?);}
        Ok(events)
    }

    /// Native operator opt-in; never infer permission from a model message.
    /// This preference never grants a tool capability.
    pub fn set_auto_continue(&self,workspace_id:&str,enabled:bool)
        ->Result<(),TaskStoreError>{
        if workspace_id.len()<3||workspace_id.len()>128
           ||!workspace_id.starts_with("c-")
           ||!workspace_id.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-'){
            return Err(TaskStoreError::InvalidTaskId);
        }
        let connection=self.connection.lock().map_err(|_|TaskStoreError::Poisoned)?;
        connection.execute(
            "INSERT INTO lu_task_auto_continue(workspace_id,enabled) VALUES(?1,?2)
             ON CONFLICT(workspace_id) DO UPDATE SET enabled=excluded.enabled",
            params![workspace_id,if enabled{1}else{0}],
        )?;
        Ok(())
    }
    pub fn auto_continue_enabled(&self,workspace_id:&str)
        ->Result<bool,TaskStoreError>{
        if workspace_id.len()<3||workspace_id.len()>128
           ||!workspace_id.starts_with("c-"){
            return Ok(false);
        }
        let connection=self.connection.lock().map_err(|_|TaskStoreError::Poisoned)?;
        let enabled:Option<i64>=connection.query_row(
            "SELECT enabled FROM lu_task_auto_continue WHERE workspace_id=?1",
            params![workspace_id],|row|row.get(0),
        ).optional()?;
        Ok(enabled==Some(1))
    }

    /// Host-owned durable delivery queue. No workspace selector from a model
    /// is accepted here; the host obtains each notice's workspace from SQLite.
    /// Deliveries are at-least-once until explicitly acknowledged.
    pub fn pending_terminal_notices(&self,limit:usize)
        ->Result<Vec<TaskTerminalNotice>,TaskStoreError>{
        self.pending_terminal_notices_after(0,limit)
    }
    /// Cursor pagination keeps more recent terminal events reachable even
    /// when many earlier events await model acknowledgement.
    pub fn pending_terminal_notices_after(&self,after:i64,limit:usize)
        ->Result<Vec<TaskTerminalNotice>,TaskStoreError>{
        if after<0||limit==0||limit>128{return Err(TaskStoreError::InvalidProgress);}
        let connection=self.connection.lock().map_err(|_|TaskStoreError::Poisoned)?;
        let mut stmt=connection.prepare(
            "SELECT e.sequence,e.task_id,t.workspace_id,e.event,e.progress,e.detail
             FROM lu_task_events e JOIN lu_tasks t ON t.task_id=e.task_id
             LEFT JOIN lu_task_notice_ack a ON a.event_sequence=e.sequence
             WHERE a.event_sequence IS NULL AND e.sequence>?1
               AND e.event IN ('COMPLETED','FAILED','INTERRUPTED','CANCELLED','TIMED_OUT')
               AND t.workspace_id IS NOT NULL
             ORDER BY e.sequence ASC LIMIT ?2"
        )?;
        let rows=stmt.query_map(params![after,limit as i64],|row|{
            Ok((row.get::<_,i64>(0)?,row.get::<_,String>(1)?,
                row.get::<_,String>(2)?,row.get::<_,String>(3)?,
                row.get::<_,u8>(4)?,row.get::<_,String>(5)?))
        })?;
        let mut result=Vec::new();
        for row in rows {
            let (sequence,task_id,workspace_id,event,progress,detail)=row?;
            result.push(TaskTerminalNotice{
                sequence,task_id,workspace_id,
                state:TaskState::parse(&event)
                    .ok_or(TaskStoreError::InvalidStoredState)?,
                progress,detail:detail.chars().take(1024).collect(),
            });
        }
        Ok(result)
    }
    /// Model delivery is scoped to the originating conversation. Query the
    /// owning workspace directly so unrelated unacknowledged jobs never
    /// starve this conversation, even after a long offline period.
    pub fn pending_notices_for_workspace(
        &self,workspace_id:&str,limit:usize
    )->Result<Vec<TaskTerminalNotice>,TaskStoreError>{
        if workspace_id.is_empty()||workspace_id.len()>128
            ||limit==0||limit>32{return Err(TaskStoreError::InvalidProgress);}
        let connection=self.connection.lock().map_err(|_|TaskStoreError::Poisoned)?;
        let mut stmt=connection.prepare(
            "SELECT e.sequence,e.task_id,e.event,e.progress,e.detail
             FROM lu_task_events e JOIN lu_tasks t ON t.task_id=e.task_id
             LEFT JOIN lu_task_notice_ack a ON a.event_sequence=e.sequence
             WHERE t.workspace_id=?1 AND a.event_sequence IS NULL
               AND e.event IN ('COMPLETED','FAILED','INTERRUPTED','CANCELLED','TIMED_OUT')
             ORDER BY e.sequence ASC LIMIT ?2"
        )?;
        let rows=stmt.query_map(params![workspace_id,limit as i64],|row|{
            Ok((row.get::<_,i64>(0)?,row.get::<_,String>(1)?,
                row.get::<_,String>(2)?,row.get::<_,u8>(3)?))
        })?;
        let mut result=Vec::new();
        for row in rows {
            let (sequence,task_id,event,progress)=row?;
            result.push(TaskTerminalNotice{
                sequence,task_id,workspace_id:workspace_id.to_owned(),
                state:TaskState::parse(&event)
                    .ok_or(TaskStoreError::InvalidStoredState)?,
                progress,detail:String::new(), // never pass worker text to LLM
            });
        }
        Ok(result)
    }
    /// Acknowledgement is explicit, scoped, idempotent, and only valid for
    /// an actual terminal event belonging to the supplied trusted workspace.
    pub fn acknowledge_terminal_notice(
        &self,sequence:i64,workspace_id:&str
    )->Result<bool,TaskStoreError>{
        if sequence<=0||workspace_id.is_empty()||workspace_id.len()>128{
            return Err(TaskStoreError::InvalidProgress);
        }
        let connection=self.connection.lock().map_err(|_|TaskStoreError::Poisoned)?;
        let changed=connection.execute(
            "INSERT OR IGNORE INTO lu_task_notice_ack(event_sequence)
             SELECT e.sequence FROM lu_task_events e
             JOIN lu_tasks t ON t.task_id=e.task_id
             WHERE e.sequence=?1 AND t.workspace_id=?2
               AND e.event IN ('COMPLETED','FAILED','INTERRUPTED','CANCELLED','TIMED_OUT')",
            params![sequence,workspace_id],
        )?;
        Ok(changed==1)
    }

    /// Host-local overview: active tasks first, followed by recent history.
    /// This query is read-only; no model authorization is inferred from it.
    pub fn dashboard(&self,limit:usize)->Result<Vec<TaskDashboardEntry>,TaskStoreError>{
        if limit==0||limit>200{return Err(TaskStoreError::InvalidProgress);}
        let connection=self.connection.lock().map_err(|_|TaskStoreError::Poisoned)?;
        let mut stmt=connection.prepare(
            "SELECT task_id,workspace_id,state,progress,detail,last_heartbeat,updated_at
             FROM lu_tasks
             ORDER BY CASE state
               WHEN 'RUNNING' THEN 0 WHEN 'PENDING' THEN 1
               WHEN 'PAUSED' THEN 2 WHEN 'INTERRUPTED' THEN 3
               WHEN 'FAILED' THEN 4 WHEN 'CANCELLED' THEN 5
               WHEN 'TIMED_OUT' THEN 6 ELSE 7 END,
               updated_at DESC,task_id ASC LIMIT ?1"
        )?;
        let rows=stmt.query_map(params![limit as i64],|row|{
            Ok((row.get::<_,String>(0)?,row.get::<_,Option<String>>(1)?,
                row.get::<_,String>(2)?,row.get::<_,u8>(3)?,
                row.get::<_,String>(4)?,row.get::<_,Option<String>>(5)?,
                row.get::<_,String>(6)?))
        })?;
        let mut result=Vec::new();
        for row in rows{
            let (task_id,workspace_id,state,progress,detail,last_heartbeat,updated_at)=row?;
            result.push(TaskDashboardEntry{
                task_id,workspace_id,
                state:TaskState::parse(&state).ok_or(TaskStoreError::InvalidStoredState)?,
                progress,detail:detail.chars().take(2048).collect(),
                last_heartbeat,updated_at,
            });
        }
        Ok(result)
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

#[cfg(test)]
mod task_auto_continue_tests {
    use super::*;
    #[test]
    fn disabled_by_default_and_persisted_with_explicit_operator_opt_in(){
        let store=TaskStore::open(Path::new(":memory:")).unwrap();
        assert!(!store.auto_continue_enabled("c-abc").unwrap());
        store.set_auto_continue("c-abc",true).unwrap();
        assert!(store.auto_continue_enabled("c-abc").unwrap());
        assert!(!store.auto_continue_enabled("c-other").unwrap());
        assert!(!store.auto_continue_enabled("local-media").unwrap());
        assert!(store.set_auto_continue("local-media",true).is_err());
        store.set_auto_continue("c-abc",false).unwrap();
        assert!(!store.auto_continue_enabled("c-abc").unwrap());
    }
}
#[cfg(test)]
mod terminal_notification_tests{
    use super::*;
    #[test]
    fn every_terminal_outcome_produces_scoped_durable_notice(){
        let dir=std::env::temp_dir().join(format!(
            "lu-notices-{}-{:?}",std::process::id(),std::thread::current().id()
        ));
        let _=std::fs::remove_file(&dir);
        let store=TaskStore::open(&dir).unwrap();
        let cases=["done","failed","cancel","timeout","restart","launch-failed"];
        for id in cases{
            store.create(id,Some("c-owner"),None).unwrap();
            if id!="launch-failed"{store.start(id).unwrap();}
            match id{
                "done"=>store.complete(id).unwrap(),
                "failed"=>store.fail(id).unwrap(),
                "cancel"=>store.cancelled(id).unwrap(),
                "timeout"=>store.timed_out(id).unwrap(),
                "restart"=>{assert_eq!(store.reconcile_interrupted().unwrap(),1);},
                _=>store.fail_pending(id).unwrap(),
            };
        }
        let notices=store.pending_notices_for_workspace("c-owner",32).unwrap();
        assert_eq!(notices.len(),6);
        let states=notices.iter().map(|n|n.state).collect::<Vec<_>>();
        assert!(states.contains(&TaskState::Completed));
        assert!(states.contains(&TaskState::Failed));
        assert!(states.contains(&TaskState::Cancelled));
        assert!(states.contains(&TaskState::TimedOut));
        assert!(states.contains(&TaskState::Interrupted));
        assert_eq!(store.pending_notices_for_workspace("c-other",32).unwrap().len(),0);
        assert!(store.acknowledge_terminal_notice(notices[0].sequence,"c-other")
            .unwrap()==false);
        assert!(store.acknowledge_terminal_notice(notices[0].sequence,"c-owner")
            .unwrap());
        assert!(!store.acknowledge_terminal_notice(notices[0].sequence,"c-owner")
            .unwrap());
        drop(store);
        let restored=TaskStore::open(&dir).unwrap();
        assert_eq!(restored.pending_notices_for_workspace("c-owner",32)
            .unwrap().len(),5);
        assert_eq!(restored.pending_terminal_notices(32).unwrap().len(),5);
        drop(restored);
        let _=std::fs::remove_file(&dir);
        let _=std::fs::remove_file(format!("{}-wal",dir.display()));
        let _=std::fs::remove_file(format!("{}-shm",dir.display()));
    }
    #[test]
    fn only_true_terminal_events_can_be_acknowledged(){
        let store=TaskStore::open(Path::new(":memory:")).unwrap();
        store.create("running",Some("owner"),None).unwrap();
        store.start("running").unwrap();
        store.checkpoint("running",33,"private event",b"private data").unwrap();
        let events=store.events_after("running","owner",0,8).unwrap();
        for event in events{
            assert!(!store.acknowledge_terminal_notice(event.sequence,"owner").unwrap());
        }
        assert!(store.pending_terminal_notices(8).unwrap().is_empty());
    }
}
#[cfg(test)]
mod dashboard_tests{
    use super::*;
    #[test]
    fn operator_dashboard_is_bounded_and_keeps_private_checkpoints_out(){
        let store=TaskStore::open(Path::new(":memory:")).unwrap();
        store.create("finished",Some("video"),Some("/secret/output")).unwrap();
        store.start("finished").unwrap();
        store.checkpoint("finished",67,"Analyzing frames",b"secret bytes").unwrap();
        store.complete("finished").unwrap();
        store.create("running",Some("audio"),None).unwrap();
        store.start("running").unwrap();
        store.checkpoint("running",13,"Decoding audio",b"not for UI").unwrap();
        let list=store.dashboard(20).unwrap();
        assert_eq!(list.len(),2);
        assert_eq!(list[0].task_id,"running");
        assert_eq!(list[1].task_id,"finished");
        assert_eq!(list[1].progress,67);
        assert_eq!(list[0].state,TaskState::Running);
        assert_eq!(list[1].state,TaskState::Completed);
        let visible=format!("{list:?}");
        assert!(!visible.contains("secret bytes"));
        assert!(!visible.contains("/secret/output"));
        assert!(store.dashboard(0).is_err());
        assert!(store.dashboard(201).is_err());
    }
    #[test]
    fn startup_dashboard_never_presents_orphan_running_process_as_live(){
        let store=TaskStore::open(Path::new(":memory:")).unwrap();
        store.create("video",Some("research"),None).unwrap();
        store.start("video").unwrap();
        assert_eq!(store.reconcile_interrupted().unwrap(),1);
        let list=store.dashboard(10).unwrap();
        assert_eq!(list[0].state,TaskState::Interrupted);
        assert_eq!(store.reconcile_interrupted().unwrap(),0);
    }
}

#[cfg(test)]
mod cancellation_tests{
    use super::*;
    #[test]
    fn cancellation_is_terminal_audited_and_does_not_replay(){
        let store=TaskStore::open(Path::new(":memory:")).unwrap();
        store.create("task",Some("media"),None).unwrap();
        store.start("task").unwrap();
        store.checkpoint("task",45,"Decoding",b"checkpoint").unwrap();
        store.cancelled("task").unwrap();
        assert_eq!(store.get("task").unwrap().unwrap().state,TaskState::Cancelled);
        let events=store.events_after("task","media",0,30).unwrap();
        assert_eq!(events.last().unwrap().event,"CANCELLED");
        assert!(store.cancelled("task").is_err());
        assert!(store.resume("task").is_err());
        assert_eq!(store.reconcile_interrupted().unwrap(),0);
    }
    #[test]
    fn cancelled_schema_upgrades_old_profile_without_losing_history(){
        let dir=std::env::temp_dir().join(format!(
            "lu-bg-schema-{}-{:?}",std::process::id(),std::thread::current().id()
        ));
        let _=std::fs::remove_file(&dir);
        {
            let db=Connection::open(&dir).unwrap();
            db.execute_batch("CREATE TABLE lu_tasks (
                task_id TEXT PRIMARY KEY NOT NULL,
                workspace_id TEXT,
                state TEXT NOT NULL CHECK(state IN
                    ('PENDING','RUNNING','PAUSED','COMPLETED','FAILED','INTERRUPTED')),
                progress INTEGER NOT NULL CHECK(progress BETWEEN 0 AND 100),
                detail TEXT NOT NULL,
                output_folder TEXT,
                checkpoint BLOB NOT NULL,
                last_heartbeat TEXT,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);
                INSERT INTO lu_tasks(task_id,workspace_id,state,progress,detail,checkpoint)
                VALUES('old','w','RUNNING',20,'retained',x'6162');").unwrap();
        }
        let store=TaskStore::open(&dir).unwrap();
        let before=store.get("old").unwrap().unwrap();
        assert_eq!(before.progress,20);
        assert_eq!(before.checkpoint,b"ab");
        store.cancelled("old").unwrap();
        assert_eq!(store.get("old").unwrap().unwrap().state,TaskState::Cancelled);
        drop(store);
        let _=std::fs::remove_file(&dir);
        let _=std::fs::remove_file(dir.with_extension("sqlite-wal"));
        let _=std::fs::remove_file(dir.with_extension("sqlite-shm"));
    }
}
