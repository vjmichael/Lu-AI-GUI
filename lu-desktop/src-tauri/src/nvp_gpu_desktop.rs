//! LU-IPC-GPU-01D-R2: Native desktop admission of ONE installed NVP film job.
//!
//! This is an INTERNAL owner API, not a WebView command or a network/server
//! autostart. An independently installed/signed NVP host supplies a reviewed
//! registry, real signed SCM supervisor, original durable call and exact
//! read/execute plan. Until that installed provider exists the function has
//! no callable production path, and all GPU release remains default-deny.
#![cfg(windows)]

use super::*;
use lu_core::{
    contracts::{ToolAuthorizer,ToolEffect},
    conversation_authority::{
        AuthorizedCapability,BoundCallOrigin,TaskAuthority,TrustedUserTurnVerifier,
    },
    registry::NativeToolRegistry,
    workspace_ipc_core_host::{
        ApprovedNvpGpuPlan,HostCoreTask,TrustedCoreInputVerifier,
        OriginalCall,serve_host_approved_nvp_gpu_windows,
    },
    workspace_ipc_scm::VerifiedScmSupervisor,
    workspace_ipc_transport::ServerIdentity,
};
use std::{sync::atomic::Ordering,time::{Duration,SystemTime}};

/// Only the *native installer-owned* component may provide these borrowed
/// witnesses. In particular, there is no Tauri command to pass a PID,
/// executable path, OS signer, HMAC, task, descriptor or approval bit.
pub(crate) struct InstalledNvpHostInputs<'a>{
    pub endpoint:&'a str,
    pub original:&'a OriginalCall,
    pub identity:&'a ServerIdentity,
    pub registry:&'a NativeToolRegistry,
    pub plan:&'a ApprovedNvpGpuPlan<'a>,
    pub policy:Arc<dyn ToolAuthorizer>,
    pub input_verifier:&'a dyn TrustedCoreInputVerifier,
    pub scm:&'a VerifiedScmSupervisor,
}

fn original_creator_turn(
    chats:&ChatStore,
    original:&OriginalCall,
)->Result<(String,i64),String>{
    if original.module_id!="nvp.film" ||
       original.conversation_ref.is_empty()||
       original.user_turn_ref.is_empty()||
       original.task_ref.is_empty()||
       original.workspace_ref.is_empty() {
        return Err("No original film creator scope".into())
    }
    let chat=&original.conversation_ref;
    let turn=&original.user_turn_ref;
    if chats.latest_authenticated_user_turn_id(chat).as_deref()!=Some(turn){
        return Err("Film request is not the current authenticated USER turn".into())
    }
    let original_text=chats.trusted_user_turn_text(chat,turn)
        .ok_or("Film request has no authenticated persisted USER text")?;
    if original_text.trim().is_empty(){
        return Err("Original film request is empty".into())
    }
    let verifier=crate::turn_authority::PersistedHumanTurn::current_chat(
        chats,chat,turn,&original_text);
    if !verifier.verify_user_turn(chat,turn){
        return Err("Film creator turn provenance changed".into())
    }
    let watermark=chats.p0_auth_requested_watermark(chat)?;
    Ok((original_text,watermark))
}

fn exact_registered_film_tools(
    registry:&NativeToolRegistry,
    plan:&ApprovedNvpGpuPlan<'_>,
)->Result<(),String>{
    let modules=registry.modules()
        .map_err(|_|"NVP native module registry unavailable")?;
    if !modules.iter().any(|m|
        m.id=="nvp.film" &&
        m.kind==lu_core::contracts::ModuleKind::Workspace){
        return Err("No currently installed/registered NVP film workspace".into())
    }
    let tools=registry.tools().map_err(|_|"NVP tool inventory unavailable")?;
    for (invocation,descriptor,effect) in [
        (plan.read,plan.read_descriptor,ToolEffect::ReadWorkspace),
        (plan.execute,plan.execute_descriptor,ToolEffect::ExecuteCode),
    ]{
        if !descriptor.id.starts_with("nvp.film.") ||
           invocation.tool_id!=descriptor.id ||
           invocation.workspace_id.is_none()||
           invocation.input.type_url!=descriptor.input_type_url||
           descriptor.effect!=effect||
           !tools.iter().any(|current|current==descriptor){
            return Err("Original film capabilities are not installed and current".into())
        }
    }
    if plan.read.call_id==plan.execute.call_id ||
       plan.read.workspace_id!=plan.execute.workspace_id{
        return Err("Film plan is not an exact scoped READ and EXECUTE pair".into())
    }
    Ok(())
}

fn validate_installed_original(
    owner:&DesktopState,
    inputs:&InstalledNvpHostInputs<'_>,
)->Result<(String,i64),String>{
    let original=inputs.original;
    let identity=inputs.identity;
    if original.session_ref!=identity.session_ref ||
       original.core_epoch_ref!=identity.core_epoch_ref ||
       original.call_id!=inputs.plan.execute.call_id ||
       original.fingerprint_sha256!=
           lu_core::journal::fingerprint(inputs.plan.execute) ||
       inputs.plan.execute.workspace_id.as_deref()!=Some(&original.workspace_ref) ||
       inputs.plan.read.workspace_id.as_deref()!=Some(&original.workspace_ref) ||
       identity.server_nonce.iter().all(|v|*v==0){
        return Err("NVP original operation or server generation does not match".into())
    }
    exact_registered_film_tools(inputs.registry,inputs.plan)?;
    original_creator_turn(&owner.chats,original)
}

fn clear_grant(owner:&DesktopState,task:&str){
    if let Ok(mut slot)=owner.nvp_grant.lock(){
        if slot.as_ref().is_some_and(|g|g.task_id==task){
            slot.take();
        }
    }
}

/// The native desktop owner prepares its ORIGINAL persisted USER grant and
/// invokes the genuine SCM-authenticated Protobuf pipe. Approval is a native
/// OS dialog, NEVER a caller-supplied `approved=true` or model suggestion.
/// On cancellation, revocation or error, the durable GPU WAL remains fenced
/// until independent OS Job-empty + VRAM measurements establish cleanup.
#[allow(dead_code)] // Installed NVP package/issuer integration is a separate gate.
pub(crate) async fn serve_installed_nvp_for_creator_turn(
    app:AppHandle,
    owner:Arc<DesktopState>,
    inputs:InstalledNvpHostInputs<'_>,
)->Result<(),String>{
    let original=inputs.original;
    let (original_text,watermark)=validate_installed_original(&owner,&inputs)?;
    inputs.scm.require_live()
        .map_err(|_|"Installed Core SCM supervisor is not independently verified")?;
    inputs.scm.private_scm_health().await
        .map_err(|_|"Installed Core SCM private proof failed")?;
    let cancelled=Arc::new(AtomicBool::new(false));
    {
        let mut slot=owner.nvp_grant.lock()
            .map_err(|_|"NVP desktop authority lock unavailable")?;
        if slot.is_some(){return Err("An NVP film GPU handoff is already present".into())}
        *slot=Some(ActiveNvpGrant{
            task_id:original.task_ref.clone(),
            chat_id:original.conversation_ref.clone(),
            user_turn_id:original.user_turn_ref.clone(),
            cancel:Arc::clone(&cancelled),
            revocation_watermark:watermark,
        });
    }
    let task=original.task_ref.clone();
    let result=async {
        // Display the REAL persisted user request, not a model-proposed film
        // title, with a bounded excerpt for native consent ergonomics.
        let excerpt=original_text.chars().filter(|c|
            !c.is_control()||*c=='\n'
        ).take(480).collect::<String>();
        let truncated=original_text.chars().count()>480;
        let prompt=format!(
            r#"Authorize ONE NVP film GPU handoff for this original Lu conversation?

Original user request:
{}{}

Film workspace: {}
Read capability: {}
Render capability: {}

Lu unloads its current GGUF only after verified Core authorization.
NVP must independently prove GPU/Job quiescence before Lu reloads.
This film request must remain the current original USER turn.
Cancellation or uncertain worker termination NEVER authorizes a reload."#,
            excerpt,if truncated{" … [excerpt truncated]"}else{""},
            original.workspace_ref,inputs.plan.read_descriptor.id,
            inputs.plan.execute_descriptor.id
        );
        let approved=tauri::async_runtime::spawn_blocking(move||{
            app.dialog().message(prompt)
                .title("Lu — NVP film GPU authorization")
                .buttons(tauri_plugin_dialog::MessageDialogButtons::OkCancelCustom(
                    "Authorize this film operation".into(),"Cancel".into()
                )).blocking_show()
        }).await.map_err(|_|"NVP native consent dialog unavailable")?;
        if !approved{return Err("NVP film authorization refused".into())}
        // A different human turn, system Stop, or changed signed package
        // invalidates the prepared grant even if the OS modal said OK.
        if cancelled.load(Ordering::Acquire) ||
           owner.chats.p0_auth_requested_watermark(&original.conversation_ref)?
               !=watermark ||
           owner.chats.trusted_user_turn_text(
               &original.conversation_ref,&original.user_turn_ref
           ).as_deref()!=Some(original_text.as_str()) ||
           owner.chats.latest_authenticated_user_turn_id(
               &original.conversation_ref
           ).as_deref()!=Some(original.user_turn_ref.as_str()){
            return Err("NVP original USER grant was cancelled or changed".into())
        }
        validate_installed_original(&owner,&inputs)?;
        inputs.scm.require_live()
            .map_err(|_|"NVP original SCM service generation disappeared")?;
        let verifier=crate::turn_authority::PersistedHumanTurn::current_chat(
            &owner.chats,&original.conversation_ref,
            &original.user_turn_ref,&original_text
        );
        // Serialize the verified current USER/approval transition against
        // Lu's global Stop removing the creator grant. A Stop cannot slip
        // between the last cancelled check and opening this new task.
        {
            let slot=owner.nvp_grant.lock()
                .map_err(|_|"NVP creator authority lock unavailable")?;
            if cancelled.load(Ordering::Acquire) ||
               !slot.as_ref().is_some_and(|grant|
                   grant.task_id==original.task_ref &&
                   grant.chat_id==original.conversation_ref &&
                   grant.user_turn_id==original.user_turn_ref &&
                   grant.revocation_watermark==watermark){
                return Err("NVP creator grant cancelled before admission".into())
            }
            owner.nvp_authority.open_user_task(TaskAuthority{
                conversation_id:original.conversation_ref.clone(),
                user_turn_id:original.user_turn_ref.clone(),
                task_id:original.task_ref.clone(),
                execution_scope_id:original.workspace_ref.clone(),
                capabilities:vec![
                    AuthorizedCapability{
                        tool_id:inputs.plan.read_descriptor.id.clone(),
                        effect:ToolEffect::ReadWorkspace,
                    },
                    AuthorizedCapability{
                        tool_id:inputs.plan.execute_descriptor.id.clone(),
                        effect:ToolEffect::ExecuteCode,
                    },
                ],
                expires_at:Some(SystemTime::now()+Duration::from_secs(300)),
            },&verifier).map_err(|_|"Native original film grant refused")?;
        }
        let host_origin=BoundCallOrigin{
            conversation_id:original.conversation_ref.clone(),
            user_turn_id:original.user_turn_ref.clone(),
            task_id:original.task_ref.clone(),
        };
        let host=HostCoreTask{
            origin:&host_origin,workspace_scope:&original.workspace_ref,
            policy:inputs.policy.as_ref(),
            input_verifier:inputs.input_verifier,
        };
        // The only runtime dependency capable of starting this listener is
        // an installed native provider with signed SCM custody and *real*
        // registered film tool implementations. It does not autostart.
        let stop=async {
            while !cancelled.load(Ordering::Acquire){
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        };
        tokio::select!{
            result=serve_host_approved_nvp_gpu_windows(
                inputs.endpoint,inputs.identity,original,
                &host,&owner.nvp_authority,inputs.registry,inputs.plan,
                Arc::clone(&inputs.policy),&owner.gpu_async,
                &owner.gpu_leases,&owner.supervisor,inputs.scm,
            )=>result.map_err(|_|"Installed Core/NVP GPU session ended without acceptance".into()),
            _=stop=>Err("NVP creator revoked the GPU request".into()),
        }
    }.await;
    cancelled.store(true,Ordering::Release);
    let _=owner.nvp_authority.revoke_task(&task);
    clear_grant(&owner,&task);
    result
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn original_creator_requires_real_current_persisted_user_turn(){
        let root=std::env::temp_dir().join(next_id("lu-nvp-creator-test-"));
        let store=ChatStore::open(&root).unwrap();
        let chat=store.create_chat().unwrap().chat_id;
        let turn=next_id("g-");
        let original=OriginalCall{
            call_id:"a".repeat(64),fingerprint_sha256:"b".repeat(64),
            module_id:"nvp.film".into(),session_ref:"session".into(),
            core_epoch_ref:"epoch".into(),conversation_ref:chat.clone(),
            user_turn_ref:turn.clone(),task_ref:"film-task".into(),
            workspace_ref:"film-project".into(),
        };
        assert!(store.trusted_user_turn_text(&chat,&turn).is_none());
        assert!(original_creator_turn(&store,&original).is_err());
        let fake=crate::turn_authority::PersistedHumanTurn::current_chat(
            &store,&chat,&turn,"Generate this film"
        );
        assert!(!fake.verify_user_turn(&chat,&turn));
        store.append_user_with_turn_id(&chat,&turn,"Generate this film").unwrap();
        assert!(store.trusted_user_turn_text(&chat,&turn).is_some());
        let p=crate::turn_authority::PersistedHumanTurn::current_chat(
            &store,&chat,&turn,"Generate this film"
        );
        assert!(p.verify_user_turn(&chat,&turn));
        assert_eq!(original_creator_turn(&store,&original).unwrap().0,
            "Generate this film");
        let later=next_id("g-");
        store.append_user_with_turn_id(&chat,&later,"Different request").unwrap();
        assert!(!p.verify_user_turn(&chat,&turn));
        assert!(original_creator_turn(&store,&original).is_err());
        let mut fake_scope=original.clone();
        fake_scope.module_id="lu.ide".into();
        assert_ne!(fake_scope.module_id,"nvp.film");
        assert!(original_creator_turn(&store,&fake_scope).is_err());
        drop(store);
        let _=std::fs::remove_dir_all(root);
    }
}
