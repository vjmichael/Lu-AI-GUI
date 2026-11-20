//! Human-approved scheduled inference, fixed notes and session-bound live file grants.
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
    pub live_file: Option<String>,
    pub permission: Option<String>,
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
        live_file: r.get(10)?,
        permission: r.get(11)?,
    })
}
/// Immutable source observation for one occurrence. The model cannot select
/// a path, URL or other argument; fixed and live grants use the same receipt.
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
/// A native-picker-selected, session-owned read-only file. The stored path
/// is for disclosure and identity checks only; scheduled reads never reopen it.
#[derive(Debug)]
pub struct LiveFile {
    display: String,
    identity: String,
    #[cfg(target_os = "linux")]
    handle: std::fs::File,
}
impl LiveFile {
    pub fn display(&self) -> &str {
        &self.display
    }
    #[cfg(not(target_os = "linux"))]
    pub fn prepare(_: &Path) -> Result<Self, SchedulerError> {
        Err(SchedulerError::InvalidRequest)
    }
    #[cfg(target_os = "linux")]
    pub fn prepare(path: &Path) -> Result<Self, SchedulerError> {
        use cap_std::{
            ambient_authority,
            fs::{Dir, OpenOptions, OpenOptionsExt},
        };
        use std::os::fd::AsRawFd;
        let fail = |_| SchedulerError::InvalidRequest;
        let display=path.to_str().filter(|s|s.len()<=2048&&!s.chars().any(|c|c.is_control()||matches!(c,'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}'|'\u{200e}'|'\u{200f}'|'\u{061c}'))).ok_or(SchedulerError::InvalidRequest)?;
        if !path.is_absolute()
            || path.extension().and_then(|s| s.to_str()) != Some("txt")
            || path.canonicalize().ok().as_deref() != Some(path)
        {
            return Err(SchedulerError::InvalidRequest);
        }
        let mut root = Dir::open_ambient_dir("/", ambient_authority()).map_err(fail)?;
        let mut components = path.components();
        if components.next() != Some(std::path::Component::RootDir) {
            return Err(SchedulerError::InvalidRequest);
        }
        let mut parts = components.peekable();
        loop {
            let Some(std::path::Component::Normal(name)) = parts.next() else {
                return Err(SchedulerError::InvalidRequest);
            };
            let mut options = OpenOptions::new();
            options.read(true);
            if parts.peek().is_some() {
                options.custom_flags(0o200000 | 0o400000); // directory, no symlink
                root = Dir::from_std_file(root.open_with(name, &options).map_err(fail)?.into_std());
                continue;
            }
            if !root.symlink_metadata(name).map_err(fail)?.is_file() {
                return Err(SchedulerError::InvalidRequest);
            }
            // O_PATH first: no data access or special-device open before type check.
            options.custom_flags(0o10000000 | 0o400000); // O_PATH | O_NOFOLLOW
            let pinned = root.open_with(name, &options).map_err(fail)?.into_std();
            let meta = pinned.metadata().map_err(fail)?;
            if !Self::allowed(&meta) {
                return Err(SchedulerError::InvalidRequest);
            }
            let identity = Self::identity(&meta)?;
            let handle = std::fs::File::open(format!("/proc/self/fd/{}", pinned.as_raw_fd()))
                .map_err(fail)?;
            let selected = Self {
                display: display.into(),
                identity,
                handle,
            };
            selected.fingerprint()?;
            return Ok(selected);
        }
    }
    #[cfg(target_os = "linux")]
    fn allowed(m: &std::fs::Metadata) -> bool {
        use std::os::unix::fs::MetadataExt;
        if !m.is_file() || m.nlink() != 1 || m.len() == 0 || m.len() > 2048 {
            return false;
        }
        // Refuse remote, pseudo and user-space filesystems in this scoped
        // implementation. Linux dev_t encoding; no path-based filesystem probe.
        let d = m.dev();
        let major = ((d >> 8) & 0xfff) | ((d >> 32) & 0xfffff000);
        let minor = (d & 0xff) | ((d >> 12) & 0xffffff00);
        let dev = format!("{major}:{minor}");
        std::fs::read_to_string("/proc/self/mountinfo")
            .ok()
            .is_some_and(|mounts| {
                let found: Vec<_> = mounts
                    .lines()
                    .filter(|l| l.split_whitespace().nth(2) == Some(&dev))
                    .filter_map(|l| {
                        l.split_once(" - ")
                            .and_then(|(_, r)| r.split_whitespace().next())
                    })
                    .collect();
                !found.is_empty()
                    && found.iter().all(|f| {
                        matches!(
                            *f,
                            "ext4"
                                | "ext3"
                                | "ext2"
                                | "btrfs"
                                | "xfs"
                                | "tmpfs"
                                | "overlay"
                                | "f2fs"
                        )
                    })
            })
    }
    #[cfg(target_os = "linux")]
    fn identity(m: &std::fs::Metadata) -> Result<String, SchedulerError> {
        use std::os::unix::fs::MetadataExt;
        let born = m
            .created()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .ok_or(SchedulerError::InvalidRequest)?;
        Ok(format!(
            "{}:{}:{}:{}",
            m.dev(),
            m.ino(),
            born.as_secs(),
            born.subsec_nanos()
        ))
    }
    #[cfg(target_os = "linux")]
    fn fingerprint(&self) -> Result<String, SchedulerError> {
        use std::os::unix::fs::MetadataExt;
        let m = self
            .handle
            .metadata()
            .map_err(|_| SchedulerError::InvalidRequest)?;
        let at_path =
            std::fs::symlink_metadata(&self.display).map_err(|_| SchedulerError::InvalidRequest)?;
        if !Self::allowed(&m)
            || !at_path.is_file()
            || m.dev() != at_path.dev()
            || m.ino() != at_path.ino()
            || self.identity != Self::identity(&m)?
            || self.identity != Self::identity(&at_path)?
        {
            return Err(SchedulerError::InvalidRequest);
        }
        Ok(format!(
            "{}:{}:{}:{}:{}",
            m.len(),
            m.ctime(),
            m.ctime_nsec(),
            m.mtime(),
            m.mtime_nsec()
        ))
    }
    #[cfg(not(target_os = "linux"))]
    fn read(&self, _: u32) -> Result<ReaderReceipt, SchedulerError> {
        Err(SchedulerError::InvalidRequest)
    }
    #[cfg(target_os = "linux")]
    fn read(&self, line: u32) -> Result<ReaderReceipt, SchedulerError> {
        use std::os::unix::fs::FileExt;
        let before = self.fingerprint()?;
        let read = || -> Result<Vec<u8>, SchedulerError> {
            let mut data = [0u8; 2049];
            let mut n = 0;
            while n < data.len() {
                let count = self
                    .handle
                    .read_at(&mut data[n..], n as u64)
                    .map_err(|_| SchedulerError::InvalidRequest)?;
                if count == 0 {
                    break;
                }
                n += count;
            }
            if n > 2048 {
                return Err(SchedulerError::InvalidRequest);
            }
            Ok(data[..n].to_vec())
        };
        let bytes = read()?;
        if before != self.fingerprint()? || bytes != read()? || before != self.fingerprint()? {
            return Err(SchedulerError::InvalidRequest);
        }
        verify_reader(
            std::str::from_utf8(&bytes).map_err(|_| SchedulerError::InvalidRequest)?,
            line,
        )
    }
}
fn live_digest(
    id: &str,
    owner: &str,
    model: &str,
    prompt: &str,
    file: &LiveFile,
    line: u32,
    repeat: Option<u64>,
) -> String {
    let mut digest = Sha256::new();
    digest.update(b"lu-changing-text-file-v1");
    for field in [id, owner, model, prompt, &file.identity, &file.display] {
        digest.update((field.len() as u64).to_le_bytes());
        digest.update(field.as_bytes());
    }
    digest.update(line.to_le_bytes());
    digest.update(repeat.unwrap_or(0).to_le_bytes());
    format!("{:x}", digest.finalize())
}
pub(super) fn initialize(db: &Connection) -> Result<(), SchedulerError> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS lu_ai_schedules(id TEXT PRIMARY KEY,workspace_id TEXT NOT NULL,prompt TEXT NOT NULL,model_ref TEXT NOT NULL,state TEXT NOT NULL CHECK(state IN ('ACTIVE','PAUSED','COMPLETED','CANCELLED')),next_due INTEGER NOT NULL,repeat_seconds INTEGER,run_count INTEGER NOT NULL DEFAULT 0 CHECK(run_count BETWEEN 0 AND 32));
      CREATE INDEX IF NOT EXISTS lu_ai_due ON lu_ai_schedules(state,model_ref,next_due);
      CREATE TABLE IF NOT EXISTS lu_ai_runs(schedule_id TEXT NOT NULL,sequence INTEGER NOT NULL,due INTEGER NOT NULL,started INTEGER NOT NULL,finished INTEGER,state TEXT NOT NULL CHECK(state IN ('RUNNING','COMPLETED','FAILED','STOPPED','INTERRUPTED')),output TEXT NOT NULL DEFAULT '',PRIMARY KEY(schedule_id,sequence));
      CREATE UNIQUE INDEX IF NOT EXISTS lu_ai_one_running ON lu_ai_runs(state) WHERE state='RUNNING';
      CREATE TABLE IF NOT EXISTS lu_ai_reader_grants(schedule_id TEXT PRIMARY KEY, note TEXT NOT NULL,line INTEGER NOT NULL,sha256 TEXT NOT NULL,scope_sha256 TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS lu_ai_live_grants(schedule_id TEXT PRIMARY KEY,display TEXT NOT NULL,identity TEXT NOT NULL,line INTEGER NOT NULL,scope_sha256 TEXT NOT NULL,permission TEXT NOT NULL CHECK(permission IN ('APPROVED','REVIEW','REVOKED','ENDED')));
      CREATE TABLE IF NOT EXISTS lu_ai_reader_receipts(schedule_id TEXT NOT NULL,sequence INTEGER NOT NULL,sha256 TEXT NOT NULL,line INTEGER NOT NULL,verified_text TEXT NOT NULL,PRIMARY KEY(schedule_id,sequence));").map_err(db_error)?;
    // A claimed occurrence is never replayed. Its cadence was already advanced
    // atomically at claim. The next future occurrence still has original consent.
    db.execute_batch("BEGIN IMMEDIATE;
      UPDATE lu_ai_schedules SET state='COMPLETED' WHERE state='ACTIVE' AND (repeat_seconds IS NULL OR run_count>=32) AND id IN (SELECT schedule_id FROM lu_ai_runs WHERE state='RUNNING');
      UPDATE lu_ai_runs SET state='INTERRUPTED',finished=MAX(started,CAST(strftime('%s','now') AS INTEGER)),output='' WHERE state='RUNNING';
      UPDATE lu_ai_schedules SET state='PAUSED' WHERE state='ACTIVE' AND id IN (SELECT schedule_id FROM lu_ai_live_grants WHERE permission!='REVOKED');
      UPDATE lu_ai_live_grants SET permission='ENDED' WHERE permission='APPROVED' AND schedule_id IN (SELECT id FROM lu_ai_schedules WHERE state='COMPLETED');
      UPDATE lu_ai_live_grants SET permission='REVIEW' WHERE permission='APPROVED';
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
        self.create_ai_with_grant_at(id, owner, prompt, model, delay, repeat, now, None, None)
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
            None,
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
        live: Option<(LiveFile, u32)>,
    ) -> Result<(), SchedulerError> {
        validate_ai_schedule(prompt, model, delay, repeat)?;
        if let Some((note, line)) = grant {
            validate_ai_reader(note, line)?;
        }
        if grant.is_some() && live.is_some() {
            return Err(SchedulerError::InvalidRequest);
        }
        if let Some((file, line)) = &live {
            file.read(*line)?;
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
        if let Some((file, line)) = &live {
            tx.execute("INSERT INTO lu_ai_live_grants(schedule_id,display,identity,line,scope_sha256,permission) VALUES(?1,?2,?3,?4,?5,'APPROVED')",params![id,file.display,file.identity,line,live_digest(id,owner,model,prompt,file,*line,repeat)]).map_err(db_error)?;
        }
        tx.commit().map_err(db_error)?;
        if let Some((file, _)) = live {
            g.live_ai_files.insert(id.to_owned(), file);
        }
        Ok(())
    }
    pub fn create_ai_live_schedule(
        &self,
        id: &str,
        owner: &str,
        prompt: &str,
        model: &str,
        delay: u64,
        repeat: Option<u64>,
        file: LiveFile,
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
            None,
            Some((file, line)),
        )
    }
    /// Reauthorization must use a freshly picked and reviewed handle. It does
    /// not resume a paused task or alter its cadence; the operator resumes it.
    pub fn reapprove_ai_live(
        &self,
        id: &str,
        owner: &str,
        file: LiveFile,
    ) -> Result<(), SchedulerError> {
        let mut g = self.inner.lock().map_err(|_| SchedulerError::Poisoned)?;
        let tx =
            g.db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .map_err(db_error)?;
        let (display,identity,line,scope,prompt,model,repeat):(String,String,u32,String,String,String,Option<u64>)=tx.query_row("SELECT g.display,g.identity,g.line,g.scope_sha256,s.prompt,s.model_ref,s.repeat_seconds FROM lu_ai_live_grants g JOIN lu_ai_schedules s ON s.id=g.schedule_id WHERE s.id=?1 AND s.workspace_id=?2 AND s.state='PAUSED' AND g.permission='REVIEW'",params![id,owner],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?))).map_err(db_error)?;
        if file.display != display
            || file.identity != identity
            || scope != live_digest(id, owner, &model, &prompt, &file, line, repeat)
        {
            return Err(SchedulerError::InvalidRequest);
        }
        file.read(line)?;
        tx.execute(
            "UPDATE lu_ai_live_grants SET permission='APPROVED' WHERE schedule_id=?1",
            [id],
        )
        .map_err(db_error)?;
        tx.commit().map_err(db_error)?;
        g.live_ai_files.insert(id.to_owned(), file);
        Ok(())
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
        let Inner {
            db, live_ai_files, ..
        } = &mut *g;
        let tx = db
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(db_error)?;
        // A live grant is read through the original pinned descriptor only.
        let live:Option<(String,String,u32,String,String,Option<u64>)>=tx.query_row("SELECT g.display,g.identity,g.line,g.scope_sha256,s.prompt,s.repeat_seconds FROM lu_ai_live_grants g JOIN lu_ai_schedules s ON s.id=g.schedule_id JOIN lu_ai_runs r ON r.schedule_id=s.id WHERE s.id=?1 AND s.workspace_id=?2 AND s.model_ref=?3 AND s.state='ACTIVE' AND g.permission='APPROVED' AND r.sequence=?4 AND r.state='RUNNING' AND NOT EXISTS(SELECT 1 FROM lu_ai_reader_receipts t WHERE t.schedule_id=r.schedule_id AND t.sequence=r.sequence)",params![id,owner,model,to_sql(sequence)?],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).optional().map_err(db_error)?;
        if let Some((display, identity, line, scope, prompt, repeat)) = live {
            let file = live_ai_files
                .get(id)
                .ok_or(SchedulerError::InvalidRequest)?;
            if display != file.display
                || identity != file.identity
                || scope != live_digest(id, owner, model, &prompt, file, line, repeat)
            {
                return Err(SchedulerError::InvalidRequest);
            }
            let receipt = match file.read(line) {
                Ok(r) => r,
                Err(e) => {
                    tx.execute(
                        "UPDATE lu_ai_live_grants SET permission='REVIEW' WHERE schedule_id=?1",
                        [id],
                    )
                    .map_err(db_error)?;
                    tx.execute("UPDATE lu_ai_schedules SET state=CASE WHEN repeat_seconds IS NULL OR run_count>=32 THEN 'COMPLETED' ELSE 'PAUSED' END WHERE id=?1",[id]).map_err(db_error)?;
                    tx.commit().map_err(db_error)?;
                    live_ai_files.remove(id);
                    return Err(e);
                }
            };
            tx.execute("INSERT INTO lu_ai_reader_receipts(schedule_id,sequence,sha256,line,verified_text) VALUES(?1,?2,?3,?4,?5)",params![id,to_sql(sequence)?,receipt.source_sha256,receipt.line,receipt.verified_text]).map_err(db_error)?;
            tx.commit().map_err(db_error)?;
            return Ok(receipt);
        }
        if tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM lu_ai_live_grants WHERE schedule_id=?1)",
                [id],
                |r| r.get::<_, bool>(0),
            )
            .map_err(db_error)?
        {
            return Err(SchedulerError::InvalidRequest);
        }
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
        let mut q=g.db.prepare("SELECT id,workspace_id,prompt,model_ref,state,next_due,repeat_seconds,run_count,EXISTS(SELECT 1 FROM lu_ai_runs WHERE schedule_id=lu_ai_schedules.id AND state='RUNNING'),COALESCE((SELECT line FROM lu_ai_reader_grants WHERE schedule_id=lu_ai_schedules.id),(SELECT line FROM lu_ai_live_grants WHERE schedule_id=lu_ai_schedules.id)),(SELECT display FROM lu_ai_live_grants WHERE schedule_id=lu_ai_schedules.id),(SELECT permission FROM lu_ai_live_grants WHERE schedule_id=lu_ai_schedules.id) FROM lu_ai_schedules ORDER BY next_due,id").map_err(db_error)?;
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
        let Inner {
            db, live_ai_files, ..
        } = &mut *g;
        let tx = db
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
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
        let row=tx.query_row("SELECT id,workspace_id,prompt,model_ref,state,next_due,repeat_seconds,run_count,EXISTS(SELECT 1 FROM lu_ai_runs WHERE schedule_id=lu_ai_schedules.id AND state='RUNNING'),COALESCE((SELECT line FROM lu_ai_reader_grants WHERE schedule_id=lu_ai_schedules.id),(SELECT line FROM lu_ai_live_grants WHERE schedule_id=lu_ai_schedules.id)),(SELECT display FROM lu_ai_live_grants WHERE schedule_id=lu_ai_schedules.id),(SELECT permission FROM lu_ai_live_grants WHERE schedule_id=lu_ai_schedules.id) FROM lu_ai_schedules WHERE id=?1 AND workspace_id=?2 AND model_ref=?3 AND state='ACTIVE' AND next_due<=?4 AND run_count<32",params![id,owner,model,now],decode_schedule).optional().map_err(db_error)?;
        let Some(row) = row else { return Ok(None) };
        if row.live_file.is_some()
            && (row.permission.as_deref() != Some("APPROVED") || !live_ai_files.contains_key(id))
        {
            return Ok(None);
        }
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
        if !tx.query_row("SELECT EXISTS(SELECT 1 FROM lu_ai_runs WHERE schedule_id=?1 AND sequence=?2 AND state='RUNNING')",params![id,to_sql(sequence)?],|r|r.get::<_,bool>(0)).map_err(db_error)? {return Ok(false)}
        if state == "COMPLETED" {
            let has: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM lu_ai_reader_grants WHERE schedule_id=?1)",
                    [id],
                    |r| r.get(0),
                )
                .map_err(db_error)?;
            let live: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM lu_ai_live_grants WHERE schedule_id=?1)",
                    [id],
                    |r| r.get(0),
                )
                .map_err(db_error)?;
            if live {
                let expected:Option<String>=tx.query_row("SELECT t.verified_text FROM lu_ai_reader_receipts t JOIN lu_ai_live_grants g ON t.schedule_id=g.schedule_id AND t.line=g.line JOIN lu_ai_schedules s ON s.id=g.schedule_id WHERE t.schedule_id=?1 AND t.sequence=?2 AND g.permission='APPROVED' AND s.state='ACTIVE'",params![id,to_sql(sequence)?],|r|r.get(0)).optional().map_err(db_error)?;
                if expected.as_deref() != Some(output) {
                    return Err(SchedulerError::InvalidRequest);
                }
            } else if has {
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
        tx.execute("UPDATE lu_ai_live_grants SET permission='ENDED' WHERE schedule_id=?1 AND permission IN ('APPROVED','REVIEW') AND EXISTS(SELECT 1 FROM lu_ai_schedules WHERE id=?1 AND state='COMPLETED')",[id]).map_err(db_error)?;
        tx.commit().map_err(db_error)?;
        if g.db
            .query_row("SELECT state FROM lu_ai_schedules WHERE id=?1", [id], |r| {
                r.get::<_, String>(0)
            })
            .is_ok_and(|s| s == "COMPLETED")
        {
            g.live_ai_files.remove(id);
        }
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
            "resume"=>tx.execute("UPDATE lu_ai_schedules SET state='ACTIVE',next_due=CASE WHEN next_due>?3 THEN next_due ELSE ?3+COALESCE(repeat_seconds,60) END WHERE id=?1 AND workspace_id=?2 AND state='PAUSED' AND NOT EXISTS(SELECT 1 FROM lu_ai_live_grants WHERE schedule_id=?1 AND permission!='APPROVED')",params![id,owner,now]),
            "revoke"=>tx.execute("UPDATE lu_ai_schedules SET state='CANCELLED' WHERE id=?1 AND workspace_id=?2 AND state IN ('ACTIVE','PAUSED') AND EXISTS(SELECT 1 FROM lu_ai_live_grants WHERE schedule_id=?1)",params![id,owner]),
            "cancel"=>tx.execute("UPDATE lu_ai_schedules SET state='CANCELLED' WHERE id=?1 AND workspace_id=?2 AND state IN ('ACTIVE','PAUSED')",params![id,owner]),
            _=>return Err(SchedulerError::InvalidRequest),
        }.map_err(db_error)?;
        if n != 1 {
            return Err(SchedulerError::NotFound);
        }
        if matches!(action, "cancel" | "revoke") {
            tx.execute(
                "UPDATE lu_ai_live_grants SET permission='REVOKED' WHERE schedule_id=?1",
                [id],
            )
            .map_err(db_error)?;
            tx.execute("UPDATE lu_ai_runs SET state='STOPPED',finished=MAX(started,?2),output='' WHERE schedule_id=?1 AND state='RUNNING'",params![id,now]).map_err(db_error)?;
        }
        tx.commit().map_err(db_error)?;
        if matches!(action, "cancel" | "revoke") {
            g.live_ai_files.remove(id);
        }
        Ok(())
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
            None,
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
                    Some((note, line)),
                    None,
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
                None,
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
            None,
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
            None,
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
    #[cfg(target_os = "linux")]
    fn live_fixture() -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "lu-live-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        root
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn cp11br3_changes_are_observed_and_bound_to_occurrence() {
        let root = live_fixture();
        let path = root.join("source.txt");
        std::fs::write(&path, "A\nONE").unwrap();
        let s = open(Path::new(":memory:"));
        s.create_ai_with_grant_at(
            "ai-1",
            "c-a",
            "retrieve",
            "m-a",
            60,
            Some(60),
            100,
            None,
            Some((LiveFile::prepare(&path).unwrap(), 2)),
        )
        .unwrap();
        assert_eq!(
            s.ai_schedules().unwrap()[0].permission.as_deref(),
            Some("APPROVED")
        );
        s.claim_ai_at("ai-1", "c-a", "m-a", 160).unwrap().unwrap();
        assert!(s
            .execute_ai_reader("ai-1", "c-b", "m-a", 1, "READ")
            .is_err());
        assert!(s
            .execute_ai_reader("ai-1", "c-a", "m-b", 1, "READ")
            .is_err());
        assert!(s
            .execute_ai_reader("ai-1", "c-a", "m-a", 1, "READ /other")
            .is_err());
        let first = s
            .execute_ai_reader("ai-1", "c-a", "m-a", 1, "READ")
            .unwrap();
        assert_eq!(first.verified_text, "ONE");
        std::fs::write(&path, "A\nTWO").unwrap();
        assert!(s
            .execute_ai_reader("ai-1", "c-a", "m-a", 1, "READ")
            .is_err());
        assert!(s.finish_ai("ai-1", 1, "COMPLETED", "TWO").is_err());
        assert!(s.finish_ai("ai-1", 1, "COMPLETED", "ONE").unwrap());
        s.claim_ai_at("ai-1", "c-a", "m-a", 220).unwrap().unwrap();
        let second = s
            .execute_ai_reader("ai-1", "c-a", "m-a", 2, "READ")
            .unwrap();
        assert_eq!(second.verified_text, "TWO");
        assert_ne!(first.source_sha256, second.source_sha256);
        assert!(s.finish_ai("ai-1", 2, "COMPLETED", "TWO").unwrap());
        assert_eq!(s.ai_runs("ai-1", "c-a").unwrap()[1].output, "ONE");
        std::fs::remove_dir_all(root).unwrap();
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn cp11br3_replacement_symlink_and_oversize_are_fail_closed() {
        let root = live_fixture();
        let path = root.join("source.txt");
        let other = root.join("other.txt");
        std::fs::write(&path, "ONE").unwrap();
        std::fs::write(&other, "OTHER").unwrap();
        let alias = root.join("alias.txt");
        std::os::unix::fs::symlink(&path, &alias).unwrap();
        assert!(LiveFile::prepare(&alias).is_err());
        let hard = root.join("hard.txt");
        std::fs::hard_link(&path, &hard).unwrap();
        assert!(LiveFile::prepare(&path).is_err());
        std::fs::remove_file(hard).unwrap();
        let file = LiveFile::prepare(&path).unwrap();
        std::fs::write(&path, "a".repeat(2049)).unwrap();
        assert!(file.read(1).is_err());
        std::fs::write(&path, "ONE").unwrap();
        let s = open(Path::new(":memory:"));
        s.create_ai_with_grant_at(
            "ai-1",
            "c-a",
            "retrieve",
            "m-a",
            60,
            Some(60),
            100,
            None,
            Some((file, 1)),
        )
        .unwrap();
        s.claim_ai_at("ai-1", "c-a", "m-a", 160).unwrap().unwrap();
        std::fs::rename(&path, root.join("saved.txt")).unwrap();
        std::fs::rename(&other, &path).unwrap();
        assert!(s
            .execute_ai_reader("ai-1", "c-a", "m-a", 1, "READ")
            .is_err());
        assert!(s.finish_ai("ai-1", 1, "FAILED", "").unwrap());
        let row = s.ai_schedules().unwrap().remove(0);
        assert_eq!(row.state, "PAUSED");
        assert_eq!(row.permission.as_deref(), Some("REVIEW"));
        assert!(s.control_ai("ai-1", "c-a", "resume").is_err());
        assert!(s
            .reapprove_ai_live("ai-1", "c-a", LiveFile::prepare(&path).unwrap())
            .is_err());
        assert!(s.ai_runs("ai-1", "c-a").unwrap()[0]
            .reader_receipt
            .is_none());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn cp11br3_restart_reapproval_revocation_and_result_retention() {
        let root = live_fixture();
        let path = root.join("source.txt");
        let journal = root.join("journal.sqlite");
        std::fs::write(&path, "OLD").unwrap();
        {
            let s = open(&journal);
            s.create_ai_with_grant_at(
                "ai-1",
                "c-a",
                "retrieve",
                "m-a",
                60,
                Some(60),
                100,
                None,
                Some((LiveFile::prepare(&path).unwrap(), 1)),
            )
            .unwrap();
            s.claim_ai_at("ai-1", "c-a", "m-a", 160).unwrap().unwrap();
            s.execute_ai_reader("ai-1", "c-a", "m-a", 1, "READ")
                .unwrap();
            s.finish_ai("ai-1", 1, "COMPLETED", "OLD").unwrap();
            s.claim_ai_at("ai-1", "c-a", "m-a", 220).unwrap().unwrap();
            s.execute_ai_reader("ai-1", "c-a", "m-a", 2, "READ")
                .unwrap();
        }
        std::fs::write(&path, "NEW").unwrap();
        {
            let s = open(&journal);
            let row = s.ai_schedules().unwrap().remove(0);
            assert_eq!(row.state, "PAUSED");
            assert_eq!(row.permission.as_deref(), Some("REVIEW"));
            assert!(s.claim_ai_at("ai-1", "c-a", "m-a", 280).unwrap().is_none());
            assert!(s.control_ai("ai-1", "c-a", "resume").is_err());
            assert!(s
                .reapprove_ai_live("ai-1", "c-b", LiveFile::prepare(&path).unwrap())
                .is_err());
            assert_eq!(s.ai_runs("ai-1", "c-a").unwrap()[0].state, "INTERRUPTED");
            assert!(s.ai_runs("ai-1", "c-a").unwrap()[0]
                .reader_receipt
                .is_some());
            s.reapprove_ai_live("ai-1", "c-a", LiveFile::prepare(&path).unwrap())
                .unwrap();
            s.control_ai("ai-1", "c-a", "resume").unwrap();
            let due = s.ai_schedules().unwrap()[0].next_due;
            s.claim_ai_at("ai-1", "c-a", "m-a", due).unwrap().unwrap();
            assert_eq!(
                s.execute_ai_reader("ai-1", "c-a", "m-a", 3, "READ")
                    .unwrap()
                    .verified_text,
                "NEW"
            );
            assert!(s.control_ai("ai-1", "c-b", "revoke").is_err());
            s.control_ai("ai-1", "c-a", "revoke").unwrap();
            assert!(!s.ai_running("ai-1", 3).unwrap());
            assert!(!s.finish_ai("ai-1", 3, "COMPLETED", "NEW").unwrap());
        }
        let s = open(&journal);
        assert_eq!(
            s.ai_schedules().unwrap()[0].permission.as_deref(),
            Some("REVOKED")
        );
        assert!(s
            .claim_ai_at("ai-1", "c-a", "m-a", now_unix() + 600)
            .unwrap()
            .is_none());
        assert!(s
            .reapprove_ai_live("ai-1", "c-a", LiveFile::prepare(&path).unwrap())
            .is_err());
        let runs = s.ai_runs("ai-1", "c-a").unwrap();
        assert_eq!(runs[0].state, "STOPPED");
        assert_eq!(runs[2].output, "OLD");
        drop(s);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn cp11br3_missing_or_invalid_resource_requires_review_and_never_replays() {
        let root = live_fixture();
        let path = root.join("source.txt");
        std::fs::write(&path, "ONE").unwrap();
        let s = open(Path::new(":memory:"));
        s.create_ai_with_grant_at(
            "ai-1",
            "c-a",
            "retrieve",
            "m-a",
            60,
            Some(60),
            100,
            None,
            Some((LiveFile::prepare(&path).unwrap(), 1)),
        )
        .unwrap();
        s.claim_ai_at("ai-1", "c-a", "m-a", 160).unwrap().unwrap();
        std::fs::write(&path, [0xff, 0xff]).unwrap();
        assert!(s
            .execute_ai_reader("ai-1", "c-a", "m-a", 1, "READ")
            .is_err());
        assert!(s.finish_ai("ai-1", 1, "FAILED", "").unwrap());
        assert!(s.claim_ai_at("ai-1", "c-a", "m-a", 220).unwrap().is_none());
        std::fs::write(&path, "RESTORED").unwrap();
        s.reapprove_ai_live("ai-1", "c-a", LiveFile::prepare(&path).unwrap())
            .unwrap();
        s.control_ai("ai-1", "c-a", "resume").unwrap();
        let due = s.ai_schedules().unwrap()[0].next_due;
        s.claim_ai_at("ai-1", "c-a", "m-a", due).unwrap().unwrap();
        std::fs::remove_file(&path).unwrap();
        assert!(s
            .execute_ai_reader("ai-1", "c-a", "m-a", 2, "READ")
            .is_err());
        assert!(s.finish_ai("ai-1", 2, "FAILED", "").unwrap());
        assert_eq!(
            s.ai_schedules().unwrap()[0].permission.as_deref(),
            Some("REVIEW")
        );
        assert!(s
            .ai_runs("ai-1", "c-a")
            .unwrap()
            .iter()
            .all(|r| r.reader_receipt.is_none()));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn cp11br3_one_time_permission_ends_and_scope_drift_is_rejected() {
        let root = live_fixture();
        let path = root.join("source.txt");
        std::fs::write(&path, "ONE\nTWO").unwrap();
        let s = open(Path::new(":memory:"));
        s.create_ai_with_grant_at(
            "ai-1",
            "c-a",
            "retrieve",
            "m-a",
            60,
            None,
            100,
            None,
            Some((LiveFile::prepare(&path).unwrap(), 1)),
        )
        .unwrap();
        s.claim_ai_at("ai-1", "c-a", "m-a", 160).unwrap().unwrap();
        s.inner
            .lock()
            .unwrap()
            .db
            .execute("UPDATE lu_ai_live_grants SET line=2", [])
            .unwrap();
        assert!(s
            .execute_ai_reader("ai-1", "c-a", "m-a", 1, "READ")
            .is_err());
        s.inner
            .lock()
            .unwrap()
            .db
            .execute("UPDATE lu_ai_live_grants SET line=1", [])
            .unwrap();
        assert_eq!(
            s.execute_ai_reader("ai-1", "c-a", "m-a", 1, "READ")
                .unwrap()
                .verified_text,
            "ONE"
        );
        assert!(s.finish_ai("ai-1", 1, "COMPLETED", "ONE").unwrap());
        let row = s.ai_schedules().unwrap().remove(0);
        assert_eq!(row.state, "COMPLETED");
        assert_eq!(row.permission.as_deref(), Some("ENDED"));
        assert!(!s.inner.lock().unwrap().live_ai_files.contains_key("ai-1"));
        std::fs::remove_dir_all(root).unwrap();
    }
}
