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
    /// Best-effort write outside the scheduler/conversation transaction.
    /// Never invoked from the per-token callback.
    pub fn persist(&self,source:&CausalProgress)->bool{
        let trace=wire_trace(source);
        let Ok(body)=encode_diagnostic_trace(&trace) else{return false};
        let Ok(mut conn)=self.conn.try_lock() else{return false};
        let Ok(tx)=conn.transaction() else{return false};
        let prior=tx.query_row("SELECT body,state FROM dx_trace WHERE trace_ref=?1",
            [&trace.trace_ref],|r|Ok((r.get::<_,Vec<u8>>(0)?,r.get::<_,String>(1)?))).optional();
        let state=if source.terminal(){"sealed"}else{"live"};
        match prior{
            Ok(Some((previous,old_state)))=>{
                if previous==body && old_state==state{return true}
                if old_state!="live"{return false}
                let Ok(old)=decode_diagnostic_trace(&previous) else{return false};
                if old.trace_ref!=trace.trace_ref||old.dropped_events>trace.dropped_events||
                    !trace.events.starts_with(&old.events){return false}
                if tx.execute("UPDATE dx_trace SET body=?1,state=?2 WHERE trace_ref=?3",
                    params![body,state,trace.trace_ref]).is_err(){return false}
            },
            Ok(None)=>{
                if tx.execute("INSERT INTO dx_trace(trace_ref,body,state) VALUES(?1,?2,?3)",
                    params![trace.trace_ref,body,state]).is_err(){return false}
            },
            Err(_)=>return false,
        }
        if tx.execute("DELETE FROM dx_trace WHERE ordinal NOT IN
            (SELECT ordinal FROM dx_trace ORDER BY ordinal DESC LIMIT ?1)",
            [MAX_TRACES as i64]).is_err(){return false}
        tx.commit().is_ok()
    }
    // Only the trusted one-shot check controller may write this latest local
    // report. It contains fixed categories/counts, not conversation evidence.
    pub fn begin_active_check(&self,id:&str)->Result<(),String>{
        let body=serde_json::to_string(&CheckBody::default()).map_err(|_|"Check report unavailable")?;
        let conn=self.conn.lock().map_err(|_|"Check report unavailable")?;
        let rows=conn.execute("INSERT INTO active_check(slot,run_id,state,body) VALUES(1,?1,'running',?2)
            ON CONFLICT(slot) DO UPDATE SET run_id=excluded.run_id,state='running',body=excluded.body
            WHERE active_check.state<>'running'",params![id,body]).map_err(|_|"Check report unavailable")?;
        if rows!=1{return Err("A previous check has not been reconciled".into())} Ok(())
    }
    pub fn write_active_check(&self,id:&str,state:CheckState,body:&CheckBody)->Result<(),String>{
        let report=CheckReport{state,body:body.clone()};
        if !report.valid(){return Err("Check report could not be confirmed".into())}
        let conn=self.conn.lock().map_err(|_|"Check report unavailable")?;
        let old=Self::read_active_check(&conn)?.ok_or("Check report could not be confirmed")?;
        if old.state!=CheckState::Running||old.body.expanded!=body.expanded||!body.results.starts_with(&old.body.results){return Err("Check report could not be confirmed".into())}
        let body=serde_json::to_string(body).map_err(|_|"Check report unavailable")?;
        let rows=conn.execute("UPDATE active_check SET state=?1,body=?2 WHERE slot=1 AND run_id=?3 AND state='running'",
            params![state.label(),body,id]).map_err(|_|"Check report unavailable")?;
        if rows!=1{return Err("Check report could not be confirmed".into())} Ok(())
    }
    pub fn interrupt_active_check(&self,id:&str)->Result<(),String>{
        let conn=self.conn.lock().map_err(|_|"Check report unavailable")?;
        conn.execute("UPDATE active_check SET state='interrupted' WHERE slot=1 AND run_id=?1 AND state='running'",[id])
            .map_err(|_|"Check report unavailable")?;Ok(())
    }
    pub fn active_check_report(&self)->Result<Option<CheckReport>,String>{
        let conn=self.conn.lock().map_err(|_|"Check report unavailable")?;
        Self::read_active_check(&conn)
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
            if trace.trace_ref!=key||!matches!(state.as_str(),"sealed"|"live"|"recovered")||
                (state=="sealed")!=terminal{
                unavailable_records+=1;continue
            }
            let recording_state=match state.as_str(){
                "sealed"=>"sealed",
                "recovered"=>{trace.dropped_events=trace.dropped_events.saturating_add(1);"recovered_incomplete"},
                _=>{trace.dropped_events=trace.dropped_events.saturating_add(1);"live_incomplete"}
            };
            match localize_transactional_turn(&trace){
                Ok(finding)=>findings.push(ArchivedFinding{recording_state,finding}),
                Err(_)=>unavailable_records+=1,
            }
        }
        let checked=Self::read_active_check(&conn);
        let active_check_unavailable=checked.is_err();
        Ok(ArchiveReport{schema:"lu.dx.cp01.local-report.v1",findings,unavailable_records,
            active_check:checked.ok().flatten(),active_check_unavailable})
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::causal_diagnostic::Checkpoint;
    use std::time::Instant;
    fn trace(ref_id:&str)->CausalProgress{CausalProgress::new(ref_id.to_owned(),Instant::now())}
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
    fn cp11a_invalid_active_report_does_not_invent_success(){
        let archive=DiagnosticArchive::open(Path::new(":memory:")).unwrap();
        archive.begin_active_check("check-invalid").unwrap();
        archive.conn.lock().unwrap().execute("UPDATE active_check SET state='complete',body='{}'",[]).unwrap();
        assert!(archive.active_check_report().is_err());
        let report=archive.report().unwrap();assert!(report.active_check.is_none());assert!(report.active_check_unavailable);
    }

}
