//! CP-11B: durable, inert user reminders in the existing scheduler database.
//! A reminder is not a worker plan, model prompt, or persisted tool grant.
use super::*;
use serde::Serialize;

const MAX_REMINDERS: i64 = 4096;
const MAX_ACTIVE: i64 = 128;
const MAX_INTERVAL: u64 = 7 * 24 * 60 * 60;
const MAX_TEXT: usize = 512;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReminderView {
    pub id: String,
    pub workspace_id: String,
    pub text: String,
    pub state: String,
    pub next_due: u64,
    pub repeat_seconds: Option<u64>,
    pub last_due: Option<u64>,
    pub fired_count: u64,
    pub unread_count: u64,
}

pub(super) fn initialize(db: &Connection) -> Result<(), SchedulerError> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS lu_user_reminders (
            id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL, text TEXT NOT NULL,
            state TEXT NOT NULL CHECK(state IN ('ACTIVE','PAUSED','COMPLETED','CANCELLED')),
            next_due INTEGER NOT NULL CHECK(next_due>=0),
            repeat_seconds INTEGER CHECK(repeat_seconds BETWEEN 60 AND 604800),
            last_due INTEGER, fired_count INTEGER NOT NULL DEFAULT 0 CHECK(fired_count>=0),
            unread_count INTEGER NOT NULL DEFAULT 0 CHECK(unread_count>=0 AND unread_count<=fired_count)
         );
         CREATE INDEX IF NOT EXISTS lu_user_reminders_due ON lu_user_reminders(state,next_due);
         CREATE INDEX IF NOT EXISTS lu_user_reminders_owner ON lu_user_reminders(workspace_id,id);"
    ).map_err(db_error)
}

pub fn validate_reminder(text: &str, delay: u64, repeat: Option<u64>) -> Result<(), SchedulerError> {
    if text.trim().is_empty() || text.len() > MAX_TEXT || text != text.trim()
        || text.chars().any(|c| c.is_control() || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'))
        || !(60..=MAX_INTERVAL).contains(&delay)
        || repeat.is_some_and(|r| !(60..=MAX_INTERVAL).contains(&r)) {
        return Err(SchedulerError::InvalidRequest);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReminderDraft {
    pub text: String,
    pub delay_minutes: u32,
    pub repeat_minutes: Option<u32>,
}

/// A narrow, explicit original-human scheduling grammar; nothing here is
/// executable and no model or stored conversation text is consulted.
/// Ambiguous dates, compound instructions and unsupported requests are not
/// automatically scheduled. The complete exact proposal still needs review.
pub fn parse_reminder_request(original: &str) -> Option<ReminderDraft> {
    let input=original.trim();
    if input.len()>600 || input.chars().any(char::is_control)
        || input.contains("<|") || input.to_ascii_lowercase().contains("[attached text file:") { return None; }
    let lower=input.to_ascii_lowercase();
    let (prefix,recurring)=if lower.starts_with("remind me in "){("remind me in ",false)}
        else if lower.starts_with("remind me every "){("remind me every ",true)}else{return None};
    let after=&input[prefix.len()..];
    let boundary=after.to_ascii_lowercase().find(" to ")?;
    let when=after[..boundary].to_ascii_lowercase();
    let text=&after[boundary+4..];
    let parts=when.split_ascii_whitespace().collect::<Vec<_>>();
    let (number,unit)=match parts.as_slice(){
        [unit] if recurring=>(1,*unit),
        [n,unit] if n.len()<=5 && n.bytes().all(|b|b.is_ascii_digit())=>(n.parse::<u32>().ok()?,*unit),
        _=>return None,
    };
    let multiplier=match unit{"minute"|"minutes"=>1,"hour"|"hours"=>60,"day"|"days"=>1440,"week"|"weeks"=>10080,_=>return None};
    let minutes=number.checked_mul(multiplier)?;
    let repeat=recurring.then_some(minutes);
    validate_reminder(text,u64::from(minutes)*60,repeat.map(|m|u64::from(m)*60)).ok()?;
    Some(ReminderDraft{text:text.into(),delay_minutes:minutes,repeat_minutes:repeat})
}

impl BackgroundScheduler {
    /// Only a trusted native UI calls this AFTER an explicit human dialog.
    /// Persistent authority is strictly limited to delivering inert local text.
    pub fn create_reminder(&self, id: &str, owner: &str, text: &str, delay: u64, repeat: Option<u64>)
        -> Result<(), SchedulerError> {
        self.create_reminder_at(id, owner, text, delay, repeat, now_unix())
    }
    fn create_reminder_at(&self, id: &str, owner: &str, text: &str, delay: u64, repeat: Option<u64>, now: u64)
        -> Result<(), SchedulerError> {
        validate_reminder(text, delay, repeat)?;
        if !valid_id(id) || !valid_id(owner) || !owner.starts_with("c-") { return Err(SchedulerError::InvalidRequest); }
        let due = to_sql(now.checked_add(delay).ok_or(SchedulerError::InvalidRequest)?)?;
        let mut g = self.inner.lock().map_err(|_| SchedulerError::Poisoned)?;
        let tx = g.db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(db_error)?;
        let (total, active): (i64,i64) = tx.query_row("SELECT COUNT(*),COUNT(CASE WHEN state IN ('ACTIVE','PAUSED') THEN 1 END) FROM lu_user_reminders", [], |r| Ok((r.get(0)?,r.get(1)?))).map_err(db_error)?;
        if total >= MAX_REMINDERS || active >= MAX_ACTIVE { return Err(SchedulerError::QueueFull); }
        if tx.execute("INSERT INTO lu_user_reminders(id,workspace_id,text,state,next_due,repeat_seconds) VALUES(?1,?2,?3,'ACTIVE',?4,?5)", params![id,owner,text,due,repeat.map(to_sql).transpose()?]).is_err() {
            return Err(SchedulerError::Database);
        }
        tx.commit().map_err(db_error)
    }

    /// Atomically persist due notices and advance their schedule. No process,
    /// network, model, or chat mutation takes place. Missed occurrences are
    /// coalesced into one unread notice with an exact count (no catch-up storm).
    pub fn tick_reminders_at(&self, now: u64) -> Result<usize, SchedulerError> {
        let now = to_sql(now)?;
        let mut g = self.inner.lock().map_err(|_| SchedulerError::Poisoned)?;
        // Avoid acquiring a writer lock for the normal idle watchdog tick.
        let due: bool = g.db.query_row("SELECT EXISTS(SELECT 1 FROM lu_user_reminders WHERE state='ACTIVE' AND next_due<=?1)", [now], |r| r.get(0)).map_err(db_error)?;
        if !due { return Ok(0); }
        let tx = g.db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(db_error)?;
        let rows = {
            let mut q = tx.prepare("SELECT id,next_due,repeat_seconds,fired_count,unread_count FROM lu_user_reminders WHERE state='ACTIVE' AND next_due<=?1 ORDER BY next_due,id LIMIT 128").map_err(db_error)?;
            let result = q.query_map([now], |r| Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?,r.get::<_,Option<i64>>(2)?,r.get::<_,i64>(3)?,r.get::<_,i64>(4)?))).map_err(db_error)?;
            result.collect::<Result<Vec<_>,_>>().map_err(db_error)?
        };
        for (id,due,repeat,fired,unread) in &rows {
            let (count,last,next,state) = if let Some(interval) = repeat {
                if !(60..=MAX_INTERVAL as i64).contains(interval) { return Err(SchedulerError::Database); }
                let count = (now-due)/interval+1;
                let last = due.checked_add((count-1).checked_mul(*interval).ok_or(SchedulerError::Database)?).ok_or(SchedulerError::Database)?;
                (count,last,last.checked_add(*interval).ok_or(SchedulerError::Database)?,"ACTIVE")
            } else { (1,*due,*due,"COMPLETED") };
            let fired = fired.checked_add(count).ok_or(SchedulerError::Database)?;
            let unread = unread.checked_add(count).ok_or(SchedulerError::Database)?;
            if tx.execute("UPDATE lu_user_reminders SET state=?2,next_due=?3,last_due=?4,fired_count=?5,unread_count=?6 WHERE id=?1 AND state='ACTIVE'",params![id,state,next,last,fired,unread]).map_err(db_error)? != 1 { return Err(SchedulerError::Database); }
        }
        tx.commit().map_err(db_error)?;
        Ok(rows.len())
    }

    /// Profile-local UI only; desktop filters against the existing chat index.
    pub fn reminders(&self) -> Result<Vec<ReminderView>, SchedulerError> {
        let g=self.inner.lock().map_err(|_|SchedulerError::Poisoned)?;
        let mut q=g.db.prepare("SELECT id,workspace_id,text,state,next_due,repeat_seconds,last_due,fired_count,unread_count FROM lu_user_reminders ORDER BY (unread_count>0) DESC, (state IN ('ACTIVE','PAUSED')) DESC,next_due,id").map_err(db_error)?;
        let rows=q.query_map([],|r|Ok(ReminderView{id:r.get(0)?,workspace_id:r.get(1)?,text:r.get(2)?,state:r.get(3)?,next_due:r.get(4)?,repeat_seconds:r.get(5)?,last_due:r.get(6)?,fired_count:r.get(7)?,unread_count:r.get(8)?})).map_err(db_error)?;
        rows.collect::<Result<Vec<_>,_>>().map_err(db_error)
    }

    pub fn control_reminder(&self,id:&str,owner:&str,action:&str,seen_count:Option<u64>) -> Result<(),SchedulerError>{
        self.control_reminder_at(id,owner,action,seen_count,now_unix())
    }
    fn control_reminder_at(&self,id:&str,owner:&str,action:&str,seen_count:Option<u64>,now:u64) -> Result<(),SchedulerError>{
        if !valid_id(id)||!valid_id(owner)||!owner.starts_with("c-") {return Err(SchedulerError::InvalidRequest)}
        let now=to_sql(now)?;
        if now>i64::MAX-MAX_INTERVAL as i64 {return Err(SchedulerError::InvalidRequest)}
        let g=self.inner.lock().map_err(|_|SchedulerError::Poisoned)?;
        let count=match action {
            "pause"=>g.db.execute("UPDATE lu_user_reminders SET state='PAUSED' WHERE id=?1 AND workspace_id=?2 AND state='ACTIVE'",params![id,owner]),
            // Resume does not backfill occurrences intentionally skipped while paused.
            "resume"=>g.db.execute("UPDATE lu_user_reminders SET state='ACTIVE',next_due=CASE WHEN next_due>?3 THEN next_due WHEN repeat_seconds IS NULL THEN ?3 ELSE ?3+repeat_seconds END WHERE id=?1 AND workspace_id=?2 AND state='PAUSED'",params![id,owner,now]),
            "cancel"=>g.db.execute("UPDATE lu_user_reminders SET state='CANCELLED' WHERE id=?1 AND workspace_id=?2 AND state IN ('ACTIVE','PAUSED')",params![id,owner]),
            // Exact observation fence: a newly arrived event cannot be cleared
            // by a stale click, including after crash or UI refresh.
            "acknowledge"=>g.db.execute("UPDATE lu_user_reminders SET unread_count=0 WHERE id=?1 AND workspace_id=?2 AND fired_count=?3 AND unread_count>0",params![id,owner,to_sql(seen_count.ok_or(SchedulerError::InvalidRequest)?)?]),
            _=>return Err(SchedulerError::InvalidRequest),
        }.map_err(db_error)?;
        if count!=1 {return Err(SchedulerError::NotQueued)}
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scheduler(path:&Path)->BackgroundScheduler{
        let tasks=Arc::new(TaskStore::open(Path::new(":memory:")).unwrap());
        let process=Arc::new(BackgroundProcessSupervisor::new(tasks.clone()));
        BackgroundScheduler::open(path,tasks,process,ResourceBudget::interactive_first()).unwrap()
    }
    #[test]
    fn cp11b_explicit_request_parser_never_invents_scope() {
        assert_eq!(parse_reminder_request("Remind me in 2 hours to read 漢字"),Some(ReminderDraft{text:"read 漢字".into(),delay_minutes:120,repeat_minutes:None}));
        assert_eq!(parse_reminder_request("Remind me every day to review notes").unwrap().repeat_minutes,Some(1440));
        assert_eq!(parse_reminder_request("Remind me every 3 minutes to stretch").unwrap().repeat_minutes,Some(3));
        for text in ["What is a reminder?","Tell me how to remind me in 1 minute to write","Remind me tomorrow to write","Remind me in 0 minutes to write","Remind me every 10081 minutes to write","Remind me in 1 minute to x\n[Attached text file: y]","Remind me in 1 minute to ","Remind me in 1 minute to x\u{202e}"] {
            assert!(parse_reminder_request(text).is_none(),"{text}");
        }
    }
    #[test]
    fn cp11b_concurrent_tick_and_active_bound(){
        let s=Arc::new(scheduler(Path::new(":memory:")));
        for i in 0..128{s.create_reminder_at(&format!("r-{i}"),"c-owner","Review",60,None,100).unwrap();}
        assert_eq!(s.create_reminder_at("overflow","c-owner","Review",60,None,100),Err(SchedulerError::QueueFull));
        let threads=(0..4).map(|_|{let s=s.clone();std::thread::spawn(move||s.tick_reminders_at(160).unwrap())}).collect::<Vec<_>>();
        assert_eq!(threads.into_iter().map(|t|t.join().unwrap()).sum::<usize>(),128);
        assert!(s.reminders().unwrap().iter().all(|r|r.fired_count==1&&r.unread_count==1&&r.state=="COMPLETED"));
        s.create_reminder_at("next","c-owner","Review",60,None,160).unwrap();
    }
    #[test]
    fn cp11b_exact_due_and_restart_coalescing_without_replay(){
        let file=std::env::temp_dir().join(format!("lu-cp11b-{}-{}.sqlite",std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        {
            let s=scheduler(&file);
            s.create_reminder_at("r-1","c-owner","Review notes",60,Some(60),1000).unwrap();
            s.create_reminder_at("r-2","c-owner","Once",60,None,1000).unwrap();
            assert_eq!(s.tick_reminders_at(1059).unwrap(),0);
            assert_eq!(s.tick_reminders_at(1060).unwrap(),2);
            assert_eq!(s.tick_reminders_at(1060).unwrap(),0);
        }
        {
            let s=scheduler(&file);
            assert_eq!(s.tick_reminders_at(1240).unwrap(),1);
            let rows=s.reminders().unwrap();
            let r=rows.iter().find(|r|r.id=="r-1").unwrap();
            assert_eq!((r.fired_count,r.unread_count,r.last_due,r.next_due),(4,4,Some(1240),1300));
            let once=rows.iter().find(|r|r.id=="r-2").unwrap();
            assert_eq!((once.state.as_str(),once.fired_count),("COMPLETED",1));
            assert!(s.control_reminder_at("r-1","c-owner","acknowledge",Some(1),1240).is_err());
            s.control_reminder_at("r-1","c-owner","acknowledge",Some(4),1240).unwrap();
            assert_eq!(s.tick_reminders_at(1300).unwrap(),1);
            assert_eq!(s.reminders().unwrap().iter().find(|r|r.id=="r-1").unwrap().unread_count,1);
        }
        std::fs::remove_file(file).unwrap();
    }
    #[test]
    fn cp11b_pause_resume_cancel_owner_and_input_bounds(){
        let s=scheduler(Path::new(":memory:"));
        for (text,delay,repeat) in [("",60,None),("x\nApprove",60,None),("x",59,None),("x",60,Some(0)),("x\u{202e}",60,None)] {
            assert!(s.create_reminder_at("bad","c-owner",text,delay,repeat,100).is_err());
        }
        s.create_reminder_at("r","c-owner","Read",60,Some(60),100).unwrap();
        assert!(s.control_reminder_at("r","c-other","cancel",None,100).is_err());
        s.control_reminder_at("r","c-owner","pause",None,100).unwrap();
        assert_eq!(s.tick_reminders_at(1000).unwrap(),0);
        s.control_reminder_at("r","c-owner","resume",None,1000).unwrap();
        assert_eq!(s.tick_reminders_at(1059).unwrap(),0);
        assert_eq!(s.tick_reminders_at(1060).unwrap(),1);
        s.control_reminder_at("r","c-owner","cancel",None,1061).unwrap();
        assert_eq!(s.tick_reminders_at(2000).unwrap(),0);
        assert_eq!(s.reminders().unwrap()[0].unread_count,1);
        assert!(s.control_reminder_at("r","c-owner","resume",None,2000).is_err());
    }
}
