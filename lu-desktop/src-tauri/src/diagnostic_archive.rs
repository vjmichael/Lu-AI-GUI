//! CP-01E: bounded local persistence of existing validated DX v2 traces.
//! No prompts, responses, image bytes, user/chat/model IDs, paths, raw errors,
//! or authorization material. Recovery is diagnostic, NOT authority.
use crate::causal_diagnostic::CausalProgress;
use crate::active_checks::{CheckBody,CheckReport,CheckState};
use lu_core::{diagnostic_contract::{self,encode_diagnostic_trace,decode_diagnostic_trace},
    diagnostic_localization::{localize_transactional_turn,DiagnosticFinding},wire::proto};
use rusqlite::{Connection,OptionalExtension,params};
use serde::Serialize;
use std::{path::Path,sync::Mutex,time::Duration};
const MAX_TRACES:usize=64;
pub(crate) fn wire_trace(source:&CausalProgress)->proto::DiagnosticTrace{
    proto::DiagnosticTrace{
        protocol_version:diagnostic_contract::DIAGNOSTIC_PROTOCOL_VERSION,
        trace_ref:source.trace_ref().to_owned(),
        dropped_events:source.dropped(),
        events:source.observations().iter().enumerate().map(|(i,e)|proto::DiagnosticEvent{
            sequence:(i+1) as u64,parent_sequence:i as u64,stage:e.stage,outcome:e.outcome,
            elapsed_millis:e.elapsed_ms,clock_domain:2,failure_code:e.failure_code,
            attempt:1,clock_epoch:1,
        }).collect(),
    }
}
pub(crate) struct DiagnosticArchive{conn:Mutex<Connection>}
#[derive(Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct ArchivedFinding{
    recording_state:&'static str,
    finding:DiagnosticFinding,
    explanation:&'static str,
    suggestion:&'static str,
}
#[derive(Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct ArchiveReport{
    schema:&'static str,
    findings:Vec<ArchivedFinding>,
    unavailable_records:usize,
    // Additive local-check summary under the existing explicit native consent.
    active_check:Option<CheckReport>,
    active_check_unavailable:bool,
    scheduled_runs:Option<lu_core::native_background_scheduler::ScheduledDiagnosticSummary>,
    scheduled_runs_unavailable:bool,
}
impl ArchiveReport {
    /// Explicit allowlist: counts only, never diagnostic text or references.
    pub fn exchange_summary(&self)->crate::dx_ipc::Summary {
        use lu_core::diagnostic_localization::EvidenceStrength::*;
        let mut s=crate::dx_ipc::Summary{complete:0,observed_failure:0,bounded:0,incomplete:0,unavailable:self.unavailable_records as u32};
        for f in &self.findings {match f.finding.evidence_strength{
            Complete=>s.complete+=1,ObservedFailure=>s.observed_failure+=1,
            BoundedToNextStage=>s.bounded+=1,InsufficientEvidence=>s.incomplete+=1,
        }} s
    }

    pub fn with_scheduled_runs(mut self,summary:Result<lu_core::native_background_scheduler::ScheduledDiagnosticSummary,lu_core::native_background_scheduler::SchedulerError>)->Self {
        self.scheduled_runs_unavailable=summary.is_err();self.scheduled_runs=summary.ok();self
    }
}
impl DiagnosticArchive{
    // Called only after process-exclusive DesktopOwner acquisition.
    pub fn open(path:&Path)->Result<Self,String>{
        let conn=Connection::open(path).map_err(|_|"Diagnostic storage unavailable")?;
        conn.busy_timeout(Duration::ZERO).map_err(|_|"Diagnostic storage unavailable")?;
        conn.execute_batch(
            "PRAGMA synchronous=FULL; PRAGMA secure_delete=ON;
             CREATE TABLE IF NOT EXISTS dx_trace(
               ordinal INTEGER PRIMARY KEY AUTOINCREMENT,
               trace_ref TEXT NOT NULL UNIQUE CHECK(length(trace_ref)=32),
               body BLOB NOT NULL CHECK(length(body)>0 AND length(body)<=8192),
               state TEXT NOT NULL CHECK(state IN ('live','sealed','recovered'))
             );
             UPDATE dx_trace SET state='recovered' WHERE state='live';
             CREATE TABLE IF NOT EXISTS active_check(
                slot INTEGER PRIMARY KEY CHECK(slot=1), run_id TEXT NOT NULL,
                state TEXT NOT NULL CHECK(state IN ('running','complete','attention','stopped','interrupted')),
                body TEXT NOT NULL CHECK(length(body)<=4096));
             UPDATE active_check SET state='interrupted' WHERE state='running';"
        ).map_err(|_|"Diagnostic storage unavailable")?;
        Ok(Self{conn:Mutex::new(conn)})
    }
    /// Bounded checkpoint publication outside scheduler/conversation transactions.
    /// No per-token writes. Retry only SQLite busy/locked or the local mutex;
    /// release the connection between attempts, preserving zero-wait readers.
    pub fn persist(&self,source:&CausalProgress)->bool{
        let trace=wire_trace(source);
        let Ok(body)=encode_diagnostic_trace(&trace) else{return false};
        let state=if source.terminal(){"sealed"}else{"live"};
        let started=std::time::Instant::now();
        let budget=Duration::from_millis(250);
        loop {
            match self.conn.try_lock(){
                Ok(mut conn)=>match Self::persist_once(&mut conn,&trace,&body,state){
                    Ok(accepted)=>return accepted,
                    Err(rusqlite::Error::SqliteFailure(err,_)) if matches!(err.code,
                        rusqlite::ErrorCode::DatabaseBusy|rusqlite::ErrorCode::DatabaseLocked)=>{},
                    Err(_)=>return false,
                },
                Err(std::sync::TryLockError::WouldBlock)=>{},
                Err(std::sync::TryLockError::Poisoned(_))=>return false,
            }
            let remaining=budget.saturating_sub(started.elapsed());
            if remaining.is_zero(){return false}
            std::thread::sleep(Duration::from_millis(5).min(remaining));
        }
    }
    fn persist_once(conn:&mut Connection,trace:&proto::DiagnosticTrace,body:&[u8],state:&str)->rusqlite::Result<bool>{
        // Acquire the writer reservation before reading the prior version.
        // A failed commit rolls back before the next attempt, so every retry
        // rechecks monotonicity/recovery/sealing against the durable row.
        let tx=conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let prior=tx.query_row("SELECT body,state FROM dx_trace WHERE trace_ref=?1",
            [&trace.trace_ref],|r|Ok((r.get::<_,Vec<u8>>(0)?,r.get::<_,String>(1)?))).optional()?;
        match prior{
            Some((previous,old_state))=>{
                if previous==body && old_state==state{return Ok(true)}
                if old_state!="live"{return Ok(false)}
                let Ok(old)=decode_diagnostic_trace(&previous) else{return Ok(false)};
                if old.trace_ref!=trace.trace_ref||old.dropped_events>trace.dropped_events||
                    !trace.events.starts_with(&old.events){return Ok(false)}
                if tx.execute("UPDATE dx_trace SET body=?1,state=?2 WHERE trace_ref=?3",
                    params![body,state,trace.trace_ref])?!=1{return Ok(false)}
            },
            None=>{
                if tx.execute("INSERT INTO dx_trace(trace_ref,body,state) VALUES(?1,?2,?3)",
                    params![trace.trace_ref,body,state])?!=1{return Ok(false)}
            },
        }
        tx.execute("DELETE FROM dx_trace WHERE ordinal NOT IN
            (SELECT ordinal FROM dx_trace ORDER BY ordinal DESC LIMIT ?1)",
            [MAX_TRACES as i64])?;
        tx.commit()?;
        Ok(true)
    }
    // Active, explicitly approved checks retain their existing brief wait.
    // Passive checkpoint retry uses zero-wait transactions and releases the
    // mutex between attempts; sustained contention is still a failure.
    fn with_active_check<T>(&self,action:impl FnOnce(&Connection)->Result<T,String>)->Result<T,String>{
        let conn=self.conn.lock().map_err(|_|"Check report unavailable")?;
        conn.busy_timeout(Duration::from_millis(250)).map_err(|_|"Check report unavailable")?;
        let result=action(&conn);
        conn.busy_timeout(Duration::ZERO).map_err(|_|"Check report unavailable")?;
        result
    }
    // Only the trusted one-shot check controller may write this latest local
    // report. It contains fixed categories/counts, not conversation evidence.
    pub fn begin_active_check(&self,id:&str)->Result<(),String>{
        self.begin_check_suite(id,false)
    }
    pub fn begin_check_suite(&self,id:&str,model_suite:bool)->Result<(),String>{
        let body=serde_json::to_string(&CheckBody{model_suite,model_suite_version:if model_suite {crate::active_checks::MODEL_SUITE_VERSION}else{0},..CheckBody::default()}).map_err(|_|"Check report unavailable")?;
        self.with_active_check(|conn|{
            let rows=conn.execute("INSERT INTO active_check(slot,run_id,state,body) VALUES(1,?1,'running',?2)
                ON CONFLICT(slot) DO UPDATE SET run_id=excluded.run_id,state='running',body=excluded.body
                WHERE active_check.state<>'running'",params![id,body]).map_err(|_|"Check report unavailable")?;
            if rows!=1{return Err("A previous check has not been reconciled".into())} Ok(())
        })
    }
    pub fn write_active_check(&self,id:&str,state:CheckState,body:&CheckBody)->Result<(),String>{
        let report=CheckReport{state,body:body.clone()};
        if !report.valid(){return Err("Check report could not be confirmed".into())}
        self.with_active_check(|conn|{
            let old=Self::read_active_check(conn)?.ok_or("Check report could not be confirmed")?;
            if old.state!=CheckState::Running||old.body.expanded!=body.expanded||old.body.model_suite!=body.model_suite||old.body.model_suite_version!=body.model_suite_version||!body.results.starts_with(&old.body.results){return Err("Check report could not be confirmed".into())}
            let body=serde_json::to_string(body).map_err(|_|"Check report unavailable")?;
            let rows=conn.execute("UPDATE active_check SET state=?1,body=?2 WHERE slot=1 AND run_id=?3 AND state='running'",
                params![state.label(),body,id]).map_err(|_|"Check report unavailable")?;
            if rows!=1{return Err("Check report could not be confirmed".into())} Ok(())
        })
    }
    pub fn interrupt_active_check(&self,id:&str)->Result<(),String>{
        self.with_active_check(|conn|{
            conn.execute("UPDATE active_check SET state='interrupted' WHERE slot=1 AND run_id=?1 AND state='running'",[id])
                .map_err(|_|"Check report unavailable")?;Ok(())
        })
    }
    pub fn active_check_report(&self)->Result<Option<CheckReport>,String>{
        self.with_active_check(Self::read_active_check)
    }
    fn read_active_check(conn:&Connection)->Result<Option<CheckReport>,String>{
        let row:Option<(String,Option<String>)>=conn.query_row("SELECT state,CASE WHEN length(body)<=4096 THEN body END FROM active_check WHERE slot=1",[],|r|Ok((r.get(0)?,r.get(1)?)))
            .optional().map_err(|_|"Check report unavailable")?;
        let Some((state,body))=row else{return Ok(None)};
        let state=CheckState::parse(&state).ok_or("Check report could not be confirmed")?;
        let body=serde_json::from_str(&body.ok_or("Check report could not be confirmed")?).map_err(|_|"Check report could not be confirmed")?;
        let report=CheckReport{state,body};
        if !report.valid(){return Err("Check report could not be confirmed".into())} Ok(Some(report))
    }
    /// Require explicit native consent before exposing the report.
    /// Fixed-vocabulary findings only; never export stored trace references.
    pub fn report(&self)->Result<ArchiveReport,String>{
        let conn=self.conn.try_lock().map_err(|_|"Diagnostic report unavailable")?;
        let mut stmt=conn.prepare("SELECT trace_ref,body,state FROM dx_trace
            ORDER BY ordinal DESC LIMIT ?1").map_err(|_|"Diagnostic report unavailable")?;
        let rows=stmt.query_map([MAX_TRACES as i64],|r|Ok((
            r.get::<_,String>(0)?,r.get::<_,Vec<u8>>(1)?,r.get::<_,String>(2)?)))
            .map_err(|_|"Diagnostic report unavailable")?;
        let mut findings=Vec::new();
        let mut unavailable_records=0;
        for row in rows{
            let Ok((key,body,state))=row else{unavailable_records+=1;continue};
            let Ok(mut trace)=decode_diagnostic_trace(&body) else{unavailable_records+=1;continue};
            let terminal=trace.events.last().is_some_and(|e|e.stage==13||matches!(e.outcome,3..=5));
            let legacy_saved=trace.events.last().is_some_and(|e|e.stage==12 && trace.events.iter().any(|v|v.stage==6&&v.outcome==2));
            if trace.trace_ref!=key||!matches!(state.as_str(),"sealed"|"live"|"recovered")||
                (state=="sealed" && !terminal && !legacy_saved)||(state!="sealed" && terminal){
                unavailable_records+=1;continue
            }
            let recording_state=match state.as_str(){
                "sealed"=>"sealed",
                "recovered"=>{trace.dropped_events=trace.dropped_events.saturating_add(1);"recovered_incomplete"},
                _=>{trace.dropped_events=trace.dropped_events.saturating_add(1);"live_incomplete"}
            };
            match localize_transactional_turn(&trace){
                Ok(finding)=>{let (explanation,suggestion)=plain_finding(&finding);findings.push(ArchivedFinding{recording_state,finding,explanation,suggestion})},
                Err(_)=>unavailable_records+=1,
            }
        }
        let checked=Self::read_active_check(&conn);
        let active_check_unavailable=checked.is_err();
        Ok(ArchiveReport{schema:"lu.dx.cp01.local-report.v1",findings,unavailable_records,
            active_check:checked.ok().flatten(),active_check_unavailable,scheduled_runs:None,scheduled_runs_unavailable:false})
    }
}
/// Only fixed host vocabulary; never echo exception text, filenames or IDs.
fn plain_finding(f:&DiagnosticFinding)->(&'static str,&'static str){
    use lu_core::diagnostic_localization::EvidenceStrength::*;
    if f.evidence_strength==InsufficientEvidence{return ("Lu could not confirm where the request stopped.","Check the conversation before trying again. No automatic retry was made.")}
    if f.evidence_strength==Complete{return ("Lu confirmed the response was saved and reported displayed.","No action needed for delivery. This does not verify the answer's accuracy.")}
    match (f.failure_code,f.fault_region,f.confirmed_until) {
        ("authority_denied",_,_) => ("The request was not authorized.","Review the requested permission if you want to try again."),
        ("cancelled",_,_) => ("A stop was requested. Lu did not independently confirm worker exit.","Check the saved conversation before retrying."),
        ("backend_unavailable",_,_) => ("The loaded model could not complete the request.","Check that the model is loaded and available."),
        ("protocol",_,_) => ("Lu could not verify the model's structured response.","Try a clearer request or a model that supports the operation."),
        ("persistence",_,_) => ("Lu could not confirm a required saved record.","Check the conversation and available storage before trying again."),
        ("timeout",_,_) => ("The request reached its time limit.","Try a smaller request."),
        (_,"native_tool",_) => ("Lu could not confirm a complete tool response.","Check the selected resource and permissions before trying again."),
        (_,_,"assistant_saved") => ("Lu confirmed the response was saved, but not that it appeared on screen.","Reopen the conversation to check the saved response."),
        (_,"model_runtime",_) => ("The model did not complete the request.","Check the loaded model and try a smaller request."),
        _ => ("Lu could not confirm the next step of the request.","Check the conversation before trying again."),
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::causal_diagnostic::Checkpoint;
    use std::time::Instant;
    fn trace(ref_id:&str)->CausalProgress{CausalProgress::new(ref_id.to_owned(),Instant::now())}
    #[test]
    fn reader_trace_distinguishes_dispatch_feedback_saved_response_and_display(){
        use lu_core::agent_tool_loop::ToolDispatchCheckpoint::*;
        use lu_core::diagnostic_localization::EvidenceStrength;
        let archive=DiagnosticArchive::open(Path::new(":memory:")).unwrap();
        let mut t=trace("0123456789abcdef0123456789abcdef");
        for stage in [Checkpoint::SessionReady,Checkpoint::UserSaved,Checkpoint::WorkerEntered,Checkpoint::SavedAttested,Checkpoint::TaskAuthorized,Checkpoint::ModelEntered]{assert!(t.observe(stage));}
        t.tool_checkpoint(Entered);t.tool_checkpoint(Returned);
        assert!(archive.persist(&t));
        let r=archive.report().unwrap();
        assert_eq!(r.findings[0].finding.fault_region,"native_tool");
        assert_eq!(r.findings[0].finding.evidence_strength,EvidenceStrength::InsufficientEvidence);
        t.tool_checkpoint(FeedbackAudited);
        assert!(t.observe(Checkpoint::ModelReturned));
        assert!(t.observe(Checkpoint::AssistantSaved));
        assert!(archive.persist(&t));
        let r=archive.report().unwrap();
        assert_eq!(r.findings[0].recording_state,"sealed");
        assert_eq!(r.findings[0].finding.confirmed_until,"assistant_saved");
        assert_eq!(r.findings[0].finding.evidence_strength,EvidenceStrength::BoundedToNextStage);
        assert!(r.findings[0].explanation.contains("not that it appeared"));
        let mut refused=trace("fedcba9876543210fedcba9876543210");refused.human_declined();assert!(archive.persist(&refused));
        let r=archive.report().unwrap();assert_eq!(r.findings[0].finding.failure_code,"authority_denied");
        assert!(r.findings[0].explanation.contains("not authorized"));
    }
    #[test]
    fn recovery_never_promotes_open_trace_and_rejects_stale_write(){
        let path=std::env::temp_dir().join(format!("lu-cp01-dx-{}-{}.sqlite",std::process::id(),rand::random::<u64>()));
        let archive=DiagnosticArchive::open(&path).unwrap();
        let mut t=trace("0123456789abcdef0123456789abcdef");
        assert!(archive.persist(&t));
        let stale=t.clone();
        assert!(t.observe(Checkpoint::SessionReady));
        assert!(archive.persist(&t));
        assert!(!archive.persist(&stale));
        drop(archive);
        let reopened=DiagnosticArchive::open(&path).unwrap();
        assert!(!reopened.persist(&t));
        let r=reopened.report().unwrap();
        assert_eq!(r.findings.len(),1);
        assert_eq!(r.findings[0].recording_state,"recovered_incomplete");
        assert_eq!(r.findings[0].finding.evidence_strength,
            lu_core::diagnostic_localization::EvidenceStrength::InsufficientEvidence);
        drop(reopened);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn cp12ar1_cancellation_and_abrupt_recovery_never_claim_worker_exit_or_layout(){
        use lu_core::diagnostic_localization::EvidenceStrength;
        let path=std::env::temp_dir().join(format!("lu-cp12a-recover-{}-{}.sqlite",std::process::id(),rand::random::<u64>()));
        let archive=DiagnosticArchive::open(&path).unwrap();
        let mut cancelled=trace("0123456789abcdef0123456789abcdef");
        for cp in [Checkpoint::SessionReady,Checkpoint::UserSaved,Checkpoint::WorkerEntered,Checkpoint::SavedAttested,Checkpoint::ModelEntered]{assert!(cancelled.observe(cp));}
        cancelled.observed_cancel();assert!(archive.persist(&cancelled));
        assert!(!cancelled.observations().iter().any(|e|matches!(e.stage,17|18|13)));
        let r=archive.report().unwrap();assert_eq!(r.findings[0].finding.failure_code,"cancelled");assert_eq!(r.findings[0].finding.evidence_strength,EvidenceStrength::ObservedFailure);
        let mut pending=trace("fedcba9876543210fedcba9876543210").awaiting_delivery();
        for cp in [Checkpoint::SessionReady,Checkpoint::UserSaved,Checkpoint::WorkerEntered,Checkpoint::SavedAttested,Checkpoint::TaskAuthorized,Checkpoint::ModelEntered,Checkpoint::ModelReturned,Checkpoint::AssistantSaved]{assert!(pending.observe(cp));}
        assert!(archive.persist(&pending));assert!(!pending.terminal());
        drop(archive);
        let archive=DiagnosticArchive::open(&path).unwrap();
        assert!(pending.observe(Checkpoint::ReplyObserved));assert!(!archive.persist(&pending));
        let r=archive.report().unwrap();assert_eq!(r.unavailable_records,0);assert_eq!(r.findings[0].recording_state,"recovered_incomplete");assert_eq!(r.findings[0].finding.evidence_strength,EvidenceStrength::InsufficientEvidence);
        assert_eq!(r.findings[1].finding.failure_code,"cancelled");
        drop(archive);std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn storage_is_bounded_and_corrupt_row_is_not_reported_as_success(){
        let archive=DiagnosticArchive::open(Path::new(":memory:")).unwrap();
        for i in 0..80{assert!(archive.persist(&trace(&format!("{i:032x}"))));}
        assert_eq!(archive.report().unwrap().findings.len(),MAX_TRACES);
        archive.conn.lock().unwrap().execute(
            "UPDATE dx_trace SET body=x'FF' WHERE ordinal=(SELECT MAX(ordinal) FROM dx_trace)",[]).unwrap();
        let r=archive.report().unwrap();
        assert_eq!(r.findings.len(),MAX_TRACES-1);
        assert_eq!(r.unavailable_records,1);
    }
    #[test]
    fn cp12ar2r1_checkpoint_retries_read_contention_without_losing_prefix(){
        use rusqlite::OpenFlags;
        use std::sync::mpsc;
        let path=std::env::temp_dir().join(format!("lu-dx-contention-{}-{}.sqlite",std::process::id(),rand::random::<u64>()));
        let archive=DiagnosticArchive::open(&path).unwrap();
        let mut t=trace("0123456789abcdef0123456789abcdef");assert!(archive.persist(&t));
        let reader=Connection::open_with_flags(&path,OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        reader.execute_batch("BEGIN").unwrap();
        assert_eq!(reader.query_row("SELECT count(*) FROM dx_trace",[],|r|r.get::<_,u32>(0)).unwrap(),1);
        assert!(t.observe(Checkpoint::SessionReady));
        std::thread::scope(|scope|{
            let (started_tx,started_rx)=mpsc::channel();let (done_tx,done_rx)=mpsc::channel();
            let (archive,t)=(&archive,&t);
            scope.spawn(move||{started_tx.send(()).unwrap();done_tx.send(archive.persist(t)).unwrap();});
            started_rx.recv().unwrap();
            let early=done_rx.recv_timeout(Duration::from_millis(40));
            reader.execute_batch("COMMIT").unwrap();
            assert!(matches!(early,Err(mpsc::RecvTimeoutError::Timeout)),"{early:?}");
            assert!(done_rx.recv_timeout(Duration::from_secs(2)).unwrap());
        });
        let body:Vec<u8>=reader.query_row("SELECT body FROM dx_trace",[],|r|r.get(0)).unwrap();
        assert_eq!(decode_diagnostic_trace(&body).unwrap(),wire_trace(&t));
        assert_eq!(archive.conn.lock().unwrap().query_row("PRAGMA busy_timeout",[],|r|r.get::<_,u32>(0)).unwrap(),0);
        drop(reader);drop(archive);std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn cp12ar2r1_sustained_contention_never_commits_a_false_completion(){
        use rusqlite::OpenFlags;
        let path=std::env::temp_dir().join(format!("lu-dx-blocked-{}-{}.sqlite",std::process::id(),rand::random::<u64>()));
        let archive=DiagnosticArchive::open(&path).unwrap();let mut t=trace("0123456789abcdef0123456789abcdef");assert!(archive.persist(&t));
        let original=wire_trace(&t);
        let reader=Connection::open_with_flags(&path,OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();reader.execute_batch("BEGIN").unwrap();
        assert_eq!(reader.query_row("SELECT count(*) FROM dx_trace",[],|r|r.get::<_,u32>(0)).unwrap(),1);
        t.native_failed();let began=Instant::now();assert!(!archive.persist(&t));
        assert!(began.elapsed()<Duration::from_secs(2));
        reader.execute_batch("COMMIT").unwrap();
        let body:Vec<u8>=reader.query_row("SELECT body FROM dx_trace",[],|r|r.get(0)).unwrap();assert_eq!(decode_diagnostic_trace(&body).unwrap(),original);
        t.mark_lost();assert!(archive.persist(&t));
        let r=archive.report().unwrap();assert_eq!(r.findings[0].finding.evidence_strength,lu_core::diagnostic_localization::EvidenceStrength::InsufficientEvidence);
        drop(reader);drop(archive);std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn cp11a_readonly_witness_does_not_interrupt_active_publication(){
        use crate::active_checks::{CheckKind,CheckResult};
        use rusqlite::OpenFlags;
        use std::sync::mpsc::{self,RecvTimeoutError};
        let path=std::env::temp_dir().join(format!("lu-cp11a-read-contention-{}-{}.sqlite",std::process::id(),rand::random::<u64>()));
        let archive=DiagnosticArchive::open(&path).unwrap();
        archive.begin_active_check("check-reader-contention").unwrap();
        let reader=Connection::open_with_flags(&path,OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        reader.execute_batch("BEGIN").unwrap();
        assert_eq!(reader.query_row("SELECT state FROM active_check WHERE slot=1",[],|r|r.get::<_,String>(0)).unwrap(),"running");
        let body=CheckBody{results:vec![CheckResult{kind:CheckKind::Reading,passed:true,verified_operations:3,elapsed_micros:1,inference:None}],..CheckBody::default()};
        std::thread::scope(|scope|{
            let (ready_tx,ready_rx)=mpsc::channel();
            let (done_tx,done_rx)=mpsc::channel();
            let (archive,body)=(&archive,&body);
            scope.spawn(move||{
                ready_tx.send(()).unwrap();
                done_tx.send(archive.write_active_check("check-reader-contention",CheckState::Running,body)).unwrap();
            });
            ready_rx.recv().unwrap();
            let early=done_rx.recv_timeout(Duration::from_millis(40));
            reader.execute_batch("COMMIT").unwrap();
            assert!(matches!(early,Err(RecvTimeoutError::Timeout)),"publication prematurely failed or completed: {early:?}");
            done_rx.recv_timeout(Duration::from_secs(2)).unwrap().unwrap();
        });
        assert_eq!(archive.active_check_report().unwrap().unwrap().body,body);
        assert_eq!(archive.conn.lock().unwrap().query_row("PRAGMA busy_timeout",[],|r|r.get::<_,u32>(0)).unwrap(),0);
        drop(reader);drop(archive);std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn cp11a_sustained_read_contention_fails_closed_and_restores_nonblocking_policy(){
        use rusqlite::OpenFlags;
        let path=std::env::temp_dir().join(format!("lu-cp11a-sustained-contention-{}-{}.sqlite",std::process::id(),rand::random::<u64>()));
        let archive=DiagnosticArchive::open(&path).unwrap();
        let reader=Connection::open_with_flags(&path,OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        reader.execute_batch("BEGIN").unwrap();
        assert_eq!(reader.query_row("SELECT count(*) FROM active_check",[],|r|r.get::<_,u32>(0)).unwrap(),0);
        let started=Instant::now();
        assert!(archive.begin_active_check("check-blocked").is_err());
        assert!(started.elapsed()<Duration::from_secs(3));
        reader.execute_batch("COMMIT").unwrap();
        assert!(archive.active_check_report().unwrap().is_none());
        assert_eq!(archive.conn.lock().unwrap().query_row("PRAGMA busy_timeout",[],|r|r.get::<_,u32>(0)).unwrap(),0);
        archive.begin_active_check("check-fresh").unwrap();
        assert_eq!(archive.active_check_report().unwrap().unwrap().state,CheckState::Running);
        drop(reader);drop(archive);std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn cp11a_invalid_active_report_does_not_invent_success(){
        let archive=DiagnosticArchive::open(Path::new(":memory:")).unwrap();
        archive.begin_active_check("check-invalid").unwrap();
        archive.conn.lock().unwrap().execute("UPDATE active_check SET state='complete',body='{}'",[]).unwrap();
        assert!(archive.active_check_report().is_err());
        let report=archive.report().unwrap();assert!(report.active_check.is_none());assert!(report.active_check_unavailable);
    }

}
