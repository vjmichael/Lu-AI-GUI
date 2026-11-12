#![cfg(feature="p7l-native-fixture")]
//! Real OS process / loopback / SQLite contract using an explicitly built
//! test-only mock server executable. No LLM weights, model reasoning, GPU,
//! Qwen compatibility or actual KoboldCpp package is exercised here.
//! cargo test -p lu-core --features p7l-native-fixture \\
//!   --test p7l_kobold_process_custody -- --nocapture
use lu_core::{
    browser_model_codec::{BrowserModelCodec,BrowserModelKind},
    inference_host::{Cancellation,GenerationRequest,InferenceBackend,TokenChunk},
    model_admission::ModelAdmissionManager,
    p7_backend_evaluation::{CandidateKind,CandidateManifest},
    p7_koboldcpp_backend::{KoboldCppBackend,KoboldCppConfig,KoboldCppLimits},
    p7_kobold_custody::{KoboldCustodyJournal,CustodyError},
};
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use std::{
    env,fs::{self,File},io::{Read,Write},num::NonZeroUsize,
    path::{Path,PathBuf},time::{Duration,SystemTime,UNIX_EPOCH},
};

fn hash(path:&Path)->[u8;32]{
    let mut f=File::open(path).unwrap();
    let mut sha=Sha256::new();
    let mut buf=[0u8;65536];
    loop{
        let n=f.read(&mut buf).unwrap();
        if n==0{break}
        sha.update(&buf[..n]);
    }
    sha.finalize().into()
}
fn fixture_exe()->PathBuf{
    env::var_os("CARGO_BIN_EXE_lu-p7l-kobold-http-fixture")
        .map(PathBuf::from)
        .or_else(||option_env!("CARGO_BIN_EXE_lu-p7l-kobold-http-fixture")
            .map(PathBuf::from))
        .expect("cargo must build the opt-in test-only native binary")
}
fn root()->PathBuf{
    let suffix=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let folder=std::env::temp_dir().join(format!(
        "lu-p7l-native-{}-{suffix}",std::process::id()));
    fs::create_dir(&folder).unwrap();
    folder
}
fn tiny_gguf(folder:&Path)->PathBuf{
    let model=folder.join("test.gguf");
    let mut data=b"GGUF".to_vec();
    data.extend(3u32.to_le_bytes());
    data.extend(1u64.to_le_bytes());
    data.extend(1u64.to_le_bytes());
    let mut f=File::create(&model).unwrap();
    f.write_all(&data).unwrap();
    drop(f);
    model
}
fn build(folder:&Path,session:&str)->(
    KoboldCppBackend,lu_core::model_admission::AdmittedModel
){
    let exe=fixture_exe();
    let model=tiny_gguf(folder);
    let mut admission=ModelAdmissionManager::new();
    let admitted=admission.admit(folder,"test.gguf").unwrap().clone();
    let manifest=CandidateManifest{
        kind:CandidateKind::KoboldCpp,executable:exe.clone(),
        expected_executable_sha256:hash(&exe),
        expected_model_sha256:hash(&model),
        expected_projector_sha256:None,
        operator_reviewed:true,local_only:true,
    };
    let candidate=manifest.preflight_for_model(&admitted).unwrap();
    let backend=KoboldCppBackend::new(KoboldCppConfig{
        candidate,
        limits:KoboldCppLimits{
            threads:NonZeroUsize::new(2).unwrap(),
            context_tokens:512,startup_timeout:Duration::from_secs(20),
            request_timeout:Duration::from_secs(4),
        },
        host_secret:"a".repeat(64),
        session_id:session.into(),
        journal_path:folder.join("original-custody.sqlite"),
    }).unwrap();
    (backend,admitted)
}
fn request(prompt:&str)->GenerationRequest{
    GenerationRequest{
        prompt:prompt.into(),history:vec![],max_new_tokens:64,
    }
}
#[test]
fn original_process_is_authenticated_streamed_reaped_then_admissible_again(){
    let folder=root();
    let (mut engine,model)=build(&folder,"original-1");
    assert!(!engine.journal_pending().unwrap());
    engine.load(&model).unwrap();
    assert!(engine.pid().is_some());
    assert!(engine.journal_pending().unwrap());
    let mut parts=Vec::new();
    engine.generate(&request("P7L synthetic text only"),
        &Cancellation::default(),&mut |chunk:TokenChunk|{
            parts.push((chunk.index,chunk.text));Ok(())
        }).unwrap();
    assert_eq!(parts,vec![(0,"P7L_".into()),(1,"FIXED".into())]);
    engine.unload();
    assert_eq!(engine.pid(),None);
    assert!(!engine.custody_uncertain());
    assert!(!engine.journal_pending().unwrap());
    drop(engine);
    let (mut second,model)=build(&folder,"original-2");
    second.load(&model).unwrap();
    assert!(second.pid().is_some());
    second.unload();
    assert!(!second.journal_pending().unwrap());
    drop(second);
    fs::remove_dir_all(folder).unwrap();
}
#[test]
fn native_cancellation_interrupts_stream_and_reaps_original_child(){
    let folder=root();
    let (mut engine,model)=build(&folder,"cancel-1");
    engine.load(&model).unwrap();
    let signal=Cancellation::default();
    let cancel=signal.clone();
    let result=engine.generate(
        &request("stream a long fixed text reply"),
        &signal,&mut |_part|{cancel.cancel();Ok(())},
    );
    assert!(result.is_err());
    assert_eq!(engine.pid(),None);
    assert!(!engine.custody_uncertain());
    assert!(!engine.journal_pending().unwrap());
    drop(engine);
    fs::remove_dir_all(folder).unwrap();
}
#[test]
fn original_structured_read_tool_sse_is_a_proposal_not_an_action(){
    let folder=root();
    let (mut engine,model)=build(&folder,"tool-1");
    engine.load(&model).unwrap();
    let schema=BrowserModelCodec(BrowserModelKind::Read).schema();
    let tools=vec![json!({"type":"function","function":{
        "name":"lu_browser_read","parameters":schema
    }})];
    let calls=engine.generate_browser_function_calls(
        &request("propose one approved public navigation"),
        &tools,&Cancellation::default(),
    ).unwrap();
    assert_eq!(calls.len(),1);
    assert_eq!(calls[0].tool_name,"lu_browser_read");
    let data:Value=serde_json::from_str(&calls[0].arguments_json).unwrap();
    assert_eq!(data["operation"],"navigate");
    assert_eq!(data["url"],"https://example.org");
    // No browser/tool executor was instantiated or authorized.
    engine.unload();drop(engine);
    fs::remove_dir_all(folder).unwrap();
}
#[test]
fn durable_starting_state_survives_reopen_and_refuses_unverified_restart(){
    let folder=root();
    tiny_gguf(&folder);
    let journal=KoboldCustodyJournal::new(folder.join("original-custody.sqlite"))
        .unwrap();
    journal.begin("crashed-before-spawn",[3;32],[4;32]).unwrap();
    drop(journal);
    let reopened=KoboldCustodyJournal::new(
        folder.join("original-custody.sqlite")).unwrap();
    assert_eq!(reopened.pending(),Ok(true));
    assert_eq!(reopened.begin("another",[3;32],[4;32]),
        Err(CustodyError::ExistingUnreconciled));
    assert_eq!(reopened.finish_verified("crashed-before-spawn",false),
        Err(CustodyError::ExistingUnreconciled));
    // This test created no OS child, so it can explicitly record the
    // witnessed absence. A REAL interrupted child needs original OS
    // process/group/Job reconciliation, never this test's assertion.
    reopened.finish_verified("crashed-before-spawn",true).unwrap();
    drop(reopened);
    fs::remove_dir_all(folder).unwrap();
}
