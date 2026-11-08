//! E1-P07: registered model-visible Lu native reviewed-application operations.
//!
//! This is the actual ModelToolCatalog -> NativeToolRegistry -> original-user
//! ConversationAuthority -> reviewed application supervisor bridge. An LLM
//! chooses a named capability, never a binary, argv, file, scope or grant.
//! The host installs ONE previously reviewed Git repository and/or one media
//! input for an authenticated user task. Lu Core is NOT sandboxed.
//!
//! NativeToolTurn's *outer* durable dispatch and the externally supervised
//! *inner* journal use different host-derived effect IDs. The worker's
//! semantic receipt is verified before returning the typed model result.
use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::Arc,
    time::Duration,
};
use serde_json::{json,Map,Value};
use crate::{
    contracts::{
        ModuleKind,ModuleManifest,NativeTool,ProtoPayload,ToolAuthorizer,
        ToolDescriptor,ToolEffect,ToolFailure,ToolFuture,ToolInvocation,
        ToolOutput,CORE_CONTRACT_VERSION,
    },
    conversation_authority::{BoundCallOrigin,ConversationAuthority},
    journal::{EffectJournal,EffectState,IndependentOutcomeVerdict},
    model::{expect_fields,ModelToolCatalog,ModelToolCodec,
        ModelToolDefinition,ModelToolError},
    native_app_typed_git::{
        GitReadOperation,HostGitReadCatalog,HostReviewedGitRead,
        TypedGitReadBroker,INPUT_TYPE as GIT_INPUT,OUTPUT_TYPE as GIT_OUTPUT,
        RESOURCE_TYPE as GIT_READ_INPUT,RESOURCE_REQUEST as GIT_READ_BYTES,
    },
    native_app_typed_ffprobe::{
        HostMediaProbeCatalog,HostReviewedMediaProbe,ReviewedContainer,TypedMediaProbeBroker,
        INSPECT_INPUT as MEDIA_INPUT,INSPECT_OUTPUT as MEDIA_OUTPUT,
        MEDIA_READ_INPUT,SEALED_INSPECT_BYTES as MEDIA_EXEC_BYTES,
        SEALED_READ_BYTES as MEDIA_READ_BYTES,
    },
    native_background_process::BackgroundProcessSupervisor,
    reviewed_outcome_oracle::{verify_git,verify_media},
    registry::NativeToolRegistry,
    tasks::TaskStore,
};
const MODULE:&str="lu.reviewed";
const MAX_REVIEWED_TOOLS:usize=5;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
enum Operation{
    Head,TrackedStatus,Branch,History,MediaFacts,
}
impl Operation{
    fn name(self)->&'static str{
        match self{
            Self::Head=>"lu_reviewed_git_head",
            Self::TrackedStatus=>"lu_reviewed_git_status",
            Self::Branch=>"lu_reviewed_git_branch",
            Self::History=>"lu_reviewed_git_history",
            Self::MediaFacts=>"lu_reviewed_media_facts",
        }
    }
    fn description(self)->&'static str{
        match self{
            Self::Head=>"Read HEAD commit of the user's host-approved Git repository.",
            Self::TrackedStatus=>"Count changed tracked files in the user's host-approved Git repository.",
            Self::Branch=>"Read current branch of the user's approved Git repository (null when detached).",
            Self::History=>"Count reachable commits from HEAD in the user's approved Git repository.",
            Self::MediaFacts=>"Read duration, codec and audio/video stream properties of the host-approved WAV or MP4.",
        }
    }
    fn input(self)->&'static str{
        if self==Self::MediaFacts{MEDIA_INPUT}else{GIT_INPUT}
    }
    fn output(self)->&'static str{
        if self==Self::MediaFacts{MEDIA_OUTPUT}else{GIT_OUTPUT}
    }
    fn bytes(self)->&'static [u8]{
        match self{
            Self::Head=>GitReadOperation::HeadCommit.request(),
            Self::TrackedStatus=>GitReadOperation::TrackedStatus.request(),
            Self::Branch=>GitReadOperation::CurrentBranch.request(),
            Self::History=>GitReadOperation::HistoryCount.request(),
            Self::MediaFacts=>MEDIA_EXEC_BYTES,
        }
    }
    fn suffix(self)->&'static str{
        match self{
            Self::Head=>"gh",
            Self::TrackedStatus=>"gs",
            Self::Branch=>"gb",
            Self::History=>"gc",
            Self::MediaFacts=>"fm",
        }
    }
}
impl From<GitReadOperation> for Operation{
    fn from(op:GitReadOperation)->Self{
        match op{
            GitReadOperation::HeadCommit=>Self::Head,
            GitReadOperation::TrackedStatus=>Self::TrackedStatus,
            GitReadOperation::CurrentBranch=>Self::Branch,
            GitReadOperation::HistoryCount=>Self::History,
        }
    }
}

/// Each model-visible operation has zero parameters. Files, directories,
/// executable paths, aliases, review flags and permissions are ALWAYS host
/// selected; the codec refuses even a single additional JSON property.
fn append_outcome_to_model(
    mut normalized:Value,receipt:&Map<String,Value>
)->Result<Value,ModelToolError>{
    let Some(proof)=receipt.get("independentOutcome") else{
        return Ok(normalized)
    };
    let obj=proof.as_object().ok_or_else(||
        ModelToolError::InvalidResult("invalid independent outcome object".into()))?;
    if obj.len()!=2{
        return Err(ModelToolError::InvalidResult("unrecognized proof fields".into()))
    }
    let status=obj.get("status").and_then(Value::as_str)
        .ok_or_else(||ModelToolError::InvalidResult("outcome status missing".into()))?;
    let method=obj.get("method").and_then(Value::as_str)
        .ok_or_else(||ModelToolError::InvalidResult("outcome method missing".into()))?;
    if !matches!(status,"VERIFIED"|"UNSUPPORTED")
        ||!matches!(method,
            "GIT_HEAD_DIRECT_V1"|"GIT_BRANCH_DIRECT_V1"|
            "WAV_PCM_DIRECT_V1"|"GIT_STATUS_UNSUPPORTED_V1"|
            "GIT_HISTORY_UNSUPPORTED_V1"|"GIT_HISTORY_LOOSE_DAG_V1"|
            "GIT_HISTORY_OBJECT_DAG_V2"|"GIT_STATUS_INDEX_TREE_V1"|
            "MP4_BOXES_DIRECT_V1"|"MP4_UNSUPPORTED_V1")
        ||status=="VERIFIED"&&method.ends_with("_UNSUPPORTED_V1"){
        return Err(ModelToolError::InvalidResult(
            "unrecognized independent outcome classification".into()))
    }
    normalized.as_object_mut().ok_or_else(||
        ModelToolError::InvalidResult("invalid normalized payload".into()))?
        .insert("independentOutcome".into(),json!({
            "status":status,"method":method
        }));
    Ok(normalized)
}
struct ReviewedCodec(Operation);
impl ModelToolCodec for ReviewedCodec{
    fn input_type_url(&self)->&'static str{self.0.input()}
    fn output_type_url(&self)->&'static str{self.0.output()}
    fn encode(&self,args:&Map<String,Value>)->Result<ProtoPayload,ModelToolError>{
        expect_fields(args,&[],&[])?;
        Ok(ProtoPayload{
            type_url:self.0.input().into(),
            bytes:self.0.bytes().to_vec(),
        })
    }
    fn decode(&self,output:&ProtoPayload)->Result<Value,ModelToolError>{
        if output.type_url!=self.0.output()||output.bytes.len()>16_384{
            return Err(ModelToolError::InvalidResult("invalid reviewed result type".into()))
        }
        let value:Value=serde_json::from_slice(&output.bytes)
            .map_err(|_|ModelToolError::InvalidResult("invalid reviewed result bytes".into()))?;
        let object=value.as_object()
            .ok_or_else(||ModelToolError::InvalidResult("reviewed result must be object".into()))?;
        let expected=if self.0==Operation::MediaFacts{
            "lu-e1-p06e-media-facts-v1"
        }else{
            "lu-e1-p06c-typed-git-read-v1"
        };
        if object.get("schema").and_then(Value::as_str)!=Some(expected)
            ||object.get("osChildReaped")!=Some(&Value::Bool(true))
            ||object.get("luCoreSandboxed")!=Some(&Value::Bool(false)){
            return Err(ModelToolError::InvalidResult(
                "reviewed application receipt not verified".into()
            ))
        }
        // Never give the model a raw effect ID, arbitrary process output or
        // file path; only allow normalized information from the trusted broker.
        match self.0{
            Operation::Head=>{
                if object.get("kind").and_then(Value::as_str)!=Some("head_commit"){
                    return Err(ModelToolError::InvalidResult("wrong Git operation".into()))
                }
                let hash=object.get("headCommit").and_then(Value::as_str)
                    .ok_or_else(||ModelToolError::InvalidResult("missing commit".into()))?;
                if ![40,64].contains(&hash.len())||
                    !hash.bytes().all(|b|b.is_ascii_hexdigit()&&!b.is_ascii_uppercase()){
                    return Err(ModelToolError::InvalidResult("invalid commit hash".into()))
                }
                append_outcome_to_model(json!({"headCommit":hash}),object)
            }
            Operation::TrackedStatus=>{
                if object.get("kind").and_then(Value::as_str)!=Some("tracked_status"){
                    return Err(ModelToolError::InvalidResult("wrong status".into()))
                }
                let count=object.get("trackedChangedEntries").and_then(Value::as_u64)
                    .filter(|n|*n<=16_384)
                    .ok_or_else(||ModelToolError::InvalidResult("invalid count".into()))?;
                append_outcome_to_model(json!({"trackedChangedEntries":count}),object)
            }
            Operation::Branch=>{
                if object.get("kind").and_then(Value::as_str)!=Some("current_branch"){
                    return Err(ModelToolError::InvalidResult("wrong branch".into()))
                }
                let branch=object.get("currentBranch")
                    .ok_or_else(||ModelToolError::InvalidResult("branch missing".into()))?;
                if !branch.is_null() && (branch.as_str().is_none_or(|s|
                    s.len()>128||s.is_empty()||!s.bytes().all(|b|
                        b.is_ascii_alphanumeric()||matches!(b,b'.'|b'_'|b'/'|b'-')))){
                    return Err(ModelToolError::InvalidResult("branch invalid".into()))
                }
                append_outcome_to_model(json!({"currentBranch":branch}),object)
            }
            Operation::History=>{
                if object.get("kind").and_then(Value::as_str)!=Some("history_count"){
                    return Err(ModelToolError::InvalidResult("wrong history".into()))
                }
                let count=object.get("historyCommitCount").and_then(Value::as_u64)
                    .ok_or_else(||ModelToolError::InvalidResult("history invalid".into()))?;
                append_outcome_to_model(json!({"historyCommitCount":count}),object)
            }
            Operation::MediaFacts=>{
                if object.get("rawMediaPathPersisted")!=Some(&Value::Bool(false))
                    ||object.get("rawFFprobeOutputPersisted")!=Some(&Value::Bool(false))
                    ||object.get("mediaDigestReverifiedAfterProbe")!=Some(&Value::Bool(true)){
                    return Err(ModelToolError::InvalidResult("media custody invalid".into()))
                }
                let duration=object.get("durationMilliseconds").and_then(Value::as_u64)
                    .filter(|n|*n>=1&&*n<=604_800_000)
                    .ok_or_else(||ModelToolError::InvalidResult("duration invalid".into()))?;
                let streams=object.get("streams").and_then(Value::as_array)
                    .filter(|v|!v.is_empty()&&v.len()<=16)
                    .ok_or_else(||ModelToolError::InvalidResult("streams invalid".into()))?;
                append_outcome_to_model(json!({"durationMilliseconds":duration,"streams":streams}),object)
            }
        }
    }
}
fn refused(message:&'static str)->ToolFailure{
    ToolFailure::new("REVIEWED_APP_DENIED",message,false)
}
fn adapter_id(call_id:&str,operation:Operation)->Result<String,ToolFailure>{
    // Model call ID itself was generated by Lu, but still bound the result.
    if call_id.is_empty()||call_id.len()>115||!call_id.bytes().all(|b|
        b.is_ascii_alphanumeric()||matches!(b,b'.'|b'-'|b'_')){
        return Err(refused("invalid task-owned call identity"))
    }
    Ok(format!("{call_id}-{}",operation.suffix()))
}
enum ReviewedBackend{
    Git{
        catalog:Arc<HostGitReadCatalog>,
        broker:Arc<TypedGitReadBroker>,
        resource_id:String,
        read_tool_id:String,
        repository_root:PathBuf,
    },
    Media{
        catalog:Arc<HostMediaProbeCatalog>,
        broker:Arc<TypedMediaProbeBroker>,
        profile_id:String,
        read_tool_id:String,
        approved_media_path:PathBuf,
        media_sha256:String,
        reviewed_container:ReviewedContainer,
    },
}
struct ReviewedTool{
    descriptor:ToolDescriptor,
    operation:Operation,
    origin:BoundCallOrigin,
    authority:ConversationAuthority,
    policy:Arc<dyn ToolAuthorizer>,
    journal:Arc<EffectJournal>,
    backend:Arc<ReviewedBackend>,
}
impl NativeTool for ReviewedTool{
    fn descriptor(&self)->&ToolDescriptor{&self.descriptor}
    fn invoke(&self,call:ToolInvocation)->ToolFuture<'_>{
        let operation=self.operation;
        let descriptor=self.descriptor.clone();
        let origin=self.origin.clone();
        let authority=self.authority.clone();
        let policy=Arc::clone(&self.policy);
        let journal=Arc::clone(&self.journal);
        let backend=Arc::clone(&self.backend);
        Box::pin(async move{
            let invocation=call;
            tokio::task::spawn_blocking(move ||{
                let allowed=authority.permitted_tool_ids(&origin)
                    .map_err(|_|refused("original task unavailable"))?;
                if !allowed.contains(&descriptor.id){
                    return Err(refused("original task did not grant this operation"))
                }
                if invocation.tool_id!=descriptor.id
                    ||invocation.input.type_url!=descriptor.input_type_url
                    ||invocation.input.bytes.as_slice()!=operation.bytes(){
                    return Err(refused("invalid sealed application operation"))
                }
                let worker_id=adapter_id(&invocation.call_id,operation)?;
                let read_id=format!("{worker_id}-r");
                let mut execute=invocation.clone();
                execute.call_id=worker_id.clone();
                let (read_tool_id,read_type,read_bytes)=match backend.as_ref(){
                    ReviewedBackend::Git{read_tool_id,..}=>
                        (read_tool_id.as_str(),GIT_READ_INPUT,GIT_READ_BYTES),
                    ReviewedBackend::Media{read_tool_id,..}=>
                        (read_tool_id.as_str(),MEDIA_READ_INPUT,MEDIA_READ_BYTES),
                };
                let read=ToolInvocation{
                    call_id:read_id,tool_id:read_tool_id.into(),
                    workspace_id:invocation.workspace_id.clone(),
                    input:ProtoPayload{
                        type_url:read_type.into(),
                        bytes:read_bytes.to_vec(),
                    },
                };
                let read_descriptor=ToolDescriptor{
                    id:read_tool_id.into(),
                    display_name:"Host-authorized application resource read".into(),
                    effect:ToolEffect::ReadWorkspace,
                    input_type_url:read_type.into(),
                    output_type_url:descriptor.output_type_url.clone(),
                };
                if !allowed.contains(read_tool_id){
                    return Err(refused("current user task did not authorize resource read"))
                }
                match backend.as_ref(){
                    ReviewedBackend::Git{catalog,broker,resource_id,..}=>{
                        broker.launch(
                            catalog,resource_id,&origin,&execute,&descriptor,
                            &read,&read_descriptor,policy.as_ref()
                        ).map_err(|_|refused("reviewed Git admission failed"))?;
                    },
                    ReviewedBackend::Media{catalog,broker,profile_id,..}=>{
                        broker.launch(
                            catalog,profile_id,&origin,&execute,&descriptor,
                            &read,&read_descriptor,policy.as_ref()
                        ).map_err(|_|refused("reviewed media admission failed"))?;
                    },
                }
                // The broker owns the actual OS child and kills/reaps on Drop.
                // This closure only reads a bounded, independently parsed
                // receipt after the broker returns from poll().
                let deadline=std::time::Instant::now()+Duration::from_secs(45);
                loop{
                    let active=match backend.as_ref(){
                        ReviewedBackend::Git{broker,..}=>{
                            broker.poll().map_err(|_|refused("Git worker poll failed"))?;
                            broker.active_count().map_err(|_|refused("Git worker state unavailable"))?
                        },
                        ReviewedBackend::Media{broker,..}=>{
                            broker.poll().map_err(|_|refused("media worker poll failed"))?;
                            broker.active_count().map_err(|_|refused("media worker state unavailable"))?
                        },
                    };
                    if active==0{break}
                    if std::time::Instant::now()>=deadline{
                        let _=match backend.as_ref(){
                            ReviewedBackend::Git{broker,..}=>broker.cancel_current_task(&origin).is_ok(),
                            ReviewedBackend::Media{broker,..}=>broker.cancel_current_task(&origin).is_ok(),
                        };
                        return Err(refused("external application did not complete"))
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                let receipt=journal.get(&worker_id)
                    .map_err(|_|refused("effect receipt unavailable"))?
                    .ok_or_else(||refused("OS worker produced no journal receipt"))?;
                if receipt.state!=EffectState::Applied{
                    return Err(refused("OS worker failed or effect was revoked"))
                }
                let bytes=receipt.receipt
                    .ok_or_else(||refused("verified OS receipt missing"))?;
                if bytes.len()>16_384{
                    return Err(refused("verified OS receipt exceeds output limit"))
                }
                let value:Value=serde_json::from_slice(&bytes)
                    .map_err(|_|refused("verified OS receipt malformed"))?;
                if value.get("callId").and_then(Value::as_str)!=Some(worker_id.as_str()){
                    return Err(refused("OS effect receipt ID mismatch"))
                }
                // Independently validate before recording the conversation
                // reply. Raw application output is never re-published.
                ReviewedCodec(operation).decode(&ProtoPayload{
                    type_url:descriptor.output_type_url.clone(),
                    bytes:bytes.clone(),
                }).map_err(|_|refused("OS effect receipt schema mismatch"))?;
                // The actual OS effect is APPLIED, but its reported facts are
                // not independently trusted until a DIFFERENT host-only
                // observation of the original reviewed resource agrees.
                let oracle=match backend.as_ref(){
                    ReviewedBackend::Git{repository_root,..}=>{
                        let git_op=match operation{
                            Operation::Head=>GitReadOperation::HeadCommit,
                            Operation::TrackedStatus=>GitReadOperation::TrackedStatus,
                            Operation::Branch=>GitReadOperation::CurrentBranch,
                            Operation::History=>GitReadOperation::HistoryCount,
                            _=>return Err(refused("wrong Git outcome verifier"))
                        };
                        verify_git(repository_root,git_op,&value)
                    },
                    ReviewedBackend::Media{
                        approved_media_path,media_sha256,reviewed_container,..
                    }=>verify_media(
                        approved_media_path,media_sha256,*reviewed_container,&value
                    ),
                };
                // Persist the SECOND observation against the EXACT effect
                // bytes. Journal failure MUST deny model-visible success.
                journal.record_independent_outcome(
                    &worker_id,&bytes,oracle.verdict,oracle.method,
                    oracle.observed_sha256.as_deref()
                ).map_err(|_|refused("independent outcome journal unavailable"))?;
                if oracle.verdict==IndependentOutcomeVerdict::Conflict{
                    return Err(refused(
                        "host-observed resource contradicts reviewed application result"
                    ))
                }
                // A small status tag is the only new model-visible data.
                // Never forward source paths, evidence digests or process
                // output. The original inner effect receipt is immutable.
                let label=if oracle.verdict==IndependentOutcomeVerdict::Verified{
                    "VERIFIED"
                }else{"UNSUPPORTED"};
                let mut annotated=value;
                annotated.as_object_mut()
                    .ok_or_else(||refused("reviewed result not an object"))?
                    .insert("independentOutcome".into(),json!({
                        "status":label,"method":oracle.method
                    }));
                let result=serde_json::to_vec(&annotated)
                    .map_err(|_|refused("reviewed outcome could not be encoded"))?;
                if result.len()>16_384{
                    return Err(refused("reviewed verified result exceeds bound"))
                }
                Ok(ToolOutput{
                    payload:ProtoPayload{
                        type_url:descriptor.output_type_url,
                        bytes:result,
                    },
                })
            }).await.map_err(|_|refused("reviewed app worker stopped"))?
        })
    }
}

/// An opt-in, host-only installation for the *authenticated current turn*.
/// The host selects one reviewed Git repository (up to four named operations)
/// and at most one reviewed media file. No model-supplied paths or config
/// are involved. Caller provides same journal used by NativeToolTurn's
/// durable dispatch; inner external worker entries have different call IDs.
///
/// Requires a fresh NativeToolRegistry + ModelToolCatalog scoped to the
/// user's current tool session, and a real policy implementation. The host
/// MUST open the task through its persisted human-turn verifier first.
#[allow(clippy::too_many_arguments)]
pub fn register_reviewed_application_tools(
    registry:&NativeToolRegistry,
    catalog:&mut ModelToolCatalog,
    authority:ConversationAuthority,
    origin:BoundCallOrigin,
    tasks:Arc<TaskStore>,
    supervisor:Arc<BackgroundProcessSupervisor>,
    effects:Arc<EffectJournal>,
    policy:Arc<dyn ToolAuthorizer>,
    reviewed_git:Vec<HostReviewedGitRead>,
    reviewed_media:Option<HostReviewedMediaProbe>,
)->Result<Vec<String>,String>{
    let allowed=authority.permitted_tool_ids(&origin)
        .map_err(|e|format!("user-turn authority unavailable: {e:?}"))?;
    let mut descriptors=Vec::new();
    let mut native:Vec<Arc<dyn NativeTool>>=Vec::new();
    let mut definitions=Vec::new();
    let mut names=BTreeSet::new();
    let mut git_catalog=HostGitReadCatalog::new();
    let mut git_configs=Vec::new();
    for record in reviewed_git{
        let operation=Operation::from(record.operation);
        if !names.insert(operation.name().to_owned()){
            return Err("duplicate model-visible Git operation".into())
        }
        if !record.execute_tool_id.starts_with("lu.reviewed.")
            ||!allowed.contains(&record.execute_tool_id)
            ||!allowed.contains(&record.read_tool_id){
            return Err("host did not grant reviewed Git read and execution".into())
        }
        git_catalog.register_host_reviewed(record.clone())
            .map_err(|e|format!("host Git review refused: {e:?}"))?;
        git_configs.push((operation,record));
    }
    let git_backend=if git_configs.is_empty(){None}else{
        Some(Arc::new(ReviewedBackend::Git{
            catalog:Arc::new(git_catalog),
            broker:Arc::new(TypedGitReadBroker::new(
                authority.clone(),Arc::clone(&supervisor),Arc::clone(&tasks),
                Arc::clone(&effects)
            ).map_err(|e|format!("Git broker unavailable: {e:?}"))?),
            resource_id:String::new(),
            read_tool_id:String::new(),
            repository_root:PathBuf::new(),
        }))
    };
    for (operation,record) in git_configs{
        let base=git_backend.as_ref().expect("Git backend configured");
        let backend=match base.as_ref(){
            ReviewedBackend::Git{catalog,broker,..}=>Arc::new(ReviewedBackend::Git{
                catalog:Arc::clone(catalog),broker:Arc::clone(broker),
                resource_id:record.resource_id.clone(),
                read_tool_id:record.read_tool_id.clone(),
                repository_root:record.repository_root.clone(),
            }),
            _=>unreachable!(),
        };
        let descriptor=ToolDescriptor{
            id:record.execute_tool_id,
            display_name:operation.description().into(),
            effect:ToolEffect::ExecuteCode,
            input_type_url:operation.input().into(),
            output_type_url:operation.output().into(),
        };
        append_tool(
            &mut descriptors,&mut native,&mut definitions,
            descriptor,operation,backend,&authority,&origin,&policy,&effects,
        );
    }
    if let Some(record)=reviewed_media{
        let operation=Operation::MediaFacts;
        if !names.insert(operation.name().into()){
            return Err("duplicate reviewed media capability".into())
        }
        if !record.execute_tool_id.starts_with("lu.reviewed.")
            ||!allowed.contains(&record.execute_tool_id)
            ||!allowed.contains(&record.read_tool_id){
            return Err("host did not grant reviewed media read and execution".into())
        }
        let mut media_catalog=HostMediaProbeCatalog::new();
        media_catalog.register_host_reviewed(record.clone())
            .map_err(|e|format!("host media review refused: {e:?}"))?;
        let backend=Arc::new(ReviewedBackend::Media{
            catalog:Arc::new(media_catalog),
            broker:Arc::new(TypedMediaProbeBroker::new(
                authority.clone(),Arc::clone(&supervisor),Arc::clone(&tasks),
                Arc::clone(&effects)
            ).map_err(|e|format!("media broker unavailable: {e:?}"))?),
            profile_id:record.profile_id,
            read_tool_id:record.read_tool_id,
            approved_media_path:record.approved_media_path,
            media_sha256:record.media_sha256,
            reviewed_container:record.reviewed_container,
        });
        let descriptor=ToolDescriptor{
            id:record.execute_tool_id,
            display_name:operation.description().into(),
            effect:ToolEffect::ExecuteCode,
            input_type_url:operation.input().into(),
            output_type_url:operation.output().into(),
        };
        append_tool(
            &mut descriptors,&mut native,&mut definitions,
            descriptor,operation,backend,&authority,&origin,&policy,&effects,
        );
    }
    if descriptors.is_empty()||descriptors.len()>MAX_REVIEWED_TOOLS{
        return Err("no current-turn reviewed application operations".into())
    }
    registry.register(ModuleManifest{
        id:MODULE.into(),display_name:"Lu reviewed local applications".into(),
        contract_version:CORE_CONTRACT_VERSION,
        // Per-turn registry has an unloadable workspace lifecycle.
        kind:ModuleKind::Workspace,
        tools:descriptors,panels:vec![],workflows:vec![],
    },native).map_err(|e|format!("reviewed registry rejected: {e:?}"))?;
    let mut registered=Vec::new();
    for def in definitions{
        registered.push(def.name.clone());
        catalog.insert(registry,def)
            .map_err(|e|format!("reviewed model codec refused: {e:?}"))?;
    }
    Ok(registered)
}
#[allow(clippy::too_many_arguments)]
fn append_tool(
    descriptors:&mut Vec<ToolDescriptor>,
    implementations:&mut Vec<Arc<dyn NativeTool>>,
    definitions:&mut Vec<ModelToolDefinition>,
    descriptor:ToolDescriptor,operation:Operation,
    backend:Arc<ReviewedBackend>,
    authority:&ConversationAuthority,origin:&BoundCallOrigin,
    policy:&Arc<dyn ToolAuthorizer>,effects:&Arc<EffectJournal>,
){
    let name=operation.name();
    definitions.push(ModelToolDefinition{
        name:name.into(),native_tool_id:descriptor.id.clone(),
        description:operation.description().into(),
        // It is impossible to propose a file path or binary via this JSON.
        parameters:json!({
            "type":"object","properties":{},
            "required":[],"additionalProperties":false
        }),
        codec:Arc::new(ReviewedCodec(operation)),
    });
    implementations.push(Arc::new(ReviewedTool{
        descriptor:descriptor.clone(),operation,
        origin:origin.clone(),authority:authority.clone(),
        policy:Arc::clone(policy),journal:Arc::clone(effects),backend,
    }));
    descriptors.push(descriptor);
}

/// Monitor a real host-owned cancellation token while a blocking reviewed
/// subprocess is running. On cancellation revoke the human task; the
/// supervisor's authority listener immediately kills/reaps that OS child.
/// Dropping this guard always joins the monitor; no detached lifetime leak.
struct ReviewedCancelWatch{
    running:Arc<std::sync::atomic::AtomicBool>,
    thread:Option<std::thread::JoinHandle<()>>,
}
impl ReviewedCancelWatch{
    fn start(
        cancel:crate::inference_host::Cancellation,
        authority:ConversationAuthority,
        task_id:String
    )->Self{
        use std::sync::atomic::{AtomicBool,Ordering};
        let running=Arc::new(AtomicBool::new(true));
        let witness=Arc::clone(&running);
        let thread=std::thread::spawn(move||{
            while witness.load(Ordering::Acquire){
                if cancel.is_cancelled(){
                    let _=authority.revoke_task(&task_id);
                    break
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        });
        Self{running,thread:Some(thread)}
    }
}
impl Drop for ReviewedCancelWatch{
    fn drop(&mut self){
        use std::sync::atomic::Ordering;
        self.running.store(false,Ordering::Release);
        if let Some(thread)=self.thread.take(){let _=thread.join();}
    }
}

/// Model-enabled reviewed application turn for an authenticated user request.
/// This does NOT create a grant. Host must first record and verify the real
/// human turn, review/install the program and resource, open the exact
/// ReadWorkspace + ExecuteCode task, and supply a policy instance.
///
/// The model is given ONLY schemas of those already host-approved choices;
/// file paths, binaries and permission fields are absent from the schema.
pub struct ReviewedApplicationTurnSession{
    registry:NativeToolRegistry,
    catalog:ModelToolCatalog,
    authority:ConversationAuthority,
    turn:crate::agent_tool_loop::NativeToolTurn,
    policy:Arc<dyn ToolAuthorizer>,
    effects:Arc<EffectJournal>,
    origin:BoundCallOrigin,
}
impl ReviewedApplicationTurnSession{
    #[allow(clippy::too_many_arguments)]
    pub fn from_verified_host_task(
        authority:ConversationAuthority,
        origin:BoundCallOrigin,
        workspace_id:String,
        tasks:Arc<TaskStore>,
        supervisor:Arc<BackgroundProcessSupervisor>,
        effects:Arc<EffectJournal>,
        policy:Arc<dyn ToolAuthorizer>,
        git:Vec<HostReviewedGitRead>,
        media:Option<HostReviewedMediaProbe>,
    )->Result<Self,String>{
        if workspace_id.is_empty()||workspace_id.len()>128{
            return Err("trusted workspace missing".into())
        }
        let registry=NativeToolRegistry::new();
        let mut catalog=ModelToolCatalog::new();
        let names=register_reviewed_application_tools(
            &registry,&mut catalog,authority.clone(),origin.clone(),
            tasks,supervisor,Arc::clone(&effects),Arc::clone(&policy),
            git,media
        )?;
        let allowed=names.into_iter().collect::<BTreeSet<_>>();
        let turn=crate::agent_tool_loop::NativeToolTurn::new(
            &catalog,&authority,origin.clone(),workspace_id,&allowed
        ).map_err(|e|format!("reviewed model turn denied: {e:?}"))?;
        Ok(Self{registry,catalog,authority,turn,policy,effects,origin})
    }
    pub fn schemas(&self)->Result<Vec<Value>,String>{
        self.turn.schemas(&self.catalog).map_err(|e|format!("{e:?}"))
    }
    pub fn capabilities(
        &self
    )->Result<Vec<crate::native_capability_inventory::NativeCapabilityView>,String>{
        crate::native_capability_inventory::discover_turn_capabilities(
            &self.registry,&self.catalog,&self.authority,&self.origin,false
        ).map_err(|e|format!("{e:?}"))
    }
    /// Uses the production supervised model/tool loop and BOTH durable
    /// journals: conversation effect dispatcher + OS worker effect receipt.
    /// No host-auto-routing and no fallback to non-durable dispatch.
    pub async fn run_journaled_observed(
        &mut self,user_text:&str,
        model:&mut dyn crate::supervised_tool_generation::ModelStepGenerator,
        cancel:&crate::inference_host::Cancellation,
        audit:&crate::model_turn_journal::ModelTurnJournal,
        observe:&(dyn Fn(crate::agent_tool_loop::ToolDispatchCheckpoint)+Send+Sync),
    )->Result<crate::supervised_tool_generation::NativeTurnReport,
               crate::supervised_tool_generation::NativeGenerationError>{
        let _cancel_watch=ReviewedCancelWatch::start(
            cancel.clone(),self.authority.clone(),self.origin.task_id.clone()
        );
        let services=crate::supervised_tool_generation::NativeTurnServices{
            catalog:&self.catalog,registry:&self.registry,
            authority:&self.authority,host_policy:self.policy.as_ref(),
        };
        let result=crate::supervised_tool_generation::
            run_supervised_native_turn_journaled_observed_with_host_routing(
                user_text,user_text,None,&[],
                &mut self.turn,model,&services,cancel,audit,
                self.effects.as_ref(),observe
            ).await;
        self.revoke();
        result
    }
    pub fn revoke(&mut self){
        self.turn.halt();
        let _=self.authority.revoke_task(&self.origin.task_id);
    }
}
impl Drop for ReviewedApplicationTurnSession{
    fn drop(&mut self){
        self.revoke();
    }
}
