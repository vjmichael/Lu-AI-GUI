//! E1-P05 measured local utility-vs-reviewed-application task executor.
//! Harness-only: host chooses a disposable test directory, reviewed worker,
//! format, exact user turn and sealed expected task. No model or webview can
//! choose OS program, path, argv or inherited permission through this entry.
//!
//! Outputs independent JSON observations; does not invent a model result.
use std::{env,fs,io,path::PathBuf,sync::Arc,time::{Duration,Instant}};
use cap_std::{ambient_authority,fs::Dir};
use serde_json::json;
use sha2::{Digest,Sha256};
use lu_core::{
    ansf::{AnsfStore,EntryRole},
    contracts::{
        AuthorizationError,ProtoPayload,ToolAuthorizer,
        ToolDescriptor,ToolEffect,ToolInvocation
    },
    conversation_authority::{
        AuthorizedCapability,BoundCallOrigin,ConversationAuthority,
        TaskAuthority,TrustedUserTurnVerifier,
    },
    journal::{EffectJournal,EffectState,PrepareOutcome},
    native_app_admission::HostReviewedExecutable,
    native_app_catalog::HostReviewedApplicationCatalog,
    native_app_artifact_custody::{
        APP_ID,EXEC_TOOL_ID,PUBLISH_TOOL_ID,INPUT_TYPE,PUBLISH_TYPE,
        OUTPUT_TYPE,EXEC_REQUEST,PUBLISH_REQUEST,
        FIXED_XLSX_TEXT,FIXED_DOCX_TEXT,
        ArtifactKind,HostApprovedArtifact,E1ArtifactCustodyBroker,
        independently_validate_office,
    },
    native_atomic_write::create_new_atomic_in_dir,
    native_background_process::BackgroundProcessSupervisor,
    native_spreadsheet_create::{render_xlsx,CellValue,SpreadsheetCell},
    native_docx_create::{render_docx,DocxBlock,DocxBlockKind},
    tasks::TaskStore,
};

struct RealUser<'a>(&'a AnsfStore);
impl TrustedUserTurnVerifier for RealUser<'_>{
    fn verify_user_turn(&self,session:&str,turn:&str)->bool{
        self.0.find_message(session,turn).ok().flatten()
            .is_some_and(|entry|entry.role=="user")
    }
}
struct Policy(bool);
impl ToolAuthorizer for Policy{
    fn authorize(&self,_:&ToolInvocation,_:&ToolDescriptor)
        ->Result<(),AuthorizationError>{
        if self.0{Ok(())}else{Err(AuthorizationError::new("denied by host"))}
    }
}
fn err(label:&str)->io::Error{io::Error::other(label)}
fn call(id:&str,tool:&str,input:&str,bytes:&[u8])->ToolInvocation{
    ToolInvocation{
        call_id:id.into(),tool_id:tool.into(),
        workspace_id:Some("e1-benchmark".into()),
        input:ProtoPayload{type_url:input.into(),bytes:bytes.to_vec()},
    }
}
fn descriptor(tool:&str,effect:ToolEffect,input:&str)->ToolDescriptor{
    ToolDescriptor{
        id:tool.into(),display_name:"E1 benchmark sealed task".into(),
        effect,input_type_url:input.into(),output_type_url:OUTPUT_TYPE.into(),
    }
}
fn cells()->Vec<SpreadsheetCell>{
    vec![
        SpreadsheetCell{row:1,column:1,value:CellValue::Text("Status".into())},
        SpreadsheetCell{row:1,column:2,value:CellValue::Text("Reviewed".into())},
        SpreadsheetCell{row:2,column:1,value:CellValue::Text("Frames".into())},
        SpreadsheetCell{row:2,column:2,value:CellValue::Number(48.0)}
    ]
}
fn run()->Result<(),Box<dyn std::error::Error>>{
    let args=env::args().skip(1).collect::<Vec<_>>();
    if args.len()!=4{return Err(err("Expected mode, kind, disposable-root, worker").into())}
    let mode=args[0].as_str();
    let kind=match args[1].as_str(){
        "docx"=>ArtifactKind::Docx,"xlsx"=>ArtifactKind::Xlsx,
        _=>return Err(err("Wrong sealed format").into())
    };
    if !matches!(mode,"native_utility"|"reviewed_app"){
        return Err(err("Only sealed utility or reviewed app arms").into())
    }
    let root=PathBuf::from(&args[2]);
    let binary=PathBuf::from(&args[3]);
    if !root.is_absolute()||root.exists()||!binary.is_absolute()||!binary.is_file(){
        return Err(err("Inputs must be a new disposable absolute root and actual native binary").into())
    }
    fs::create_dir_all(&root)?;
    let setup_start=Instant::now();
    let book=AnsfStore::open(&root.join("originals.sqlite"))
        .map_err(|_|err("ANSF store failed"))?;
    book.create_session("c-e1","e1-benchmark","Sealed actual benchmark")
        .map_err(|_|err("Create session failed"))?;
    book.start_chapter("c-e1","Original task")
        .map_err(|_|err("Create chapter failed"))?;
    book.append_entry("c-e1","u-e1-actual",EntryRole::User,
        "Approved Office task","Create fixed, verified DOCX/XLSX artifact")
        .map_err(|_|err("Original user turn failed"))?;
    let origin=BoundCallOrigin{
        conversation_id:"c-e1".into(),user_turn_id:"u-e1-actual".into(),
        task_id:"e1-real-task".into(),
    };
    let auth=ConversationAuthority::new();
    let mut grants=vec![AuthorizedCapability{
        tool_id:PUBLISH_TOOL_ID.into(),effect:ToolEffect::WriteWorkspace,
    }];
    if mode=="reviewed_app"{
        grants.push(AuthorizedCapability{
            tool_id:EXEC_TOOL_ID.into(),effect:ToolEffect::ExecuteCode
        });
    }
    auth.open_user_task(TaskAuthority{
        conversation_id:origin.conversation_id.clone(),
        user_turn_id:origin.user_turn_id.clone(),
        task_id:origin.task_id.clone(),execution_scope_id:"e1-benchmark".into(),
        capabilities:grants,expires_at:None,
    },&RealUser(&book)).map_err(|_|err("Authenticated task grant failed"))?;
    let tasks=Arc::new(TaskStore::open(&root.join("tasks.sqlite"))
        .map_err(|_|err("Task store failed"))?);
    let effects=Arc::new(EffectJournal::open(&root.join("effects.sqlite"))
        .map_err(|_|err("Journal failed"))?);
    let dir=Arc::new(Dir::open_ambient_dir(&root,ambient_authority())?);
    let output_name=match kind{
        ArtifactKind::Docx=>"result.docx",
        ArtifactKind::Xlsx=>"result.xlsx",
    };
    let expected=match kind{
        ArtifactKind::Docx=>FIXED_DOCX_TEXT,
        ArtifactKind::Xlsx=>FIXED_XLSX_TEXT,
    };
    let setup_ms=setup_start.elapsed().as_secs_f64()*1000.0;
    let start=Instant::now();
    let mut blocked=0usize;
    // Native DOCX and XLSX creators are both present: compare like-for-like.
    let supported=true;
    if mode=="native_utility"{
        let publish=call("utility-write",PUBLISH_TOOL_ID,PUBLISH_TYPE,
            PUBLISH_REQUEST);
        let desc=descriptor(PUBLISH_TOOL_ID,ToolEffect::WriteWorkspace,
            PUBLISH_TYPE);
        // Independently test that an explicit host veto prevents any file
        // or journal transition before the real approved utility run.
        if auth.authorize_worker_call(
            &origin,&publish,&desc,&Policy(false)).is_err(){blocked+=1}
        auth.with_authorized_worker_effect(
            &origin,&publish,&desc,&Policy(true),||{
                if effects.prepare(&publish)!=Ok(PrepareOutcome::New){
                    return Err(err("Failed utility journal admission"))
                }
                effects.mark_executing(&publish.call_id)
                    .map_err(|_|err("Failed executing utility journal"))?;
                let bytes=match kind{
                    ArtifactKind::Docx=>render_docx(&[DocxBlock{
                        kind:DocxBlockKind::Paragraph,
                        text:"Lu E1 artifact custody verified".into()
                    }]).map_err(|_|err("Native Lu Word rendering failed"))?,
                    ArtifactKind::Xlsx=>render_xlsx("E1",&cells())
                        .map_err(|_|err("Native Lu spreadsheet rendering failed"))?,
                };
                independently_validate_office(kind,&bytes,expected)
                    .map_err(|_|err("Independent utility Office semantic read failed"))?;
                create_new_atomic_in_dir(
                    &dir,output_name,"bench-e1-utility-20261107",&bytes
                ).map_err(|_|err("Host atomic utility publication failed"))?;
                let digest=format!("{:x}",Sha256::digest(&bytes));
                effects.mark_applied(&publish.call_id,&serde_json::to_vec(
                    &json!({"schema":"e1-native-utility-v1",
                      "sha256":digest,"semanticVerified":true})
                ).map_err(|_|err("Utility JSON receipt failed"))?)
                    .map_err(|_|err("Utility receipt publication failed"))?;
                Ok(())
            }
        ).map_err(|_|err("Utility origin or host policy refused"))??;
    }else if mode=="reviewed_app"{
        let workers=Arc::new(BackgroundProcessSupervisor::new(
            Arc::clone(&tasks)
        ));
        let mut catalog=HostReviewedApplicationCatalog::new();
        let exe_hash=format!("{:x}",Sha256::digest(fs::read(&binary)?));
        catalog.register_reviewed(HostReviewedExecutable{
            app_id:APP_ID.into(),app_version:"e1-fixed-v1".into(),
            tool_id:"lu.e1.artifact.inspect".into(),
            workspace_id:"e1-benchmark".into(),
            executable:binary,
            expected_sha256:exe_hash,
        }).map_err(|_|err("Reviewed app catalog failed"))?;
        let broker=E1ArtifactCustodyBroker::new(
            auth.clone(),Arc::clone(&workers),
            Arc::clone(&tasks),Arc::clone(&effects),Arc::clone(&dir)
        ).map_err(|_|err("Host broker init failed"))?;
        let exec=call("reviewed-execute",EXEC_TOOL_ID,INPUT_TYPE,EXEC_REQUEST);
        let exec_desc=descriptor(EXEC_TOOL_ID,ToolEffect::ExecuteCode,INPUT_TYPE);
        let publish=call("reviewed-publish",PUBLISH_TOOL_ID,PUBLISH_TYPE,
            PUBLISH_REQUEST);
        let publish_desc=descriptor(PUBLISH_TOOL_ID,ToolEffect::WriteWorkspace,
            PUBLISH_TYPE);
        let approval=HostApprovedArtifact{
            kind,target_name:output_name.into(),
            stage_id:"bench-e1-approved-20261107".into(),
            expected_semantics:expected.into(),
        };
        // Denied host policy must not start a real OS worker or mutate journal.
        if broker.launch(
            &catalog,&origin,&exec,&exec_desc,&publish,&publish_desc,
            approval.clone(),&Policy(false)
        ).is_err()&&effects.get(&exec.call_id).map_err(|_|
            err("Journal veto probe failed"))?.is_none(){blocked+=1}
        broker.launch(
            &catalog,&origin,&exec,&exec_desc,&publish,&publish_desc,
            approval,&Policy(true)
        ).map_err(|_|err("Actual reviewed app OS start failed"))?;
        let mut done=false;
        for _ in 0..1250{
            broker.poll(&Policy(true))
                .map_err(|_|err("Actual reviewed worker polling failed"))?;
            if broker.active_count()
                .map_err(|_|err("Broker lock failed"))?==0{
                done=true;break
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        if !done||effects.get(&exec.call_id)
            .map_err(|_|err("Exec effect lookup failed"))?
            .is_none_or(|e|e.state!=EffectState::Applied){
            return Err(err("Actual reviewed app failed to publish verified output").into())
        }
        if effects.get(&publish.call_id)
            .map_err(|_|err("Publish effect lookup failed"))?
            .is_none_or(|e|e.state!=EffectState::Applied){
            return Err(err("Publication journal was not APPLIED").into())
        }
        drop(broker);
    }
    let latency_ms=start.elapsed().as_secs_f64()*1000.0;
    let file=root.join(output_name);
    let (actual_bytes,digest,semantic_ok)=if supported{
        let bytes=fs::read(&file)?;
        let digest=format!("{:x}",Sha256::digest(&bytes));
        let pass=independently_validate_office(kind,&bytes,expected)
            .is_ok();
        (bytes.len(),Some(digest),pass)
    }else{(0,None,false)};
    println!("{}",serde_json::to_string(&json!({
        "schema":"lu-e1-p05-native-real-arm-v2",
        "mode":mode,
        "kind":args[1],
        "nativeOs":env::consts::OS,
        "setupMs":setup_ms,
        "latencyMs":latency_ms,
        "supported":supported,
        "artifactCreated":file.exists(),
        "artifactBytes":actual_bytes,
        "artifactSha256":digest,
        "semanticAccuracy":semantic_ok,
        "unauthorizedAttempts":if supported{1}else{0},
        "unauthorizedAttemptsBlocked":blocked,
        "journalBacked":supported,
        "producerAttribution":if mode=="reviewed_app"{
            "actual-reviewed-first-party-worker-stdout"
        }else{match kind{
            ArtifactKind::Docx=>"host-native-lu-word-utility",
            ArtifactKind::Xlsx=>"host-native-lu-spreadsheet-utility",
        }},
        "actualThirdPartyApp":false,
        "installedWindows":false,
        "modelUsed":false,
    }))?);
    Ok(())
}
fn main(){
    if let Err(e)=run(){
        eprintln!("E1-P05 actual native arm failed: {e}");
        std::process::exit(2);
    }
}
