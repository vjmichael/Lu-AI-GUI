//! Multi-tool read workspace bridge under an authenticated human origin.
use lu_core::{
    conversation_authority::TrustedUserTurnVerifier,
    inference_host::Cancellation,
    model_turn_journal::ModelTurnJournal,
    native_workspace_read_turn::{WorkspaceReadTurnSession,WorkspaceReadError},
    supervised_tool_generation::{ModelStepGenerator,NativeGenerationError},
};
use std::{collections::VecDeque,fs,io::Write,path::PathBuf,sync::atomic::{AtomicU64,Ordering}};
use zip::{ZipWriter,write::SimpleFileOptions,CompressionMethod};
static ID:AtomicU64=AtomicU64::new(0);
struct Verifier;
impl TrustedUserTurnVerifier for Verifier{
    fn verify_user_turn(&self,c:&str,u:&str)->bool{c=="chat-a"&&u=="human-1"}
}
struct Scripted(VecDeque<String>);
impl ModelStepGenerator for Scripted{
    fn generate_step(&mut self,_:&str,_:u32,_:&Cancellation)
    ->Result<String,NativeGenerationError>{
        self.0.pop_front().ok_or(NativeGenerationError::MissingCompletion)
    }
}
fn fixture()->PathBuf{
    let p=std::env::temp_dir().join(format!("lu-read-turn-{}-{}",std::process::id(),ID.fetch_add(1,Ordering::SeqCst)));
    fs::create_dir(&p).unwrap();p
}
#[test]
fn refuses_unverified_origin_and_unavailable_root(){
    let root=fixture();
    assert!(matches!(WorkspaceReadTurnSession::new(&Verifier,"chat-b","human-1","task-1",&root),
        Err(WorkspaceReadError::InvalidScope)));
    assert!(matches!(WorkspaceReadTurnSession::new(&Verifier,"chat-a","human-1","task-1",&root.join("absent")),
        Err(WorkspaceReadError::RootUnavailable)));
    fs::remove_dir_all(root).unwrap();
}
#[tokio::test]
async fn authorized_file_read_and_archive_inspection_share_one_model_session(){
    let root=fixture();
    fs::write(root.join("memo.txt"),b"approved workspace").unwrap();
    let zip_file=fs::File::create(root.join("data.zip")).unwrap();
    let mut zip=ZipWriter::new(zip_file);
    zip.start_file("inside.txt",SimpleFileOptions::default().compression_method(CompressionMethod::Stored)).unwrap();
    zip.write_all(b"metadata").unwrap();zip.finish().unwrap();
    let journal_path=root.join("audit.sqlite");
    let journal=ModelTurnJournal::open(&journal_path).unwrap();
    let mut session=WorkspaceReadTurnSession::new(
        &Verifier,"chat-a","human-1","task-1",&root).unwrap();
    let schemas=session.schemas().unwrap();
    let capabilities=session.capabilities().unwrap();
    assert!(!capabilities.is_empty());
    assert!(capabilities.iter().all(|c|c.effect=="read_workspace"));
    assert_eq!(capabilities.len(),schemas.len());
    assert!(capabilities.iter().any(|c|c.public_name=="lu_files_read_text"));
    assert!(schemas.iter().any(|s|s["function"]["name"]=="lu_archive_list"));
    assert!(schemas.iter().any(|s|s["function"]["name"]=="lu_files_read_text"));
    let mut model=Scripted(VecDeque::from(vec![
        r#"{"kind":"tool","name":"lu_files_read_text","arguments":{"path":"memo.txt"}}"#.into(),
        r#"{"kind":"tool","name":"lu_archive_list","arguments":{"path":"data.zip"}}"#.into(),
        r#"{"kind":"final","text":"The workspace has approved text and a ZIP."}"#.into(),
    ]));
    let result=session.run_journaled("Inspect this approved workspace",&mut model,&Cancellation::default(),&journal).await.unwrap();
    assert_eq!(result.tool_calls,2);
    assert_eq!(result.answer,"The workspace has approved text and a ZIP.");
    drop(session);drop(journal);
    fs::remove_dir_all(root).unwrap();
}
#[tokio::test]
async fn model_traversal_is_rejected_without_reading_outside_workspace(){
    let root=fixture();
    fs::write(root.join("inside.txt"),b"safe").unwrap();
    let audit=root.join("audit.sqlite");
    let journal=ModelTurnJournal::open(&audit).unwrap();
    let mut session=WorkspaceReadTurnSession::new(
        &Verifier,"chat-a","human-1","task-2",&root).unwrap();
    let mut model=Scripted(VecDeque::from(vec![
        r#"{"kind":"tool","name":"lu_files_read_text","arguments":{"path":"../secrets.txt"}}"#.into(),
        r#"{"kind":"final","text":"Unable to access outside files."}"#.into(),
    ]));
    // Shape rejection can trigger bounded schema regeneration, but may never
    // dispatch the unauthorized path or broaden the root grant.
    let result=session.run_journaled("Read the file outside the approved root",&mut model,&Cancellation::default(),&journal).await;
    assert!(result.is_err() || result.as_ref().is_ok_and(|r|r.tool_calls==0));
    drop(session);drop(journal);fs::remove_dir_all(root).unwrap();
}
