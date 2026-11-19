//! CP-06A audit only. The externally supplied runtime availability in this
//! fixture is deliberately hypothetical; no browser/external worker is started.
//! Built-in/Git lanes use real private fixtures and the production selector,
//! registry, authority, codecs, supervisor, and native feedback journal.
use super::*;
use lu_core::native_capability_inventory::{discover_ordinary_pending,OrdinaryRuntimeEvidence,RuntimeEvidence::*};
use std::{collections::BTreeSet, path::Path};

const CASES:[(&str,&str);17]=[
 ("lu_files_read_text","What is the exact UTF-8 text in my workspace file notes.txt?"),
 ("lu_files_list_directory","Which files are in my project folder?"),
 ("lu_files_stat","What is the exact byte size of notes.txt in my workspace?"),
 ("lu_files_search_text","Which files in my folder contain the literal text audit-needle?"),
 ("lu_files_discover","Where is the file named needle-audit.txt in my workspace?"),
 ("lu_reader_read","What does my document say?"),
 ("lu_archive_list","What are the member names inside bundle.zip in my folder?"),
 ("lu_artifact_inspect","What is the SHA-256 digest of notes.txt in my workspace?"),
 ("lu_git_status","What is the status of my Git repository?"),
 ("lu_git_log","What is the recent history of my Git repository?"),
 ("lu_git_diff","What is the tracked diff of notes.txt in my Git repository?"),
 ("lu_validate_schema",r#"Could you validate this JSON schema {"schema":{"type":"integer"},"data":71}"#),
 ("lu_check_env","Could you tell me whether git is installed here?"),
 ("lu_browser_read","What information is on https://example.org/report?"),
 ("lu_reviewed_pdf_facts","How many pages are in my PDF?"),
 ("lu_reviewed_image_facts","What are the dimensions of my PNG?"),
 ("lu_reviewed_media_facts","How long is my WAV?"),
];
fn evidence(allowed:bool)->OrdinaryRuntimeEvidence{
 OrdinaryRuntimeEvidence{workspace:Builtin,git:CandidatePresent,browser:PinnedPackagePresent,media:CandidatePresent,pdf:CandidatePresent,image:CandidatePresent,program_specs:crate::environment_context::fixed_ambient_program_candidates(),allow_tools:allowed}
}
#[test]
fn exhaustive_ordinary_offer_identity_and_refusal(){
 let views=discover_ordinary_pending(&evidence(true)).unwrap();
 let all:BTreeSet<_>=views.iter().map(|v|v.public_name.as_str()).collect();
 let expected:BTreeSet<_>=CASES.iter().map(|(n,_)|*n).chain(["lu_browser_interact"]).collect();
 assert_eq!(all,expected);
 assert!(views.iter().all(|v|v.authorization_required&&!v.executable_now&&!v.resource_bound&&!v.execution_instance_registered&&!v.task_outcome_verified));
 assert_eq!(views.iter().find(|v|v.public_name=="lu_browser_interact").unwrap().state,PendingToolState::PolicyBlocked);
 let blocked=discover_ordinary_pending(&evidence(false)).unwrap();
 for (name,prompt) in CASES{
  let offered=eligible_candidates(&views,prompt);
  assert!(offered.iter().any(|(c,_)|c.public_name==name),"not offered: {name} {prompt}");
  let exact=serde_json::json!({"selection":name}).to_string();
  assert_eq!(parse_selection(&exact,&offered).unwrap().0.unwrap().public_name,name);
  assert!(parse_selection(r#"{"selection":null}"#,&offered).unwrap().0.is_none());
  assert!(parse_selection(&serde_json::json!({"selection":name,"path":"/foreign"}).to_string(),&offered).is_err());
  assert!(parse_selection(r#"{"selection":"lu_browser_interact"}"#,&offered).is_err());
  assert!(parse_selection(r#"{"selection":"lu_file_move"}"#,&offered).is_err());
  assert!(eligible_candidates(&blocked,prompt).is_empty());
  assert!(eligible_candidates(&views,&format!("{prompt} without tools")).is_empty());
  println!("CP06A_OFFER_CONTRACT_PASS {name} offered={}",offered.len());
 }
 // Record the uncovered false-negative on an ordinary read request.
 println!("CP06A_COMMIT_READ_ELIGIBLE={}",eligible("What is the latest commit subject in my Git repository?"));
 let mut absent=evidence(true);absent.workspace=Missing;absent.git=Missing;absent.browser=Missing;absent.media=Missing;absent.pdf=Missing;absent.image=Missing;
 let absent=discover_ordinary_pending(&absent).unwrap();
 for v in absent{
  let expected=match v.public_name.as_str(){"lu_check_env"|"lu_validate_schema"=>PendingToolState::NeedsAuthorization,"lu_browser_interact"=>PendingToolState::PolicyBlocked,_=>PendingToolState::RuntimeUnavailable};
  assert_eq!(v.state,expected,"{}",v.public_name);
 }
}

#[test]
#[ignore="requires reviewed local llama-server, pinned GGUF, native Git, and fixture-host approval only"]
fn genuine_selection_and_native_reachability_matrix(){
 use lu_core::{inference_supervisor::TrustedInferenceSupervisor,model_admission::ModelAdmissionManager,
  native_workspace_read_turn::WorkspaceReadTurnSession,native_reader_turn::ReaderTurnSession,
  native_core_utility_turn::CoreUtilityTurn,native_environment_discovery::HostProgramInventory,
  persistent_llama_backend::{PersistentEngineConfig,PersistentLlamaBackend}};
 use sha2::{Digest,Sha256};
 use std::{io::Read,num::NonZeroUsize,path::PathBuf,process::Command,time::Duration};
 fn hash(p:&Path)->String{let mut h=Sha256::new();let mut f=std::fs::File::open(p).unwrap();let mut b=[0;65536];loop{let n=f.read(&mut b).unwrap();if n==0{break}h.update(&b[..n]);}format!("{:x}",h.finalize())}
 fn from_hex(s:&str)->Vec<u8>{(0..s.len()).step_by(2).map(|i|u8::from_str_radix(&s[i..i+2],16).unwrap()).collect()}
 let exe=PathBuf::from(std::env::var("LU_CP05E_SERVER").unwrap());let model=PathBuf::from(std::env::var("LU_CP05E_GGUF").unwrap());
 let esha=std::env::var("LU_CP05E_SERVER_SHA256").unwrap();let msha=std::env::var("LU_CP05E_GGUF_SHA256").unwrap();
 assert_eq!(hash(&exe),esha);assert_eq!(hash(&model),msha);let pin:[u8;32]=from_hex(&esha).try_into().unwrap();
 let auth=std::env::var("LU_CP05E_API_KEY").unwrap();let mut manager=ModelAdmissionManager::new();let admitted=manager.admit(model.parent().unwrap(),model.file_name().unwrap().to_str().unwrap()).unwrap().clone();
 let supervisor=TrustedInferenceSupervisor::new("llama-cpu".into(),8192,vec![("model-1".into(),admitted)],move||PersistentLlamaBackend::new(PersistentEngineConfig{executable:exe.clone(),expected_sha256:pin,threads:NonZeroUsize::new(3).unwrap(),context_tokens:8192,startup_timeout:Duration::from_secs(120),request_timeout:Duration::from_secs(120),api_key:auth.clone(),load_observer:None})).unwrap();
 let sid="cp06a-selection";supervisor.load("model-1",sid.into(),"cp06a-load").unwrap();assert!(supervisor.status().unwrap().pid.is_some());
 let base=std::env::temp_dir().join(next_id("cp06a-audit-"));std::fs::create_dir(&base).unwrap();
 let root=base.join("workspace");std::fs::create_dir(&root).unwrap();std::fs::create_dir(root.join("sub")).unwrap();
 let marker="AUDIT-ORCHID-76421";let contents=format!("{marker}\naudit-needle\n");
 std::fs::write(root.join("notes.txt"),&contents).unwrap();std::fs::write(root.join("sub/needle-audit.txt"),"fixture").unwrap();std::fs::write(root.join("orchid-root-31415.txt"),"root entry").unwrap();
 std::fs::write(root.join("bundle.zip"),from_hex("504b0304140000000000293b735dee40e5050700000007000000110000006f72636869642d6d656d6265722e74787466697874757265504b01021403140000000000293b735dee40e50507000000070000001100000000000000000000008001000000006f72636869642d6d656d6265722e747874504b050600000000010001003f000000360000000000")).unwrap();
 let git=PathBuf::from(std::env::var("LU_TOOLS03_TEST_GIT_PATH").unwrap());assert!(git.is_absolute()&&git.is_file());
 let repo=base.join("repo");std::fs::create_dir(&repo).unwrap();let git_run=|args:&[&str]|{assert!(Command::new(&git).arg("-C").arg(&repo).args(args).output().unwrap().status.success())};
 git_run(&["init","-q"]);std::fs::write(repo.join("notes.txt"),"CP06A_OLD_8462\n").unwrap();git_run(&["add","notes.txt"]);git_run(&["-c","user.name=Lu audit","-c","user.email=lu@example.invalid","commit","--no-gpg-sign","-qm","CP06A_COMMIT_7496"]);std::fs::write(repo.join("notes.txt"),"CP06A_NEW_8462\n").unwrap();
 let views=discover_ordinary_pending(&evidence(true)).unwrap();let chats=ChatStore::open(&base.join("chats")).unwrap();let queue=Arc::new(generation_queue::GenerationQueue::default());let mut failures=Vec::new();
 let selected=std::env::var("LU_CP06A_CASES").ok().map(|v|v.split(',').map(str::to_owned).collect::<BTreeSet<_>>());
 if let Some(s)=&selected{assert!(!s.is_empty()&&s.iter().all(|n|CASES.iter().any(|(name,_)|*name==n)));}
 for (name,human) in CASES{
  if selected.as_ref().is_some_and(|s|!s.contains(name)){continue}
  let chat=chats.create_chat().unwrap().chat_id;let gid=next_id("g-");let verifier=PersistedHumanTurn::current_chat(&chats,&chat,&gid,human);
  let (ticket,_)=queue.enqueue_with_class_committing(sid,&chat,&gid,TurnClass::Foreground,||chats.append_user_with_turn_id(&chat,&gid,human)).unwrap();ticket.wait_for_slot().unwrap();
  let audit=ModelTurnJournal::open(&base.join(format!("selection-{gid}.sqlite"))).unwrap();let tool=ModelTurnJournal::open(&base.join(format!("tool-{gid}.sqlite"))).unwrap();
  let cancel=Cancellation::default();let check=||{ticket.verify_active_foreground(sid,&chat,&gid)?;if verifier.verify_user_turn(&chat,&gid){Ok(())}else{Err("original changed".into())}};
  let record=|kind,payload:&str,repair|audit.append(&chat,&gid,kind,payload,repair).map_err(|_|"audit unavailable".into());record(ModelAuditKind::UserRequest,human,None).unwrap();
  let offered=eligible_candidates(&views,human);let route=crate::turn_coordinator::select(human,&[],512,crate::turn_coordinator::SendMode::Ordinary).unwrap();let route=if matches!(route,crate::turn_coordinator::TurnRoute::Model){"ordinary_model"}else{"host_preroute"};
  let mut selector=SupervisorModelStep::new(&supervisor,sid.into(),next_id("cp06sel-")).unwrap();let sel=audited_selection(&mut selector,&selection_prompt(human,&offered).unwrap(),&offered,&cancel,&check,&record);
  println!("CP06A_SELECTION {name} route={route} result={sel:?}");
  let result=(||->Result<String,String>{
   let choice=sel?.ok_or("model abstained")?;if choice.public_name!=name{return Err(format!("wrong capability {}",choice.public_name))}check()?;
   if name=="lu_browser_read"||name.starts_with("lu_reviewed_"){return Ok("SELECTION_ONLY_EXTERNAL_NOT_QUALIFIED".into())}
   let mut step=SupervisorModelStep::new(&supervisor,sid.into(),next_id("cp06exec-")).unwrap();
   let outcome=if name=="lu_validate_schema"||name=="lu_check_env"{
    let exact=crate::core_utility_desktop::negotiable_core_utility_request(human).unwrap();let mut lane=CoreUtilityTurn::new(&verifier,&chat,&gid,&gid,exact).map_err(|e|format!("{e:?}"))?;assert!(matches_live(&choice,&lane.capabilities().unwrap()));
    tauri::async_runtime::block_on(lane.run_journaled_observed(&crate::core_utility_desktop::utility_model_prompt(name,human),&mut step,&cancel,&tool,&|_|{}))
   }else if name=="lu_reader_read"{
    let mut lane=ReaderTurnSession::new(&verifier,&chat,&gid,&gid,"approved.txt",contents.as_bytes().to_vec()).unwrap();assert!(crate::reader_desktop::verified_excerpt_requested(human));lane.require_verified_excerpt().unwrap();assert!(matches_live(&choice,&lane.capabilities().unwrap()));tauri::async_runtime::block_on(lane.run_journaled(&crate::reader_desktop::verified_excerpt_prompt(human),&mut step,&cancel,&tool))
   }else{
    let op=exact_workspace_read(human)?.ok_or("Missing exact original workspace input")?;
    if op.native_id()!=choice.native_tool_id{return Err("Selected capability differs from exact input".into())}
    let (approved,exe)=if choice.git{(&repo,Some(git.as_path()))}else{(&root,None)};
    let mut lane=WorkspaceReadTurnSession::new_exact(&verifier,&chat,&gid,&gid,approved,exe,op).map_err(|e|format!("{e:?}"))?;
    let caps=lane.capabilities().unwrap();assert!(matches_live(&choice,&caps));let mut preferred=lu_core::ai_operations::recommended_read_tools(human,&caps);preferred.retain(|n|n!=name);preferred.insert(0,name.into());tauri::async_runtime::block_on(lane.run_journaled(&crate::workspace_desktop::workspace_model_prompt(choice.git,human,&preferred),&mut step,&cancel,&tool))
   }.map_err(|e|format!("native loop {e:?}"))?;
   let events=tool.events_for(&chat,&gid).unwrap();let values:Vec<serde_json::Value>=events.iter().filter(|e|e.kind==ModelAuditKind::ToolFeedback).filter_map(|e|serde_json::from_str(&e.payload).ok()).filter(|v: &serde_json::Value|v["name"]==name&&v["response"]["ok"]==true).collect();
   if outcome.tool_calls==0||outcome.host_routed_calls!=0||values.is_empty(){return Err("no verified feedback for selected model tool".into())}
   if exact_workspace_read(human)?.is_some(){
    let feedbacks=events.iter().filter(|e|e.kind==ModelAuditKind::ToolFeedback).count();
    if outcome.tool_calls!=1||feedbacks!=1||values.len()!=1{return Err("Exact read did not produce exactly one selected native result".into())}
   }
   let v=&values[0]["response"]["result"];let serialized=v.to_string();let sha=hash(&root.join("notes.txt"));
   let (facts,grounded)=match name{
    "lu_files_read_text"=>(v["content"].as_str()==Some(contents.as_str()),outcome.answer.contains(marker)),
    "lu_files_list_directory"=>(serialized.contains("orchid-root-31415.txt"),outcome.answer.contains("orchid-root-31415.txt")),
    "lu_files_stat"=>(v["kind"]=="file"&&v["size_bytes"]==contents.len(),outcome.answer.contains(&contents.len().to_string())),
    "lu_files_search_text"=>(serialized.contains("notes.txt")&&serialized.contains("audit-needle"),outcome.answer.contains("notes.txt")),
    "lu_files_discover"=>(serialized.contains("sub/needle-audit.txt"),outcome.answer.contains("needle-audit.txt")),
    "lu_reader_read"=>(serialized.contains(marker),outcome.answer.contains(marker)),
    "lu_archive_list"=>(serialized.contains("orchid-member.txt"),outcome.answer.contains("orchid-member.txt")),
    "lu_artifact_inspect"=>(v["sha256"]==sha&&v["bytes"]==contents.len(),outcome.answer.contains(&sha)),
    "lu_git_status"=>(serialized.contains("notes.txt"),outcome.answer.contains("notes.txt")),
    "lu_git_log"=>(serialized.contains("CP06A_COMMIT_7496"),outcome.answer.contains("CP06A_COMMIT_7496")),
    "lu_git_diff"=>(serialized.contains("CP06A_OLD_8462")&&serialized.contains("CP06A_NEW_8462"),outcome.answer.contains("CP06A_NEW_8462")),
    "lu_validate_schema"=>(v["valid"]==true,outcome.answer=="The supplied data is valid against the supplied schema."),
    "lu_check_env"=>{let expected=HostProgramInventory::new(evidence(true).program_specs).unwrap().check("git").unwrap();(v["program"]=="git"&&v["candidate_present"]==expected.candidate_present&&v["execution_verified"]==false,{let a=outcome.answer.to_ascii_lowercase();a.contains("git")&&a.contains("file")&&(a.contains("not proof of installation")||a.contains("does not prove installation")||a.contains("does not verify installation"))&&!a.contains("git is installed")&&!a.contains("git is available")})},
    _=>unreachable!()
   };
   let provenance=events.iter().filter(|e|e.kind==ModelAuditKind::EvidencePresentation).collect::<Vec<_>>();
   let final_events=events.iter().filter(|e|e.kind==ModelAuditKind::FinalAnswer).collect::<Vec<_>>();
   let origin=if name=="lu_validate_schema"{
    assert!(provenance.is_empty());assert_eq!(final_events.len(),1);"schema_model_verdict"
   }else{
    assert_eq!(provenance.len(),1,"missing or duplicate provenance");
    assert_eq!(values.len(),1);assert_eq!(outcome.tool_calls,1);
    let receipt:serde_json::Value=serde_json::from_str(&provenance[0].payload).unwrap();
    assert_eq!(receipt["tool"],name);assert_eq!(receipt["call_id"],values[0]["call_id"]);
    assert_eq!(receipt["feedback_sha256"],format!("{:x}",Sha256::digest(serde_json::to_vec(&values[0]).unwrap())));
    assert_eq!(receipt["answer"],outcome.answer);
    let mode=receipt["origin"].as_str().unwrap();
    if mode=="model_exact_reproduction"{assert_eq!(final_events.len(),1);assert_eq!(final_events[0].payload,outcome.answer);assert!(events.iter().any(|e|e.kind==ModelAuditKind::ModelOutput&&serde_json::from_str::<serde_json::Value>(&e.payload).ok()==Some(serde_json::json!({"kind":"final","text":outcome.answer}))));"model_exact_reproduction"}
    else{assert_eq!(mode,"host_evidence_fallback");assert!(final_events.is_empty());assert!(outcome.answer.starts_with("I couldn't verify my generated explanation."));assert!(events.iter().any(|e|e.kind==ModelAuditKind::Rejected&&e.payload=="FINAL_NOT_EXACTLY_GROUNDED_IN_CURRENT_NATIVE_EVIDENCE"));"host_evidence_fallback"}
   };
   // Stronger value and limitation checks, not just the old marker oracle.
   match name{
    "lu_files_list_directory"=>{for e in v["entries"].as_array().unwrap(){assert!(outcome.answer.contains(e["name"].as_str().unwrap()));}},
    "lu_files_search_text"=>{for e in v["hits"].as_array().unwrap(){assert!(outcome.answer.contains(e["path"].as_str().unwrap()));}if v["truncated"]==true{assert!(outcome.answer.contains("incomplete or truncated"));}assert!(!outcome.answer.contains("No other files were found"));},
    "lu_files_discover"=>assert!(outcome.answer.contains("sub/needle-audit.txt")&&outcome.answer.contains("does not verify file contents")),
    "lu_archive_list"=>{for e in v["entries"].as_array().unwrap(){assert!(outcome.answer.contains(e["path"].as_str().unwrap()));}assert!(outcome.answer.contains("integrity was not verified"));},
    "lu_artifact_inspect"=>assert!(outcome.answer.contains("factual accuracy were not verified")),
    "lu_git_log"=>assert!(outcome.answer.contains("bounded history")),
    "lu_git_diff"=>assert!(outcome.answer.contains("CP06A_OLD_8462")&&outcome.answer.contains("CP06A_NEW_8462")),
    "lu_check_env"=>assert!(outcome.answer.contains("not proof of installation")&&outcome.answer.contains("does not prove runtime execution")),
    _=>{},
   }
   println!("CP06C_PRESENTATION {name} origin={origin}");
   println!("CP06A_WITNESS {name} facts={facts} grounded={grounded} calls={} answer={:?}",outcome.tool_calls,outcome.answer);
   if !facts{return Err("feedback did not match independent fixture".into())}
   ticket.commit_completed(sid,||{chats.append_progress(&chat,&gid,&outcome.answer)?;chats.complete(&chat,&gid)})?;
   if !grounded{return Err("verified dispatch but final answer failed fixture grounding".into())}
   Ok(format!("NATIVE_FEEDBACK_AND_PRESENTATION_PASS {origin}"))
  })();
  for e in tool.events_for(&chat,&gid).unwrap(){println!("CP06A_TOOL {name} {:?} {}",e.kind,e.payload);}
  match result{Ok(status)=>println!("CP06A_CASE {name} {status}"),Err(e)=>{println!("CP06A_CASE {name} FAIL {e}");failures.push(format!("{name}: {e}"));if chats.require(&chat).unwrap().generation_terminal_kind(&chat,&gid).unwrap().is_none(){let _=chats.cancel(&chat,&gid);}}}
  drop(ticket);drop(audit);drop(tool);
  if supervisor.status().unwrap().session_id.as_deref()!=Some(sid){failures.push("worker no longer available, remaining cases unexecuted".into());break}
 }
 if supervisor.status().unwrap().session_id.as_deref()==Some(sid){supervisor.unload(sid,"cp06a-unload").unwrap();}
 println!("CP06A_RECEIPTS {}",base.display());assert!(failures.is_empty(),"CP06A genuine probe failures: {failures:?}");
}

#[test]
#[ignore="requires pinned real local llama-server/GGUF and fixture-host workspace approval"]
fn cp06cr1_genuine_multistep_synthesis(){
 use lu_core::{inference_supervisor::TrustedInferenceSupervisor,model_admission::ModelAdmissionManager,
  native_workspace_read_turn::WorkspaceReadTurnSession,
  persistent_llama_backend::{PersistentEngineConfig,PersistentLlamaBackend}};
 use sha2::{Digest,Sha256};
 use std::{io::Read,num::NonZeroUsize,path::PathBuf,time::Duration};
 fn hash(p:&Path)->String{let mut h=Sha256::new();let mut f=std::fs::File::open(p).unwrap();let mut b=[0;65536];loop{let n=f.read(&mut b).unwrap();if n==0{break}h.update(&b[..n]);}format!("{:x}",h.finalize())}
 fn from_hex(s:&str)->Vec<u8>{(0..s.len()).step_by(2).map(|i|u8::from_str_radix(&s[i..i+2],16).unwrap()).collect()}
 let exe=PathBuf::from(std::env::var("LU_CP05E_SERVER").unwrap());let model=PathBuf::from(std::env::var("LU_CP05E_GGUF").unwrap());
 let esha=std::env::var("LU_CP05E_SERVER_SHA256").unwrap();let msha=std::env::var("LU_CP05E_GGUF_SHA256").unwrap();
 assert_eq!(hash(&exe),esha);assert_eq!(hash(&model),msha);let pin:[u8;32]=from_hex(&esha).try_into().unwrap();
 let auth=std::env::var("LU_CP05E_API_KEY").unwrap();let mut manager=ModelAdmissionManager::new();let admitted=manager.admit(model.parent().unwrap(),model.file_name().unwrap().to_str().unwrap()).unwrap().clone();
 let supervisor=TrustedInferenceSupervisor::new("llama-cpu".into(),8192,vec![("model-1".into(),admitted)],move||PersistentLlamaBackend::new(PersistentEngineConfig{executable:exe.clone(),expected_sha256:pin,threads:NonZeroUsize::new(3).unwrap(),context_tokens:8192,startup_timeout:Duration::from_secs(120),request_timeout:Duration::from_secs(120),api_key:auth.clone(),load_observer:None})).unwrap();
 let sid="cp06cr1-synthesis";supervisor.load("model-1",sid.into(),"cp06cr1-load").unwrap();assert!(supervisor.status().unwrap().pid.is_some());
 let base=std::env::temp_dir().join(next_id("cp06cr1-native-"));std::fs::create_dir(&base).unwrap();let root=base.join("approved");std::fs::create_dir(&root).unwrap();
 std::fs::write(root.join("A.txt"),b"123456789").unwrap();std::fs::write(root.join("B.txt"),b"123").unwrap();
 let chats=ChatStore::open(&base.join("chats")).unwrap();let queue=Arc::new(generation_queue::GenerationQueue::default());
 let human="Compare the byte sizes of A.txt and B.txt in my approved folder. Use lu_files_stat on A.txt, then lu_files_stat on B.txt, and compare their observed sizes using compare_bytes with both evidence sources.";
 let chat=chats.create_chat().unwrap().chat_id;let gid=next_id("g-");let verifier=PersistedHumanTurn::current_chat(&chats,&chat,&gid,human);
 let (ticket,_)=queue.enqueue_with_class_committing(sid,&chat,&gid,TurnClass::Foreground,||chats.append_user_with_turn_id(&chat,&gid,human)).unwrap();ticket.wait_for_slot().unwrap();ticket.verify_active_foreground(sid,&chat,&gid).unwrap();
 let journal=ModelTurnJournal::open(&base.join("tool.sqlite")).unwrap();let mut lane=WorkspaceReadTurnSession::new(&verifier,&chat,&gid,&gid,&root).unwrap();lane.require_grounded_synthesis().unwrap();
 let preferred=lu_core::ai_operations::recommended_read_tools(human,&lane.capabilities().unwrap());assert_eq!(preferred.first().map(String::as_str),Some("lu_files_stat"));let prompt=crate::workspace_desktop::workspace_model_prompt(false,human,&preferred);
 let mut model=SupervisorModelStep::new(&supervisor,sid.into(),next_id("cp06cr1" )).unwrap();let outcome=tauri::async_runtime::block_on(lane.run_journaled(&prompt,&mut model,&Cancellation::default(),&journal));
 let rows=journal.events_for(&chat,&gid).unwrap();for e in &rows{println!("CP06CR1 {:?} {}",e.kind,e.payload);}
 println!("CP06CR1_RECEIPTS {}",base.display());
 let mut passed=false;if let Ok(ref result)=outcome{
  ticket.commit_completed(sid,||{chats.append_progress(&chat,&gid,&result.answer)?;chats.complete(&chat,&gid)}).unwrap();
  let p=rows.iter().find(|e|e.kind==ModelAuditKind::EvidencePresentation).map(|e|serde_json::from_str::<serde_json::Value>(&e.payload).unwrap());
  if let Some(p)=p{let stats=rows.iter().filter(|e|e.kind==ModelAuditKind::ToolFeedback).filter_map(|e|serde_json::from_str::<serde_json::Value>(&e.payload).ok()).filter(|e|e["name"]=="lu_files_stat"&&e["response"]["ok"]==true).collect::<Vec<_>>();
   passed=stats.len()==2&&stats.iter().any(|v|v["response"]["result"]["size_bytes"]==9)&&stats.iter().any(|v|v["response"]["result"]["size_bytes"]==3)&&p["origin"]=="model_selected_claims_host_rendered"&&result.answer.contains("absolute difference: 6 bytes")&&result.answer.contains("A.txt")&&result.answer.contains("B.txt");
  }
 }else{let _=chats.cancel(&chat,&gid);}
 drop(ticket);drop(journal);drop(lane);if supervisor.status().unwrap().session_id.as_deref()==Some(sid){supervisor.unload(sid,"cp06cr1-unload").unwrap();}
 println!("CP06CR1_RESULT {passed} {outcome:?}");assert!(passed,"genuine native synthesis did not complete the two-source comparison");
}
