//! CORE-MERGE-C2: one normal Send -> original TOOLS-05 or TOOLS-06.
 //! Only explicit, CURRENT human requests can enter this adapter; neither
 //! injected documents nor recalled memory nor model text can select a tool.
 //! CoreUtilityTurn owns the same NativeToolTurn, catalog, and authority used
 //! by the existing supervised native tool loops. No new execution runtime.
use super::*;
use lu_core::{
    agent_tool_loop::ToolDispatchCheckpoint,
    conversation_authority::TrustedUserTurnVerifier,
    inference_host::Cancellation,
    model_json_boundary::parse_exact_boundary_object,
    model_turn_journal::ModelTurnJournal,
    native_core_utility_turn::{CoreUtilityRequest,CoreUtilityTurn},
    supervised_tool_generation::SupervisorModelStep,
};
use std::sync::{Arc,Mutex};

#[derive(Clone,serde::Serialize)]
#[serde(rename_all="camelCase")]
struct CoreUtilityProgress{chat_id:String,generation_id:String,phase:String}

struct OriginalUtilityHumanTurn<'a>{
    chats:&'a ChatStore,
    chat:&'a str,
    turn:&'a str,
    text:&'a str,
}
impl TrustedUserTurnVerifier for OriginalUtilityHumanTurn<'_>{
    fn verify_user_turn(&self,chat:&str,turn:&str)->bool{
        chat==self.chat && turn==self.turn &&
        self.chats.trusted_user_turn_text(chat,turn).as_deref()==Some(self.text) &&
        self.chats.latest_authenticated_user_turn_id(chat).as_deref()==Some(turn)
    }
}

/// Deliberately small task classifier, not a generative planner. A positive
/// result selects ONE read-only tool and one exact source value, not a grant.
pub(crate) fn explicit_core_utility_request(text:&str)->Option<CoreUtilityRequest>{
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
        if (heading.starts_with("validate ")||heading.starts_with("check ")) &&
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
        lower.starts_with("is ");
    let presence=["installed","available","present"].iter()
        .any(|term|lower.contains(term));
    if !localized||!action||!presence{return None}
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
    chat_id:String,prompt:String,request:CoreUtilityRequest,
)->Result<String,String>{
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
    let gid=next_id("g-");
    let (ticket,position)=state.generation_queue
        .enqueue(&session_id,&chat_id,&gid)?;
    state.chats.append_user_with_turn_id(&chat_id,&gid,&prompt)?;
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
            if ticket.cancelled()||cancellation.is_cancelled(){
                return Err("Cancelled utility turn".into())
            }
            let current=host.supervisor.status()
                .map_err(|_|"Core utility model unavailable")?;
            if current.session_id.as_deref()!=Some(session_id.as_str()) ||
                current.model_ref!=initial.model_ref{
                return Err("Core utility model changed while queued".into())
            }
            let verifier=OriginalUtilityHumanTurn{
                chats:&host.chats,chat:&chat_id,turn:&gid,text:&prompt,
            };
            if !verifier.verify_user_turn(&chat_id,&gid){
                return Err("Core utility original current turn no longer authenticated".into())
            }
            // Grants exist only after a saved exact human turn and expire
            // automatically at end of the one supervised model/tool session.
            let mut turn=CoreUtilityTurn::new(
                &verifier,&chat_id,&gid,&gid,canonical
            )?;
            let capabilities=turn.capabilities()?;
            if capabilities.len()!=1 || capabilities[0].effect!="read_workspace"{
                return Err("Core utility advertised unexpected capabilities".into())
            }
            let label=capabilities[0].public_name.as_str();
            if !matches!(label,"lu_validate_schema"|"lu_check_env"){
                return Err("Core utility model schema was widened".into())
            }
            let model_prompt=if label=="lu_validate_schema"{
                format!("Use only the registered lu_validate_schema tool to validate \
                    the exact ORIGINAL human-supplied schema and data. Never \
                    invent missing JSON, change the input, write files or \
                    claim remote schema checks. First obtain real native \
                    feedback. An invalid instance is different from a failed \
                    tool. Then explain the bounded issues. \
                    Original human request: {prompt}")
            }else{
                format!("Use only lu_check_env for the single program explicitly \
                    named in the original human request. Host-approved file \
                    presence is NOT proof of installation, version, usable \
                    runtime, GPU operation or permission to execute. Do not \
                    invoke commands, search PATH, install or guess paths. \
                    Obtain the native observation before answering. \
                    Original human request: {prompt}")
            };
            let audit=ModelTurnJournal::open(
                &host.catalog_path.with_file_name("lu-core-utility-model-audit.sqlite")
            ).map_err(|_|"Core utility model audit unavailable")?;
            let observed=Arc::clone(&current_step);
            let notify_app=event_app.clone();
            let notify_chat=chat_id.clone();
            let notify_gid=gid.clone();
            let observer=Arc::new(move|step:&str|{
                if let Ok(mut id)=observed.lock(){*id=Some(step.into())}
                let _=notify_app.emit("lu-native-tool-progress",CoreUtilityProgress{
                    chat_id:notify_chat.clone(),generation_id:notify_gid.clone(),
                    phase:"model_step".into()
                });
            });
            let mut model=SupervisorModelStep::new(
                &host.supervisor,session_id.clone(),gid.clone()
            ).map_err(|e|format!("Core utility model bridge unavailable: {e:?}"))?
                .with_step_start_observer(observer);
            let _=event_app.emit("lu-generation-start",GenerationEvent{
                chat_id:chat_id.clone(),generation_id:gid.clone(),
                phase:"running",position:0,
            });
            let progress_app=event_app.clone();
            let progress_chat=chat_id.clone();
            let progress_gid=gid.clone();
            let dispatch=move|event:ToolDispatchCheckpoint|{
                let phase=match event{
                    ToolDispatchCheckpoint::Entered=>"tool_started",
                    ToolDispatchCheckpoint::Returned=>"tool_returned",
                };
                let _=progress_app.emit("lu-native-tool-progress",CoreUtilityProgress{
                    chat_id:progress_chat.clone(),generation_id:progress_gid.clone(),
                    phase:phase.into()
                });
            };
            let result=tauri::async_runtime::block_on(
                turn.run_journaled_observed(&model_prompt,&mut model,
                    &cancellation,&audit,&dispatch)
            ).map_err(|e|format!("Core utility native turn failed: {e:?}"))?;
            if cancellation.is_cancelled()||ticket.cancelled(){
                return Err("Cancelled utility turn after model feedback".into())
            }
            if result.tool_calls==0{
                return Err("Model did not use the exact approved utility".into())
            }
            host.chats.append_progress(&chat_id,&gid,&result.answer)?;
            host.chats.complete(&chat_id,&gid)?;
            Ok(result.answer)
        })();
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
#[cfg(test)]
mod tests{
    use super::*;
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
