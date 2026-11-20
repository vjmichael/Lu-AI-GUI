//! CORE-MERGE-C2: one normal Send -> original TOOLS-05 or TOOLS-06.
 //! Only explicit, CURRENT human requests can enter this adapter; neither
 //! injected documents nor recalled memory nor model text can select a tool.
 //! CoreUtilityTurn owns the same NativeToolTurn, catalog, and authority used
 //! by the existing supervised native tool loops. No new execution runtime.
use super::*;
use lu_core::{
    conversation_authority::TrustedUserTurnVerifier,
    inference_host::Cancellation,
    model_json_boundary::{parse_exact_boundary_object,parse_host_tool_feedback_boundary,MAX_BOUNDARY_JSON_BYTES},
    model_turn_journal::{ModelAuditKind,ModelTurnJournal},
    native_core_utility_turn::{CoreUtilityRequest,CoreUtilityTurn},
    supervised_tool_generation::SupervisorModelStep,
};
use std::sync::{Arc,Mutex};


use crate::{turn_authority::PersistedHumanTurn,causal_diagnostic::NativeDiagnosticTrace};
/// Deliberately small task classifier, not a generative planner. A positive
/// result selects ONE read-only tool and one exact source value, not a grant.
pub(crate) fn explicit_core_utility_request(text:&str)->Option<CoreUtilityRequest>{
    parse_core_utility_request(text,false)
}
/// General selection can propose a natural question, but never supply the
/// program ID, schema, or data. Reconstruct the bounded input from human bytes.
pub(crate) fn negotiable_core_utility_request(text:&str)->Option<CoreUtilityRequest>{
    parse_core_utility_request(text,true)
}
pub(crate) fn utility_native_id(request:&CoreUtilityRequest)->&'static str{
    match request{CoreUtilityRequest::Validate{..}=>"lu.validate.schema",CoreUtilityRequest::CheckProgram{..}=>"lu.exec.check_env"}
}
fn parse_core_utility_request(text:&str,negotiated:bool)->Option<CoreUtilityRequest>{
    let trimmed=text.trim();
    if trimmed.is_empty()||trimmed.len()>8192 ||
        trimmed.contains("<|") || trimmed.contains("[Attached text file:") ||
        trimmed.contains("[/Attached text file]"){return None}
    let lower=trimmed.to_ascii_lowercase();
    let forbidden=[
        "how to ","how do i ","explain ","do not ","don't ","without tools",
        "ignore previous","ignore all","shell ","execute ","run command",
        "download ","upload ","install ","uninstall ","delete ","modify ",
        "replace ","edit ","write ","grant ","approve ",
    ];
    if forbidden.iter().any(|word|lower.contains(word)){return None}
    // A schema/data request has to contain a complete, strict, literal JSON
    // object in the current human text. This stops a model from inventing
    // missing schema or data and calling its own example an observed check.
    if let Some(brace)=trimmed.find('{'){
        let heading=lower[..brace].trim();
        if (heading.starts_with("validate ")||heading.starts_with("check ")||
            (negotiated&&(heading.starts_with("could you validate ")||heading.starts_with("can you check ")||heading.starts_with("does ")))) &&
            heading.contains("json") && heading.contains("schema"){
            let object=parse_exact_boundary_object(&trimmed[brace..],8192).ok()?;
            if object.len()!=2{return None}
            let schema=object.get("schema")?.clone();
            let data=object.get("data")?.clone();
            if !(schema.is_object()||schema.is_boolean()){return None}
            return Some(CoreUtilityRequest::Validate{schema,data})
        }
    }
    // Environment checks are only for the exact single named program in the
    // already reviewed HOST candidate set; no dynamic PATH or package search.
    let localized=[
        "on this computer","on my computer","on my system","on this system",
        "locally","installed here","available here",
    ].iter().any(|term|lower.contains(term));
    let action=lower.starts_with("check ")||lower.starts_with("verify ")||
        lower.starts_with("is ")||(negotiated&&["could you tell me whether ","can you tell me if ",
            "could you check whether ","can you check if ","do i have "].iter().any(|p|lower.starts_with(p)));
    let presence=["installed","available","present"].iter()
        .any(|term|lower.contains(term));
    if !localized||!action||!presence{return None}
    if negotiated&&(trimmed.contains(['/','\\',':'])){return None}

    let words=lower.split(|c:char|!c.is_ascii_alphanumeric())
        .filter(|s|!s.is_empty()).collect::<Vec<_>>();
    let matched=crate::environment_context::fixed_ambient_program_candidates()
        .into_iter().filter(|spec|words.contains(&spec.id.as_str()))
        .collect::<Vec<_>>();
    if matched.len()!=1{return None}
    let spec=matched.into_iter().next()?;
    Some(CoreUtilityRequest::CheckProgram{program:spec.id.clone(),spec})
}
pub(crate) async fn ask_utility_for_chat(
    app:AppHandle,state:Arc<DesktopState>,session_id:String,
    chat_id:String,prompt:String,request:CoreUtilityRequest,diagnostic:Option<(String,std::time::Instant)>,
)->Result<String,String>{
    let trace=NativeDiagnosticTrace::correlated(&state,diagnostic);
    // Recompute from the SAME original human bytes. No frontend-selected
    // utility ID or privately constructed request is authoritative.
    let canonical=explicit_core_utility_request(&prompt)
        .ok_or("No current explicit human utility request")?;
    let same=match (&canonical,&request){
        (CoreUtilityRequest::Validate{schema:a,data:b},
         CoreUtilityRequest::Validate{schema:c,data:d})=>a==c&&b==d,
        (CoreUtilityRequest::CheckProgram{program:a,..},
         CoreUtilityRequest::CheckProgram{program:b,..})=>a==b,
        _=>false,
    };
    if !same{return Err("Utility request differs from original human text".into())}
    state.chats.history_window(&chat_id,None,"older",1)?;
    let initial=state.supervisor.status()
        .map_err(|_|"Core utility model status unavailable")?;
    if initial.session_id.as_deref()!=Some(session_id.as_str()){
        return Err("Core utility model session changed".into())
    }
    trace.point(CausalCheckpoint::SessionReady);
    let gid=next_id("g-");
    let (ticket,position)=state.generation_queue
        .enqueue_with_class_committing(&session_id,&chat_id,&gid,TurnClass::Foreground,
            ||state.chats.append_user_with_turn_id(&chat_id,&gid,&prompt))?;
    trace.point(CausalCheckpoint::UserSaved);
    let cancellation=Cancellation::default();
    let current_step=Arc::new(Mutex::new(None));
    state.reader_generations.lock()
        .map_err(|_|"Utility cancellation registry unavailable")?
        .insert(gid.clone(),ReaderRunControl{
            cancellation:cancellation.clone(),
            current_step:Arc::clone(&current_step),
            session_id:session_id.clone(),
        });
    let _=app.emit("lu-generation-queued",GenerationEvent{
        chat_id:chat_id.clone(),generation_id:gid.clone(),
        phase:"queued",position,
    });
    let host=Arc::clone(&state);
    let event_app=app.clone();
    let outcome=tauri::async_runtime::spawn_blocking(move||{
        let work=(||->Result<String,String>{
            ticket.wait_for_slot()?;
            trace.point(CausalCheckpoint::WorkerEntered);
            if ticket.cancelled()||cancellation.is_cancelled(){
                return Err("Cancelled utility turn".into())
            }
            let current=host.supervisor.status()
                .map_err(|_|"Core utility model unavailable")?;
            if current.session_id.as_deref()!=Some(session_id.as_str()) ||
                current.model_ref!=initial.model_ref{
                return Err("Core utility model changed while queued".into())
            }
            execute_approved_utility(&event_app,&host,session_id,chat_id.clone(),gid.clone(),prompt,canonical,&ticket,&cancellation,&current_step,None,&trace)
        })();
        if work.is_err(){trace.incomplete(ticket.cancelled()||cancellation.is_cancelled());}
        if let Err(error)=&work{
            if error.contains("Cancelled"){
                let _=host.chats.cancel(&chat_id,&gid);
            }else{
                let _=host.chats.append_progress(&chat_id,&gid,
                    "Native utility turn could not be verified. No unapproved \
                     operation was performed or automatically retried.");
            }
        }
        let phase=match &work{
            Ok(_)=>"completed",
            Err(s) if s.contains("Cancelled")=>"cancelled",
            Err(_)=>"failed",
        };
        let _=event_app.emit("lu-generation-finish",GenerationEvent{
            chat_id:chat_id.clone(),generation_id:gid.clone(),phase,position:0
        });
        let _=host.reader_generations.lock().map(|mut rows|rows.remove(&gid));
        work
    }).await.map_err(|_|"Core utility model worker unavailable")?;
    outcome
}

/// Recompute the host-owned input after separate native consent. No second
/// human message, queue ticket, arbitrary PATH, or model-supplied arguments.
pub(crate) fn execute_negotiated_utility(app:&AppHandle,host:&Arc<DesktopState>,session:&str,chat:&str,gid:&str,prompt:&str,
    ticket:&generation_queue::Ticket,cancellation:&Cancellation,current_step:&Arc<Mutex<Option<String>>>,choice:&crate::capability_negotiation::ReadCandidate)->Result<String,String>{
    let trace=NativeDiagnosticTrace::new(host);
    let request=negotiable_core_utility_request(prompt).ok_or("No exact original utility input")?;
    if utility_native_id(&request)!=choice.native_tool_id{return Err("Selected utility differs from original human input".into())}
    let result=execute_approved_utility(app,host,session.into(),chat.into(),gid.into(),prompt.into(),request,ticket,cancellation,current_step,Some(choice),&trace);
    if result.is_err(){trace.incomplete(ticket.cancelled()||cancellation.is_cancelled());}result
}
#[allow(clippy::too_many_arguments)]
fn execute_approved_utility(event_app:&AppHandle,host:&Arc<DesktopState>,session_id:String,chat_id:String,gid:String,prompt:String,
    canonical:CoreUtilityRequest,ticket:&generation_queue::Ticket,cancellation:&Cancellation,current_step:&Arc<Mutex<Option<String>>>,selected:Option<&crate::capability_negotiation::ReadCandidate>,trace:&NativeDiagnosticTrace)->Result<String,String>{
            if ticket.cancelled()||cancellation.is_cancelled(){return Err("Cancelled utility turn".into())}
            let current=host.supervisor.status().map_err(|_|"Core utility model unavailable")?;
            if current.session_id.as_deref()!=Some(session_id.as_str()){return Err("Core utility model changed".into())}
            let verifier=PersistedHumanTurn::current_chat(&host.chats,&chat_id,&gid,&prompt);
            if !verifier.verify_user_turn(&chat_id,&gid){
                return Err("Core utility original current turn no longer authenticated".into())
            }
            if selected.is_some(){
                trace.point(CausalCheckpoint::SessionReady);trace.point(CausalCheckpoint::UserSaved);
                if ticket.verify_active_foreground(&session_id,&chat_id,&gid).is_ok(){trace.point(CausalCheckpoint::WorkerEntered)}else{trace.lost()}
            }
            trace.point(CausalCheckpoint::SavedAttested);
            // Grants exist only after a saved exact human turn and expire
            // automatically at end of the one supervised model/tool session.
            let mut turn=CoreUtilityTurn::new(
                &verifier,&chat_id,&gid,&gid,canonical
            )?;
            let capabilities=turn.capabilities()?;
            if capabilities.len()!=1 || capabilities[0].effect!="read_workspace"{
                return Err("Core utility advertised unexpected capabilities".into())
            }
            if let Some(choice)=selected{
                if !crate::capability_negotiation::matches_live(choice,&capabilities){return Err("Selected utility is not in the live approved registry".into())}
            }
            let label=capabilities[0].public_name.as_str();
            if !matches!(label,"lu_validate_schema"|"lu_check_env"){
                return Err("Core utility model schema was widened".into())
            }
            trace.point(CausalCheckpoint::TaskAuthorized);
            let model_prompt=utility_model_prompt(label,&prompt);
            let audit=ModelTurnJournal::open(
                &host.catalog_path.with_file_name("lu-core-utility-model-audit.sqlite")
            ).map_err(|_|"Core utility model audit unavailable")?;
            let progress=crate::native_turn_lifecycle::NativeTurnProgress::new(
                &event_app,&chat_id,&gid,&current_step);
            let observer=progress.model_observer();
            let mut model=SupervisorModelStep::new(
                &host.supervisor,session_id.clone(),gid.clone()
            ).map_err(|e|format!("Core utility model bridge unavailable: {e:?}"))?
                .with_step_start_observer(observer);
            if selected.is_none(){let _=event_app.emit("lu-generation-start",GenerationEvent{
                chat_id:chat_id.clone(),generation_id:gid.clone(),
                phase:"running",position:0,
            });}
            let progress_observer=progress.dispatch_observer();
            let dispatch=|point|{progress_observer(point);trace.record(|t|t.tool_checkpoint(point));};
            trace.point(CausalCheckpoint::ModelEntered);
            let result=tauri::async_runtime::block_on(
                turn.run_journaled_observed(&model_prompt,&mut model,
                    &cancellation,&audit,&dispatch)
            ).map_err(|e|trace.model_error(&e).to_owned())?;
            trace.point(CausalCheckpoint::ModelReturned);
            if cancellation.is_cancelled()||ticket.cancelled(){
                return Err("Cancelled utility turn after model feedback".into())
            }
            if result.tool_calls==0{
                return Err("Model did not use the exact approved utility".into())
            }
            ticket.commit_completed(&session_id,||{
                if !verifier.verify_user_turn(&chat_id,&gid){return Err("Utility original request changed".into())}
                host.chats.append_progress(&chat_id,&gid,&result.answer)?;
                host.chats.complete(&chat_id,&gid)
            })?;
            trace.saved_response(&host,&chat_id,&gid,&result.answer,false);
            Ok(result.answer)
}

/// C1→C3: carry the ORIGINAL native utility's bounded feedback into a later
/// ordinary GGUF chat turn. A model final answer is not a source observation.
/// This is historic read-only DATA, not a current probe or reusable grant.
pub(crate) fn prior_utility_observation_for_human(
    chats:&ChatStore,profile:&std::path::Path,chat:&str,turn:&str,text:&str,
)->Result<Option<String>,String>{
    if chats.latest_authenticated_user_turn_id(chat).as_deref()!=Some(turn)
        ||chats.trusted_user_turn_text(chat,turn).as_deref()!=Some(text){
        return Err("Original current human utility-context turn unavailable".into())
    }
    let file=profile.join("lu-core-utility-model-audit.sqlite");
    let meta=match std::fs::symlink_metadata(&file){
        Ok(meta)=>meta,
        Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Ok(None),
        Err(_)=>return Err("Historical utility journal unavailable".into()),
    };
    if !meta.is_file()||meta.file_type().is_symlink()||meta.len()>32*1024*1024{
        return Err("Historical utility journal identity or bound invalid".into())
    }
    // The trusted host chooses the existing profile-private journal, never
    // a path, task or claimed effect supplied by a model or retrieved text.
    let mut rows=Vec::new();
    for past in chats.prior_authenticated_turn_ids(chat,turn,12)?{
        let original=chats.trusted_user_turn_text(chat,&past)
            .ok_or("Original utility turn lost its human identity")?;
        let Some(request)=explicit_core_utility_request(&original)else{continue};
        let (name,expected_args)=match request{
            CoreUtilityRequest::Validate{schema,data}=>(
                "lu_validate_schema",serde_json::json!({"schema":schema,"data":data})
            ),
            CoreUtilityRequest::CheckProgram{program,..}=>(
                "lu_check_env",serde_json::json!({"program":program})
            ),
        };
        let events=ModelTurnJournal::read_events_read_only(&file,chat,&past,64)
            .map_err(|_|"Historical native utility audit cannot be reopened")?;
        let mut tool_reply=None;
        let mut tool_request=None;
        let mut final_count=0usize;
        let mut human_request_count=0usize;
        let mut human_request_seq=0i64;
        let mut current_reference_prompt=false;
        let mut suspicious=false;
        let mut request_seq=0i64;
        let mut feedback_seq=0i64;
        let mut final_seq=0i64;
        for event in &events{
            match event.kind{
                ModelAuditKind::UserRequest=>{
                    human_request_count+=1;
                    human_request_seq=event.sequence;
                    // A reference-style historic reply must be tied to the
                    // exact host-built prompt of the saved original human turn.
                    current_reference_prompt=event.payload==utility_model_prompt(name,&original);
                }
                ModelAuditKind::ModelOutput=>{
                    match serde_json::from_str::<serde_json::Value>(&event.payload){
                        Ok(value) if value["kind"]=="tool"=>{
                            if tool_request.is_some(){suspicious=true}
                            request_seq=event.sequence;
                            tool_request=Some(value);
                        }
                        Ok(value) if value["kind"]=="final"=>{},
                        _=>suspicious=true,
                    }
                }
                ModelAuditKind::ToolFeedback=>{
                    if tool_reply.is_some(){suspicious=true}
                    feedback_seq=event.sequence;
                    tool_reply=parse_host_tool_feedback_boundary(
                        &event.payload,MAX_BOUNDARY_JSON_BYTES
                    ).ok().map(serde_json::Value::Object);
                    if tool_reply.is_none(){suspicious=true}
                }
                ModelAuditKind::FinalAnswer=>{
                    final_count+=1;
                    final_seq=event.sequence;
                }
                ModelAuditKind::Rejected|ModelAuditKind::GenerationFailure
                    |ModelAuditKind::HostRouteSelected=>suspicious=true,
                ModelAuditKind::EvidencePresentation=>{
                    if serde_json::from_str::<serde_json::Value>(&event.payload).ok().is_none_or(|v|v["origin"]!="model_exact_reproduction"){suspicious=true;}
                },
                _=>{}
            }
        }
        let matching=tool_request.as_ref().is_some_and(|value|
            value["kind"]=="tool"&&value["name"]==name&&
            (value["arguments"]==expected_args || (current_reference_prompt &&
                value["arguments"]==serde_json::json!({"source":"current_request"}))) &&
            value.as_object().is_some_and(|o|o.len()==3)
        );
        let observed=tool_reply.as_ref().and_then(|value|{
            if value["name"]!=name || value["kind"]!="tool_result"
                ||value["response"]["tool"]!=name
                ||value["response"]["ok"]!=true{return None}
            let result=&value["response"]["result"];
            if name=="lu_validate_schema"{
                if result["draft"]!="2020-12" || !result["valid"].is_boolean()
                    ||!result["issues"].is_array()
                    ||!result["truncated"].is_boolean()
                    ||(result["valid"]==true && result["issues"]
                        .as_array().is_some_and(|a|!a.is_empty())){
                    return None
                }
                Some(serde_json::json!({
                    "validAtObservation":result["valid"],
                    "draft":"2020-12"
                }))
            }else{
                if result["program"]!=expected_args["program"]
                    ||result["execution_verified"]!=false
                    ||!result["candidate_present"].is_boolean()
                    ||!result["version"].is_null()
                    ||!result["os"].is_string()
                    ||!result["arch"].is_string()
                    ||!matches!(result["evidence"].as_str(),
                        Some("file_present_only"|"not_found"))
                    ||(result["candidate_present"]==true
                        &&result["evidence"]!="file_present_only")
                    ||(result["candidate_present"]==false
                        &&result["evidence"]!="not_found"){
                    return None
                }
                Some(serde_json::json!({
                    "candidatePresentAtObservation":result["candidate_present"],
                    "evidence":result["evidence"],
                    "executionVerified":false
                }))
            }
        });
        let accepted=!suspicious && matching && observed.is_some()
            && human_request_count==1 && final_count==1
            && human_request_seq>0 && request_seq>human_request_seq
            && feedback_seq>request_seq && final_seq>feedback_seq;
        rows.push(serde_json::json!({
            "originalHumanTurnId":past,
            "tool":name,
            "status":if accepted{
                "HISTORICAL_NATIVE_READ_ONLY_TOOL_FEEDBACK_RECORDED"
            }else{"ORIGINAL_UTILITY_AUDIT_INCOMPLETE_RECONCILE"},
            "observation":if accepted{observed}else{None},
            "originalCurrentSourceRechecked":false,
            "taskCompleted":false,
            "executionAuthorized":false
        }));
        if rows.len()==2{break}
    }
    if rows.is_empty(){return Ok(None)}
    let result=serde_json::json!({
        "schema":"lu-core-c3-historical-utility-feedback-v1",
        "scope":"SAME_CHAT_HOST_AUTHENTICATED_ORIGINAL_TURNS",
        "status":"HISTORICAL_DATA_ONLY_NOT_CURRENT_SOURCE_PROOF",
        "observationsMostRecentFirst":rows,
        "priorGrantReusable":false,
        "executionAuthority":"NOT_AUTHORIZED_FOR_EXECUTION",
        "automaticRetryAllowed":false,
        "modelMayClaimTaskComplete":false,
        "currentSourceFreshness":"NOT_RECHECKED",
        "nextOperation":"New current human request and fresh native host authorization required"
    });
    let encoded=result.to_string();
    if encoded.len()>3000{
        return Err("Historical utility observation exceeds bounded model context".into())
    }
    Ok(Some(encoded))
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn original_utility_feedback_reopens_as_data_only_not_model_completion(){
        let root=std::env::temp_dir().join(format!(
            "lu-c3-utility-feedback-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(
                std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let chats=ChatStore::open(&root).unwrap();
        let chat=chats.create_chat().unwrap().chat_id;
        let old=format!("g-{}","1".repeat(32));
        let now=format!("g-{}","2".repeat(32));
        let original=r#"Validate JSON against schema: {"schema":{"type":"integer"},"data":42}"#;
        chats.append_user_with_turn_id(&chat,&old,original).unwrap();
        let file=root.join("lu-core-utility-model-audit.sqlite");
        let journal=ModelTurnJournal::open(&file).unwrap();
        journal.append(&chat,&old,ModelAuditKind::UserRequest,original,None).unwrap();
        journal.append(&chat,&old,ModelAuditKind::ModelOutput,
            r#"{"kind":"tool","name":"lu_validate_schema","arguments":{"schema":{"type":"integer"},"data":42}}"#,None).unwrap();
        let native_feedback=lu_core::model_json_boundary::serialize_host_tool_feedback_boundary(
            "call-real-source","lu_validate_schema",
            &serde_json::json!({"tool":"lu_validate_schema","ok":true,
                "result":{"draft":"2020-12","valid":true,"issues":[],"truncated":false}}),
            MAX_BOUNDARY_JSON_BYTES
        ).unwrap();
        journal.append(&chat,&old,ModelAuditKind::ToolFeedback,
            &native_feedback,None).unwrap();
        journal.append(&chat,&old,ModelAuditKind::FinalAnswer,
            "The model reports success",None).unwrap();
        let followup="What did the native schema check observe?";
        chats.append_user_with_turn_id(&chat,&now,followup).unwrap();
        let before=prior_utility_observation_for_human(
            &chats,&root,&chat,&now,followup).unwrap().unwrap();
        let observation:serde_json::Value=serde_json::from_str(&before).unwrap();
        let row=&observation["observationsMostRecentFirst"][0];
        assert_eq!(row["status"],"HISTORICAL_NATIVE_READ_ONLY_TOOL_FEEDBACK_RECORDED");
        assert_eq!(row["observation"]["validAtObservation"],true);
        assert_eq!(row["taskCompleted"],false);
        assert_eq!(observation["priorGrantReusable"],false);
        assert_eq!(observation["currentSourceFreshness"],"NOT_RECHECKED");
        assert!(!before.contains("The model reports success"));
        assert!(!before.contains("model reports"));
        assert!(!before.contains("Validate JSON against"));
        assert!(prior_utility_observation_for_human(
            &chats,&root,&chat,&old,original).is_err(),
            "old human turn cannot impersonate current authorization");
        drop(journal);drop(chats);
        let reopened=ChatStore::open(&root).unwrap();
        assert_eq!(prior_utility_observation_for_human(
            &reopened,&root,&chat,&now,followup).unwrap().unwrap(),before);
        let sql=rusqlite::Connection::open(&file).unwrap();
        // New model references are only historic evidence if the saved host
        // prompt is exactly the one produced for the authenticated turn.
        sql.execute("UPDATE lu_model_boundary_audit SET payload=?1 WHERE kind='USER_REQUEST'",
            [utility_model_prompt("lu_validate_schema",original)]).unwrap();
        sql.execute("UPDATE lu_model_boundary_audit SET payload=?1 WHERE kind='MODEL_OUTPUT'",
            [r#"{"kind":"tool","name":"lu_validate_schema","arguments":{"source":"current_request"}}"#]).unwrap();
        assert_eq!(prior_utility_observation_for_human(&reopened,&root,&chat,&now,followup).unwrap().unwrap(),before);
        sql.execute("UPDATE lu_model_boundary_audit SET payload=?1 WHERE kind='USER_REQUEST'",
            [utility_model_prompt("lu_validate_schema","unrelated human request")]).unwrap();
        assert!(!prior_utility_observation_for_human(&reopened,&root,&chat,&now,followup).unwrap().unwrap().contains("validAtObservation"));
        sql.execute("UPDATE lu_model_boundary_audit
                     SET payload=?1 WHERE kind='MODEL_OUTPUT'",
            [r#"{"kind":"tool","name":"lu_validate_schema","arguments":{"schema":{"type":"string"},"data":42}}"#]).unwrap();
        drop(sql);
        let damaged=prior_utility_observation_for_human(
            &reopened,&root,&chat,&now,followup).unwrap().unwrap();
        assert_eq!(serde_json::from_str::<serde_json::Value>(&damaged).unwrap()
            ["observationsMostRecentFirst"][0]["status"],
            "ORIGINAL_UTILITY_AUDIT_INCOMPLETE_RECONCILE");
        assert!(!damaged.contains("validAtObservation"));
        drop(reopened);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn original_negative_program_observation_is_historical_not_installed_proof(){
        if !(cfg!(windows)||cfg!(target_os="linux")){return}
        let root=std::env::temp_dir().join(format!(
            "lu-c3-negative-env-{}-{}",
            std::process::id(),std::time::SystemTime::now().duration_since(
                std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let chats=ChatStore::open(&root).unwrap();
        let chat=chats.create_chat().unwrap().chat_id;
        let old=format!("g-{}","3".repeat(32));
        let now=format!("g-{}","4".repeat(32));
        let original="Check if python is installed on my computer";
        assert!(explicit_core_utility_request(original).is_some());
        chats.append_user_with_turn_id(&chat,&old,original).unwrap();
        let file=root.join("lu-core-utility-model-audit.sqlite");
        let journal=ModelTurnJournal::open(&file).unwrap();
        journal.append(&chat,&old,ModelAuditKind::UserRequest,original,None).unwrap();
        journal.append(&chat,&old,ModelAuditKind::ModelOutput,
            r#"{"kind":"tool","name":"lu_check_env","arguments":{"program":"python"}}"#,None).unwrap();
        let response=lu_core::model_json_boundary::serialize_host_tool_feedback_boundary(
            "call-local","lu_check_env",
            &serde_json::json!({
                "tool":"lu_check_env","ok":true,
                "result":{"program":"python","candidate_present":false,
                    "evidence":"not_found","os":std::env::consts::OS,
                    "arch":std::env::consts::ARCH,"version":null,
                    "execution_verified":false}
            }),MAX_BOUNDARY_JSON_BYTES
        ).unwrap();
        journal.append(&chat,&old,ModelAuditKind::ToolFeedback,&response,None).unwrap();
        journal.append(&chat,&old,ModelAuditKind::FinalAnswer,
            "The model says Python was installed",None).unwrap();
        let prompt="What happened to the native environment check?";
        chats.append_user_with_turn_id(&chat,&now,prompt).unwrap();
        let text=prior_utility_observation_for_human(&chats,&root,&chat,&now,prompt)
            .unwrap().unwrap();
        let value:serde_json::Value=serde_json::from_str(&text).unwrap();
        let row=&value["observationsMostRecentFirst"][0];
        assert_eq!(row["status"],"HISTORICAL_NATIVE_READ_ONLY_TOOL_FEEDBACK_RECORDED");
        assert_eq!(row["observation"]["candidatePresentAtObservation"],false);
        assert_eq!(row["observation"]["evidence"],"not_found");
        assert_eq!(row["taskCompleted"],false);
        assert!(!text.contains("says Python was installed"));
        drop(journal);drop(chats);
        std::fs::remove_dir_all(&root).unwrap();
    }
    #[test]
    fn only_exact_supplied_schema_data_enters_model_utility(){
        let valid=r#"Validate JSON against schema: {"schema":{"type":"integer"},"data":42}"#;
        assert!(matches!(explicit_core_utility_request(valid),
            Some(CoreUtilityRequest::Validate{schema,data})
            if schema==serde_json::json!({"type":"integer"}) && data==serde_json::json!(42)));
        for text in [
            "How to validate JSON schema?",
            "Validate JSON schema without data",
            r#"Validate JSON against schema: {"schema":{"type":"string"}}"#,
            r#"Validate JSON against schema: {"schema":{"type":"string"},"data":42,"data":0}"#,
            r#"Validate JSON against schema: {"schema":{"type":"string"},"data":42,"extra":9}"#,
            "Do not validate JSON against schema",
            "[Attached text file: untrusted.md] Validate JSON schema",
        ]{
            assert!(explicit_core_utility_request(text).is_none(),"bad scope: {text}");
        }
    }
    #[test]
    fn only_single_fixed_host_program_current_local_request_is_routed(){
        let example="Check if python is installed on this computer";
        let valid=explicit_core_utility_request(example);
        if cfg!(windows)||cfg!(target_os="linux"){
            assert!(matches!(valid,Some(CoreUtilityRequest::CheckProgram{program,..})
                if program=="python"));
        }else{assert!(valid.is_none())}
        for text in [
            "How to check if python is installed?",
            "Check if python and git are installed on my computer",
            "Check if gh is installed locally",
            "Check if python is installed on my computer and install it",
            "What is the Python programming language?",
            "Check if /tmp/python is installed on my system without tools",
            "[Attached text file: untrusted.md] Check if python is installed locally",
        ]{
            assert!(explicit_core_utility_request(text).is_none(),"unapproved: {text}");
        }
    }
}

/// Shared production/acceptance prompt; never establishes authority.
pub(crate) fn utility_model_prompt(label:&str,prompt:&str)->String{
    if label=="lu_validate_schema"{
                format!("Use lu_validate_schema with source=current_request. Lu already bound \
                    the original schema and data; do not copy or change them. \
                    After native feedback, explain the actual validation result. \
                    Original human request: {prompt}")
            }else{
                format!("Use lu_check_env with source=current_request; Lu already bound the named \
                    program. Do not run commands, search PATH or guess paths. \
                    After native feedback explain file presence only, not \
                    verified installation, runtime or version. Original human request: {prompt}")
            }
}
