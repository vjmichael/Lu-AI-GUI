//! CP-11A: human-initiated, bounded local checks. Not a model tool, shell,
//! scheduler, permission grant, source updater or general benchmark runner.
//! Only fixed synthetic data is used. Reports contain no user content.
use crate::diagnostic_archive::DiagnosticArchive;
use lu_core::{AnsfStore, EntryRole, RevisionOrigin, native_reader::{self, ReaderFailure, ReaderFormat}};
use serde::{Deserialize, Serialize};
use std::{path::{Path, PathBuf}, sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}}, time::{Duration, Instant}};

const SCRATCH: &str = "lu-check-work";
// Fixed bounded slots; never open, move or delete an abandoned directory.
const SCRATCH_SLOTS: [&str; 4] = [SCRATCH, "lu-check-work-2", "lu-check-work-3", "lu-check-work-4"];
const ROWS: u32 = 128;
const QUERIES: u32 = 16;
const SCALE_ROWS: u32 = 1024;
const SCALE_QUERIES: u32 = 64;
const WINDOW_QUERIES: u32 = 16;
const RUN_LIMIT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all="snake_case")]
pub(crate) enum CheckKind { Reading, SavedHistory, SearchSpeed, DocumentWindows, SearchScale, ModelExtract, ModelArithmetic, ModelHistory }
pub(crate) const MODEL_CHECKS:[CheckKind;3]=[CheckKind::ModelExtract,CheckKind::ModelArithmetic,CheckKind::ModelHistory];
const LEGACY_CHECKS: [CheckKind; 3] = [CheckKind::Reading, CheckKind::SavedHistory, CheckKind::SearchSpeed];
const CHECKS: [CheckKind; 5] = [CheckKind::Reading, CheckKind::SavedHistory, CheckKind::SearchSpeed, CheckKind::DocumentWindows, CheckKind::SearchScale];
impl CheckKind {
    fn expected(self)->u32 { match self { Self::Reading=>3, Self::SavedHistory=>4, Self::SearchSpeed=>QUERIES, Self::DocumentWindows=>WINDOW_QUERIES, Self::SearchScale=>SCALE_QUERIES,Self::ModelExtract|Self::ModelArithmetic|Self::ModelHistory=>1 } }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all="snake_case")]
pub(crate) enum CheckState { Running, Complete, Attention, Stopped, Interrupted }
impl CheckState {
    pub fn label(self)-> &'static str { match self { Self::Running=>"running",Self::Complete=>"complete",Self::Attention=>"attention",Self::Stopped=>"stopped",Self::Interrupted=>"interrupted" } }
    pub fn parse(s:&str)->Option<Self> { Some(match s { "running"=>Self::Running,"complete"=>Self::Complete,"attention"=>Self::Attention,"stopped"=>Self::Stopped,"interrupted"=>Self::Interrupted,_=>return None }) }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all="snake_case")]
pub(crate) enum CheckReason { UserStopped, ChatBusy, TimeLimit, WorkspaceUnavailable, CleanupUnavailable, CheckFailed, ModelUnavailable }
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct CheckResult { pub kind:CheckKind, pub passed:bool, pub verified_operations:u32, pub elapsed_micros:u64, #[serde(default,skip_serializing_if="Option::is_none")] pub inference:Option<crate::model_benchmark::InferenceMetrics> }
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct CheckBody { pub results:Vec<CheckResult>, pub reason:Option<CheckReason>, #[serde(default)] pub expanded:bool, #[serde(default,skip_serializing_if="std::ops::Not::not")] pub model_suite:bool }
impl Default for CheckBody { fn default()->Self { Self { results:Vec::new(),reason:None,expanded:true,model_suite:false } } }
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct CheckReport { pub state:CheckState, #[serde(flatten)] pub body:CheckBody }
impl CheckReport {
    pub fn valid(&self)->bool {
        let suite:&[CheckKind]=if self.body.model_suite { &MODEL_CHECKS } else if self.body.expanded { &CHECKS } else { &LEGACY_CHECKS };
        if self.body.model_suite&&!self.body.expanded{return false}
        if self.body.results.len()>suite.len() || self.body.results.iter().zip(suite).enumerate().any(|(i,(r,k))|
            r.kind!=*k || r.verified_operations!=if r.passed {k.expected()}else{0} || (!self.body.model_suite && !r.passed && i+1!=self.body.results.len()) ||
            if self.body.model_suite {r.inference.as_ref().is_none_or(|m|!m.valid(r.elapsed_micros)||(r.passed&&m.output_bytes==0))} else {r.inference.is_some()}) { return false }
        let all_pass=self.body.results.len()==suite.len()&&self.body.results.iter().all(|r|r.passed);
        match self.state {
            CheckState::Running=>self.body.reason.is_none(),
            CheckState::Complete=>all_pass&&self.body.reason.is_none(),
            CheckState::Attention=>matches!(self.body.reason,Some(CheckReason::CheckFailed|CheckReason::WorkspaceUnavailable|CheckReason::CleanupUnavailable|CheckReason::ModelUnavailable)),
            CheckState::Stopped=>matches!(self.body.reason,Some(CheckReason::UserStopped|CheckReason::ChatBusy|CheckReason::TimeLimit)),
            CheckState::Interrupted=>self.body.reason.is_none(),
        }
    }
}
#[derive(Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct CheckStatus { pub active:bool, pub report:Option<CheckReport> }
#[derive(Default)]
struct Gate { active:Option<Arc<AtomicBool>>, closed:bool }
pub(crate) struct CheckController { root:PathBuf, gate:Mutex<Gate> }
struct Reservation<'a> { owner:&'a CheckController, cancel:Arc<AtomicBool> }
impl Drop for Reservation<'_> { fn drop(&mut self){ if let Ok(mut gate)=self.owner.gate.lock(){
    if gate.active.as_ref().is_some_and(|a|Arc::ptr_eq(a,&self.cancel)){gate.active=None;}
} } }
impl CheckController {
    pub fn new(root:&Path)->Self { Self{root:root.to_owned(),gate:Mutex::new(Gate::default())} }
    fn reserve(&self)->Result<Reservation<'_>,String>{
        let mut gate=self.gate.lock().map_err(|_|"Local checks unavailable")?;
        if gate.closed||gate.active.is_some(){return Err("A check is already active or Lu is closing".into())}
        let cancel=Arc::new(AtomicBool::new(false)); gate.active=Some(cancel.clone());
        Ok(Reservation{owner:self,cancel})
    }
    pub fn stop(&self)->bool { self.gate.lock().is_ok_and(|g|g.active.as_ref().is_some_and(|c|{c.store(true,Ordering::Release);true})) }
    pub fn shutdown(&self){ if let Ok(mut g)=self.gate.lock(){g.closed=true;if let Some(c)=&g.active{c.store(true,Ordering::Release);}} }
    pub fn active(&self)->bool { self.gate.lock().map_or(true,|g|g.active.is_some()) }
    pub fn status(&self,archive:&DiagnosticArchive)->Result<CheckStatus,String>{
        // Read active state first; a new reservation cannot replace a running
        // report without a fresh native confirmation.
        let gate=self.gate.lock().map_err(|_|"Local checks unavailable")?;
        let active=gate.active.is_some(); let mut report=archive.active_check_report()?;
        if !active {if let Some(r)=&mut report {if r.state==CheckState::Running {r.state=CheckState::Interrupted;}}}
        Ok(CheckStatus{active,report})
    }
    pub fn run(&self, archive:&DiagnosticArchive, consent:impl FnOnce()->bool, user_busy:impl Fn()->bool+Sync)->Result<Option<CheckReport>,String>{
        self.run_suite(archive,consent,user_busy,false,|stop,publish|run_checks(&self.root,stop,publish))
    }
    pub(crate) fn run_suite(&self, archive:&DiagnosticArchive, consent:impl FnOnce()->bool,user_busy:impl Fn()->bool+Sync,model_suite:bool,
        execute:impl FnOnce(&(dyn Fn()->Option<CheckReason>+Sync),&mut dyn FnMut(&CheckBody)->Result<(),String>)->Result<CheckReport,String>)->Result<Option<CheckReport>,String>{
        let reservation=self.reserve()?;
        if user_busy(){return Err("Finish your current conversation before running a check".into())}
        if !consent()||reservation.cancel.load(Ordering::Acquire){return Ok(None)}
        if user_busy(){return Err("Finish your current conversation before running a check".into())}
        let id=crate::next_id("check-");
        archive.begin_check_suite(&id,model_suite)?;
        let started=Instant::now();
        let stop=|| {
            if reservation.cancel.load(Ordering::Acquire){Some(CheckReason::UserStopped)}
            else if user_busy(){Some(CheckReason::ChatBusy)}
            else if started.elapsed()>=if model_suite {Duration::from_secs(120)} else {RUN_LIMIT}{Some(CheckReason::TimeLimit)}else{None}
        };
        let result=execute(&stop, &mut |body|archive.write_active_check(&id,CheckState::Running,body));
        let report=match result {Ok(report)=>report,Err(e)=>{
            let _=archive.interrupt_active_check(&id);return Err(e)
        }};
        // A stop accepted while finishing must not publish a new complete
        // verdict. Serialize final publication with Stop and shutdown.
        let mut gate=self.gate.lock().map_err(|_|"Local checks unavailable")?;
        let mut report=report;
        if reservation.cancel.load(Ordering::Acquire)&&(report.state==CheckState::Complete||(model_suite&&report.state==CheckState::Attention)) {
            report.state=CheckState::Stopped; report.body.reason=Some(CheckReason::UserStopped);
        }
        if let Err(e)=archive.write_active_check(&id,report.state,&report.body){let _=archive.interrupt_active_check(&id);return Err(e)}
        // Independent typed reread of the committed report, not a model claim.
        let saved=archive.active_check_report()?.ok_or("Check report unavailable")?;
        if saved!=report{return Err("Check report could not be confirmed".into())}
        gate.active=None;drop(gate);
        Ok(Some(saved))
    }
}

// A bounded set of exclusively created disposable workspaces, never the actual chat
// database. Do not reuse or delete a pre-existing directory. Only explicitly
// named files created by this run are removed (no recursive deletion).
struct Scratch { path:PathBuf, dir:cap_std::fs::Dir, cleaned:bool }
impl Scratch {
    fn new(root:&Path)->Result<Self,()> {
        let mut builder=std::fs::DirBuilder::new();
        #[cfg(unix)] {use std::os::unix::fs::DirBuilderExt;builder.mode(0o700);}
        for name in SCRATCH_SLOTS {
            let path=root.join(name);
            match builder.create(&path) {
                Ok(())=>{
                    let dir=cap_std::fs::Dir::open_ambient_dir(&path,cap_std::ambient_authority()).map_err(|_|())?;
                    return Ok(Self{path,dir,cleaned:false})
                },
                Err(e) if e.kind()==std::io::ErrorKind::AlreadyExists=>continue,
                Err(_)=>return Err(()),
            }
        }
        Err(())
    }
    fn cleanup(&mut self)->Result<(),()> {
        for name in ["history.sqlite","history.sqlite-wal","history.sqlite-shm","history.sqlite-journal"] {
            match self.dir.remove_file(name){Ok(())=>(),Err(e) if e.kind()==std::io::ErrorKind::NotFound=>(),Err(_)=>return Err(())}
        }
        std::fs::remove_dir(&self.path).map_err(|_|())?;
        self.cleaned=true;Ok(())
    }
}
impl Drop for Scratch { fn drop(&mut self){if !self.cleaned{let _=self.cleanup();}} }
pub(crate) fn finish(body:CheckBody,state:CheckState,reason:Option<CheckReason>)->CheckReport {
    CheckReport{state,body:CheckBody{reason,..body}}
}
fn run_checks(root:&Path,stop:&(impl Fn()->Option<CheckReason>+?Sized),mut publish:impl FnMut(&CheckBody)->Result<(),String>)->Result<CheckReport,String>{
    let mut body=CheckBody::default();
    if let Some(reason)=stop(){return Ok(finish(body,CheckState::Stopped,Some(reason)))}
    let mut scratch=match Scratch::new(root){Ok(s)=>s,Err(_)=>return Ok(finish(body,CheckState::Attention,Some(CheckReason::WorkspaceUnavailable)))};
    let work=scratch.path.join("history.sqlite");
    let mut reason=None;
    for kind in CHECKS {
        if let Some(r)=stop(){reason=Some(r);break}
        let before=Instant::now();
        let checked=match kind {
            CheckKind::Reading=>reader_check(),
            CheckKind::SavedHistory=>history_check(&work),
            CheckKind::SearchSpeed=>search_check(&work,stop),
            CheckKind::DocumentWindows=>document_windows_check(stop),
            CheckKind::SearchScale=>search_scale_check(&work,stop),
            _=>Err(()),
        };
        let elapsed_micros=before.elapsed().as_micros().min(u64::MAX as u128) as u64;
        if let Some(r)=stop(){reason=Some(r);break}
        let passed=checked.is_ok();
        body.results.push(CheckResult{kind,passed,verified_operations:if passed{kind.expected()}else{0},elapsed_micros,inference:None});
        publish(&body)?;
        // Later search checks require the successfully created fixture.
        if !passed{reason=Some(CheckReason::CheckFailed);break}
    }
    if scratch.cleanup().is_err(){return Ok(finish(body,CheckState::Attention,Some(CheckReason::CleanupUnavailable)))}
    if let Some(r)=reason.or_else(stop){
        let state=if r==CheckReason::CheckFailed{CheckState::Attention}else{CheckState::Stopped};
        return Ok(finish(body,state,Some(r)))
    }
    Ok(finish(body,CheckState::Complete,None))
}
fn reader_check()->Result<(),()> {
    let text="Lu check — local document\nSecond line";
    let result=native_reader::read_text_bytes("sample.txt",text.as_bytes(),128).map_err(|_|())?;
    if result.text!=text||result.truncated||result.format!=ReaderFormat::PlainText||result.extracted_from_image{return Err(())}
    let window=native_reader::read_plain_text_window("sample.txt",text.as_bytes(),9,1,false).map_err(|_|())?.ok_or(())?;
    if window.text!="—"||window.total_chars!=text.chars().count(){return Err(())}
    if native_reader::read_text_bytes("sample.txt",b"\xffbad",128)!=Err(ReaderFailure::UnsupportedEncoding){return Err(())}
    Ok(())
}
fn history_check(path:&Path)->Result<(),()> {
    let store=AnsfStore::open(path).map_err(|_|())?;
    store.create_session("check-a","test","Sample").map_err(|_|())?;
    store.create_session("check-b","test","Other sample").map_err(|_|())?;
    store.start_chapter("check-a","Sample").map_err(|_|())?;
    store.start_chapter("check-b","Sample").map_err(|_|())?;
    let original=store.append_entry("check-a","old",EntryRole::User,"sample recall","sample recall old value").map_err(|_|())?;
    // This is an observed synthetic revision, not an attestation of a real person's edit.
    let revision=store.append_edited_entry("check-a","old","sample recall","sample recall corrected value",RevisionOrigin::ImportObserved).map_err(|_|())?.ok_or(())?;
    drop(store);
    let store=AnsfStore::open(path).map_err(|_|())?;
    if store.read_entry("check-a",original).map_err(|_|())?.is_none_or(|e|e.content!="sample recall old value"){return Err(())}
    if store.current_message("check-a","old").map_err(|_|())?.is_none_or(|e|e.entry.id!=revision.revised_entry_id||e.entry.content!="sample recall corrected value"){return Err(())}
    if store.read_current_entry("check-a",original).map_err(|_|())?.is_some(){return Err(())}
    if store.read_entry("check-b",original).map_err(|_|())?.is_some(){return Err(())}
    Ok(())
}
fn search_check(path:&Path,stop:&(impl Fn()->Option<CheckReason>+?Sized))->Result<(),()> {
    let store=AnsfStore::open(path).map_err(|_|())?;
    // Small fixed workload; includes bounded fixture construction and query
    // validation. Do not call this a model, paired or system-wide benchmark.
    for i in 0..ROWS {
        if stop().is_some(){return Err(())}
        let id=format!("sample{i:03}"); let text=format!("retrieval experiment {id}");
        store.append_entry("check-a",&id,EntryRole::User,&text,&text).map_err(|_|())?;
    }
    for i in 0..QUERIES {
        if stop().is_some(){return Err(())}
        let id=format!("sample{:03}",i*7); let text=format!("retrieval experiment {id}");
        let rows=store.search_current_messages("check-a",&id,2).map_err(|_|())?;
        if rows.len()!=1||rows[0].message_id!=id||rows[0].content!=text{return Err(())}
    }
    Ok(())
}

// Fixed sample corpus and independent expected character windows. No user data.
fn document_windows_check(stop:&(impl Fn()->Option<CheckReason>+?Sized))->Result<(),()> {
    let pattern="a—漢🙂";
    let text=pattern.repeat(8192);
    let expected:Vec<char>=text.chars().collect();
    for i in 0..WINDOW_QUERIES as usize {
        if stop().is_some(){return Err(())}
        let start=i*2047;
        let end=(start+257).min(expected.len());
        let window=native_reader::read_plain_text_window("sample.txt",text.as_bytes(),start,257,false).map_err(|_|())?.ok_or(())?;
        if window.total_chars!=expected.len()||window.text!=expected[start..end].iter().collect::<String>(){return Err(())}
    }
    Ok(())
}
fn search_scale_check(path:&Path,stop:&(impl Fn()->Option<CheckReason>+?Sized))->Result<(),()> {
    let store=AnsfStore::open(path).map_err(|_|())?;
    store.create_session("check-scale","test","Scaled sample").map_err(|_|())?;
    store.start_chapter("check-scale","Sample").map_err(|_|())?;
    for i in 0..SCALE_ROWS {
        if stop().is_some(){return Err(())}
        let id=format!("scaled{i:04}"); let text=format!("scaled retrieval sample {id}");
        store.append_entry("check-scale",&id,EntryRole::User,&text,&text).map_err(|_|())?;
    }
    drop(store);
    let store=AnsfStore::open(path).map_err(|_|())?;
    for i in 0..SCALE_QUERIES {
        if stop().is_some(){return Err(())}
        let id=format!("scaled{:04}",i*16+7); let text=format!("scaled retrieval sample {id}");
        let rows=store.search_current_messages("check-scale",&id,2).map_err(|_|())?;
        if rows.len()!=1||rows[0].message_id!=id||rows[0].content!=text{return Err(())}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn root()->PathBuf {let p=std::env::temp_dir().join(crate::next_id("lu-check-test-"));std::fs::create_dir(&p).unwrap();p}
    #[test] fn cp11a_real_checks_and_durable_report() {
        let root=root(); let path=root.join("trace.sqlite");
        let archive=DiagnosticArchive::open(&path).unwrap();let c=CheckController::new(&root);
        assert!(c.status(&archive).unwrap().report.is_none());
        assert!(c.run(&archive,||false,||false).unwrap().is_none());
        assert!(c.status(&archive).unwrap().report.is_none());
        let report=c.run(&archive,||true,||false).unwrap().unwrap();
        assert_eq!(report.state,CheckState::Complete);assert!(report.valid());
        assert_eq!(report.body.results.iter().map(|r|r.verified_operations).collect::<Vec<_>>(),vec![3,4,16,16,64]);
        println!("CHECK_REPORT {}",serde_json::to_string(&report).unwrap());
        let support=serde_json::to_value(archive.report().unwrap()).unwrap();
        assert_eq!(support["activeCheck"],serde_json::to_value(&report).unwrap());
        assert_eq!(support["activeCheckUnavailable"],false);
        assert!(!root.join(SCRATCH).exists());drop(archive);
        let reopened=DiagnosticArchive::open(&path).unwrap(); assert_eq!(reopened.active_check_report().unwrap(),Some(report));
        drop(reopened);std::fs::remove_dir_all(root).unwrap();
    }
    #[test] fn cp11a_refusal_duplicate_busy_stop_and_shutdown() {
        let root=root();let a=DiagnosticArchive::open(&root.join("trace.sqlite")).unwrap();let c=CheckController::new(&root);
        assert!(c.run(&a,||panic!("busy must not review"),||true).is_err());
        let r=c.run(&a,||{assert!(c.run(&a,||panic!("no second review"),||false).is_err());assert!(c.stop());true},||false).unwrap();
        assert!(r.is_none());assert!(a.active_check_report().unwrap().is_none());
        c.shutdown();assert!(c.run(&a,||true,||false).is_err());drop(a);std::fs::remove_dir_all(root).unwrap();
    }
    #[test] fn cp11a_cancellation_during_work_keeps_only_verified_prefix() {
        let root=root();let stop=AtomicBool::new(false);
        let report=run_checks(&root,&||stop.load(Ordering::Acquire).then_some(CheckReason::UserStopped),|body|{
            if body.results.len()==1{stop.store(true,Ordering::Release)} Ok(())
        }).unwrap();assert_eq!(report.state,CheckState::Stopped);assert_eq!(report.body.results.len(),1);
        assert!(!root.join(SCRATCH).exists());std::fs::remove_dir(root).unwrap();
    }
    #[test] fn cp11a_controller_stop_during_native_work_is_durable() {
        let root=root();let path=root.join("trace.sqlite");let a=DiagnosticArchive::open(&path).unwrap();let c=CheckController::new(&root);
        std::thread::scope(|scope|{
            let run=scope.spawn(||c.run(&a,||true,||false));
            let deadline=Instant::now()+Duration::from_secs(10);
            loop {
                if let Some(r)=a.active_check_report().unwrap(){
                    if r.state==CheckState::Running&&r.body.results.len()>=3&&r.body.results.len()<CHECKS.len(){break}
                    assert_eq!(r.state,CheckState::Running,"native run ended before cancellation witness");
                }
                assert!(Instant::now()<deadline,"native cancellation witness timed out");
                std::thread::sleep(Duration::from_millis(5));
            }
            let stopped_at=Instant::now();assert!(c.stop());
            let report=run.join().unwrap().unwrap().unwrap();
            assert_eq!(report.state,CheckState::Stopped);assert_eq!(report.body.reason,Some(CheckReason::UserStopped));
            assert!(report.body.results.len()<CHECKS.len());assert!(report.valid());assert!(stopped_at.elapsed()<Duration::from_secs(5));
            assert!(!c.active());assert!(!c.stop());assert_eq!(a.active_check_report().unwrap(),Some(report));
        });
        drop(a);let reopened=DiagnosticArchive::open(&path).unwrap();assert_eq!(reopened.active_check_report().unwrap().unwrap().state,CheckState::Stopped);
        assert!(!root.join(SCRATCH).exists());drop(reopened);std::fs::remove_dir_all(root).unwrap();
    }
    #[test] fn cp11a_existing_workspace_is_not_touched() {
        let root=root();let work=root.join(SCRATCH);std::fs::create_dir(&work).unwrap();std::fs::write(work.join("history.sqlite"),b"original").unwrap();
        let r=run_checks(&root,&||None,|_|Ok(())).unwrap();assert_eq!(r.state,CheckState::Complete);
        assert_eq!(std::fs::read(work.join("history.sqlite")).unwrap(),b"original");
        for slot in &SCRATCH_SLOTS[1..]{std::fs::create_dir(root.join(slot)).unwrap();}
        let r=run_checks(&root,&||None,|_|panic!("no fixture allowed")).unwrap();assert_eq!(r.state,CheckState::Attention);
        assert_eq!(r.body.reason,Some(CheckReason::WorkspaceUnavailable));
        assert_eq!(std::fs::read(work.join("history.sqlite")).unwrap(),b"original");std::fs::remove_dir_all(root).unwrap();
    }
    #[test] fn cp11a_restart_interrupted_and_invalid_report_refused() {
        let root=root();let path=root.join("trace.sqlite");let a=DiagnosticArchive::open(&path).unwrap();
        a.begin_active_check("check-test").unwrap();assert!(a.write_active_check("other",CheckState::Complete,&CheckBody::default()).is_err());
        assert!(a.write_active_check("check-test",CheckState::Complete,&CheckBody::default()).is_err());drop(a);
        let reopened=DiagnosticArchive::open(&path).unwrap();assert_eq!(reopened.active_check_report().unwrap().unwrap().state,CheckState::Interrupted);
        drop(reopened);std::fs::remove_dir_all(root).unwrap();
    }
    #[test] fn cp11a_durable_interruption_and_no_automatic_replay() {
        use std::process::{Command,Stdio};
        let root=root();let path=root.join("trace.sqlite");
        let mut child=Command::new(std::env::current_exe().unwrap())
            .args(["--exact","active_checks::tests::cp11a_crash_child","--ignored","--nocapture"])
            .env("LU_CP11A_TEST_ROOT",&root).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
        let end=Instant::now()+Duration::from_secs(10);
        let status=loop{if let Some(status)=child.try_wait().unwrap(){break status}
            if Instant::now()>=end{let _=child.kill();let _=child.wait();panic!("crash fixture deadline")}
            std::thread::sleep(Duration::from_millis(20));};
        assert_eq!(status.code(),Some(71));
        let archive=DiagnosticArchive::open(&path).unwrap();let c=CheckController::new(&root);
        let state=c.status(&archive).unwrap();assert!(!state.active);let report=state.report.unwrap();
        assert_eq!(report.state,CheckState::Interrupted);assert_eq!(report.body.results.len(),1);assert!(report.body.results[0].passed);
        // Do not silently reuse or remove the interrupted sample database.
        assert!(root.join(SCRATCH).exists());
        assert!(c.run(&archive,||false,||false).unwrap().is_none());
        assert_eq!(archive.active_check_report().unwrap(),Some(report));
        let fresh=c.run(&archive,||true,||false).unwrap().unwrap();
        assert_eq!(fresh.state,CheckState::Complete);assert!(root.join(SCRATCH).exists());
        assert!(!root.join(SCRATCH_SLOTS[1]).exists());
        drop(archive);std::fs::remove_dir_all(root).unwrap();
    }
    #[test] fn cp11a_legacy_report_not_promoted_and_invalid_prefix_refused() {
        let results=LEGACY_CHECKS.iter().map(|kind|CheckResult{kind:*kind,passed:true,verified_operations:kind.expected(),elapsed_micros:1,inference:None}).collect::<Vec<_>>();
        let old=serde_json::json!({"results":results,"reason":null});
        let mut report=CheckReport{state:CheckState::Complete,body:serde_json::from_value(old).unwrap()};
        assert!(report.valid());assert!(!report.body.expanded);
        report.body.expanded=true;assert!(!report.valid());
        report.state=CheckState::Running;assert!(report.valid());
        report.body.results[0].passed=false;report.body.results[0].verified_operations=0;assert!(!report.valid());
    }
    #[test] #[ignore="subprocess-only interruption fixture"] fn cp11a_crash_child(){
        let root=PathBuf::from(std::env::var_os("LU_CP11A_TEST_ROOT").expect("disposable fixture root"));
        let archive=DiagnosticArchive::open(&root.join("trace.sqlite")).unwrap();
        archive.begin_active_check("check-crash-test").unwrap();
        let _=run_checks(&root,&||None,|body|{
            archive.write_active_check("check-crash-test",CheckState::Running,body)?;
            // Abruptly exit without destructor or graceful journal publication.
            if body.results.len()==1{std::process::exit(71)} Ok(())
        });panic!("interruption witness not reached");
    }

}
