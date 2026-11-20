//! CP-11B-R1: human-approved, text-only scheduled inference. No tool authority.
use super::*;
use serde::Serialize;
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
    })
}
pub(super) fn initialize(db: &Connection) -> Result<(), SchedulerError> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS lu_ai_schedules(id TEXT PRIMARY KEY,workspace_id TEXT NOT NULL,prompt TEXT NOT NULL,model_ref TEXT NOT NULL,state TEXT NOT NULL CHECK(state IN ('ACTIVE','PAUSED','COMPLETED','CANCELLED')),next_due INTEGER NOT NULL,repeat_seconds INTEGER,run_count INTEGER NOT NULL DEFAULT 0 CHECK(run_count BETWEEN 0 AND 32));
      CREATE INDEX IF NOT EXISTS lu_ai_due ON lu_ai_schedules(state,model_ref,next_due);
      CREATE TABLE IF NOT EXISTS lu_ai_runs(schedule_id TEXT NOT NULL,sequence INTEGER NOT NULL,due INTEGER NOT NULL,started INTEGER NOT NULL,finished INTEGER,state TEXT NOT NULL CHECK(state IN ('RUNNING','COMPLETED','FAILED','STOPPED','INTERRUPTED')),output TEXT NOT NULL DEFAULT '',PRIMARY KEY(schedule_id,sequence));
      CREATE UNIQUE INDEX IF NOT EXISTS lu_ai_one_running ON lu_ai_runs(state) WHERE state='RUNNING';").map_err(db_error)?;
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
        validate_ai_schedule(prompt, model, delay, repeat)?;
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
        tx.commit().map_err(db_error)
    }
    pub fn ai_schedules(&self) -> Result<Vec<AiSchedule>, SchedulerError> {
        let g = self.inner.lock().map_err(|_| SchedulerError::Poisoned)?;
        let mut q=g.db.prepare("SELECT id,workspace_id,prompt,model_ref,state,next_due,repeat_seconds,run_count,EXISTS(SELECT 1 FROM lu_ai_runs WHERE schedule_id=lu_ai_schedules.id AND state='RUNNING') FROM lu_ai_schedules ORDER BY next_due,id").map_err(db_error)?;
        let rows = q.query_map([], decode_schedule).map_err(db_error)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(db_error)
    }
    pub fn ai_runs(&self, id: &str, owner: &str) -> Result<Vec<AiRun>, SchedulerError> {
        let g = self.inner.lock().map_err(|_| SchedulerError::Poisoned)?;
        let mut q=g.db.prepare("SELECT r.schedule_id,r.sequence,r.due,r.started,r.finished,r.state,r.output FROM lu_ai_runs r JOIN lu_ai_schedules s ON s.id=r.schedule_id WHERE s.id=?1 AND s.workspace_id=?2 ORDER BY r.sequence DESC LIMIT 32").map_err(db_error)?;
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
        let row=tx.query_row("SELECT id,workspace_id,prompt,model_ref,state,next_due,repeat_seconds,run_count,EXISTS(SELECT 1 FROM lu_ai_runs WHERE schedule_id=lu_ai_schedules.id AND state='RUNNING') FROM lu_ai_schedules WHERE id=?1 AND workspace_id=?2 AND model_ref=?3 AND state='ACTIVE' AND next_due<=?4 AND run_count<32",params![id,owner,model,now],decode_schedule).optional().map_err(db_error)?;
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
}
