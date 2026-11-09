//! MEMORY-LOOP-03C-G5: genuine Lu-reviewed Git + FFprobe worker outcomes
//! flow into the original ANSF/SQLite procedural-belief/CCM readers.
//!
//! The model responses below are deliberately SCRIPTED test requests, not
//! GGUF generations. Native Git and FFprobe are real pinned host-selected
//! executables launched through Lu's protected OS worker supervisor.
//! The independent Git/PCM source observers in native_reviewed_app_tool,
//! not this test or the model, write the separate durable verdicts.
//! Does not claim an OS sandbox, delegated cgroup escape custody, or an
//! installed Lu application's end-to-end performance.
#![cfg(test)]
use super::*;
use crate::memory_loop03b_context::for_current_human_turn;
use lu_core::{
    ansf_belief_revision::{Claim,ClaimReceipt},
    ansf_verified_procedures::ProcedureCandidate,
    contracts::{AuthorizationError,ProtoPayload,ToolAuthorizer,ToolDescriptor,ToolEffect,ToolInvocation},
    conversation_authority::{AuthorizedCapability,BoundCallOrigin,ConversationAuthority,TaskAuthority},
    continuity::{ContinuityStore,TaskCheckpoint,VerifiedEffectEvidence},
    inference_host::Cancellation,
    journal::{EffectJournal,EffectState},
    model_turn_journal::ModelTurnJournal,
    native_app_admission::HostReviewedExecutable,
    native_app_typed_git::{GitReadOperation,HostReviewedGitRead},
    native_app_typed_ffprobe::{HostReviewedMediaProbe,ReviewedContainer},
    native_background_process::BackgroundProcessSupervisor,
    native_reviewed_app_tool::ReviewedApplicationTurnSession,
    supervised_tool_generation::{ModelStepGenerator,NativeGenerationError,NativeTurnRecord},
    tasks::TaskStore,
};
use sha2::{Digest,Sha256};
use std::{
    collections::VecDeque,
    fs,
    path::{Path,PathBuf},
    process::Command,
    sync::Arc,
    time::{SystemTime,UNIX_EPOCH},
};
use serde_json::{json,Value};

const PROVENANCE:&str=
    "REAL_REVIEWED_LU_SUPERVISED_GIT_FFPROBE_WORKER_INDEPENDENT_SOURCE_ORACLE_TEST";
const GIT_TOOL:&str="lu.reviewed.git.head";
const MEDIA_TOOL:&str="lu.reviewed.media.facts";
const GIT_READ:&str="lu.reviewed.git.repo-read";
const MEDIA_READ:&str="lu.reviewed.media.file-read";

struct ExplicitHostPolicy{
    allowed:std::collections::BTreeSet<String>
}
impl ExplicitHostPolicy{
    fn new(media:bool)->Self{
        Self{allowed:capabilities(media).into_iter()
            .map(|c|c.tool_id).collect()}
    }
}
impl ToolAuthorizer for ExplicitHostPolicy {
    fn authorize(&self,call:&ToolInvocation,descriptor:&ToolDescriptor)
        ->Result<(),AuthorizationError>{
        if call.tool_id!=descriptor.id
            ||call.input.type_url!=descriptor.input_type_url
            ||!matches!(descriptor.effect,ToolEffect::ExecuteCode|ToolEffect::ReadWorkspace)
            ||!self.allowed.contains(&descriptor.id){
            return Err(AuthorizationError::new(
                "test host denied unselected reviewed native operation"
            ))
        }
        Ok(())
    }
}

/// Only drives the test's real OS tool-dispatch path, NOT the model evaluation.
/// No scripted answer can be counted as a genuine GGUF completion.
struct ScriptedModel{
    replies:VecDeque<String>,
    observed_prompts:Vec<String>,
}
impl ModelStepGenerator for ScriptedModel {
    fn generate_step(&mut self,prompt:&str,max_tokens:u32,cancel:&Cancellation)
        ->Result<String,NativeGenerationError>{
        assert!(max_tokens>0 && !cancel.is_cancelled());
        self.observed_prompts.push(prompt.to_owned());
        Ok(self.replies.pop_front().expect("unexpected scripted model generation"))
    }
}
fn scripted(names:&[&str])->ScriptedModel{
    let mut replies=VecDeque::new();
    for name in names {
        replies.push_back(format!(
            r#"{{"kind":"tool","name":"{name}","arguments":{{}}}}"#
        ));
    }
    replies.push_back(r#"{"kind":"final","text":"Historical receipts need new human review."}"#.into());
    ScriptedModel{replies,observed_prompts:Vec::new()}
}
fn pin(path:&Path)->String{
    format!("{:x}",Sha256::digest(fs::read(path).unwrap()))
}
fn host_binary(env:&str)->PathBuf{
    PathBuf::from(std::env::var_os(env).expect("real Git/FFprobe executable required"))
        .canonicalize().expect("native application must exist")
}
fn actual_git(git:&Path,repo:&Path,args:&[&str])->String{
    let output=Command::new(git).arg("-C").arg(repo).args(args)
        .output().expect("real native Git could not spawn");
    assert!(output.status.success(),
        "real Git operation failed: {}",String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}
fn wav_one_second()->Vec<u8>{
    let count=8000u32;
    let mut data=Vec::with_capacity(44+(count as usize)*2);
    data.extend_from_slice(b"RIFF");
    data.extend_from_slice(&(36+count*2).to_le_bytes());
    data.extend_from_slice(b"WAVEfmt ");
    data.extend_from_slice(&16u32.to_le_bytes());
    data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(&8000u32.to_le_bytes());
    data.extend_from_slice(&16000u32.to_le_bytes());
    data.extend_from_slice(&2u16.to_le_bytes());
    data.extend_from_slice(&16u16.to_le_bytes());
    data.extend_from_slice(b"data");
    data.extend_from_slice(&(count*2).to_le_bytes());
    data.extend(std::iter::repeat_n(0u8,(count as usize)*2));
    data
}
fn origin(chat:&str,turn:&str)->BoundCallOrigin{
    BoundCallOrigin{
        conversation_id:chat.into(),user_turn_id:turn.into(),
        task_id:turn.into()
    }
}
fn capabilities(media:bool)->Vec<AuthorizedCapability>{
    let mut out=vec![
        AuthorizedCapability{tool_id:GIT_TOOL.into(),effect:ToolEffect::ExecuteCode},
        AuthorizedCapability{tool_id:GIT_READ.into(),effect:ToolEffect::ReadWorkspace}
    ];
    if media {
        out.push(AuthorizedCapability{
            tool_id:MEDIA_TOOL.into(),effect:ToolEffect::ExecuteCode
        });
        out.push(AuthorizedCapability{
            tool_id:MEDIA_READ.into(),effect:ToolEffect::ReadWorkspace
        });
    }
    out
}
fn configs(git:&Path,ffprobe:Option<&Path>,repo:&Path,wav:&Path,scope:&str)
    ->(Vec<HostReviewedGitRead>,Option<HostReviewedMediaProbe>){
    let git_entry=HostReviewedGitRead{
        resource_id:"source-controlled-repo".into(),
        executable:HostReviewedExecutable{
            app_id:"lu-memory03c-reviewed-git".into(),
            app_version:"native-tested".into(),
            tool_id:"lu.reviewed.git.inventory".into(),
            workspace_id:scope.into(),
            executable:git.to_path_buf(),expected_sha256:pin(git),
        },
        repository_root:repo.to_path_buf(),
        execute_tool_id:GIT_TOOL.into(),read_tool_id:GIT_READ.into(),
        operation:GitReadOperation::HeadCommit,
        stdout_limit:16_384,timeout_seconds:15,
    };
    let media=ffprobe.map(|binary|HostReviewedMediaProbe{
        profile_id:"source-controlled-wav".into(),
        executable:HostReviewedExecutable{
            app_id:"lu-memory03c-reviewed-ffprobe".into(),
            app_version:"native-tested".into(),
            tool_id:"lu.reviewed.media.inventory".into(),
            workspace_id:scope.into(),
            executable:binary.to_path_buf(),expected_sha256:pin(binary),
        },
        approved_media_path:wav.to_path_buf(),media_sha256:pin(wav),
        reviewed_container:ReviewedContainer::Wave,
        execute_tool_id:MEDIA_TOOL.into(),read_tool_id:MEDIA_READ.into(),
        stdout_limit:16_384,timeout_seconds:15,
    });
    (vec![git_entry],media)
}
fn host_verified(
    journal:&EffectJournal,call:&str,tool:&str,suffix:&str
)->(String,String){
    let inner=format!("{call}-{suffix}");
    let outer=journal.get(call).unwrap().expect("durable outer effect");
    assert_eq!(outer.state,EffectState::Applied);
    let record=journal.get(&inner).unwrap().expect("real inner OS worker effect");
    assert_eq!(record.tool_id,tool);
    assert_eq!(record.state,EffectState::Applied);
    let bytes=record.receipt.expect("native OS worker durable receipt");
    let digest=format!("{:x}",Sha256::digest(&bytes));
    assert!(journal.independent_verified_receipt(
        &inner,tool,&digest).unwrap(),
        "worker's model reply/APPLIED state alone is not an independent source oracle");
    (inner,digest)
}
fn current_source(chats:&ChatStore,chat:&str,exact_text:&str)->i64{
    chats.require(chat).unwrap().recent_context_entries(chat,100).unwrap()
        .into_iter().find(|x|x.role=="user" && x.content==exact_text)
        .expect("source is an authentic original human ANSF entry").id
}
fn live_capsule(chats:&ChatStore,continuity:&ContinuityStore,
    journal:&EffectJournal,chat:&str,index:usize,
    prompt:&str,goal:&str)->Value{
    let turn=format!("g-{:032x}",index+100);
    chats.append_user_with_turn_id(chat,&turn,prompt).unwrap();
    let raw=for_current_human_turn(
        chats,continuity,journal,chat,&turn,prompt,
        goal,"NO_REVIEWED_EFFECTS_IN_BOUNDED_WINDOW"
    ).unwrap().expect("explicit original human selector");
    let capsule:Value=serde_json::from_str(&raw).unwrap();
    assert_eq!(capsule["executionAuthority"],"NOT_AUTHORIZED_FOR_EXECUTION");
    assert_eq!(capsule["priorGrantReusable"],false);
    assert_eq!(capsule["automaticRetryAllowed"],false);
    assert_eq!(capsule["currentSourceFreshness"],"NOT_RECHECKED");
    assert_eq!(capsule["oldOsCustodyNotImplied"],true);
    assert_eq!(capsule["modelMayOverrideHostDecision"],false);
    capsule
}
fn observation_row(id:&str,prompt:&str,index:usize,capsule:Value)->Value{
    json!({
        "caseId":id,"originalHumanPrompt":prompt,
        "originalHumanTurnId":format!("g-{:032x}",index+100),
        "expectedStatus":capsule["status"],
        "expectedHistoricalVerified":capsule["historicallyVerifiedDataAvailable"],
        "expectedHumanReview":capsule["humanReviewRequired"],
        "capsule":capsule
    })
}

/// A real native OS acceptance test, explicitly invoked by the task-group
/// workflow with installed Git and FFprobe. Never silently passed by cargo
/// test on a machine without those executables.
#[test]
#[ignore="real reviewed Git/FFprobe workers, current human authority and OS runner required"]
fn memory_loop03c_reviewed_git_ffprobe_changed_source_and_exclusive_restart(){
    let git=host_binary("LU_MEMORY_LOOP03C_GIT");
    let ffprobe=host_binary("LU_MEMORY_LOOP03C_FFPROBE");
    let root=std::env::temp_dir().join(format!(
        "lu-03c-reviewed-{}-{}",std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    ));
    let repo=root.join("approved-git");
    fs::create_dir_all(&repo).unwrap();
    actual_git(&git,&repo,&["init","--quiet"]);
    fs::write(repo.join("approved.txt"),b"approved initial source").unwrap();
    actual_git(&git,&repo,&["add","approved.txt"]);
    actual_git(&git,&repo,&["-c","user.name=Lu","-c",
        "user.email=lu@example.invalid","commit","--quiet","-m","initial"]);
    let expected_head=actual_git(&git,&repo,&["rev-parse","HEAD"]);
    let wav=root.join("approved.wav");
    fs::write(&wav,wav_one_second()).unwrap();

    let profile=root.join("profile");
    let chats=ChatStore::open(&profile).unwrap();
    let chat=chats.create_chat().unwrap().chat_id;
    let first_turn=format!("g-{:032x}",1);
    let original="Please inspect approved Git HEAD and approved WAV with native reviewed tools";
    chats.append_user_with_turn_id(&chat,&first_turn,original).unwrap();
    let source=current_source(&chats,&chat,original);
    let book=chats.require(&chat).unwrap();
    let continuity_path=root.join("continuity.sqlite");
    let continuity=ContinuityStore::open(&continuity_path).unwrap();
    // Production reviewed-app custody uses an immutable per-turn worker
    // journal and task owner binding; do not silently share a new grant.
    let effect_path=root.join("effects-first-turn.sqlite");
    let effects=Arc::new(EffectJournal::open(&effect_path).unwrap());
    let tasks=Arc::new(TaskStore::open(&root.join("tasks-first-turn.sqlite")).unwrap());
    tasks.bind_reviewed_human_turn(&chat,&first_turn).unwrap();
    let supervisor=Arc::new(BackgroundProcessSupervisor::new_for_reviewed_host(
        Arc::clone(&tasks),profile.clone()
    ));
    let authority=ConversationAuthority::new();
    let scope="m03c-e1-owner-scope";
    let first_origin=origin(&chat,&first_turn);
    authority.open_user_task(TaskAuthority{
        conversation_id:chat.clone(),
        user_turn_id:first_turn.clone(),task_id:first_turn.clone(),
        execution_scope_id:scope.into(),capabilities:capabilities(true),
        expires_at:None,
    },&chats).unwrap();
    let (git_review,media_review)=configs(
        &git,Some(&ffprobe),&repo,&wav,scope
    );
    let mut session=ReviewedApplicationTurnSession::from_verified_host_task(
        authority.clone(),first_origin.clone(),scope.into(),
        Arc::clone(&tasks),Arc::clone(&supervisor),Arc::clone(&effects),
        Arc::new(ExplicitHostPolicy::new(true)),git_review,media_review
    ).unwrap();
    assert_eq!(session.schemas().unwrap().len(),2);
    let audit=ModelTurnJournal::open(&root.join("model-turn-audit.sqlite")).unwrap();
    let mut scripted_model=scripted(&["lu_reviewed_git_head","lu_reviewed_media_facts"]);
    let first_report=tauri::async_runtime::block_on(
        session.run_journaled_observed(
            original,&mut scripted_model,&Cancellation::default(),&audit,&|_|{}
        )
    ).unwrap();
    assert_eq!(first_report.host_routed_calls,0);
    assert_eq!(first_report.tool_calls,2);
    assert_eq!(first_report.model_generations,3);
    assert_eq!(scripted_model.observed_prompts.len(),3);
    assert!(scripted_model.observed_prompts.iter().any(
        |p|p.contains(&expected_head)
    ));
    let mut git_call=None;
    let mut wav_call=None;
    for item in &first_report.records {
        if let NativeTurnRecord::ToolFeedback(row)=item {
            let call=row["call_id"].as_str().unwrap().to_string();
            if row["name"]=="lu_reviewed_git_head" {
                assert_eq!(row["response"]["result"]["headCommit"],expected_head);
                assert_eq!(row["response"]["result"]["independentOutcome"]["status"],
                    "VERIFIED");
                git_call=Some(call);
            }else if row["name"]=="lu_reviewed_media_facts" {
                assert_eq!(row["response"]["result"]["durationMilliseconds"],1000);
                assert_eq!(row["response"]["result"]["independentOutcome"]["status"],
                    "VERIFIED");
                wav_call=Some(call);
            }
        }
    }
    let (git_effect,git_digest)=host_verified(
        &effects,&git_call.expect("real Git model tool feedback"),GIT_TOOL,"gh"
    );
    let (wav_effect,_wav_digest)=host_verified(
        &effects,&wav_call.expect("real FFprobe model tool feedback"),MEDIA_TOOL,"fm"
    );
    // A completed reviewed turn is revoked by the EXISTING broker; do not
    // interpret a historical success as extending its human permission.
    assert!(authority.permitted_tool_ids(&first_origin).is_err());
    drop(session);

    let checkpoint="cp-m03c-reviewed-original";
    continuity.append_verified(&TaskCheckpoint{
        checkpoint_id:checkpoint.into(),project_id:"lu".into(),
        task_id:"review".into(),conversation_id:chat.clone(),
        source_entry_ids:vec![source],
        completed_work:"Two native OS effects independently checked".into(),
        next_action:"Review source freshness before more operations".into(),
        blockers:vec![],
    },&book).unwrap();
    continuity.link_verified_effect(checkpoint,&git_effect,&effects).unwrap();
    continuity.link_verified_effect(checkpoint,&wav_effect,&effects).unwrap();
    let independent=|e:&VerifiedEffectEvidence|effects
        .independent_verified_receipt(
            &e.call_id,&e.tool_id,&e.receipt_sha256).unwrap_or(false);
    for (proc_id,call,tool,label) in [
        ("proc-m03c-git",&git_effect,GIT_TOOL,"Historical Git HEAD"),
        ("proc-m03c-wav",&wav_effect,MEDIA_TOOL,"Historical WAV facts")
    ] {
        book.promote_verified_procedure(&ProcedureCandidate{
            procedure_id:proc_id.into(),project_id:"lu".into(),
            task_id:"review".into(),session_id:chat.clone(),
            source_entry_id:source,checkpoint_id:checkpoint.into(),
            call_id:call.clone(),tool_id:tool.into(),
            tool_version:"1.0".into(),application_version:"0.1".into(),
            title:label.into(),
            prerequisites:vec!["Original selected resource and current human approval".into()],
            steps:vec!["Review earlier independently observed result only".into()],
            failure_conditions:vec!["Source revision or worker version changed".into()],
            replaces_id:None,
        },&continuity,&effects,independent).unwrap();
    }
    book.record_verified_claim(&Claim{
        claim_id:"claim-m03c-reviewed-head".into(),session_id:chat.clone(),
        source_entry_id:source,project_id:"lu".into(),task_id:"review".into(),
        subject_key:"source".into(),predicate_key:"head".into(),
        value:expected_head.clone(),application_version:"0.1".into(),
        expires_unix:SystemTime::now().duration_since(UNIX_EPOCH)
            .unwrap().as_secs()+3600,supersedes_id:None,
    },ClaimReceipt{
        checkpoint_id:checkpoint.into(),call_id:git_effect.clone(),
        tool_id:GIT_TOOL.into()
    },&continuity,&effects,independent).unwrap();

    let git_prompt="Recall procedure project lu task review tool lu.reviewed.git.head tool-version 1.0 application-version 0.1";
    let wav_prompt="Recall procedure project lu task review tool lu.reviewed.media.facts tool-version 1.0 application-version 0.1";
    let belief_prompt="Recall belief project lu task review subject source predicate head version 0.1";
    let ccm_prompt="Resume project lu task review";
    let mut rows=Vec::new();
    for (idx,(case,prompt,status)) in [
        ("protected_git_history",git_prompt,"HISTORICAL_SOURCE_ORACLE_VERIFIED_PROCEDURE"),
        ("protected_ffprobe_history",wav_prompt,"HISTORICAL_SOURCE_ORACLE_VERIFIED_PROCEDURE"),
        ("protected_git_belief",belief_prompt,"HISTORICAL_SOURCE_ORACLE_VERIFIED_BELIEF"),
    ].into_iter().enumerate() {
        let capsule=live_capsule(
            &chats,&continuity,&effects,&chat,idx,prompt,"CURRENT_GOAL_REVIEWABLE_ONLY"
        );
        assert_eq!(capsule["status"],status);
        assert_eq!(capsule["historicallyVerifiedDataAvailable"],true);
        rows.push(observation_row(case,prompt,idx,capsule));
    }

    // The original observed source is externally revised AFTER the real
    // protected worker returned. Memory is still only historical and cannot
    // tell the model "CURRENT_VERIFIED" or reopen the earlier grant.
    fs::write(repo.join("approved.txt"),b"approved source changed after verification").unwrap();
    actual_git(&git,&repo,&["add","approved.txt"]);
    actual_git(&git,&repo,&["-c","user.name=Lu","-c",
        "user.email=lu@example.invalid","commit","--quiet","-m","revised"]);
    let current_head=actual_git(&git,&repo,&["rev-parse","HEAD"]);
    assert_ne!(current_head,expected_head);
    let old=live_capsule(
        &chats,&continuity,&effects,&chat,3,git_prompt,
        "CURRENT_GOAL_REVIEWABLE_ONLY"
    );
    assert_eq!(old["status"],"HISTORICAL_SOURCE_ORACLE_VERIFIED_PROCEDURE");
    assert_eq!(old["currentSourceFreshness"],"NOT_RECHECKED");
    rows.push(observation_row("changed_source_old_history_not_fresh",git_prompt,3,old));

    // A remembered procedure and a model reply are not authenticated human
    // turns. The old task was revoked. Neither can mint a fresh resource
    // permission or schedule an automatic replay of the earlier OS effect.
    let forged=format!("g-{:032x}",999);
    let denied=authority.open_user_task(TaskAuthority{
        conversation_id:chat.clone(),user_turn_id:forged.clone(),
        task_id:forged.clone(),execution_scope_id:scope.into(),
        capabilities:capabilities(false),expires_at:None,
    },&chats);
    assert!(denied.is_err(),"recalled model text cannot act as human consent");
    assert!(authority.permitted_tool_ids(&first_origin).is_err());

    // A DIFFERENT authenticated current human message is necessary to
    // obtain a new native Git source observation. No old grant is copied.
    let second_turn=format!("g-{:032x}",2);
    let new_human="Please perform a NEW read-only reviewed Git HEAD check for the changed repository";
    chats.append_user_with_turn_id(&chat,&second_turn,new_human).unwrap();
    let changed_source=current_source(&chats,&chat,new_human);
    let fresh_origin=origin(&chat,&second_turn);
    authority.open_user_task(TaskAuthority{
        conversation_id:chat.clone(),user_turn_id:second_turn.clone(),
        task_id:second_turn.clone(),execution_scope_id:scope.into(),
        capabilities:capabilities(false),expires_at:None,
    },&chats).unwrap();
    // The next user turn has a DIFFERENT production-style worker journal,
    // not a new grant inside the old effect ledger. Both journals survive.
    let new_task_path=root.join("tasks-second-turn.sqlite");
    let new_effect_path=root.join("effects-second-turn.sqlite");
    let new_tasks=Arc::new(TaskStore::open(&new_task_path).unwrap());
    new_tasks.bind_reviewed_human_turn(&chat,&second_turn).unwrap();
    let new_effects=Arc::new(EffectJournal::open(&new_effect_path).unwrap());
    let new_supervisor=Arc::new(BackgroundProcessSupervisor::new_for_reviewed_host(
        Arc::clone(&new_tasks),profile.clone()
    ));
    let (new_review,_)=configs(&git,None,&repo,&wav,scope);
    let mut fresh_session=ReviewedApplicationTurnSession::from_verified_host_task(
        authority.clone(),fresh_origin.clone(),scope.into(),
        Arc::clone(&new_tasks),Arc::clone(&new_supervisor),
        Arc::clone(&new_effects),
        Arc::new(ExplicitHostPolicy::new(false)),new_review,None
    ).unwrap();
    let mut fresh_model=scripted(&["lu_reviewed_git_head"]);
    let fresh_report=tauri::async_runtime::block_on(
        fresh_session.run_journaled_observed(
            new_human,&mut fresh_model,&Cancellation::default(),&audit,&|_|{}
        )
    ).unwrap();
    assert_eq!(fresh_report.tool_calls,1);
    let new_tool_call=fresh_report.records.iter().find_map(|x|{
        if let NativeTurnRecord::ToolFeedback(v)=x {
            if v["name"]=="lu_reviewed_git_head" {
                assert_eq!(v["response"]["result"]["independentOutcome"]["status"],
                    "VERIFIED");
                assert_eq!(v["response"]["result"]["headCommit"],current_head);
                return v["call_id"].as_str().map(str::to_owned)
            }
        }
        None
    }).expect("separate human-authorized native Git feedback");
    let (new_effect,new_sha)=host_verified(&new_effects,&new_tool_call,GIT_TOOL,"gh");
    assert_ne!(new_effect,git_effect);
    assert_ne!(new_sha,git_digest);
    assert!(authority.permitted_tool_ids(&fresh_origin).is_err());
    drop(fresh_session);

    // Changed source is NOT silently corrected by an LLM. The host writes
    // a NEW source-linked checkpoint with a real, independently observed
    // different HEAD. CCM must block autonomous resumption for human review.
    continuity.append_verified(&TaskCheckpoint{
        checkpoint_id:"cp-m03c-reviewed-revision".into(),
        project_id:"lu".into(),task_id:"review".into(),
        conversation_id:chat.clone(),source_entry_ids:vec![changed_source],
        completed_work:"New reviewed Git observation differs from the old one".into(),
        next_action:"Ask human to reconcile source revision".into(),
        blockers:vec!["Approved source HEAD changed".into()],
    },&book).unwrap();
    let review=chats.monitor_project_context(&chat,"lu","review",&continuity)
        .unwrap().expect("two original checkpoints");
    assert!(review.needs_review && review.source_evidence_changed);
    let changed=live_capsule(
        &chats,&continuity,&effects,&chat,4,ccm_prompt,
        "CURRENT_GOAL_REVIEWABLE_ONLY"
    );
    assert_eq!(changed["status"],"ORIGINAL_CONTEXT_CHANGED_HUMAN_REVIEW");
    assert_eq!(changed["humanReviewRequired"],true);
    rows.push(observation_row("new_worker_changed_ccm_review",ccm_prompt,4,changed));

    // Explicit new human correction, never an old procedure or autonomous
    // source change, invalidates the old procedure and retracts the belief.
    let correction_turn=format!("g-{:032x}",3);
    let correction="I reviewed the new Git HEAD: retract the historical claim and procedure";
    chats.append_user_with_turn_id(&chat,&correction_turn,correction).unwrap();
    let correction_source=current_source(&chats,&chat,correction);
    book.invalidate_verified_procedure(
        "proc-m03c-git",&chat,correction_source,"Human revoked old HEAD procedure"
    ).unwrap();
    book.retract_reported_or_verified_claim(
        "claim-m03c-reviewed-head",&chat,correction_source,
        "New Git HEAD makes earlier claim obsolete"
    ).unwrap();
    let retracted=live_capsule(
        &chats,&continuity,&effects,&chat,5,belief_prompt,
        "CURRENT_GOAL_REVIEWABLE_ONLY"
    );
    assert_eq!(retracted["status"],"RETRACTED_OR_SUPERSEDED_BELIEF");
    assert_eq!(retracted["historicallyVerifiedDataAvailable"],false);
    rows.push(observation_row("old_git_belief_human_retracted",belief_prompt,5,retracted));
    let invalid=live_capsule(
        &chats,&continuity,&effects,&chat,6,git_prompt,
        "CURRENT_GOAL_REVIEWABLE_ONLY"
    );
    assert_eq!(invalid["status"],"NO_CURRENTLY_REVALIDATED_VERIFIED_PROCEDURE");
    rows.push(observation_row("old_git_procedure_revoked",git_prompt,6,invalid));

    drop(independent);
    // Recovery: real supervised Git and FFprobe OS processes have already
    // completed/reaped. Test an additional JOURNAL-ONLY interrupted intent;
    // it does not launch a child and cannot prove runaway-descendant custody.
    let uncertain_id="m03c-uncertain-no-child";
    new_effects.prepare(&ToolInvocation{
        call_id:uncertain_id.into(),tool_id:GIT_TOOL.into(),
        workspace_id:Some(scope.into()),
        input:ProtoPayload{type_url:"lu.memory03c.UnfinishedObservation".into(),
            bytes:b"no OS child launched".to_vec()}
    }).unwrap();
    new_effects.mark_executing(uncertain_id).unwrap();
    drop(audit);drop(new_supervisor);drop(new_tasks);drop(new_effects);
    drop(supervisor);drop(tasks);drop(effects);
    let reopened=EffectJournal::open(&new_effect_path).unwrap();
    let old_reopened=EffectJournal::open(&effect_path).unwrap();
    let reopened_tasks=TaskStore::open(&new_task_path).unwrap();
    let (_native_task_recovery,uncertain)=
        lu_core::native_app_execution::reconcile_after_exclusive_host_restart(
            &reopened_tasks,&reopened
        ).unwrap();
    assert_eq!(uncertain,1);
    assert_eq!(reopened.reconcile_interrupted().unwrap(),0);
    drop(reopened_tasks);
    assert_eq!(reopened.get(uncertain_id).unwrap().unwrap().state,EffectState::Uncertain);
    assert_eq!(old_reopened.get(&git_effect).unwrap().unwrap().state,EffectState::Applied);
    assert_eq!(old_reopened.get(&wav_effect).unwrap().unwrap().state,EffectState::Applied);
    assert_eq!(reopened.get(&new_effect).unwrap().unwrap().state,EffectState::Applied);
    assert!(!reopened.independent_verified_receipt(
        uncertain_id,GIT_TOOL,&"0".repeat(64)).unwrap());

    // ANSF and durable source+human correction survive reopen unchanged.
    drop(book);drop(continuity);drop(chats);
    let recalled=ChatStore::open(&profile).unwrap();
    let old_continuity=ContinuityStore::open(&continuity_path).unwrap();
    let after=live_capsule(
        &recalled,&old_continuity,&old_reopened,&chat,7,belief_prompt,
        "CURRENT_GOAL_REVIEWABLE_ONLY"
    );
    assert_eq!(after["status"],"RETRACTED_OR_SUPERSEDED_BELIEF");
    assert_eq!(after["executionAuthority"],"NOT_AUTHORIZED_FOR_EXECUTION");
    rows.push(observation_row("restart_preserves_retracted_belief",belief_prompt,7,after));

    let result=json!({
        "schema":"lu-memory-loop03c-reviewed-git-ffprobe-native-v1",
        "producer":"lu-desktop::memory_loop03c_reviewed_worker_tests",
        "provenance":PROVENANCE,
        "modelGenerations":"SCRIPTED_ONLY_REAL_OS_WORKERS",
        "originalHumanAuthenticated":true,
        "sameGrantReused":false,
        "sourceRevisionDetectedOnlyWithFreshHumanGrant":true,
        "historicalPermissionNeverGrantsExecution":true,
        "osDescendantEscapeCustodyProven":false,
        "installedLu":false,
        "originalGitHead":expected_head,
        "authorizedRecheckGitHead":current_head,
        "reviewedWorkerOperations":3,
        "independentOutcomeVerifiedAtObservation":3,
        "journalOnlyUncertainRecovery":true,
        "cases":rows
    });
    if let Some(dest)=std::env::var_os("LU_MEMORY_LOOP03C_REVIEWED_FIXTURE_PATH"){
        fs::write(dest,serde_json::to_vec_pretty(&result).unwrap()).unwrap();
    }
    drop(recalled);drop(old_continuity);drop(old_reopened);drop(reopened);
    fs::remove_dir_all(root).unwrap();
}
