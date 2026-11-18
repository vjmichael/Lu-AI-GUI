//! Authority-aware native capability discovery: pre-grant registration
//! contracts and post-grant live registry/codec projection. Discovery never
//! opens a grant, executes a tool, or turns metadata into runtime authority.
use std::collections::BTreeSet;
use crate::{
    contracts::{ToolDescriptor,ToolEffect},
    conversation_authority::{BoundCallOrigin,ConversationAuthority},
    model::{ModelToolCatalog,ModelToolDefinition},
    registry::NativeToolRegistry,
};
#[derive(Clone,Debug,PartialEq,Eq,serde::Serialize)]
#[serde(rename_all="camelCase")]
pub struct NativeCapabilityView{
    pub native_tool_id:String,
    pub public_name:String,
    pub display_name:String,
    pub effect:String,
    pub input_type_url:String,
    pub output_type_url:String,
    /// Exactly one of the owner-approved three classifications when reviewed.
    /// None means this installed capability has not been classified yet.
    pub meta_class:Option<String>,
    /// A tool name and per-turn grant do not attest external prerequisites.
    pub availability_claim:String,
    pub external_dependencies_verified:bool,
    /// Never infer successful execution from discovery or model confidence.
    pub task_outcome_verified:bool,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum CapabilityInventoryError{
    AuthorityUnavailable,RegistryUnavailable,ModelCatalogUnavailable,
    MissingNativeContract,PrivilegeEscalation,
}
fn effect_label(effect:ToolEffect)->&'static str{
    match effect{
        ToolEffect::ReadWorkspace=>"read_workspace",
        ToolEffect::WriteWorkspace=>"write_workspace",
        ToolEffect::ExecuteCode=>"execute_code",
        ToolEffect::NetworkRead=>"network_read",
        ToolEffect::NetworkWrite=>"network_write",
        ToolEffect::DeviceControl=>"device_control",
        ToolEffect::Elevated=>"elevated",
    }
}
/// Classification is a reviewed host mapping, NOT model-controlled text.
/// Unlisted native IDs are unclassified until reviewed; no fourth class exists.
fn reviewed_meta_class(id:&str)->Option<&'static str>{
    match id{
        // Exact native tool identities reviewed against their current sources.
        "lu.files.read_text"|"lu.files.list_directory"|
        "lu.inspect.stat"|"lu.inspect.search_text"|
        "lu.file_discovery.discover"|"lu.reader.read"|
        "lu.artifact.inspect"|"lu.background.status"|
        "lu.background_receipt.get"|"lu.web.browser.read"|
        "lu.web.browser.interact"=>Some("foundational"),
        "lu.archive.list"|"lu.git.status"|"lu.git.log"|
        "lu.git.diff"|
        "lu.reviewed.git.head"|"lu.reviewed.git.status"|
        "lu.reviewed.git.branch"|"lu.reviewed.git.history"|
        "lu.reviewed.media.facts"|
        "lu.validate.schema"|"lu.exec.check_env"=>Some("utility"),
        // An unreviewed tool remains unclassified rather than being promoted
        // by merely choosing a familiar module prefix.
        _=>None
    }
}
/// Only exact, *currently permitted* native IDs with a matching installed
/// codec are projected. This is not a global ability catalog. A revoked
/// or unregistered tool is absent rather than advertised with fake access.
pub fn discover_turn_capabilities(
    registry:&NativeToolRegistry,catalog:&ModelToolCatalog,
    authority:&ConversationAuthority,origin:&BoundCallOrigin,
    only_read:bool,
)->Result<Vec<NativeCapabilityView>,CapabilityInventoryError>{
    let allowed=authority.permitted_tool_ids(origin)
        .map_err(|_|CapabilityInventoryError::AuthorityUnavailable)?;
    let descriptors=registry.tools()
        .map_err(|_|CapabilityInventoryError::RegistryUnavailable)?;
    let names=catalog.names_for_native_ids(&allowed);
    let schemas=catalog.live_schemas(&names,registry)
        .map_err(|_|CapabilityInventoryError::ModelCatalogUnavailable)?;
    let schema_names=schemas.iter().filter_map(|s|
        s.pointer("/function/name").and_then(serde_json::Value::as_str)
    ).collect::<BTreeSet<_>>();
    let mut result=Vec::new();
    for desc in descriptors{
        if !allowed.contains(&desc.id){continue}
        if only_read && desc.effect!=ToolEffect::ReadWorkspace{
            return Err(CapabilityInventoryError::PrivilegeEscalation)
        }
        let registered=catalog.names_for_native_ids(&BTreeSet::from([desc.id.clone()]));
        if registered.is_empty(){
            return Err(CapabilityInventoryError::MissingNativeContract)
        }
        for public_name in registered{
            if !schema_names.contains(public_name.as_str()){
                return Err(CapabilityInventoryError::MissingNativeContract)
            }
            result.push(NativeCapabilityView{
                native_tool_id:desc.id.clone(),
                public_name,
                display_name:desc.display_name.clone(),
                effect:effect_label(desc.effect).into(),
                input_type_url:desc.input_type_url.clone(),
                output_type_url:desc.output_type_url.clone(),
                meta_class:reviewed_meta_class(&desc.id).map(str::to_owned),
                availability_claim:"native_contract_registered_and_turn_granted_only".into(),
                external_dependencies_verified:false,
                task_outcome_verified:false,
            });
        }
    }
    // An allowed registered model tool cannot disappear silently from the
    // published inventory while leaving an exposed inference schema behind.
    if result.len()!=schema_names.len(){
        return Err(CapabilityInventoryError::MissingNativeContract)
    }
    result.sort_by(|a,b|a.public_name.cmp(&b.public_name));
    Ok(result)
}

/// The exact descriptor and codec definition consumed by a real registrar.
/// Deliberately owns no NativeTool, registry, handles, task or authority.
/// Deferred resource-bound registration is NOT an installed executor.
pub(crate) struct RegistrationContract {
    descriptor:ToolDescriptor,
    definition:ModelToolDefinition,
}
impl RegistrationContract {
    pub(crate) fn new(descriptor:ToolDescriptor,definition:ModelToolDefinition)->Self{
        Self{descriptor,definition}
    }
    pub(crate) fn into_parts(self)->(ToolDescriptor,ModelToolDefinition){
        (self.descriptor,self.definition)
    }
}
/// External dependencies are observed without execution or resource selection.
/// A candidate file is NOT a verified executable, and even a pinned package
/// does not demonstrate network/service availability or successful execution.
#[derive(Debug,Clone,Copy,PartialEq,Eq,serde::Serialize)]
#[serde(rename_all="snake_case")]
pub enum RuntimeEvidence {
    Builtin,
    CandidatePresent,
    PinnedPackagePresent,
    Missing,
    ProbeUnavailable,
}
#[derive(Debug,Clone,Copy,PartialEq,Eq,serde::Serialize)]
#[serde(rename_all="snake_case")]
pub enum PendingToolState { NeedsAuthorization, RuntimeUnavailable, PolicyBlocked }
#[derive(Debug,Clone,PartialEq,Eq,serde::Serialize)]
#[serde(rename_all="camelCase")]
pub struct PendingCapabilityView {
    pub native_tool_id:String,
    pub public_name:String,
    pub description:String,
    pub effect:String,
    pub input_type_url:String,
    pub output_type_url:String,
    pub runtime:RuntimeEvidence,
    pub state:PendingToolState,
    pub authorization_required:bool,
    pub executable_now:bool,
    pub resource_bound:bool,
    pub execution_instance_registered:bool,
    pub task_outcome_verified:bool,
}
/// Trusted desktop measurements, never a deserialized WebView or model input.
#[derive(Debug,Clone)]
pub struct OrdinaryRuntimeEvidence {
    pub workspace:RuntimeEvidence,
    pub git:RuntimeEvidence,
    pub browser:RuntimeEvidence,
    pub media:RuntimeEvidence,
    pub pdf:RuntimeEvidence,
    pub image:RuntimeEvidence,
    pub program_specs:Vec<crate::native_environment_discovery::HostProgramSpec>,
    pub allow_tools:bool,
}
fn project_pending(contracts:Vec<RegistrationContract>,runtime:RuntimeEvidence,
    policy_allowed:bool,out:&mut Vec<PendingCapabilityView>)->Result<(),CapabilityInventoryError>{
    for RegistrationContract{descriptor,definition} in contracts{
        ModelToolCatalog::validate_contract(&descriptor,&definition)
            .map_err(|_|CapabilityInventoryError::MissingNativeContract)?;
        let policy_allowed=policy_allowed && crate::privilege_guard::enforce_model_ceiling(descriptor.effect).is_ok()
            // ordinary Send only has NetworkRead approval; no browser interaction
            && !matches!(descriptor.effect,ToolEffect::NetworkWrite|ToolEffect::WriteWorkspace);
        let state=if !policy_allowed{PendingToolState::PolicyBlocked}
            else if matches!(runtime,RuntimeEvidence::Missing|RuntimeEvidence::ProbeUnavailable){PendingToolState::RuntimeUnavailable}
            else{PendingToolState::NeedsAuthorization};
        out.push(PendingCapabilityView{
            native_tool_id:descriptor.id,public_name:definition.name,description:definition.description,
            effect:effect_label(descriptor.effect).into(),input_type_url:descriptor.input_type_url,
            output_type_url:descriptor.output_type_url,runtime,state,authorization_required:true,
            executable_now:false,resource_bound:false,execution_instance_registered:false,task_outcome_verified:false,
        });
    }
    Ok(())
}
/// Describe scoped registrars that are actually reachable from ordinary Send.
/// No callable JSON schemas are returned and no execution instance is opened.
/// Capability selection, resource approval and live registration are distinct.
pub fn discover_ordinary_pending(e:&OrdinaryRuntimeEvidence)->Result<Vec<PendingCapabilityView>,CapabilityInventoryError>{
    use RuntimeEvidence::Builtin;
    let mut out=Vec::new();
    let mut workspace=crate::native_files::registration_contracts();
    workspace.extend(crate::native_file_extra::registration_contracts());
    workspace.push(crate::native_file_discovery::registration_contract());
    workspace.push(crate::native_reader_tool::registration_contract());
    workspace.push(crate::native_archive_inspect::registration_contract());
    workspace.push(crate::native_artifact_inspect::registration_contract());
    project_pending(workspace,e.workspace,e.allow_tools,&mut out)?;
    project_pending(crate::native_git_inspect::registration_contracts(),e.git,e.allow_tools,&mut out)?;
    project_pending(crate::native_browser_tool::registration_contracts().map_err(|_|CapabilityInventoryError::MissingNativeContract)?,e.browser,e.allow_tools,&mut out)?;
    project_pending(vec![crate::native_schema_validation::registration_contract()],Builtin,e.allow_tools,&mut out)?;
    // An environment query may inspect one fixed candidate even if that
    // executable is absent. Validate the same host inventory as registration.
    if !e.program_specs.is_empty(){
        let inventory=crate::native_environment_discovery::HostProgramInventory::new(e.program_specs.clone())
            .map_err(|_|CapabilityInventoryError::MissingNativeContract)?;
        let allowed=inventory.registration_ids();
        project_pending(vec![crate::native_environment_discovery::registration_contract(allowed)],Builtin,e.allow_tools,&mut out)?;
    }
    use crate::native_reviewed_app_tool::{DESKTOP_MEDIA_TOOL_ID,DESKTOP_PDF_TOOL_ID,DESKTOP_IMAGE_TOOL_ID};
    for contract in crate::native_reviewed_app_tool::ordinary_registration_contracts(){
        let runtime=match contract.descriptor.id.as_str(){
            DESKTOP_MEDIA_TOOL_ID=>e.media,DESKTOP_PDF_TOOL_ID=>e.pdf,DESKTOP_IMAGE_TOOL_ID=>e.image,
            _=>return Err(CapabilityInventoryError::MissingNativeContract),
        };
        project_pending(vec![contract],runtime,e.allow_tools,&mut out)?;
    }
    out.sort_by(|a,b|a.public_name.cmp(&b.public_name));
    if out.windows(2).any(|w|w[0].public_name==w[1].public_name){return Err(CapabilityInventoryError::MissingNativeContract)}
    Ok(out)
}

#[cfg(test)]
mod classification_tests{
    use super::*;
    #[test]
    fn capability_roles_are_reviewed_and_missing_app_is_not_misadvertised(){
        assert_eq!(reviewed_meta_class("lu.files.read_text"),Some("foundational"));
        assert_eq!(reviewed_meta_class("lu.artifact.inspect"),Some("foundational"));
        assert_eq!(reviewed_meta_class("lu.git.diff"),Some("utility"));
        assert_eq!(reviewed_meta_class("lu.archive.list"),Some("utility"));
        assert_eq!(reviewed_meta_class("unreviewed.app.run"),None);
        assert_eq!(reviewed_meta_class("lu.files.unreviewed_write"),None);
    }
}

#[cfg(test)]
mod pregrant_tests {
    use super::*;
    use crate::{native_environment_discovery::HostProgramSpec,
        conversation_authority::{AuthorizedCapability,TaskAuthority,TrustedUserTurnVerifier}};
    use std::{sync::atomic::{AtomicU64,Ordering},time::{Duration,SystemTime}};
    fn evidence()->OrdinaryRuntimeEvidence{
        OrdinaryRuntimeEvidence{
            workspace:RuntimeEvidence::Builtin,git:RuntimeEvidence::CandidatePresent,
            browser:RuntimeEvidence::Missing,media:RuntimeEvidence::Missing,
            pdf:RuntimeEvidence::ProbeUnavailable,image:RuntimeEvidence::Missing,
            program_specs:vec![HostProgramSpec{id:"git".into(),candidate_files:vec![
                if cfg!(windows){"C:\\Program Files\\Git\\cmd\\git.exe"}else{"/usr/bin/git"}.into()
            ]}],allow_tools:true,
        }
    }
    #[test]
    fn pregrant_discovery_is_inert_and_policy_and_runtime_truthful(){
        let mut e=evidence();
        let views=discover_ordinary_pending(&e).unwrap();
        assert_eq!(views.len(),18);
        assert!(views.iter().all(|v|v.authorization_required&&!v.executable_now&&
            !v.resource_bound&&!v.execution_instance_registered&&!v.task_outcome_verified));
        let find=|name:&str|views.iter().find(|v|v.public_name==name).unwrap();
        assert_eq!(find("lu_browser_read").state,PendingToolState::RuntimeUnavailable);
        assert_eq!(find("lu_browser_interact").state,PendingToolState::PolicyBlocked);
        assert_eq!(find("lu_git_status").runtime,RuntimeEvidence::CandidatePresent);
        assert_eq!(find("lu_validate_schema").state,PendingToolState::NeedsAuthorization);
        assert_eq!(find("lu_check_env").runtime,RuntimeEvidence::Builtin);
        let serialized=serde_json::to_string(&views).unwrap();
        assert!(!serialized.contains("/usr/bin/git")&&!serialized.contains("Program Files"));
        assert!(!serialized.contains("additionalProperties"));
        e.browser=RuntimeEvidence::PinnedPackagePresent;
        let next=discover_ordinary_pending(&e).unwrap();
        assert!(next.iter().any(|v|v.public_name=="lu_browser_read"&&v.state==PendingToolState::NeedsAuthorization));
        assert!(next.iter().any(|v|v.public_name=="lu_browser_interact"&&v.state==PendingToolState::PolicyBlocked));
        e.allow_tools=false;
        assert!(discover_ordinary_pending(&e).unwrap().iter().all(|v|v.state==PendingToolState::PolicyBlocked));
    }
    struct Human;
    impl TrustedUserTurnVerifier for Human {
        fn verify_user_turn(&self,c:&str,u:&str)->bool{c=="chat"&&u=="human"}
    }
    #[test]
    fn actual_registered_workspace_and_pending_names_types_agree(){
        static NEXT:AtomicU64=AtomicU64::new(0);
        let dir=std::env::temp_dir().join(format!("lu-cp05b-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
        std::fs::create_dir(&dir).unwrap();
        let pending=discover_ordinary_pending(&evidence()).unwrap();
        let session=crate::native_workspace_read_turn::WorkspaceReadTurnSession::new(&Human,"chat","human","task",&dir).unwrap();
        let registered=session.capabilities().unwrap();
        assert_eq!(registered.len(),8);
        for live in registered{
            let view=pending.iter().find(|v|v.public_name==live.public_name).unwrap();
            assert_eq!(view.native_tool_id,live.native_tool_id);
            assert_eq!(view.effect,live.effect);
            assert_eq!(view.input_type_url,live.input_type_url);
            assert_eq!(view.output_type_url,live.output_type_url);
            assert!(!view.executable_now);
        }
        drop(session);std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn actual_ungranted_registry_does_not_publish_callable_inventory(){
        let registry=NativeToolRegistry::new();
        let mut catalog=ModelToolCatalog::new();
        crate::native_schema_validation::register_schema_validation_tool(&registry,&mut catalog).unwrap();
        let authority=ConversationAuthority::new();
        let origin=BoundCallOrigin{conversation_id:"chat".into(),user_turn_id:"human".into(),task_id:"task".into()};
        assert!(discover_turn_capabilities(&registry,&catalog,&authority,&origin,true).is_err());
        let id=registry.tools().unwrap()[0].id.clone();
        authority.open_user_task(TaskAuthority{conversation_id:"chat".into(),user_turn_id:"human".into(),task_id:"task".into(),execution_scope_id:"task".into(),
            capabilities:vec![AuthorizedCapability{tool_id:id,effect:ToolEffect::ReadWorkspace}],expires_at:Some(SystemTime::now()+Duration::from_secs(60))},&Human).unwrap();
        let registered=discover_turn_capabilities(&registry,&catalog,&authority,&origin,true).unwrap();
        assert_eq!(registered.len(),1);
        // A grant paired with a stale catalog must not be reported as live.
        assert!(discover_turn_capabilities(&NativeToolRegistry::new(),&catalog,&authority,&origin,true).is_err());
        authority.revoke_task("task").unwrap();
        assert!(discover_turn_capabilities(&registry,&catalog,&authority,&origin,true).unwrap().is_empty());
    }
    #[test]
    fn mismatched_factory_codec_is_rejected_before_projection(){
        let mut contract=crate::native_schema_validation::registration_contract();
        contract.descriptor.input_type_url="type.googleapis.com/lu.v1.NotTheSame".into();
        assert_eq!(project_pending(vec![contract],RuntimeEvidence::Builtin,true,&mut Vec::new()),Err(CapabilityInventoryError::MissingNativeContract));
    }
}
