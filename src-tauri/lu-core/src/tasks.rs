//! Durable task progress and checkpoint store.
//!
//! Heartbeats are observations, not timeout-triggered cancellation. A power
//! outage marks RUNNING tasks INTERRUPTED on next exclusive startup;
//! checkpoint/result recovery and external effects need explicit orchestration.

use std::{path::Path, sync::Mutex};

use rusqlite::{params, Connection, OptionalExtension};
use crate::native_awareness_lane::Note;

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
/// Original-journal recovery projection; no PID, argv, path, checkpoint,
/// private bytes, tool grant or action capability is exported.
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct SecurityRecoveryRow{
    pub task_id:String,
    pub state:TaskState,
    pub custody_reason:Option<String>,
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
/// Bounded, enum-only record of an authorized Observe session's diagnostic
/// hints. This is NOT a saved ANSF memory or a model-generated instruction.
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct AwarenessNoteRow{
    pub task_id:String,
    pub workspace_id:String,
    pub sequence:u64,
    pub code:String,
    pub source:String,
    pub priority:String,
    pub created_at:String,
    pub data_origin:String,
    pub network_egress_allowed:bool,
}
/// Verified non-file output returned from the first-party bounded SHA-256 scanner.
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct TaskVerifiedResult {
    pub kind:String,pub sha256:String,pub bytes:u64,
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
             CREATE TABLE IF NOT EXISTS lu_task_verified_results (
               task_id TEXT PRIMARY KEY NOT NULL,
               kind TEXT NOT NULL CHECK(kind='sha256_scan'),
               sha256 TEXT NOT NULL CHECK(length(sha256)=64),
               byte_count INTEGER NOT NULL CHECK(byte_count>0),
               recorded_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
             );
             CREATE TABLE IF NOT EXISTS lu_reviewed_turn_origin (
               singleton INTEGER PRIMARY KEY CHECK(singleton=1),
               conversation_id TEXT NOT NULL,
               user_turn_id TEXT NOT NULL,
               created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
             );
             CREATE TABLE IF NOT EXISTS lu_task_auto_continue (
               workspace_id TEXT PRIMARY KEY NOT NULL,
               enabled INTEGER NOT NULL CHECK(enabled IN (0,1))
             );
             CREATE TABLE IF NOT EXISTS lu_awareness_notes(
               task_id TEXT NOT NULL,
               workspace_id TEXT NOT NULL,
               sequence INTEGER NOT NULL CHECK(sequence > 0),
               code TEXT NOT NULL CHECK(code IN
                 ('error_indicator','unexpected_ui_state','verified_test_failure','stalled_progress')),
               source TEXT NOT NULL CHECK(source IN
                 ('renderer_hint','native_accessibility','native_runtime','native_test_receipt')),
               priority TEXT NOT NULL CHECK(priority IN ('observe','investigate')),
               created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
               PRIMARY KEY(task_id,sequence)
             );
             CREATE INDEX IF NOT EXISTS lu_awareness_notes_workspace
               ON lu_awareness_notes(workspace_id,task_id,sequence);",
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
        // P06: previous Observe schema omitted native_runtime despite the
        // trusted host producing it. Preserve existing rows exactly, in one
        // transaction, before allowing verified host-only runtime notes.
        let awareness_schema:String=connection.query_row(
            "SELECT sql FROM sqlite_master
             WHERE type='table' AND name='lu_awareness_notes'",
            [],|row|row.get(0)
        )?;
        if !awareness_schema.contains("'native_runtime'"){
            let tx=connection.transaction()?;
            tx.execute_batch(
                "CREATE TABLE lu_awareness_notes_p06 (
                    task_id TEXT NOT NULL,
                    workspace_id TEXT NOT NULL,
                    sequence INTEGER NOT NULL CHECK(sequence > 0),
                    code TEXT NOT NULL CHECK(code IN
                      ('error_indicator','unexpected_ui_state','verified_test_failure','stalled_progress')),
                    source TEXT NOT NULL CHECK(source IN
                      ('renderer_hint','native_accessibility','native_runtime','native_test_receipt')),
                    priority TEXT NOT NULL CHECK(priority IN ('observe','investigate')),
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    PRIMARY KEY(task_id,sequence)
                 );
                 INSERT INTO lu_awareness_notes_p06
                  (task_id,workspace_id,sequence,code,source,priority,created_at)
                 SELECT task_id,workspace_id,sequence,code,source,priority,created_at
                 FROM lu_awareness_notes;
                 DROP TABLE lu_awareness_notes;
                 ALTER TABLE lu_awareness_notes_p06 RENAME TO lu_awareness_notes;
                 CREATE INDEX lu_awareness_notes_workspace
                   ON lu_awareness_notes(workspace_id,task_id,sequence);"
            )?;
            tx.commit()?;
        }
        // Upgrade legacy and fresh note tables. Defaults label existing
        // rows and SQLite CHECK clauses prohibit observation declassification.
        for (name,ddl) in [
            ("data_origin","ALTER TABLE lu_awareness_notes ADD COLUMN data_origin TEXT NOT NULL DEFAULT 'observe_learn' CHECK(data_origin='observe_learn')"),
            ("network_egress_allowed","ALTER TABLE lu_awareness_notes ADD COLUMN network_egress_allowed INTEGER NOT NULL DEFAULT 0 CHECK(network_egress_allowed=0)")
        ]{
            let count:i64=connection.query_row(
                "SELECT COUNT(*) FROM pragma_table_info('lu_awareness_notes') WHERE name=?1",
                params![name],|row|row.get(0))?;
            if count==0{connection.execute_batch(ddl)?;}
        }
        // Native enum-only notes are bounded to seven days. This is a
        // logical SQLite retention limit, not secure erasure of WAL backups.
        connection.execute(
            "DELETE FROM lu_awareness_notes
             WHERE created_at < datetime('now','-7 days')",[]
        )?;
        // Establish the OS custody child table only AFTER legacy lu_tasks
        // rebuilds, so the foreign key never points at a dropped pre-upgrade
        // task table. No old task receives a retroactive OS proof.
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS lu_task_windows_job_custody(
               task_id TEXT PRIMARY KEY REFERENCES lu_tasks(task_id),
               workspace_id TEXT NOT NULL,
               method TEXT NOT NULL CHECK(method='WINDOWS_JOB_ACTIVE_ZERO_V1'),
               recorded_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
             );
             CREATE TRIGGER IF NOT EXISTS lu_task_os_custody_no_update
               BEFORE UPDATE ON lu_task_windows_job_custody
               BEGIN SELECT RAISE(ABORT,'Immutable Windows Job observation'); END;
             CREATE TRIGGER IF NOT EXISTS lu_task_os_custody_no_delete
               BEFORE DELETE ON lu_task_windows_job_custody
               BEGIN SELECT RAISE(ABORT,'Immutable Windows Job observation'); END;
             CREATE TABLE IF NOT EXISTS lu_task_custody_uncertainty(
               task_id TEXT PRIMARY KEY NOT NULL REFERENCES lu_tasks(task_id),
               reason TEXT NOT NULL CHECK(reason IN
                 ('LINUX_GUARDIAN_LOST','OS_CUSTODY_UNVERIFIED')),
               recorded_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
             );
             CREATE TRIGGER IF NOT EXISTS lu_task_uncertainty_no_update
               BEFORE UPDATE ON lu_task_custody_uncertainty
               BEGIN SELECT RAISE(ABORT,'Immutable OS custody uncertainty'); END;
             CREATE TRIGGER IF NOT EXISTS lu_task_uncertainty_no_delete
               BEFORE DELETE ON lu_task_custody_uncertainty
               BEGIN SELECT RAISE(ABORT,'Immutable OS custody uncertainty'); END;"
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

    /// E1-P15: durable singleton origin of a reviewed native tool turn.
    /// The caller MUST first verify the original persisted human chat turn.
    /// This records identity, never authorization or executable permission.
    /// Rebinding an existing journal to another conversation fails closed.
    pub fn bind_reviewed_human_turn(
        &self, conversation_id:&str, user_turn_id:&str
    )->Result<(),TaskStoreError>{
        let valid=|value:&str,prefix:&str|
            value.len()==prefix.len()+32
            &&value.starts_with(prefix)
            &&value[prefix.len()..].bytes().all(|b|
                b.is_ascii_hexdigit()&&!b.is_ascii_uppercase());
        if !valid(conversation_id,"c-")||!valid(user_turn_id,"g-"){
            return Err(TaskStoreError::InvalidTaskId)
        }
        let mut conn=self.connection.lock().map_err(|_|TaskStoreError::Poisoned)?;
        let tx=conn.transaction()?;
        tx.execute(
            "INSERT OR IGNORE INTO lu_reviewed_turn_origin
             (singleton,conversation_id,user_turn_id) VALUES(1,?1,?2)",
            params![conversation_id,user_turn_id],
        )?;
        let existing:(String,String)=tx.query_row(
            "SELECT conversation_id,user_turn_id FROM lu_reviewed_turn_origin
             WHERE singleton=1",[],|row|Ok((row.get(0)?,row.get(1)?))
        )?;
        if existing.0!=conversation_id||existing.1!=user_turn_id{
            return Err(TaskStoreError::InvalidTransition)
        }
        tx.commit()?;
        Ok(())
    }

    /// AGENT-01L: write a *historical* OS kernel observation only after
    /// verified Windows Job admission, termination and ActiveProcesses==0.
    /// The proof type has no public constructor: only the native Win32 Job
    /// object can create it by querying its actual still-owned handle.
    /// This does not attest an unrelated process, a Linux escaped session,
    /// or an ongoing permission to start another worker.
    #[cfg(windows)]
    pub(crate) fn record_verified_windows_job_empty(
        &self,task_id:&str,workspace_id:&str,
        proof:lu_win_worker_job::WindowsJobEmptyProof,
    )->Result<(),TaskStoreError>{
        let trusted=|v:&str,p:&str|
            v.len()==p.len()+32&&v.starts_with(p)
                &&v[p.len()..].bytes().all(|b|
                    b.is_ascii_hexdigit()&&!b.is_ascii_uppercase());
        if !trusted(workspace_id,"g-")||task_id.len()>128||task_id.is_empty()
            ||!["-gh","-gs","-gb","-gc","-fm"]
                .iter().any(|suffix|task_id.ends_with(suffix)){
            return Err(TaskStoreError::InvalidTaskId)
        }
        let mut db=self.connection.lock().map_err(|_|TaskStoreError::Poisoned)?;
        let tx=db.transaction()?;
        let matching:i64=tx.query_row(
            "SELECT COUNT(*) FROM lu_tasks t
             JOIN lu_reviewed_turn_origin o ON o.singleton=1
             WHERE t.task_id=?1 AND t.workspace_id=?2
               AND o.user_turn_id=?2 AND t.state='COMPLETED'",
            params![task_id,workspace_id],|r|r.get(0)
        )?;
        if matching!=1{return Err(TaskStoreError::InvalidTransition)}
        tx.execute(
            "INSERT INTO lu_task_windows_job_custody(
              task_id,workspace_id,method) VALUES(?1,?2,?3)",
            params![task_id,workspace_id,proof.method()]
        )?;
        tx.commit()?;
        Ok(())
    }
    /// Read the exact sealed human origin for a reviewed native worker.
    /// The cgroup custodian never receives a model or caller-supplied chat.
    pub fn reviewed_human_origin(
        &self
    )->Result<Option<(String,String)>,TaskStoreError>{
        let db=self.connection.lock().map_err(|_|TaskStoreError::Poisoned)?;
        db.query_row(
            "SELECT conversation_id,user_turn_id FROM lu_reviewed_turn_origin
             WHERE singleton=1",[],|r|Ok((r.get(0)?,r.get(1)?))
        ).optional().map_err(TaskStoreError::from)
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
             WHERE task_id=?1 AND state IN ('PAUSED','INTERRUPTED')
               AND NOT EXISTS(SELECT 1 FROM lu_task_custody_uncertainty
                              WHERE task_id=?1)",params![task_id]
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

    /// P0-SEC: persist the FIRST host-observed OS custody failure without
    /// changing RUNNING into a misleading completed/cancelled state. Restart
    /// later reconciles RUNNING to INTERRUPTED; this immutable row survives.
    /// Only host-owned fixed enum reason codes are admitted.
    pub fn record_process_uncertain(
        &self,task_id:&str,reason:&str,
    )->Result<(),TaskStoreError>{
        if !matches!(reason,"LINUX_GUARDIAN_LOST"|"OS_CUSTODY_UNVERIFIED"){
            return Err(TaskStoreError::InvalidTransition)
        }
        let mut conn=self.connection.lock().map_err(|_|TaskStoreError::Poisoned)?;
        let tx=conn.transaction()?;
        let changed=tx.execute(
            "INSERT OR IGNORE INTO lu_task_custody_uncertainty(task_id,reason)
             SELECT task_id,?2 FROM lu_tasks
             WHERE task_id=?1 AND state IN ('PENDING','RUNNING')",
            params![task_id,reason],
        )?;
        if changed==1{
            tx.execute(
                "INSERT INTO lu_task_events(task_id,event,progress,detail)
                 SELECT task_id,'PROCESS_UNCERTAIN',progress,detail
                 FROM lu_tasks WHERE task_id=?1",
                params![task_id],
            )?;
        }else{
            let exists:i64=tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM lu_task_custody_uncertainty
                 WHERE task_id=?1)",params![task_id],|row|row.get(0)
            )?;
            if exists!=1{return Err(TaskStoreError::InvalidTransition)}
        }
        tx.commit()?;
        Ok(())
    }
    /// Host-only historical uncertainty view, never an execution grant.
    pub fn process_uncertainty(&self,task_id:&str)
        ->Result<Option<String>,TaskStoreError>{
        let conn=self.connection.lock().map_err(|_|TaskStoreError::Poisoned)?;
        conn.query_row(
            "SELECT reason FROM lu_task_custody_uncertainty
             WHERE task_id=?1",params![task_id],|row|row.get(0)
        ).optional().map_err(Into::into)
    }

    /// SEC-04: exact journal-scoped operator rows. Historical projection,
    /// never authorization or automatic resume. Reviewed callers verify
    /// original (chat,turn) binding; global callers pass an exact chat.
    pub fn security_recovery_rows(
        &self,workspace:Option<&str>,limit:usize,
    )->Result<(Vec<SecurityRecoveryRow>,bool),TaskStoreError>{
        if limit==0||limit>64||workspace.is_some_and(|w|
            w.len()<3||w.len()>128||!w.starts_with("c-")
            ||!w.bytes().all(|x|x.is_ascii_alphanumeric()||x==b'-'||x==b'_')){
            return Err(TaskStoreError::InvalidTaskId)
        }
        let connection=self.connection.lock()
            .map_err(|_|TaskStoreError::Poisoned)?;
        let mut stmt=connection.prepare(
            "SELECT t.task_id,t.state,u.reason FROM lu_tasks t
             LEFT JOIN lu_task_custody_uncertainty u ON u.task_id=t.task_id
             WHERE (?1 IS NULL OR t.workspace_id=?1)
               AND (t.state='INTERRUPTED' OR u.task_id IS NOT NULL)
             ORDER BY t.updated_at DESC,t.task_id ASC LIMIT ?2"
        )?;
        let rows=stmt.query_map(params![workspace,(limit+1) as i64],|r|{
            let state:String=r.get(1)?;
            Ok((r.get::<_,String>(0)?,state,r.get::<_,Option<String>>(2)?))
        })?;
        let mut result=Vec::new();
        for row in rows{
            let (task_id,state,reason)=row?;
            result.push(SecurityRecoveryRow{
                task_id,state:TaskState::parse(&state)
                    .ok_or(TaskStoreError::InvalidStoredState)?,
                custody_reason:reason,
            });
        }
        let more=result.len()>limit;
        result.truncate(limit);
        Ok((result,more))
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
             WHERE task_id=?1 AND state IN ('PENDING','RUNNING','PAUSED')
               AND NOT EXISTS(SELECT 1 FROM lu_task_custody_uncertainty
                              WHERE task_id=?1)",
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
        if matches!(to,TaskState::Running|TaskState::Completed|TaskState::Cancelled|TaskState::TimedOut){
            let uncertain:i64=tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM lu_task_custody_uncertainty
                 WHERE task_id=?1)",params![task_id],|row|row.get(0)
            )?;
            if uncertain!=0{return Err(TaskStoreError::InvalidTransition)}
        }
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
             WHERE state='RUNNING' OR (state='PENDING' AND EXISTS(
                 SELECT 1 FROM lu_task_custody_uncertainty u
                 WHERE u.task_id=lu_tasks.task_id))",[],
        )?;
        let changed=tx.execute(
            "UPDATE lu_tasks SET state='INTERRUPTED',updated_at=CURRENT_TIMESTAMP
             WHERE state='RUNNING' OR (state='PENDING' AND EXISTS(
                 SELECT 1 FROM lu_task_custody_uncertainty u
                 WHERE u.task_id=lu_tasks.task_id))",[],
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

    /// Durable provenance latch: an Observe session itself marks its
    /// conversation LOCAL_ONLY even after Stop/Forget and across restart.
    /// Ordinary user content in another conversation is not auto-tainted.
    /// A DB error at any host egress caller must DENY rather than allow.
    pub fn has_observe_provenance(&self,workspace_id:&str)->Result<bool,TaskStoreError>{
        if !workspace_id.starts_with("c-")||workspace_id.len()>128||
           !workspace_id.bytes().all(|c|c.is_ascii_alphanumeric()||c==b'-'||c==b'_'){
            return Err(TaskStoreError::InvalidTaskId);
        }
        let connection=self.connection.lock().map_err(|_|TaskStoreError::Poisoned)?;
        let found:i64=connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM lu_tasks WHERE workspace_id=?1 AND substr(task_id,1,4)='obs-')",
            params![workspace_id],|row|row.get(0))?;
        Ok(found!=0)
    }

    /// Deny Observation activation when an existing research worker is
    /// already alive. Network authorization can never overlap the moment
    /// local-only provenance begins without review of the in-flight task.
    pub fn active_research_network_tasks(&self)->Result<bool,TaskStoreError>{
        let connection=self.connection.lock().map_err(|_|TaskStoreError::Poisoned)?;
        let count:i64=connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM lu_tasks WHERE substr(task_id,1,9)='research-' AND state IN ('PENDING','RUNNING','PAUSED'))",
            [],|row|row.get(0))?;
        Ok(count!=0)
    }

    /// Persist enum-only notation in the EXISTING task SQLite database.
    /// Trusted ObserveHost owns task ID/sequence; no raw UI text, pixels,
    /// model output, commands, file paths or prompt bodies enter this table.
    pub fn append_awareness_note(
        &self,task_id:&str,workspace_id:&str,note:&Note
    )->Result<(),TaskStoreError>{
        if !task_id.starts_with("obs-")||task_id.len()>128
            ||!workspace_id.starts_with("c-")||workspace_id.len()>128
            ||!workspace_id.bytes().all(|x|
                x.is_ascii_alphanumeric()||x==b'-'||x==b'_')
            ||note.sequence==0||note.sequence>i64::MAX as u64{
            return Err(TaskStoreError::InvalidTaskId);
        }
        let connection=self.connection.lock()
            .map_err(|_|TaskStoreError::Poisoned)?;
        let changed=connection.execute(
            "INSERT OR IGNORE INTO lu_awareness_notes
              (task_id,workspace_id,sequence,code,source,priority)
             SELECT task_id,workspace_id,?3,?4,?5,?6
               FROM lu_tasks WHERE task_id=?1 AND workspace_id=?2 AND state='RUNNING'",
            params![task_id,workspace_id,note.sequence as i64,
                note.kind.as_str(),note.source.as_str(),note.priority.as_str()],
        )?;
        if changed!=1{return Err(TaskStoreError::InvalidTransition);}
        Ok(())
    }
    /// P06 consented operator privacy control: forget ALL historical
    /// enum-only Observe notes for one locally owned conversation. Task
    /// execution receipts remain separate and are never erased here.
    /// No model, tool, file, capture or ANSF record is modified.
    pub fn delete_awareness_notes_for_workspace(
        &self,workspace_id:&str
    )->Result<usize,TaskStoreError>{
        if !workspace_id.starts_with("c-")||workspace_id.len()>128
            ||!workspace_id.bytes().all(|b|
                b.is_ascii_alphanumeric()||b==b'-'||b==b'_'){
            return Err(TaskStoreError::InvalidTaskId);
        }
        let connection=self.connection.lock()
            .map_err(|_|TaskStoreError::Poisoned)?;
        Ok(connection.execute(
            "DELETE FROM lu_awareness_notes
             WHERE workspace_id=?1 AND task_id LIKE 'obs-%'",
            params![workspace_id]
        )?)
    }
    /// Host-selected retention sweep is independent of the current renderer
    /// and cannot be asked to retain data indefinitely by model text.
    pub fn prune_expired_awareness_notes(&self)->Result<usize,TaskStoreError>{
        let connection=self.connection.lock()
            .map_err(|_|TaskStoreError::Poisoned)?;
        Ok(connection.execute(
            "DELETE FROM lu_awareness_notes
             WHERE created_at < datetime('now','-7 days')",[]
        )?)
    }
    /// Read only the exact conversation/task's native notations. No status
    /// query may expose another chat or an arbitrary path or checkpoint.
    pub fn awareness_notes(
        &self,task_id:&str,workspace_id:&str,limit:usize
    )->Result<Vec<AwarenessNoteRow>,TaskStoreError>{
        if !task_id.starts_with("obs-")||task_id.len()>128
          ||!workspace_id.starts_with("c-")||workspace_id.len()>128
          ||!workspace_id.bytes().all(|x|
              x.is_ascii_alphanumeric()||x==b'-'||x==b'_')
          ||limit==0||limit>64{
            return Err(TaskStoreError::InvalidTaskId);
        }
        let connection=self.connection.lock()
            .map_err(|_|TaskStoreError::Poisoned)?;
        let mut stmt=connection.prepare(
            "SELECT task_id,workspace_id,sequence,code,source,priority,created_at,
                    data_origin,network_egress_allowed
             FROM lu_awareness_notes
             WHERE task_id=?1 AND workspace_id=?2
             ORDER BY sequence DESC LIMIT ?3"
        )?;
        let rows=stmt.query_map(params![task_id,workspace_id,limit as i64],|row|{
            Ok(AwarenessNoteRow{
                task_id:row.get(0)?,workspace_id:row.get(1)?,
                sequence:row.get::<_,i64>(2)? as u64,
                code:row.get(3)?,source:row.get(4)?,
                priority:row.get(5)?,created_at:row.get(6)?,
                data_origin:row.get(7)?,
                network_egress_allowed:row.get::<_,i64>(8)?!=0,
            })
        })?;
        let mut notes=Vec::new();
        for row in rows{notes.push(row?);}
        Ok(notes)
    }

    /// Host-only chat-scoped status: filter before limiting, not after.
    pub fn dashboard_for_workspace(&self,workspace_id:&str,limit:usize)
        ->Result<Vec<TaskDashboardEntry>,TaskStoreError>{
        if !workspace_id.starts_with("c-")||workspace_id.len()>128||
            !workspace_id.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-'||b==b'_')||
            limit==0||limit>100{
            return Err(TaskStoreError::InvalidTaskId);
        }
        let connection=self.connection.lock().map_err(|_|TaskStoreError::Poisoned)?;
        let mut stmt=connection.prepare(
            "SELECT task_id,workspace_id,state,progress,detail,last_heartbeat,updated_at
             FROM lu_tasks WHERE workspace_id=?1
             ORDER BY CASE state
               WHEN 'RUNNING' THEN 0 WHEN 'PENDING' THEN 1
               WHEN 'PAUSED' THEN 2 WHEN 'INTERRUPTED' THEN 3
               WHEN 'FAILED' THEN 4 WHEN 'CANCELLED' THEN 5
               WHEN 'TIMED_OUT' THEN 6 ELSE 7 END,
               updated_at DESC,task_id ASC LIMIT ?2"
        )?;
        let rows=stmt.query_map(params![workspace_id,limit as i64],|row|{
            Ok((row.get::<_,String>(0)?,row.get::<_,Option<String>>(1)?,
                row.get::<_,String>(2)?,row.get::<_,u8>(3)?,
                row.get::<_,String>(4)?,row.get::<_,Option<String>>(5)?,
                row.get::<_,String>(6)?))
        })?;
        let mut result=Vec::new();
        for row in rows{
            let (task_id,scope,state,progress,detail,last_heartbeat,updated_at)=row?;
            result.push(TaskDashboardEntry{
                task_id,workspace_id:scope,
                state:TaskState::parse(&state).ok_or(TaskStoreError::InvalidStoredState)?,
                progress,detail:detail.chars().take(2048).collect(),
                last_heartbeat,updated_at,
            });
        }
        Ok(result)
    }

    /// Only the trusted, owned-process supervisor calls this after successful
    /// worker exit and a complete, authenticated protocol transcript. The
    /// SQLite transaction writes the result and COMPLETED receipt together;
    /// a crash cannot publish one without the other.
    pub fn complete_scan_with_receipt(
        &self,task_id:&str,digest:&str,bytes:u64
    )->Result<(),TaskStoreError>{
        if bytes==0||bytes>i64::MAX as u64||digest.len()!=64||
            !digest.bytes().all(|b|b.is_ascii_hexdigit()&&!b.is_ascii_uppercase()){
            return Err(TaskStoreError::InvalidStoredState)
        }
        let expected=format!("SHA-256: {digest}");
        let mut conn=self.connection.lock().map_err(|_|TaskStoreError::Poisoned)?;
        let tx=conn.transaction()?;
        let changed=tx.execute(
            "UPDATE lu_tasks SET state='COMPLETED',updated_at=CURRENT_TIMESTAMP
             WHERE task_id=?1 AND state='RUNNING' AND progress=100 AND detail=?2
               AND NOT EXISTS(SELECT 1 FROM lu_task_custody_uncertainty
                              WHERE task_id=?1)",
            params![task_id,expected]
        )?;
        if changed!=1{return Err(TaskStoreError::InvalidTransition)}
        tx.execute(
            "INSERT INTO lu_task_verified_results(task_id,kind,sha256,byte_count)
             VALUES(?1,'sha256_scan',?2,?3)",
            params![task_id,digest,bytes as i64]
        )?;
        tx.execute(
            "INSERT INTO lu_task_events(task_id,event,progress,detail)
             SELECT task_id,'COMPLETED',progress,detail FROM lu_tasks WHERE task_id=?1",
            params![task_id]
        )?;
        tx.commit()?;
        Ok(())
    }
    /// An owned and completed task may expose a bounded digest receipt,
    /// never a source file path, an output artifact, or raw checkpoint bytes.
    pub fn verified_result(&self,task_id:&str,workspace:&str)
        ->Result<Option<TaskVerifiedResult>,TaskStoreError>{
        if task_id.is_empty()||task_id.len()>128||workspace.is_empty()
            ||workspace.len()>128||!task_id.bytes().all(|x|
                x.is_ascii_alphanumeric()||x==b'-'||x==b'_')
            ||!workspace.bytes().all(|x|
                x.is_ascii_alphanumeric()||x==b'-'||x==b'_'){
            return Err(TaskStoreError::InvalidTaskId)
        }
        let conn=self.connection.lock().map_err(|_|TaskStoreError::Poisoned)?;
        conn.query_row(
            "SELECT r.kind,r.sha256,r.byte_count FROM lu_task_verified_results r
             JOIN lu_tasks t ON t.task_id=r.task_id
             WHERE r.task_id=?1 AND t.workspace_id=?2 AND t.state='COMPLETED'",
            params![task_id,workspace],|row|{
                Ok(TaskVerifiedResult{
                    kind:row.get(0)?,sha256:row.get(1)?,
                    bytes:row.get::<_,i64>(2)? as u64,
                })
            }
        ).optional().map_err(Into::into)
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
    #[test]
    fn continuation_consent_survives_restart_and_revoke(){
        let path=std::env::temp_dir().join(format!(
            "lu-autoresume-{}-{:?}.sqlite",
            std::process::id(),std::thread::current().id()
        ));
        let _=std::fs::remove_file(&path);
        {
            let store=TaskStore::open(&path).unwrap();
            assert!(!store.auto_continue_enabled("c-abc").unwrap());
            store.set_auto_continue("c-abc",true).unwrap();
        }
        {
            let store=TaskStore::open(&path).unwrap();
            assert!(store.auto_continue_enabled("c-abc").unwrap());
            assert!(!store.auto_continue_enabled("c-other").unwrap());
            store.set_auto_continue("c-abc",false).unwrap();
        }
        {
            let store=TaskStore::open(&path).unwrap();
            assert!(!store.auto_continue_enabled("c-abc").unwrap());
        }
        let _=std::fs::remove_file(&path);
        let _=std::fs::remove_file(format!("{}-wal",path.display()));
        let _=std::fs::remove_file(format!("{}-shm",path.display()));
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
    fn chat_scoped_query_filters_before_limit(){
        let store=TaskStore::open(Path::new(":memory:")).unwrap();
        for n in 0..30{
            let id=format!("foreign-{n}");
            store.create(&id,Some("c-other"),None).unwrap();
            store.start(&id).unwrap();
        }
        store.create("owned",Some("c-owner"),Some("/secret")).unwrap();
        store.start("owned").unwrap();
        assert_eq!(store.dashboard_for_workspace("c-owner",1).unwrap()[0].task_id,"owned");
        assert_eq!(store.dashboard_for_workspace("c-other",2).unwrap().len(),2);
        assert!(store.dashboard_for_workspace("local-media",2).is_err());
        assert!(store.dashboard_for_workspace("c-owner",0).is_err());
    }
    #[test]
    fn existing_active_research_is_reported_before_observe_consent(){
        let store=TaskStore::open(Path::new(":memory:")).unwrap();
        assert!(!store.active_research_network_tasks().unwrap());
        store.create("research-fetch",Some("c-work"),None).unwrap();
        assert!(store.active_research_network_tasks().unwrap());
        store.start("research-fetch").unwrap();
        store.cancelled("research-fetch").unwrap();
        assert!(!store.active_research_network_tasks().unwrap());
    }
    #[test]
    fn observe_session_provenance_is_durable_after_stop_and_note_forget(){
        let store=TaskStore::open(Path::new(":memory:")).unwrap();
        store.create("job-ordinary",Some("c-clean"),None).unwrap();
        assert!(!store.has_observe_provenance("c-clean").unwrap());
        store.create("obs-privacy",Some("c-marked"),None).unwrap();
        store.start("obs-privacy").unwrap();
        store.cancelled("obs-privacy").unwrap();
        assert!(store.has_observe_provenance("c-marked").unwrap());
        assert!(!store.has_observe_provenance("c-clean").unwrap());
        store.delete_awareness_notes_for_workspace("c-marked").unwrap();
        assert!(store.has_observe_provenance("c-marked").unwrap());
        assert!(store.has_observe_provenance("c-other/../../").is_err());
    }
    #[test]
    fn awareness_notes_are_durable_chat_scoped_and_not_training_data(){
        use crate::native_awareness_lane::{Note,NoteKind,Source,Priority};
        let store=TaskStore::open(Path::new(":memory:")).unwrap();
        store.create("obs-first",Some("c-one"),None).unwrap();
        store.create("obs-second",Some("c-two"),None).unwrap();
        store.start("obs-first").unwrap();
        store.start("obs-second").unwrap();
        let note=Note{
            sequence:1,kind:NoteKind::ErrorIndicator,
            source:Source::RendererHint,priority:Priority::Observe,
            verified_failure:false,
        };
        store.append_awareness_note("obs-first","c-one",&note).unwrap();
        assert_eq!(store.awareness_notes("obs-first","c-one",8).unwrap().len(),1);
        assert!(store.awareness_notes("obs-first","c-two",8).unwrap().is_empty());
        assert_eq!(store.append_awareness_note("obs-first","c-two",&note),
            Err(TaskStoreError::InvalidTransition));
        assert_eq!(store.append_awareness_note("obs-first","c-one",&note),
            Err(TaskStoreError::InvalidTransition));
        assert_eq!(store.awareness_notes("obs-first","c-one",0),
            Err(TaskStoreError::InvalidTaskId));
        store.cancelled("obs-first").unwrap();
        assert_eq!(store.append_awareness_note("obs-first","c-one",&Note{
            sequence:2,..note
        }),Err(TaskStoreError::InvalidTransition));
        // No ANSF write is performed by this note API.
    }
    #[test]
    fn p06_native_runtime_notes_migrate_and_private_deletion_is_chat_scoped(){
        use crate::native_awareness_lane::{Note,NoteKind,Source,Priority};
        let tmp=std::env::temp_dir().join(format!(
            "lu-p06-note-migration-{}-{:?}.db",std::process::id(),
            std::thread::current().id()
        ));
        let _=std::fs::remove_file(&tmp);
        {
            let old=Connection::open(&tmp).unwrap();
            old.execute_batch("CREATE TABLE lu_awareness_notes(
              task_id TEXT NOT NULL,workspace_id TEXT NOT NULL,
              sequence INTEGER NOT NULL CHECK(sequence>0),
              code TEXT NOT NULL CHECK(code IN
               ('error_indicator','unexpected_ui_state','verified_test_failure','stalled_progress')),
              source TEXT NOT NULL CHECK(source IN
               ('renderer_hint','native_accessibility','native_test_receipt')),
              priority TEXT NOT NULL CHECK(priority IN ('observe','investigate')),
              created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
              PRIMARY KEY(task_id,sequence));
              INSERT INTO lu_awareness_notes(task_id,workspace_id,sequence,code,source,priority)
              VALUES ('obs-legacy','c-one',1,'error_indicator','renderer_hint','observe');"
            ).unwrap();
        }
        let store=TaskStore::open(&tmp).unwrap();
        assert_eq!(store.awareness_notes("obs-legacy","c-one",8).unwrap().len(),1);
        store.create("obs-now",Some("c-one"),None).unwrap();
        store.create("obs-other",Some("c-two"),None).unwrap();
        store.start("obs-now").unwrap();
        store.start("obs-other").unwrap();
        let note=Note{sequence:1,kind:NoteKind::ErrorIndicator,
            source:Source::NativeRuntime,priority:Priority::Observe,
            verified_failure:false};
        store.append_awareness_note("obs-now","c-one",&note).unwrap();
        store.append_awareness_note("obs-other","c-two",&note).unwrap();
        let migrated=store.awareness_notes("obs-legacy","c-one",8).unwrap();
        assert_eq!(migrated[0].data_origin,"observe_learn");
        assert!(!migrated[0].network_egress_allowed);
        assert_eq!(store.awareness_notes("obs-now","c-one",8).unwrap()[0].source,
            "native_runtime");
        assert_eq!(store.delete_awareness_notes_for_workspace("c-one").unwrap(),2);
        assert!(store.awareness_notes("obs-now","c-one",8).unwrap().is_empty());
        assert_eq!(store.awareness_notes("obs-other","c-two",8).unwrap().len(),1);
        assert_eq!(store.delete_awareness_notes_for_workspace("c-one/../c-two"),
            Err(TaskStoreError::InvalidTaskId));
        drop(store);
        let _=std::fs::remove_file(tmp);
    }
    #[test]
    fn p06_note_retention_prunes_stale_only(){
        let store=TaskStore::open(Path::new(":memory:")).unwrap();
        store.create("obs-expired",Some("c-note"),None).unwrap();
        store.start("obs-expired").unwrap();
        {
            let db=store.connection.lock().unwrap();
            db.execute(
                "INSERT INTO lu_awareness_notes
                 (task_id,workspace_id,sequence,code,source,priority,created_at)
                 VALUES ('obs-expired','c-note',1,'error_indicator','renderer_hint','observe',
                         datetime('now','-9 days'))",[]
            ).unwrap();
            db.execute(
                "INSERT INTO lu_awareness_notes
                 (task_id,workspace_id,sequence,code,source,priority)
                 VALUES ('obs-expired','c-note',2,'error_indicator','native_runtime','observe')",[]
            ).unwrap();
        }
        assert_eq!(store.prune_expired_awareness_notes().unwrap(),1);
        assert_eq!(store.awareness_notes("obs-expired","c-note",8).unwrap().len(),1);
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

#[cfg(test)]
mod p02_verified_result_tests {
    use super::*;
    #[test]
    fn scan_result_and_terminal_notice_are_atomic_and_owner_scoped(){
        let store=TaskStore::open(Path::new(":memory:")).unwrap();
        store.create("scan-s1",Some("c-owner"),None).unwrap();
        store.start("scan-s1").unwrap();
        let digest="a".repeat(64);
        assert!(store.complete_scan_with_receipt("scan-s1",&digest,100).is_err());
        store.checkpoint("scan-s1",100,&format!("SHA-256: {digest}"),&[]).unwrap();
        store.complete_scan_with_receipt("scan-s1",&digest,100).unwrap();
        let r=store.verified_result("scan-s1","c-owner").unwrap().unwrap();
        assert_eq!(r.kind,"sha256_scan");
        assert_eq!(r.bytes,100);
        assert_eq!(r.sha256,digest);
        assert!(store.verified_result("scan-s1","c-other").unwrap().is_none());
        assert!(store.complete_scan_with_receipt("scan-s1",&digest,100).is_err());
        let notices=store.pending_terminal_notices(10).unwrap();
        assert_eq!(notices.len(),1);
    }
    #[test]
    fn invalid_digest_cannot_publish_output_proof(){
        let store=TaskStore::open(Path::new(":memory:")).unwrap();
        store.create("scan-s2",Some("c-owner"),None).unwrap();
        store.start("scan-s2").unwrap();
        store.checkpoint("scan-s2",100,"SHA-256: invalid",&[]).unwrap();
        assert!(store.complete_scan_with_receipt("scan-s2","0",42).is_err());
        assert!(store.verified_result("scan-s2","c-owner").unwrap().is_none());
    }
}


#[cfg(test)]
mod e1_p15_origin_binding_tests{
    use super::*;
    #[test]
    fn reviewed_origin_is_durable_idempotent_and_cannot_be_reassigned(){
        let file=std::env::temp_dir().join(format!(
            "lu-e1-p15-origin-{}-{}.sqlite",std::process::id(),
            std::time::SystemTime::now().duration_since(
                std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        let a=format!("c-{:032x}",11);
        let b=format!("c-{:032x}",12);
        let turn=format!("g-{:032x}",21);
        {
            let store=TaskStore::open(&file).unwrap();
            assert!(store.bind_reviewed_human_turn("c-not-a-chat",&turn).is_err());
            store.bind_reviewed_human_turn(&a,&turn).unwrap();
            store.bind_reviewed_human_turn(&a,&turn).unwrap();
            assert_eq!(store.bind_reviewed_human_turn(&b,&turn),
                Err(TaskStoreError::InvalidTransition));
        }
        {
            let reopened=TaskStore::open(&file).unwrap();
            reopened.bind_reviewed_human_turn(&a,&turn).unwrap();
            assert!(reopened.bind_reviewed_human_turn(&a,
                &format!("g-{:032x}",22)).is_err());
        }
        let _=std::fs::remove_file(&file);
        let _=std::fs::remove_file(format!("{}-wal",file.display()));
        let _=std::fs::remove_file(format!("{}-shm",file.display()));
    }
}

#[cfg(test)]
mod p0_sec_custody_uncertainty_tests{
    use super::*;
    #[test]
    fn uncertainty_is_immutable_durable_and_blocks_false_terminal_success(){
        let path=std::env::temp_dir().join(format!(
            "lu-p0-sec-{}-{}.sqlite",std::process::id(),
            std::time::SystemTime::now().duration_since(
                std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        {
            let store=TaskStore::open(&path).unwrap();
            store.create("sec-1",Some("c-owner"),None).unwrap();
            store.start("sec-1").unwrap();
            assert!(store.record_process_uncertain("sec-1","MODEL_CLAIM").is_err());
            store.record_process_uncertain("sec-1","LINUX_GUARDIAN_LOST").unwrap();
            store.record_process_uncertain("sec-1","OS_CUSTODY_UNVERIFIED").unwrap();
            assert_eq!(store.process_uncertainty("sec-1").unwrap().as_deref(),
                Some("LINUX_GUARDIAN_LOST"));
            assert!(store.complete("sec-1").is_err());
            assert!(store.cancelled("sec-1").is_err());
            assert!(store.timed_out("sec-1").is_err());
        }
        {
            let store=TaskStore::open(&path).unwrap();
            assert_eq!(store.process_uncertainty("sec-1").unwrap().as_deref(),
                Some("LINUX_GUARDIAN_LOST"));
            assert_eq!(store.reconcile_interrupted().unwrap(),1);
            assert_eq!(store.get("sec-1").unwrap().unwrap().state,
                TaskState::Interrupted);
            assert!(store.complete("sec-1").is_err());
        }
        let _=std::fs::remove_file(&path);
        let _=std::fs::remove_file(format!("{}-wal",path.display()));
        let _=std::fs::remove_file(format!("{}-shm",path.display()));
    }
}
