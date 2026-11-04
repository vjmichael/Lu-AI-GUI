//! TOOLS-06: physical filesystem availability, no program spawning or PATH.
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
use lu_core::{
    agent_tool_loop::{NativeToolTurn, ToolTurnOutcome},
    contracts::{AuthorizationError, ToolAuthorizer, ToolDescriptor, ToolEffect, ToolInvocation},
    conversation_authority::{
        AuthorizedCapability, BoundCallOrigin, ConversationAuthority, TaskAuthority,
        TrustedUserTurnVerifier,
    },
    model::ModelToolCatalog,
    native_environment_discovery::{
        register_environment_discovery_tool, HostProgramInventory, HostProgramSpec,
    },
    registry::{DispatchError, NativeToolRegistry},
};
use prost::Message;

static NEXT: AtomicU64 = AtomicU64::new(0);
fn workspace() -> PathBuf {
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let root = std::env::temp_dir().join(format!("lu-env-{stamp}-{n}-{}",std::process::id()));
    fs::create_dir(&root).unwrap();
    root
}
fn candidate(root: &Path, stem: &str) -> PathBuf {
    let name = if cfg!(windows) { format!("{stem}.exe") } else { stem.into() };
    root.join(name)
}
fn write_candidate(p: &Path) {
    fs::write(p,b"not a real runtime; never execute").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(p,fs::Permissions::from_mode(0o700)).unwrap();
    }
}
fn specs(root:&Path) -> Vec<HostProgramSpec> {
    vec![
        HostProgramSpec {id:"python".into(),candidate_files:vec![candidate(root,"python")]},
        HostProgramSpec {id:"git".into(),candidate_files:vec![candidate(root,"git")]},
    ]
}
struct Verified;
impl TrustedUserTurnVerifier for Verified {
    fn verify_user_turn(&self,c:&str,t:&str)->bool { c=="env-chat" && t=="env-turn" }
}
struct Allow;
impl ToolAuthorizer for Allow {
    fn authorize(&self,_:&ToolInvocation,_:&ToolDescriptor)->Result<(),AuthorizationError>{Ok(())}
}
struct Deny;
impl ToolAuthorizer for Deny {
    fn authorize(&self,_:&ToolInvocation,_:&ToolDescriptor)->Result<(),AuthorizationError>{
        Err(AuthorizationError::new("environment check not approved"))
    }
}
fn authority()->(ConversationAuthority,BoundCallOrigin,BTreeSet<String>){
    let auth=ConversationAuthority::new();
    let origin=BoundCallOrigin{
        conversation_id:"env-chat".into(),user_turn_id:"env-turn".into(),
        task_id:"env-task".into(),
    };
    auth.open_user_task(TaskAuthority{
        conversation_id:origin.conversation_id.clone(),
        user_turn_id:origin.user_turn_id.clone(),
        task_id:origin.task_id.clone(),
        execution_scope_id:"env-scope".into(),
        capabilities:vec![AuthorizedCapability{
            tool_id:"lu.exec.check_env".into(),effect:ToolEffect::ReadWorkspace,
        }],
        expires_at:None,
    },&Verified).unwrap();
    (auth,origin,["lu_check_env".to_owned()].into_iter().collect())
}
#[tokio::test]
async fn ambient_inventory_and_native_turn_are_fresh_without_execution() {
    let root=workspace();
    let python=candidate(&root,"python");
    write_candidate(&python);
    let (reg,mut catalog)=(NativeToolRegistry::new(),ModelToolCatalog::new());
    let inventory=register_environment_discovery_tool(&reg,&mut catalog,specs(&root)).unwrap();
    assert_eq!(inventory.ambient_snapshot().programs.len(),2);
    let snapshot=inventory.ambient_snapshot();
    assert_eq!(snapshot.operating_system,std::env::consts::OS);
    assert_eq!(snapshot.architecture,std::env::consts::ARCH);
    assert!(inventory.check("python").unwrap().candidate_present);
    assert!(!inventory.check("git").unwrap().candidate_present);
    assert!(inventory.check("gh").is_err());

    let (auth,origin,allowed)=authority();
    let mut turn=NativeToolTurn::new_read_only(
        &catalog,&reg,&auth,origin,"env-scope".into(),&allowed,
    ).unwrap();
    let advertised=catalog.schemas(&allowed).unwrap();
    assert_eq!(advertised.len(),1);
    let enum_values=advertised[0]["function"]["parameters"]["properties"]["program"]["enum"]
        .as_array().unwrap();
    assert_eq!(enum_values.len(),2);
    assert!(!enum_values.contains(&serde_json::json!("gh")));
    let call=|id:&str| format!(r#"{{"kind":"tool","name":"lu_check_env","arguments":{{"program":"{id}"}}}}"#);
    let output=turn.handle_model_reply(&call("python"),&catalog,&reg,&auth,&Allow).await.unwrap();
    let ToolTurnOutcome::ToolResult{data,..}=output else {panic!("typed status missing")};
    assert_eq!(data["ok"],true);
    assert_eq!(data["result"]["candidate_present"],true);
    assert_eq!(data["result"]["evidence"],"file_present_only");
    assert_eq!(data["result"]["execution_verified"],false);
    assert!(data["result"]["version"].is_null());
    assert!(!data.to_string().contains(root.to_string_lossy().as_ref()));
    assert_eq!(fs::read(&python).unwrap(),b"not a real runtime; never execute");
    // Refresh after an actual change: host file presence is sampled each time.
    fs::remove_file(&python).unwrap();
    let output=turn.handle_model_reply(&call("python"),&catalog,&reg,&auth,&Allow).await.unwrap();
    let ToolTurnOutcome::ToolResult{data,..}=output else {panic!("typed refresh missing")};
    assert_eq!(data["result"]["candidate_present"],false);
    assert_eq!(data["result"]["evidence"],"not_found");
    assert_eq!(turn.step_count(),2);
    drop(turn);drop(auth);drop(catalog);drop(reg);drop(inventory);
    fs::remove_dir_all(root).unwrap();
}
#[tokio::test]
async fn model_cannot_choose_any_path_or_run_or_install_program() {
    let root=workspace();
    let (reg,mut catalog)=(NativeToolRegistry::new(),ModelToolCatalog::new());
    let _=register_environment_discovery_tool(&reg,&mut catalog,specs(&root)).unwrap();
    let (_,_,allowed)=authority();
    for invalid in [
        r#"{}"#,
        r#"{"program":"gh"}"#,
        r#"{"program":"cat"}"#,
        r#"{"program":"python","command":"-c print(1)"}"#,
        r#"{"program":"python","path":"/etc/passwd"}"#,
        r#"{"program":"/usr/bin/python"}"#,
        r#"{"program":"python","program":"git"}"#,
        r#"{"program":"python",}"#,
        r#"{"program":false}"#,
    ]{
        assert!(catalog.encode_call(&allowed,"lu_check_env","deny",None,invalid).is_err(),
            "accepted unsafe model program lookup: {invalid}");
    }
    let correct=catalog.encode_call(&allowed,"lu_check_env","native-1",None,
        r#"{"program":"python"}"#).unwrap();
    assert!(matches!(reg.dispatch(correct.clone(),&Deny).await,Err(DispatchError::Denied(_))));
    let mut bad_type=correct.clone();
    bad_type.input.type_url="type.googleapis.com/lu.v1.ReadTextFileRequest".into();
    assert!(reg.dispatch(bad_type,&Allow).await.is_err());
    let mut forged=correct;
    forged.input.bytes.extend_from_slice(&[0x78,0x01]); // protobuf field 15
    assert!(matches!(reg.dispatch(forged,&Allow).await,Err(DispatchError::Execution(_))));
    drop(catalog);drop(reg);fs::remove_dir_all(root).unwrap();
}
#[tokio::test]
async fn revocation_returns_typed_denial_not_a_fake_program_availability_claim() {
    let root=workspace();
    write_candidate(&candidate(&root,"git"));
    let (reg,mut catalog)=(NativeToolRegistry::new(),ModelToolCatalog::new());
    let _=register_environment_discovery_tool(&reg,&mut catalog,specs(&root)).unwrap();
    let (auth,origin,allowed)=authority();
    let mut turn=NativeToolTurn::new_read_only(
        &catalog,&reg,&auth,origin,"env-scope".into(),&allowed,
    ).unwrap();
    auth.revoke_task("env-task").unwrap();
    let reply=turn.handle_model_reply(
        r#"{"kind":"tool","name":"lu_check_env","arguments":{"program":"git"}}"#,
        &catalog,&reg,&auth,&Allow,
    ).await.unwrap();
    let ToolTurnOutcome::ToolResult{data,..}=reply else {panic!("missing denied receipt")};
    assert_eq!(data["ok"],false);
    assert_eq!(data["error"]["code"],"PERMISSION_DENIED");
    assert_eq!(turn.step_count(),1);
    drop(turn);drop(auth);drop(catalog);drop(reg);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn host_inventory_rejects_unapproved_or_ambiguous_candidates() {
    let root=workspace();
    let valid=specs(&root);
    assert!(HostProgramInventory::new(valid.clone()).is_ok());
    assert!(HostProgramInventory::new(vec![]).is_err());
    let mut same=valid;
    same.push(HostProgramSpec {
        id:"python".into(),candidate_files:vec![candidate(&root,"python")]});
    assert!(HostProgramInventory::new(same).is_err());
    assert!(HostProgramInventory::new(vec![HostProgramSpec{
        id:"gh".into(),candidate_files:vec![candidate(&root,"gh")]}]).is_err());
    assert!(HostProgramInventory::new(vec![HostProgramSpec{
        id:"git".into(),candidate_files:vec![PathBuf::from("relative-program")]}]).is_err());
    assert!(HostProgramInventory::new(vec![HostProgramSpec{
        id:"git".into(),candidate_files:vec![root.join("..").join("outside")]}]).is_err());
    #[cfg(windows)]
    assert!(HostProgramInventory::new(vec![HostProgramSpec{
        id:"python".into(),candidate_files:vec![root.join("python.cmd")]}]).is_err());
    fs::remove_dir_all(root).unwrap();
}
#[cfg(unix)]
#[test]
fn symlinks_and_nonexecutable_files_are_not_installed_program_evidence() {
    use std::os::unix::fs::symlink;
    let root=workspace();
    let target=root.join("target");
    fs::write(&target,b"not executable").unwrap();
    let python=candidate(&root,"python");
    symlink(&target,&python).unwrap();
    let inventory=HostProgramInventory::new(vec![HostProgramSpec{
        id:"python".into(),candidate_files:vec![python.clone()],
    }]).unwrap();
    assert!(!inventory.check("python").unwrap().candidate_present);
    fs::remove_file(&python).unwrap();
    fs::copy(&target,&python).unwrap();
    assert!(!inventory.check("python").unwrap().candidate_present);
    drop(inventory);fs::remove_dir_all(root).unwrap();
}
