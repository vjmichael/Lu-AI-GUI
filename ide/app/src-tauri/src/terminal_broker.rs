//! IDE-INT05-D: ephemeral, typed terminal sessions, NEVER shell access.
//! A session/sequence is an anti-replay correlator, not execution authority.
use serde::Serialize;
use std::{collections::BTreeMap, sync::{Mutex, atomic::{AtomicU64,Ordering}}};

#[derive(Debug,PartialEq,Eq)]
pub enum TerminalAction {
    Help, Status, Toolchains, Packages, Run(&'static str), Cancel(Option<String>),
}
pub fn parse(line:&str)->Result<TerminalAction,&'static str>{
    if line.is_empty() || line.len()>96 || !line.is_ascii()
        || line.bytes().any(|b| b.is_ascii_control()){
        return Err("INT05D_COMMAND_FORMAT_DENIED")
    }
    match line.trim(){
        "help"=>Ok(TerminalAction::Help),
        "status"=>Ok(TerminalAction::Status),
        "toolchains"=>Ok(TerminalAction::Toolchains),
        "packages"=>Ok(TerminalAction::Packages),
        "run rust_check"=>Ok(TerminalAction::Run("rust_check")),
        "run rust_build"=>Ok(TerminalAction::Run("rust_build")),
        "run rust_tests"=>Ok(TerminalAction::Run("rust_tests")),
        "run lu_ide_host_tests"=>Ok(TerminalAction::Run("lu_ide_host_tests")),
        "run python_pytest"=>Ok(TerminalAction::Run("python_pytest")),
        "run js_tests"=>Ok(TerminalAction::Run("js_tests")),
        "run cpp_cmake_build"=>Ok(TerminalAction::Run("cpp_cmake_build")),
        "cancel preflight"=>Ok(TerminalAction::Cancel(None)),
        other=>{
            if let Some(job)=other.strip_prefix("cancel "){
                if job.len()==36 && job.starts_with("job-") &&
                    job[4..].bytes().all(|b|b.is_ascii_hexdigit() && !b.is_ascii_uppercase()){
                    return Ok(TerminalAction::Cancel(Some(job.to_owned())))
                }
            }
            Err("INT05D_NO_ARBITRARY_SHELL_OR_COMMANDS")
        }
    }
}
#[derive(Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct TerminalSessionView {
    pub project_id:String, pub session_id:String, pub sequence:u64,
    pub interactive_typed:bool, pub arbitrary_shell_enabled:bool,
    pub authorization_granted:bool,
}
#[derive(Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct TerminalReply {
    pub project_id:String, pub session_id:String, pub sequence:u64,
    pub kind:&'static str, pub message:&'static str,
    pub native_result:Option<serde_json::Value>,
    pub arbitrary_shell_enabled:bool,
    pub authorization_granted_by_terminal:bool,
    pub installed_app_accepted:bool,
}
impl TerminalReply {
    pub fn new(project_id:String,session_id:String,sequence:u64,
        kind:&'static str,message:&'static str,
        native_result:Option<serde_json::Value>)->Self {
        Self{project_id,session_id,sequence,kind,message,native_result,
            arbitrary_shell_enabled:false,authorization_granted_by_terminal:false,
            installed_app_accepted:false}
    }
}
struct Session {id:String,sequence:u64}
#[derive(Default)]
pub struct TerminalSessions {
    epoch:AtomicU64, active:Mutex<BTreeMap<String,Session>>,
}
impl TerminalSessions {
    pub fn open(&self,project:&str)->Result<TerminalSessionView,&'static str>{
        if project.len()!=36 || !project.starts_with("ide-") ||
            !project[4..].bytes().all(|b|b.is_ascii_hexdigit() && !b.is_ascii_uppercase()){
            return Err("INT05D_PROJECT_SCOPE_DENIED")
        }
        let next=self.epoch.fetch_add(1,Ordering::SeqCst)
            .checked_add(1).ok_or("INT05D_SESSION_EXHAUSTED")?;
        let id=format!("term-{next:016x}"); // Not a grant or secret.
        let mut sessions=self.active.lock().map_err(|_|"INT05D_SESSION_UNAVAILABLE")?;
        if sessions.len()>=32 && !sessions.contains_key(project){
            return Err("INT05D_TOO_MANY_SESSIONS")
        }
        sessions.insert(project.to_owned(),Session{id:id.clone(),sequence:0});
        Ok(TerminalSessionView{project_id:project.to_owned(),session_id:id,
            sequence:0,interactive_typed:true,arbitrary_shell_enabled:false,
            authorization_granted:false})
    }
    pub fn consume(&self,project:&str,id:&str,sequence:u64,line:&str)
        ->Result<TerminalAction,&'static str>{
        let action=parse(line)?; // No side effects or secrets saved.
        let mut sessions=self.active.lock().map_err(|_|"INT05D_SESSION_UNAVAILABLE")?;
        let session=sessions.get_mut(project).ok_or("INT05D_SESSION_NOT_OPEN")?;
        if session.id!=id || session.sequence.checked_add(1)!=Some(sequence){
            return Err("INT05D_STALE_OR_REPLAYED_COMMAND")
        }
        session.sequence=sequence; // Consume BEFORE a native OS operation.
        Ok(action)
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    const PROJECT:&str="ide-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    #[test] fn never_interprets_shell_syntax(){
        for s in ["bash","powershell","cmd","sh","run rust_build && curl",
            "run rust_check; whoami","cd /","npm install foo",
            "run rust_build --release","help\nrun rust_build","", "curl example.com"]{
            assert!(parse(s).is_err(),"{s}")
        }
    }
    #[test] fn fixed_recipe_and_exact_cancel_only(){
        assert_eq!(parse("run rust_build"),Ok(TerminalAction::Run("rust_build")));
        assert_eq!(parse("cancel preflight"),Ok(TerminalAction::Cancel(None)));
        assert!(parse("cancel job-bad").is_err());
    }
    #[test] fn replay_and_cross_project_denied(){
        let sessions=TerminalSessions::default();let first=sessions.open(PROJECT).unwrap();
        assert_eq!(sessions.consume(PROJECT,&first.session_id,1,"status"),
            Ok(TerminalAction::Status));
        assert!(sessions.consume(PROJECT,&first.session_id,1,"status").is_err());
        assert!(sessions.consume(PROJECT,&first.session_id,3,"help").is_err());
        let second=sessions.open(PROJECT).unwrap();
        assert!(sessions.consume(PROJECT,&first.session_id,2,"help").is_err());
        assert!(sessions.consume("ide-bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            &second.session_id,1,"help").is_err());
    }
}