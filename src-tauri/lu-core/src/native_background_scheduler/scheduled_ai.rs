//! Human-approved scheduled inference and bounded immutable-note Reader grants.
use super::*;
use serde::Serialize;
use sha2::{Digest, Sha256};
const MAX_SCHEDULES: i64 = 64;
#[cfg(test)]
const MAX_RUNS: i64 = 32;
pub const MAX_SCHEDULE_OUTPUT: usize = 16384;
const MAX_INTERVAL: u64 = 604800;
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSchedule {
    pub id: String,
    pub workspace_id: String,
    pub prompt: String,
    pub model_ref: String,
    pub state: String,
    pub next_due: u64,
    pub repeat_seconds: Option<u64>,
    pub run_count: u64,
    pub running: bool,
    pub reader_line: Option<u32>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiRun {
    pub schedule_id: String,
    pub sequence: u64,
    pub due: u64,
    pub started: u64,
    pub finished: Option<u64>,
    pub state: String,
    pub output: String,
    pub reader_receipt: Option<ReaderReceipt>,
}
fn decode_schedule(r: &rusqlite::Row<'_>) -> rusqlite::Result<AiSchedule> {
    Ok(AiSchedule {
        id: r.get(0)?,
        workspace_id: r.get(1)?,
        prompt: r.get(2)?,
        model_ref: r.get(3)?,
        state: r.get(4)?,
        next_due: r.get(5)?,
        repeat_seconds: r.get(6)?,
        run_count: r.get(7)?,
        running: r.get(8)?,
        reader_line: r.get(9)?,
    })
}
/// Only literal user-approved text is admitted. No path, URL, model-selected
/// argument, historical grant or mutable external resource is accepted.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReaderReceipt {
    pub source_sha256: String,
    pub line: u32,
    pub verified_text: String,
}
fn note_line(note: &str, line: u32) -> Result<&str, SchedulerError> {
    if note.is_empty()
        || note.len() > 2048
        || line == 0
        || line > 32
        || note.split('\n').count() > 32
        || crate::native_reader::classify("approved-note.txt", note.as_bytes())
            != crate::native_reader::ReaderFormat::PlainText
        || note.starts_with('\u{feff}')
        || note.chars().any(|c| {
            (c.is_control() && c != '\n')
                || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        })
    {
        return Err(SchedulerError::InvalidRequest);
    }
    let text = note
        .split('\n')
        .nth((line - 1) as usize)
        .ok_or(SchedulerError::InvalidRequest)?;
    if text.is_empty() || text != text.trim() || text.len() > 160 {
        return Err(SchedulerError::InvalidRequest);
    }
    Ok(text)
}
pub fn validate_ai_reader(note: &str, line: u32) -> Result<(), SchedulerError> {
    note_line(note, line).map(|_| ())
}
// A digest binds the immutable scope against accidental record drift. This is
// not a MAC and does not authenticate a hostile local profile.
fn grant_digest(
    id: &str,
    owner: &str,
    model: &str,
    prompt: &str,
    note: &str,
    line: u32,
    repeat: Option<u64>,
) -> String {
    let mut digest = Sha256::new();
    digest.update(b"lu-approved-note-v1");
    for field in [id, owner, model, prompt, note] {
        digest.update((field.len() as u64).to_le_bytes());
        digest.update(field.as_bytes());
    }
    digest.update(line.to_le_bytes());
    digest.update(repeat.unwrap_or(0).to_le_bytes());
    format!("{:x}", digest.finalize())
}
fn verify_reader(note: &str, line: u32) -> Result<ReaderReceipt, SchedulerError> {
    let expected = note_line(note, line)?;
    let offset = note
        .split('\n')
        .take((line - 1) as usize)
        .map(|s| s.chars().count() + 1)
        .sum();
    let window = crate::native_reader::read_plain_text_window(
        "approved-note.txt",
        note.as_bytes(),
        offset,
        expected.chars().count(),
        false,
    )
    .map_err(|_| SchedulerError::InvalidRequest)?
    .ok_or(SchedulerError::InvalidRequest)?;
    if window.text.as_bytes() != expected.as_bytes() || window.total_chars != note.chars().count() {
        return Err(SchedulerError::InvalidRequest);
    }
    Ok(ReaderReceipt {
        source_sha256: format!("{:x}", Sha256::digest(note.as_bytes())),
        line,
        verified_text: window.text,
    })
}
pub(super) fn initialize(db: &Connection) -> Result<(), SchedulerError> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS lu_ai_schedules(id TEXT PRIMARY KEY,workspace_id TEXT NOT NULL,prompt TEXT NOT NULL,model_ref TEXT NOT NULL,state TEXT NOT NULL CHECK(state IN ('ACTIVE','PAUSED','COMPLETED','CANCELLED')),next_due INTEGER NOT NULL,repeat_seconds INTEGER,run_count INTEGER NOT NULL DEFAULT 0 CHECK(run_count BETWEEN 0 AND 32));
      CREATE INDEX IF NOT EXISTS lu_ai_due ON lu_ai_schedules(state,model_ref,next_due);
      CREATE TABLE IF NOT EXISTS lu_ai_runs(schedule_id TEXT NOT NULL,sequence INTEGER NOT NULL,due INTEGER NOT NULL,started INTEGER NOT NULL,finished INTEGER,state TEXT NOT NULL CHECK(state IN ('RUNNING','COMPLETED','FAILED','STOPPED','INTERRUPTED')),output TEXT NOT NULL DEFAULT '',PRIMARY KEY(schedule_id,sequence));
      CREATE UNIQUE INDEX IF NOT EXISTS lu_ai_one_running ON lu_ai_runs(state) WHERE state='RUNNING';
      CREATE TABLE IF NOT EXISTS lu_ai_reader_grants(schedule_id TEXT PRIMARY KEY, note TEXT NOT NULL,line INTEGER NOT NULL,sha256 TEXT NOT NULL,scope_sha256 TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS lu_ai_reader_receipts(schedule_id TEXT NOT NULL,sequence INTEGER NOT NULL,sha256 TEXT NOT NULL,line INTEGER NOT NULL,verified_text TEXT NOT NULL,PRIMARY KEY(schedule_id,sequence));").map_err(db_error)?;
    // A claimed occurrence is never replayed. Its cadence was already advanced
    // atomically at claim. The next future occurrence still has original consent.
    db.execute_batch("BEGIN IMMEDIATE;
      UPDATE lu_ai_schedules SET state='COMPLETED' WHERE state='ACTIVE' AND (repeat_seconds IS NULL OR run_count>=32) AND id IN (SELECT schedule_id FROM lu_ai_runs WHERE state='RUNNING');
      UPDATE lu_ai_runs SET state='INTERRUPTED',finished=MAX(started,CAST(strftime('%s','now') AS INTEGER)),output='' WHERE state='RUNNING';
      COMMIT;").map_err(db_error)
}
pub fn validate_ai_schedule(
    prompt: &str,
    model: &str,
    delay: u64,
    repeat: Option<u64>,
) -> Result<(), SchedulerError> {
    reminders::validate_reminder(prompt, delay, repeat)?;
    if !valid_id(model) || !model.starts_with("m-") {
        return Err(SchedulerError::InvalidRequest);
    }
    Ok(())
}
impl BackgroundScheduler {
    pub fn create_ai_schedule(
        &self,
        id: &str,
        owner: &str,
        prompt: &str,
        model: &str,
        delay: u64,
        repeat: Option<u64>,
    ) -> Result<(), SchedulerError> {
        self.create_ai_at(id, owner, prompt, model, delay, repeat, now_unix())
    }
    fn create_ai_at(
        &self,
        id: &str,
        owner: &str,
        prompt: &str,
        model: &str,
        delay: u64,
        repeat: Option<u64>,
        now: u64,
    ) -> Result<(), SchedulerError> {
        self.create_ai_with_grant_at(id, owner, prompt, model, delay, repeat, now, None)
    }
    pub fn create_ai_reader_schedule(
        &self,
        id: &str,
        owner: &str,
        prompt: &str,
        model: &str,
        delay: u64,
        repeat: Option<u64>,
        note: &str,
        line: u32,
    ) -> Result<(), SchedulerError> {
        self.create_ai_with_grant_at(
            id,
            owner,
            prompt,
            model,
            delay,
            repeat,
            now_unix(),
            Some((note, line)),
        )
    }
    fn create_ai_with_grant_at(
        &self,
        id: &str,
        owner: &str,
        prompt: &str,
        model: &str,
        delay: u64,
        repeat: Option<u64>,
        now: u64,
        grant: Option<(&str, u32)>,
    ) -> Result<(), SchedulerError> {
        validate_ai_schedule(prompt, model, delay, repeat)?;
        if let Some((note, line)) = grant {
            validate_ai_reader(note, line)?;
        }
        if !valid_id(id) || !valid_id(owner) || !owner.starts_with("c-") {
            return Err(SchedulerError::InvalidRequest);
        }
        let due = to_sql(
            now.checked_add(delay)
                .ok_or(SchedulerError::InvalidRequest)?,
        )?;
        let mut g = self.inner.lock().map_err(|_| SchedulerError::Poisoned)?;
        let tx =
            g.db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .map_err(db_error)?;
        let count: i64 = tx
            .query_row("SELECT COUNT(*) FROM lu_ai_schedules", [], |r| r.get(0))
            .map_err(db_error)?;
        if count >= MAX_SCHEDULES {
            return Err(SchedulerError::QueueFull);
        }
        tx.execute("INSERT INTO lu_ai_schedules(id,workspace_id,prompt,model_ref,state,next_due,repeat_seconds) VALUES(?1,?2,?3,?4,'ACTIVE',?5,?6)",params![id,owner,prompt,model,due,repeat.map(to_sql).transpose()?]).map_err(db_error)?;
        if let Some((note, line)) = grant {
            tx.execute(
                "INSERT INTO lu_ai_reader_grants(schedule_id,note,line,sha256,scope_sha256) VALUES(?1,?2,?3,?4,?5)",
                params![
                    id,
                    note,
                    line,
                    format!("{:x}", Sha256::digest(note.as_bytes())),
                    grant_digest(id,owner,model,prompt,note,line,repeat)
                ],
            )
            .map_err(db_error)?;
        }
        tx.commit().map_err(db_error)
    }
    /// Record the verified tool observation at most once, under the original
    /// owner/model and a still-running occurrence. No path or argument comes
    /// from inference; the only permitted request is the literal READ.
    pub fn execute_ai_reader(
        &self,
        id: &str,
        owner: &str,
        model: &str,
        sequence: u64,
        request: &str,
    ) -> Result<ReaderReceipt, SchedulerError> {
        if request.trim() != "READ" {
            return Err(SchedulerError::InvalidRequest);
        }
        let mut g = self.inner.lock().map_err(|_| SchedulerError::Poisoned)?;
        let tx =
            g.db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .map_err(db_error)?;
        let (note,line,hash,scope,prompt,repeat):(String,u32,String,String,String,Option<u64>)=tx.query_row("SELECT g.note,g.line,g.sha256,g.scope_sha256,s.prompt,s.repeat_seconds FROM lu_ai_reader_grants g JOIN lu_ai_schedules s ON s.id=g.schedule_id JOIN lu_ai_runs r ON r.schedule_id=s.id WHERE s.id=?1 AND s.workspace_id=?2 AND s.model_ref=?3 AND s.state='ACTIVE' AND r.sequence=?4 AND r.state='RUNNING' AND NOT EXISTS(SELECT 1 FROM lu_ai_reader_receipts t WHERE t.schedule_id=r.schedule_id AND t.sequence=r.sequence)",params![id,owner,model,to_sql(sequence)?],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).map_err(db_error)?;
        if hash != format!("{:x}", Sha256::digest(note.as_bytes()))
            || scope != grant_digest(id, owner, model, &prompt, &note, line, repeat)
        {
            return Err(SchedulerError::InvalidRequest);
        }
        let receipt = verify_reader(&note, line)?;
        if hash != receipt.source_sha256 {
            return Err(SchedulerError::InvalidRequest);
        }
        tx.execute("INSERT INTO lu_ai_reader_receipts(schedule_id,sequence,sha256,line,verified_text) VALUES(?1,?2,?3,?4,?5)",params![id,to_sql(sequence)?,receipt.source_sha256,receipt.line,receipt.verified_text]).map_err(db_error)?;
        tx.commit().map_err(db_error)?;
        Ok(receipt)
    }
    pub fn ai_schedules(&self) -> Result<Vec<AiSchedule>, SchedulerError> {
        let g = self.inner.lock().map_err(|_| SchedulerError::Poisoned)?;
        let mut q=g.db.prepare("SELECT id,workspace_id,prompt,model_ref,state,next_due,repeat_seconds,run_count,EXISTS(SELECT 1 FROM lu_ai_runs WHERE schedule_id=lu_ai_schedules.id AND state='RUNNING'),(SELECT line FROM lu_ai_reader_grants WHERE schedule_id=lu_ai_schedules.id) FROM lu_ai_schedules ORDER BY next_due,id").map_err(db_error)?;
        let rows = q.query_map([], decode_schedule).map_err(db_error)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(db_error)
    }
    pub fn ai_runs(&self, id: &str, owner: &str) -> Result<Vec<AiRun>, SchedulerError> {
        let g = self.inner.lock().map_err(|_| SchedulerError::Poisoned)?;
        let mut q=g.db.prepare("SELECT r.schedule_id,r.sequence,r.due,r.started,r.finished,r.state,r.output,t.sha256,t.line,t.verified_text FROM lu_ai_runs r LEFT JOIN lu_ai_reader_receipts t ON t.schedule_id=r.schedule_id AND t.sequence=r.sequence JOIN lu_ai_schedules s ON s.id=r.schedule_id WHERE s.id=?1 AND s.workspace_id=?2 ORDER BY r.sequence DESC LIMIT 32").map_err(db_error)?;
        let rows = q
            .query_map(params![id, owner], |r| {
                Ok(AiRun {
                    schedule_id: r.get(0)?,
                    sequence: r.get(1)?,
                    due: r.get(2)?,
                    started: r.get(3)?,
                    finished: r.get(4)?,
                    state: r.get(5)?,
                    output: r.get(6)?,
                    reader_receipt: match r.get::<_, Option<String>>(7)? {
                        Some(hash) => Some(ReaderReceipt {
                            source_sha256: hash,
                            line: r.get(8)?,
                            verified_text: r.get(9)?,
                        }),
                        None => None,
                    },
                })
            })
            .map_err(db_error)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(db_error)
    }
    /// Claim exactly one occurrence; coalesce overdue intervals to latest due.
    /// Caller holds the host inference reservation and rechecks model/owner.
    pub fn claim_ai_at(
        &self,
        id: &str,
        owner: &str,
        model: &str,
        now: u64,
    ) -> Result<Option<(AiSchedule, u64)>, SchedulerError> {
        let now = to_sql(now)?;
        if now > i64::MAX - MAX_INTERVAL as i64 {
            return Err(SchedulerError::InvalidRequest);
        }
        let mut g = self.inner.lock().map_err(|_| SchedulerError::Poisoned)?;
        if g.interactive_admission_hold {
            return Ok(None);
        }
        let tx =
            g.db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .map_err(db_error)?;
        if tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM lu_ai_runs WHERE state='RUNNING')",
                [],
                |r| r.get::<_, bool>(0),
            )
            .map_err(db_error)?
        {
            return Ok(None);
        }
        let row=tx.query_row("SELECT id,workspace_id,prompt,model_ref,state,next_due,repeat_seconds,run_count,EXISTS(SELECT 1 FROM lu_ai_runs WHERE schedule_id=lu_ai_schedules.id AND state='RUNNING'),(SELECT line FROM lu_ai_reader_grants WHERE schedule_id=lu_ai_schedules.id) FROM lu_ai_schedules WHERE id=?1 AND workspace_id=?2 AND model_ref=?3 AND state='ACTIVE' AND next_due<=?4 AND run_count<32",params![id,owner,model,now],decode_schedule).optional().map_err(db_error)?;
        let Some(row) = row else { return Ok(None) };
        validate_ai_schedule(&row.prompt, &row.model_ref, 60, row.repeat_seconds)?;
        let sequence = row.run_count + 1;
        let original = to_sql(row.next_due)?;
        let (due, next) = if let Some(r) = row.repeat_seconds {
            let r = to_sql(r)?;
            let latest = now - (now - original) % r;
            (latest, latest + r)
        } else {
            (original, original)
        };
        tx.execute("INSERT INTO lu_ai_runs(schedule_id,sequence,due,started,state) VALUES(?1,?2,?3,?4,'RUNNING')",params![id,to_sql(sequence)?,due,now]).map_err(db_error)?;
        tx.execute(
            "UPDATE lu_ai_schedules SET next_due=?2,run_count=?3 WHERE id=?1",
            params![id, next, to_sql(sequence)?],
        )
        .map_err(db_error)?;
        tx.commit().map_err(db_error)?;
        Ok(Some((row, sequence)))
    }
    pub fn ai_running(&self, id: &str, sequence: u64) -> Result<bool, SchedulerError> {
        let g = self.inner.lock().map_err(|_| SchedulerError::Poisoned)?;
        g.db.query_row("SELECT EXISTS(SELECT 1 FROM lu_ai_runs WHERE schedule_id=?1 AND sequence=?2 AND state='RUNNING')",params![id,to_sql(sequence)?],|r|r.get(0)).map_err(db_error)
    }
    pub fn finish_ai(
        &self,
        id: &str,
        sequence: u64,
        state: &str,
        output: &str,
    ) -> Result<bool, SchedulerError> {
        if !matches!(state, "COMPLETED" | "FAILED" | "STOPPED" | "INTERRUPTED")
            || output.len() > MAX_SCHEDULE_OUTPUT
            || (state != "COMPLETED" && !output.is_empty())
            || (state == "COMPLETED" && output.trim().is_empty())
        {
            return Err(SchedulerError::InvalidRequest);
        }
        let mut g = self.inner.lock().map_err(|_| SchedulerError::Poisoned)?;
        let tx =
            g.db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .map_err(db_error)?;
        if state == "COMPLETED" {
            let has: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM lu_ai_reader_grants WHERE schedule_id=?1)",
                    [id],
                    |r| r.get(0),
                )
                .map_err(db_error)?;
            if has {
                let expected:Option<String>=tx.query_row("SELECT t.verified_text FROM lu_ai_reader_receipts t JOIN lu_ai_reader_grants g ON t.schedule_id=g.schedule_id AND t.sha256=g.sha256 AND t.line=g.line WHERE t.schedule_id=?1 AND t.sequence=?2",params![id,to_sql(sequence)?],|r|r.get(0)).optional().map_err(db_error)?;
                if expected.as_deref() != Some(output) {
                    return Err(SchedulerError::InvalidRequest);
                }
            }
        }
        let n=tx.execute("UPDATE lu_ai_runs SET state=?3,output=?4,finished=MAX(started,?5) WHERE schedule_id=?1 AND sequence=?2 AND state='RUNNING'",params![id,to_sql(sequence)?,state,output,to_sql(now_unix())?]).map_err(db_error)?;
        if n == 1 {
            tx.execute("UPDATE lu_ai_schedules SET state='COMPLETED' WHERE id=?1 AND state='ACTIVE' AND (repeat_seconds IS NULL OR run_count>=32)",[id]).map_err(db_error)?;
        }
        tx.commit().map_err(db_error)?;
        Ok(n == 1)
    }
    pub fn control_ai(&self, id: &str, owner: &str, action: &str) -> Result<(), SchedulerError> {
        let now = to_sql(now_unix())?;
        let mut g = self.inner.lock().map_err(|_| SchedulerError::Poisoned)?;
        let tx =
            g.db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .map_err(db_error)?;
        let n=match action{
            "pause"=>tx.execute("UPDATE lu_ai_schedules SET state='PAUSED' WHERE id=?1 AND workspace_id=?2 AND state='ACTIVE' AND NOT EXISTS(SELECT 1 FROM lu_ai_runs WHERE schedule_id=?1 AND state='RUNNING')",params![id,owner]),
            "resume"=>tx.execute("UPDATE lu_ai_schedules SET state='ACTIVE',next_due=CASE WHEN next_due>?3 THEN next_due ELSE ?3+COALESCE(repeat_seconds,60) END WHERE id=?1 AND workspace_id=?2 AND state='PAUSED'",params![id,owner,now]),
            "cancel"=>tx.execute("UPDATE lu_ai_schedules SET state='CANCELLED' WHERE id=?1 AND workspace_id=?2 AND state IN ('ACTIVE','PAUSED')",params![id,owner]),
            _=>return Err(SchedulerError::InvalidRequest),
        }.map_err(db_error)?;
        if n != 1 {
            return Err(SchedulerError::NotFound);
        }
        if action == "cancel" {
            tx.execute("UPDATE lu_ai_runs SET state='STOPPED',finished=MAX(started,?2),output='' WHERE schedule_id=?1 AND state='RUNNING'",params![id,now]).map_err(db_error)?;
        }
        tx.commit().map_err(db_error)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn open(path: &Path) -> BackgroundScheduler {
        let t = Arc::new(TaskStore::open(Path::new(":memory:")).unwrap());
        let p = Arc::new(BackgroundProcessSupervisor::new(t.clone()));
        BackgroundScheduler::open(path, t, p, ResourceBudget::interactive_first()).unwrap()
    }
    #[test]
    fn cp11br1_exact_claim_and_owner_model_binding() {
        let s = open(Path::new(":memory:"));
        s.create_ai_at("ai-1", "c-a", "Write a note", "m-a", 60, Some(60), 100)
            .unwrap();
        assert!(s.claim_ai_at("ai-1", "c-a", "m-a", 159).unwrap().is_none());
        assert!(s.claim_ai_at("ai-1", "c-b", "m-a", 160).unwrap().is_none());
        assert!(s.claim_ai_at("ai-1", "c-a", "m-b", 160).unwrap().is_none());
        let (_, seq) = s.claim_ai_at("ai-1", "c-a", "m-a", 300).unwrap().unwrap();
        assert_eq!(seq, 1);
        assert!(s.claim_ai_at("ai-1", "c-a", "m-a", 400).unwrap().is_none());
        assert!(s.finish_ai("ai-1", seq, "COMPLETED", "note").unwrap());
        assert!(!s.finish_ai("ai-1", seq, "COMPLETED", "duplicate").unwrap());
        assert!(s.ai_runs("ai-1", "c-b").unwrap().is_empty());
        let r = s.ai_runs("ai-1", "c-a").unwrap();
        assert_eq!((r[0].due, r[0].output.as_str()), (280, "note"));
        assert_eq!(s.ai_schedules().unwrap()[0].next_due, 340);
    }
    #[test]
    fn cp11br1_restart_no_replay_and_cancel_race() {
        let f = std::env::temp_dir().join(format!(
            "lu-ai-schedule-{}-{}.sqlite",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        {
            let s = open(&f);
            s.create_ai_at("ai-1", "c-a", "note", "m-a", 60, Some(60), 100)
                .unwrap();
            s.claim_ai_at("ai-1", "c-a", "m-a", 160).unwrap().unwrap();
        }
        {
            let s = open(&f);
            assert_eq!(s.ai_runs("ai-1", "c-a").unwrap()[0].state, "INTERRUPTED");
            assert!(s.claim_ai_at("ai-1", "c-a", "m-a", 160).unwrap().is_none());
            let (_, seq) = s.claim_ai_at("ai-1", "c-a", "m-a", 220).unwrap().unwrap();
            assert_eq!(seq, 2);
            assert!(s.control_ai("ai-1", "c-b", "cancel").is_err());
            s.control_ai("ai-1", "c-a", "cancel").unwrap();
            assert!(!s.finish_ai("ai-1", seq, "COMPLETED", "late").unwrap());
            assert_eq!(s.ai_runs("ai-1", "c-a").unwrap()[0].state, "STOPPED");
            assert!(s.claim_ai_at("ai-1", "c-a", "m-a", 500).unwrap().is_none());
        }
        std::fs::remove_file(f).unwrap();
    }
    #[test]
    fn cp11br1_one_time_and_bounds() {
        let s = open(Path::new(":memory:"));
        assert!(s
            .create_ai_at("id", "c-a", "bad\ntext", "m-a", 60, None, 100)
            .is_err());
        s.create_ai_at("ai-1", "c-a", "note", "m-a", 60, None, 100)
            .unwrap();
        let (_, seq) = s.claim_ai_at("ai-1", "c-a", "m-a", 160).unwrap().unwrap();
        assert!(s.control_ai("ai-1", "c-a", "pause").is_err());
        assert!(s.finish_ai("ai-1", seq, "FAILED", "unverified").is_err());
        s.finish_ai("ai-1", seq, "COMPLETED", "result").unwrap();
        assert_eq!(s.ai_schedules().unwrap()[0].state, "COMPLETED");
        assert!(s.claim_ai_at("ai-1", "c-a", "m-a", 500).unwrap().is_none());
    }
    #[test]
    fn cp11br1_recurring_retention_is_explicit_and_bounded() {
        let s = open(Path::new(":memory:"));
        s.create_ai_at("ai-1", "c-a", "note", "m-a", 60, Some(60), 100)
            .unwrap();
        for i in 1..=MAX_RUNS as u64 {
            let (_, seq) = s
                .claim_ai_at("ai-1", "c-a", "m-a", 100 + i * 60)
                .unwrap()
                .unwrap();
            assert_eq!(seq, i);
            s.finish_ai("ai-1", seq, "COMPLETED", "result").unwrap();
        }
        assert_eq!(s.ai_schedules().unwrap()[0].state, "COMPLETED");
        assert_eq!(s.ai_runs("ai-1", "c-a").unwrap().len(), 32);
        assert!(s
            .claim_ai_at("ai-1", "c-a", "m-a", 10000)
            .unwrap()
            .is_none());
    }
    #[test]
    fn cp11br1_pressure_and_pause_prevent_claim() {
        let s = open(Path::new(":memory:"));
        s.create_ai_at("ai-1", "c-a", "note", "m-a", 60, Some(60), 100)
            .unwrap();
        s.inner.lock().unwrap().interactive_admission_hold = true;
        assert!(s.claim_ai_at("ai-1", "c-a", "m-a", 160).unwrap().is_none());
        s.inner.lock().unwrap().interactive_admission_hold = false;
        s.control_ai("ai-1", "c-a", "pause").unwrap();
        assert!(s.claim_ai_at("ai-1", "c-a", "m-a", 160).unwrap().is_none());
        s.control_ai("ai-1", "c-a", "resume").unwrap();
        let due = s.ai_schedules().unwrap()[0].next_due;
        assert!(s
            .claim_ai_at("ai-1", "c-a", "m-a", due - 1)
            .unwrap()
            .is_none());
        assert!(s.claim_ai_at("ai-1", "c-a", "m-a", due).unwrap().is_some());
        assert!(s.ai_schedules().unwrap()[0].running);
    }
    #[test]
    fn cp11br1_concurrent_claim_single_occurrence() {
        let s = Arc::new(open(Path::new(":memory:")));
        s.create_ai_at("ai-1", "c-a", "note", "m-a", 60, Some(60), 100)
            .unwrap();
        let threads = (0..8)
            .map(|_| {
                let s = s.clone();
                std::thread::spawn(move || {
                    s.claim_ai_at("ai-1", "c-a", "m-a", 160).unwrap().is_some()
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(
            threads
                .into_iter()
                .map(|t| t.join().unwrap() as usize)
                .sum::<usize>(),
            1
        );
    }
    #[test]
    fn cp11br2_reader_scope_and_verified_result() {
        let s = open(Path::new(":memory:"));
        let note = "Unrelated\n雪-42\nOther";
        s.create_ai_with_grant_at(
            "ai-1",
            "c-a",
            "Retrieve code",
            "m-a",
            60,
            Some(60),
            100,
            Some((note, 2)),
        )
        .unwrap();
        assert_eq!(s.ai_schedules().unwrap()[0].reader_line, Some(2));
        assert!(s
            .execute_ai_reader("ai-1", "c-a", "m-a", 1, "READ")
            .is_err());
        let (_, seq) = s.claim_ai_at("ai-1", "c-a", "m-a", 160).unwrap().unwrap();
        assert!(s.finish_ai("ai-1", seq, "COMPLETED", "雪-42").is_err());
        for (owner, model, request) in [
            ("c-b", "m-a", "READ"),
            ("c-a", "m-b", "READ"),
            ("c-a", "m-a", "READ /other"),
            ("c-a", "m-a", "READ\nREAD"),
        ] {
            assert!(s
                .execute_ai_reader("ai-1", owner, model, seq, request)
                .is_err());
        }
        let r = s
            .execute_ai_reader("ai-1", "c-a", "m-a", seq, "READ")
            .unwrap();
        assert_eq!(r.verified_text, "雪-42");
        assert_eq!(
            r.source_sha256,
            format!("{:x}", Sha256::digest(note.as_bytes()))
        );
        assert!(s
            .execute_ai_reader("ai-1", "c-a", "m-a", seq, "READ")
            .is_err());
        assert!(s.finish_ai("ai-1", seq, "COMPLETED", "wrong").is_err());
        assert!(s.finish_ai("ai-1", seq, "COMPLETED", "雪-42").unwrap());
        assert!(s.ai_runs("ai-1", "c-b").unwrap().is_empty());
        assert_eq!(
            s.ai_runs("ai-1", "c-a").unwrap()[0]
                .reader_receipt
                .as_ref()
                .unwrap()
                .verified_text,
            "雪-42"
        );
    }
    #[test]
    fn cp11br2_no_legacy_authority_or_invalid_grants() {
        let s = open(Path::new(":memory:"));
        for (note, line) in [
            ("a", 0),
            ("a", 2),
            ("a\n", 2),
            ("\u{feff}a", 1),
            ("a\u{202e}b", 1),
            (" a", 1),
        ] {
            assert!(s
                .create_ai_with_grant_at(
                    "ai-1",
                    "c-a",
                    "note",
                    "m-a",
                    60,
                    None,
                    100,
                    Some((note, line))
                )
                .is_err());
        }
        assert!(verify_reader("%PDF-1.0", 1).is_err());
        assert!(s.ai_schedules().unwrap().is_empty());
        s.create_ai_at("ai-1", "c-a", "note", "m-a", 60, None, 100)
            .unwrap();
        let (_, seq) = s.claim_ai_at("ai-1", "c-a", "m-a", 160).unwrap().unwrap();
        assert!(s
            .execute_ai_reader("ai-1", "c-a", "m-a", seq, "READ")
            .is_err());
        assert!(s.finish_ai("ai-1", seq, "COMPLETED", "plain").unwrap());
        assert!(s.ai_runs("ai-1", "c-a").unwrap()[0]
            .reader_receipt
            .is_none());
    }
    #[test]
    fn cp11br2_recovery_completed_prefix_and_receipt() {
        let f = std::env::temp_dir().join(format!(
            "lu-ai-reader-{}-{}.sqlite",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        {
            let s = open(&f);
            s.create_ai_with_grant_at(
                "ai-1",
                "c-a",
                "note",
                "m-a",
                60,
                Some(60),
                100,
                Some(("A\nB", 2)),
            )
            .unwrap();
            s.claim_ai_at("ai-1", "c-a", "m-a", 160).unwrap().unwrap();
            s.execute_ai_reader("ai-1", "c-a", "m-a", 1, "READ")
                .unwrap();
            s.finish_ai("ai-1", 1, "COMPLETED", "B").unwrap();
            s.claim_ai_at("ai-1", "c-a", "m-a", 220).unwrap().unwrap();
            s.execute_ai_reader("ai-1", "c-a", "m-a", 2, "READ")
                .unwrap();
        }
        {
            let s = open(&f);
            let runs = s.ai_runs("ai-1", "c-a").unwrap();
            assert_eq!(
                (runs[0].state.as_str(), runs[0].output.as_str()),
                ("INTERRUPTED", "")
            );
            assert!(runs[0].reader_receipt.is_some());
            assert_eq!(
                (runs[1].state.as_str(), runs[1].output.as_str()),
                ("COMPLETED", "B")
            );
            assert!(s.claim_ai_at("ai-1", "c-a", "m-a", 220).unwrap().is_none());
            assert!(s
                .execute_ai_reader("ai-1", "c-a", "m-a", 2, "READ")
                .is_err());
            s.claim_ai_at("ai-1", "c-a", "m-a", 280).unwrap().unwrap();
            s.control_ai("ai-1", "c-a", "cancel").unwrap();
            assert!(s
                .execute_ai_reader("ai-1", "c-a", "m-a", 3, "READ")
                .is_err());
            assert!(s.claim_ai_at("ai-1", "c-a", "m-a", 340).unwrap().is_none());
        }
        std::fs::remove_file(f).unwrap();
    }
    #[test]
    fn cp11br2_modified_note_fails_closed() {
        let s = open(Path::new(":memory:"));
        s.create_ai_with_grant_at(
            "ai-1",
            "c-a",
            "note",
            "m-a",
            60,
            None,
            100,
            Some(("original", 1)),
        )
        .unwrap();
        s.claim_ai_at("ai-1", "c-a", "m-a", 160).unwrap().unwrap();
        s.inner
            .lock()
            .unwrap()
            .db
            .execute("UPDATE lu_ai_reader_grants SET note='changed'", [])
            .unwrap();
        assert!(s
            .execute_ai_reader("ai-1", "c-a", "m-a", 1, "READ")
            .is_err());
        assert!(s.finish_ai("ai-1", 1, "COMPLETED", "changed").is_err());
        assert!(s.finish_ai("ai-1", 1, "FAILED", "").unwrap());
    }
    #[test]
    fn cp11br2_drift_in_approved_line_or_prompt_refused() {
        let s = open(Path::new(":memory:"));
        s.create_ai_with_grant_at(
            "ai-1",
            "c-a",
            "note",
            "m-a",
            60,
            Some(60),
            100,
            Some(("original\nother", 1)),
        )
        .unwrap();
        s.claim_ai_at("ai-1", "c-a", "m-a", 160).unwrap().unwrap();
        s.inner
            .lock()
            .unwrap()
            .db
            .execute("UPDATE lu_ai_reader_grants SET line=2", [])
            .unwrap();
        assert!(s
            .execute_ai_reader("ai-1", "c-a", "m-a", 1, "READ")
            .is_err());
        s.inner
            .lock()
            .unwrap()
            .db
            .execute("UPDATE lu_ai_reader_grants SET line=1", [])
            .unwrap();
        s.inner
            .lock()
            .unwrap()
            .db
            .execute("UPDATE lu_ai_schedules SET prompt='changed'", [])
            .unwrap();
        assert!(s
            .execute_ai_reader("ai-1", "c-a", "m-a", 1, "READ")
            .is_err());
        s.inner
            .lock()
            .unwrap()
            .db
            .execute("UPDATE lu_ai_schedules SET prompt='note'", [])
            .unwrap();
        assert_eq!(
            s.execute_ai_reader("ai-1", "c-a", "m-a", 1, "READ")
                .unwrap()
                .verified_text,
            "original"
        );
    }
}
