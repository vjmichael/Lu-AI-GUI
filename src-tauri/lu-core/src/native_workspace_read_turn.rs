//! TOOL-INTEGRATION-READ-01: a host-selected directory and one authenticated
//! human turn share a bounded, read-only multi-tool model session.
//!
//! This is a reusable Lu Core bridge, NOT a desktop opt-in UI. The native host
//! must choose the directory through a genuine user grant (never model text or
//! a WebView-supplied path) and supply a real persisted-user-turn verifier.
//! Tool registration is not itself authorization.
use std::{collections::BTreeSet, path::Path, sync::Arc, time::{Duration,SystemTime}};
use cap_std::fs::Dir;
#[cfg(test)]
use prost::Message;
use crate::{
    agent_tool_loop::{NativeToolTurn,ToolTurnError,ToolDispatchCheckpoint},
    contracts::{AuthorizationError,ToolAuthorizer,ToolDescriptor,ToolEffect,ToolInvocation,ProtoPayload},
    conversation_authority::{
        AuthorizedCapability,BoundCallOrigin,ConversationAuthority,TaskAuthority,
        TrustedUserTurnVerifier,
    },
    native_capability_inventory::{discover_turn_capabilities,NativeCapabilityView},
    inference_host::Cancellation,
    model::ModelToolCatalog,
    model_turn_journal::ModelTurnJournal,
    native_archive_inspect::{register_archive_inspection_tool,register_archive_inspection_tool_from_dir},
    native_artifact_inspect::{register_artifact_inspection,register_artifact_inspection_from_dir},
    native_file_extra::{register_inspection_tools,register_inspection_tools_from_dir},
    native_file_discovery::{register_discovery_tool,register_discovery_tool_from_dir},
    native_files::{register_bundled_file_tools,register_bundled_file_tools_from_dir},
    native_git_inspect::register_git_inspection_tools,
    native_reader_tool::{
        register_reader,register_reader_with_host_observer,register_reader_from_dir,ReaderSourceObserver
    },
    registry::NativeToolRegistry,
    supervised_tool_generation::{
        run_supervised_native_turn_journaled_observed,ModelStepGenerator,
        NativeGenerationError,NativeTurnReport,NativeTurnServices,
    },
};

const GRANT_SECONDS:u64=300;
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum WorkspaceReadError{
    InvalidScope,
    RootUnavailable,
    RegistrationFailed,
    AuthorityUnavailable,
    CatalogUnavailable,
}
/// Unlike a general permission, this policy is tied to exactly the host
/// installed read-only descriptors and their Protobuf codecs.
struct ReadOnlyPolicy { ids:BTreeSet<String>,fixed_input:Option<ProtoPayload> }
/// A native-host-only original-input read. No model path, literal, limits or
/// tool choice is accepted after approval. The existing registry, codec and
/// conversation authority still perform independent validation.
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum ExactWorkspaceRead {
    RootListing, ReadText(String), Stat(String), SearchText(String),
    Discover(String), ArchiveList(String), ArtifactInspect{path:String,format:String},
    GitStatus, GitLog{latest_only:bool}, GitDiff(Option<String>),
}
impl ExactWorkspaceRead {
    pub fn native_id(&self)->&'static str{self.contract().0}
    pub fn public_name(&self)->&'static str{self.contract().1}
    fn git(&self)->bool{matches!(self,Self::GitStatus|Self::GitLog{..}|Self::GitDiff(_))}
    fn contract(&self)->(&'static str,&'static str,serde_json::Value){
        use serde_json::json;
        match self{
            Self::RootListing=>("lu.files.list_directory","lu_files_list_directory",json!({})),
            Self::ReadText(p)=>("lu.files.read_text","lu_files_read_text",json!({"path":p})),
            Self::Stat(p)=>("lu.inspect.stat","lu_files_stat",json!({"path":p})),
            Self::SearchText(q)=>("lu.inspect.search_text","lu_files_search_text",json!({"query":q})),
            Self::Discover(q)=>("lu.file_discovery.discover","lu_files_discover",json!({"query":q,"kind":"file"})),
            Self::ArchiveList(p)=>("lu.archive.list","lu_archive_list",json!({"path":p})),
            Self::ArtifactInspect{path,format}=>("lu.artifact.inspect","lu_artifact_inspect",json!({"path":path,"format":format})),
            Self::GitStatus=>("lu.git.status","lu_git_status",json!({})),
            Self::GitLog{latest_only}=>("lu.git.log","lu_git_log",if *latest_only{json!({"limit":1})}else{json!({})}),
            Self::GitDiff(path)=>("lu.git.diff","lu_git_diff",match path{Some(p)=>json!({"path":p}),None=>json!({})}),
        }
    }
}
impl ToolAuthorizer for ReadOnlyPolicy{
    fn authorize(&self,call:&ToolInvocation,desc:&ToolDescriptor)
        ->Result<(),AuthorizationError>{
        if desc.effect!=ToolEffect::ReadWorkspace ||
            call.tool_id!=desc.id || !self.ids.contains(&desc.id) ||
            call.input.type_url!=desc.input_type_url{
            return Err(AuthorizationError::new("Not in the approved read-only workspace tool set"));
        }
        if self.fixed_input.as_ref().is_some_and(|expected|expected!=&call.input){
            return Err(AuthorizationError::new("Invocation differs from the original approved read input"));
        }
        Ok(())
    }
}
pub struct WorkspaceReadTurnSession {
    registry:NativeToolRegistry,
    catalog:ModelToolCatalog,
    authority:ConversationAuthority,
    turn:NativeToolTurn,
    policy:ReadOnlyPolicy,
    task_id:String,
    origin:BoundCallOrigin,
}
impl WorkspaceReadTurnSession{
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        verifier:&dyn TrustedUserTurnVerifier,
        conversation_id:&str,user_turn_id:&str,task_id:&str,
        host_approved_root:&Path,
    )->Result<Self,WorkspaceReadError>{
        Self::new_with_capability(verifier,conversation_id,user_turn_id,task_id,
            Some(host_approved_root),None,None,None,None)
    }
    /// P0-TOOLS-03: the trusted native desktop observes only completed real
    /// Reader source identity, bound to the SAME original human-selected
    /// workspace and authenticated turn. This is not a tool/OS grant.
    pub fn new_with_reader_observer(
        verifier:&dyn TrustedUserTurnVerifier,
        conversation_id:&str,user_turn_id:&str,task_id:&str,
        host_approved_root:&Path,observer:ReaderSourceObserver
    )->Result<Self,WorkspaceReadError>{
        Self::new_with_capability(
            verifier,conversation_id,user_turn_id,task_id,
            Some(host_approved_root),None,None,Some(observer),None
        )
    }
    /// Wave 4: read-only local Git from a HUMAN-picked repository directory
    /// and a host-selected trusted Git executable. Only three narrow Git
    /// schemas are advertised. No GitHub, shell, staging, commit or push.
    #[allow(clippy::too_many_arguments)]
    pub fn new_git(
        verifier:&dyn TrustedUserTurnVerifier,
        conversation_id:&str,user_turn_id:&str,task_id:&str,
        host_approved_repository:&Path,host_trusted_git:&Path,
    )->Result<Self,WorkspaceReadError>{
        Self::new_with_capability(verifier,conversation_id,user_turn_id,task_id,
            Some(host_approved_repository),None,Some(host_trusted_git),None,None)
    }
    /// Native host has already obtained and verified the directory identity.
    /// No pathname is accepted or reopened by the registered tools.
    pub fn new_with_approved_directory(
        verifier:&dyn TrustedUserTurnVerifier,
        conversation_id:&str,user_turn_id:&str,task_id:&str,
        directory:std::fs::File,observer:Option<ReaderSourceObserver>,
    )->Result<Self,WorkspaceReadError>{
        let meta=directory.metadata().map_err(|_|WorkspaceReadError::RootUnavailable)?;
        if !meta.is_dir(){return Err(WorkspaceReadError::RootUnavailable)}
        #[cfg(windows)]{
            use std::os::windows::fs::MetadataExt;
            if meta.file_attributes()&0x400!=0{return Err(WorkspaceReadError::RootUnavailable)}
        }
        Self::new_with_capability(verifier,conversation_id,user_turn_id,task_id,
            None,Some(Arc::new(Dir::from_std_file(directory))),None,observer,None)
    }
    /// Host-only exact root listing after the user chose a folder. The
    /// original input is bound in the existing codec; no model path is accepted.
    pub fn new_root_listing(verifier:&dyn TrustedUserTurnVerifier,conversation_id:&str,
        user_turn_id:&str,task_id:&str,root:&Path)->Result<Self,WorkspaceReadError>{
        Self::new_with_capability(verifier,conversation_id,user_turn_id,task_id,Some(root),None,None,None,Some(ExactWorkspaceRead::RootListing))
    }
    pub fn new_root_listing_with_approved_directory(verifier:&dyn TrustedUserTurnVerifier,conversation_id:&str,
        user_turn_id:&str,task_id:&str,directory:std::fs::File)->Result<Self,WorkspaceReadError>{
        let meta=directory.metadata().map_err(|_|WorkspaceReadError::RootUnavailable)?;
        if !meta.is_dir(){return Err(WorkspaceReadError::RootUnavailable)}
        #[cfg(windows)]{
            use std::os::windows::fs::MetadataExt;
            if meta.file_attributes()&0x400!=0{return Err(WorkspaceReadError::RootUnavailable)}
        }
        Self::new_with_capability(verifier,conversation_id,user_turn_id,task_id,None,Some(Arc::new(Dir::from_std_file(directory))),None,None,Some(ExactWorkspaceRead::RootListing))
    }
    /// One model-selected exact observation within the separately approved
    /// resource. Git and ordinary file lanes cannot be interchanged.
    pub fn new_exact(verifier:&dyn TrustedUserTurnVerifier,conversation_id:&str,user_turn_id:&str,task_id:&str,
        root:&Path,git:Option<&Path>,operation:ExactWorkspaceRead)->Result<Self,WorkspaceReadError>{
        if operation.git()!=git.is_some(){return Err(WorkspaceReadError::InvalidScope)}
        Self::new_with_capability(verifier,conversation_id,user_turn_id,task_id,Some(root),None,git,None,Some(operation))
    }
    pub fn new_exact_with_approved_directory(verifier:&dyn TrustedUserTurnVerifier,conversation_id:&str,user_turn_id:&str,task_id:&str,
        directory:std::fs::File,operation:ExactWorkspaceRead)->Result<Self,WorkspaceReadError>{
        if operation.git(){return Err(WorkspaceReadError::InvalidScope)}
        let meta=directory.metadata().map_err(|_|WorkspaceReadError::RootUnavailable)?;
        if !meta.is_dir(){return Err(WorkspaceReadError::RootUnavailable)}
        #[cfg(windows)]{
            use std::os::windows::fs::MetadataExt;
            if meta.file_attributes()&0x400!=0{return Err(WorkspaceReadError::RootUnavailable)}
        }
        Self::new_with_capability(verifier,conversation_id,user_turn_id,task_id,None,Some(Arc::new(Dir::from_std_file(directory))),None,None,Some(operation))
    }
    #[allow(clippy::too_many_arguments)]
    fn new_with_capability(
        verifier:&dyn TrustedUserTurnVerifier,
        conversation_id:&str,user_turn_id:&str,task_id:&str,
        host_approved_root:Option<&Path>,pinned_root:Option<Arc<Dir>>,
        git_executable:Option<&Path>,
        reader_observer:Option<ReaderSourceObserver>,exact:Option<ExactWorkspaceRead>,
    )->Result<Self,WorkspaceReadError>{
        if [conversation_id,user_turn_id,task_id].iter().any(|s|
             s.is_empty()||s.len()>128||
             !s.bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'-'|b'_'))
        ) || !verifier.verify_user_turn(conversation_id,user_turn_id){
            return Err(WorkspaceReadError::InvalidScope);
        }
        let root=if let Some(host_approved_root)=host_approved_root{
        let root=host_approved_root.canonicalize()
            .map_err(|_|WorkspaceReadError::RootUnavailable)?;
        if !root.is_dir(){return Err(WorkspaceReadError::RootUnavailable)}
        let meta=std::fs::symlink_metadata(host_approved_root)
            .map_err(|_|WorkspaceReadError::RootUnavailable)?;
        if meta.file_type().is_symlink() || !meta.is_dir(){
            return Err(WorkspaceReadError::RootUnavailable);
        }
        Some(root)
        }else{None};
        let registry=NativeToolRegistry::new();
        let mut catalog=if let Some(directory)=pinned_root{
            // Every registrar receives clones of the SAME capability. No
            // model-supplied root, ambient reopen, or path-keyed cache.
            let failed=|e:String|{eprintln!("Pinned workspace registration failed: {e}");WorkspaceReadError::RegistrationFailed};
            let mut catalog=register_bundled_file_tools_from_dir(&registry,Arc::clone(&directory))
                .map_err(|e|failed(format!("{e:?}")))?;
            register_inspection_tools_from_dir(&registry,&mut catalog,Arc::clone(&directory)).map_err(failed)?;
            register_discovery_tool_from_dir(&registry,&mut catalog,Arc::clone(&directory)).map_err(failed)?;
            let reader=register_reader_from_dir(&registry,Arc::clone(&directory),reader_observer).map_err(failed)?;
            catalog.absorb_registered(&registry,reader).map_err(|_|WorkspaceReadError::CatalogUnavailable)?;
            register_archive_inspection_tool_from_dir(&registry,&mut catalog,Arc::clone(&directory)).map_err(failed)?;
            register_artifact_inspection_from_dir(&registry,&mut catalog,directory).map_err(failed)?;
            catalog
        }else if let Some(executable)=git_executable{
            let root=root.ok_or(WorkspaceReadError::RootUnavailable)?;
            let mut catalog=ModelToolCatalog::new();
            register_git_inspection_tools(&registry,&mut catalog,&root,executable)
                .map_err(|e|{eprintln!("Workspace read tool registration failed: {e:?}");WorkspaceReadError::RegistrationFailed})?;
            catalog
        }else{
            let root=root.ok_or(WorkspaceReadError::RootUnavailable)?;
            let mut catalog=register_bundled_file_tools(&registry,&root)
                .map_err(|e|{eprintln!("Workspace read tool registration failed: {e:?}");WorkspaceReadError::RegistrationFailed})?;
            register_inspection_tools(&registry,&mut catalog,&root)
                .map_err(|e|{eprintln!("Workspace read tool registration failed: {e:?}");WorkspaceReadError::RegistrationFailed})?;
            // P01: same human-picked root and per-turn read authorization;
            // only a reusable metadata cache is shared between sessions.
            register_discovery_tool(&registry,&mut catalog,&root)
                .map_err(|e|{eprintln!("Workspace read tool registration failed: {e:?}");WorkspaceReadError::RegistrationFailed})?;
            // P0-TOOLS-01: the same *native-picked* directory and the same
            // persisted user-turn grant now expose the genuine Reader parser.
            // A model can discover a relative file and inspect its PDF text,
            // DOCX/ODT, spreadsheets, slides, EPUB or plain text within this
            // one read-only turn. The Reader's own Protobuf codec, 8 MiB cap,
            // source SHA and page/selector checks still apply. A new schema
            // is not an ambient pathname grant, and scanned PDFs need OCR.
            let reader=match reader_observer{
                Some(observe)=>register_reader_with_host_observer(
                    &registry,&root,observe
                ),
                None=>register_reader(&registry,&root),
            }.map_err(|e|{eprintln!(
                "Workspace Reader registration failed: {e:?}"
            );WorkspaceReadError::RegistrationFailed})?;
            catalog.absorb_registered(&registry,reader)
                .map_err(|e|{eprintln!("Workspace Reader schema composition failed: {e:?}");WorkspaceReadError::CatalogUnavailable})?;
            register_archive_inspection_tool(&registry,&mut catalog,&root)
                .map_err(|e|{eprintln!("Workspace read tool registration failed: {e:?}");WorkspaceReadError::RegistrationFailed})?;
            // F: a completed file is not automatically a valid artifact.
            // Inspect only the approved read-only workspace; never launch an app.
            register_artifact_inspection(&registry,&mut catalog,&root)
                .map_err(|e|{eprintln!("Workspace artifact registration failed: {e:?}");WorkspaceReadError::RegistrationFailed})?;
            catalog
        };
        let mut descriptors=registry.tools().map_err(|e|{eprintln!("Workspace read tool registration failed: {e:?}");WorkspaceReadError::RegistrationFailed})?;
        if descriptors.is_empty() || descriptors.iter().any(|d|d.effect!=ToolEffect::ReadWorkspace){
            return Err(WorkspaceReadError::RegistrationFailed);
        }
        let fixed_input=if let Some(operation)=exact{
            let (id,name,args)=operation.contract();
            descriptors.retain(|d|d.id==id);
            if descriptors.len()!=1{return Err(WorkspaceReadError::RegistrationFailed)}
            let ids=BTreeSet::from([id.to_owned()]);
            let names=catalog.names_for_native_ids(&ids);
            if names!=BTreeSet::from([name.to_owned()]){return Err(WorkspaceReadError::CatalogUnavailable)}
            let original=catalog.encode_call(&names,name,"host-exact-read",Some(task_id),&args.to_string())
                .map_err(|_|WorkspaceReadError::CatalogUnavailable)?.input;
            catalog.bind_current_input(&registry,name,args.as_object().ok_or(WorkspaceReadError::CatalogUnavailable)?.clone())
                .map_err(|_|WorkspaceReadError::CatalogUnavailable)?;
            Some(original)
        }else{None};
        let ids=descriptors.iter().map(|d|d.id.clone()).collect::<BTreeSet<_>>();
        let names=catalog.names_for_native_ids(&ids);
        let authority=ConversationAuthority::new();
        let origin=BoundCallOrigin{
            conversation_id:conversation_id.into(),
            user_turn_id:user_turn_id.into(),task_id:task_id.into(),
        };
        let expires_at=SystemTime::now().checked_add(Duration::from_secs(GRANT_SECONDS))
            .ok_or(WorkspaceReadError::AuthorityUnavailable)?;
        authority.open_user_task(TaskAuthority {
            conversation_id:conversation_id.into(),user_turn_id:user_turn_id.into(),
            task_id:task_id.into(),execution_scope_id:task_id.into(),
            capabilities:descriptors.iter().map(|d| AuthorizedCapability {
                tool_id:d.id.clone(),effect:d.effect,
            }).collect(),
            expires_at:Some(expires_at),
        },verifier).map_err(|_|WorkspaceReadError::AuthorityUnavailable)?;
        let mut turn=NativeToolTurn::new_read_only(
            &catalog,&registry,&authority,origin.clone(),task_id.into(),&names
        ).map_err(|_:ToolTurnError|WorkspaceReadError::CatalogUnavailable)?;
        // A dispatched error is not a successful observation. The host must
        // never publish a factual answer or seed source reuse from a failed read.
        if fixed_input.is_some(){turn.require_single_observed_tool_result();}else{turn.require_observed_tool_result();}
        Ok(Self {
            registry,catalog,authority,turn,
            policy:ReadOnlyPolicy{ids,fixed_input},task_id:task_id.into(),
            origin,
        })
    }
    /// Safe names/types for the actual just-authorized read-only turn.
    /// No model-visible tool name is independently treated as permission.
    pub fn capabilities(&self)->Result<Vec<NativeCapabilityView>,WorkspaceReadError>{
        discover_turn_capabilities(
            &self.registry,&self.catalog,&self.authority,&self.origin,true,
        ).map_err(|_|WorkspaceReadError::CatalogUnavailable)
    }
    pub fn schemas(&self)->Result<Vec<serde_json::Value>,WorkspaceReadError>{
        self.turn.schemas(&self.catalog).map_err(|_|WorkspaceReadError::CatalogUnavailable)
    }
    /// The host owns actual GGUF model step generation and cancellation.
    /// Every completed/failing turn revokes its ephemeral authorization.
    pub async fn run_journaled(
        &mut self,prompt:&str,model:&mut dyn ModelStepGenerator,
        cancel:&Cancellation,journal:&ModelTurnJournal,
    )->Result<NativeTurnReport,NativeGenerationError>{
        self.run_journaled_observed(prompt,model,cancel,journal,&|_|{}).await
    }
    /// Host-only content-free dispatch checkpoints. Never send model
    /// arguments, source bytes, private filenames or effect grant IDs.
    pub async fn run_journaled_observed(
        &mut self,prompt:&str,model:&mut dyn ModelStepGenerator,
        cancel:&Cancellation,journal:&ModelTurnJournal,
        observe:&(dyn Fn(ToolDispatchCheckpoint)+Send+Sync),
    )->Result<NativeTurnReport,NativeGenerationError>{
        let services=NativeTurnServices{
            catalog:&self.catalog,registry:&self.registry,
            authority:&self.authority,host_policy:&self.policy,
        };
        let result=run_supervised_native_turn_journaled_observed(
            prompt,&mut self.turn,model,&services,cancel,journal,observe
        ).await;
        let _=self.authority.revoke_task(&self.task_id);
        result
    }
}
impl Drop for WorkspaceReadTurnSession{
    fn drop(&mut self){let _=self.authority.revoke_task(&self.task_id);}
}

#[cfg(test)]
mod integrated_reader_tests{
    use super::*;
    use std::{collections::VecDeque,path::PathBuf};
    struct OriginalTurn;
    impl TrustedUserTurnVerifier for OriginalTurn{
        fn verify_user_turn(&self,chat:&str,turn:&str)->bool{
            chat=="c-source"&&turn=="g-human"
        }
    }
    struct Scripted{replies:VecDeque<String>}
    impl ModelStepGenerator for Scripted{
        fn generate_step(&mut self,_prompt:&str,_tokens:u32,
            _cancel:&Cancellation)->Result<String,NativeGenerationError>{
            self.replies.pop_front().ok_or(NativeGenerationError::MissingCompletion)
        }
    }
    fn directory()->PathBuf{
        let name=format!("lu-p0-reader-{}-{:?}",std::process::id(),
            std::thread::current().id());
        std::env::temp_dir().join(name)
    }
    #[tokio::test]
    async fn cp_dev_01d_root_listing_only_binds_approved_root(){
        let base=directory();let _=std::fs::remove_dir_all(&base);let root=base.join("approved");
        std::fs::create_dir_all(root.join("child")).unwrap();
        std::fs::write(root.join("original.txt"),b"source").unwrap();
        std::fs::write(root.join("child/hidden.txt"),b"not a root member").unwrap();
        let mut lane=WorkspaceReadTurnSession::new_root_listing(&OriginalTurn,"c-source","g-human","task-list",&root).unwrap();
        let caps=lane.capabilities().unwrap();assert_eq!(caps.len(),1);assert_eq!(caps[0].public_name,"lu_files_list_directory");
        let allowed=BTreeSet::from(["lu_files_list_directory".to_owned()]);
        let encode=|raw|lane.catalog.encode_call(&allowed,"lu_files_list_directory","call",Some("task-list"),raw);
        assert!(encode(r#"{"source":"current_request","path":"child"}"#).is_err());
        assert!(encode(r#"{"path":"child"}"#).is_err());
        assert!(encode(r#"{"source":"other_request"}"#).is_err());
        let call=encode(r#"{"source":"current_request"}"#).unwrap();
        assert_eq!(crate::wire::proto::ListDirectoryRequest::decode(call.input.bytes.as_slice()).unwrap().path,"");
        let journal=ModelTurnJournal::open(&base.join("audit.sqlite")).unwrap();
        let mut model=Scripted{replies:VecDeque::from([
            r#"{"kind":"tool","name":"lu_files_list_directory","arguments":{"source":"current_request"}}"#.into(),
            r#"{"kind":"final","text":"original.txt and child"}"#.into(),
        ])};
        let result=lane.run_journaled("List files in my folder",&mut model,&Cancellation::default(),&journal).await.unwrap();
        let events=journal.events_for("c-source","task-list").unwrap();
        let feedback=events.iter().filter(|e|e.kind==crate::model_turn_journal::ModelAuditKind::ToolFeedback).collect::<Vec<_>>();
        assert_eq!(result.tool_calls,1);assert_eq!(feedback.len(),1);
        assert!(feedback[0].payload.contains("original.txt")&&feedback[0].payload.contains("child")&&!feedback[0].payload.contains("hidden.txt"));
        drop(lane);drop(journal);std::fs::remove_dir_all(base).unwrap();
    }
    #[tokio::test]
    async fn cp06br1_exact_stat_denies_other_tool_path_and_payload(){
        let base=directory();let _=std::fs::remove_dir_all(&base);let root=base.join("approved");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("Notes.TXT"),b"original").unwrap();
        std::fs::write(root.join("other.txt"),b"wrong").unwrap();
        let mut lane=WorkspaceReadTurnSession::new_exact(&OriginalTurn,"c-source","g-human","task-stat",&root,None,ExactWorkspaceRead::Stat("Notes.TXT".into())).unwrap();
        let caps=lane.capabilities().unwrap();assert_eq!(caps.len(),1);assert_eq!(caps[0].public_name,"lu_files_stat");
        let allowed=BTreeSet::from(["lu_files_stat".to_owned()]);
        let encode=|raw|lane.catalog.encode_call(&allowed,"lu_files_stat","call",Some("task-stat"),raw);
        assert!(encode(r#"{"source":"current_request","path":"other.txt"}"#).is_err());
        assert!(encode(r#"{"path":"other.txt"}"#).is_err());
        assert!(encode(r#"{"source":"other_request"}"#).is_err());
        assert!(lane.catalog.encode_call(&allowed,"lu_reader_read","call",Some("task-stat"),r#"{"path":"other.txt"}"#).is_err());
        let mut call=encode(r#"{"source":"current_request"}"#).unwrap();
        assert_eq!(crate::wire::proto::FileStatRequest::decode(call.input.bytes.as_slice()).unwrap().path,"Notes.TXT");
        let descriptor=lane.registry.tools().unwrap().into_iter().find(|d|d.id=="lu.inspect.stat").unwrap();
        assert!(lane.policy.authorize(&call,&descriptor).is_ok());
        call.input.bytes=crate::wire::proto::FileStatRequest{path:"other.txt".into()}.encode_to_vec();
        assert!(lane.policy.authorize(&call,&descriptor).is_err());
        let journal=ModelTurnJournal::open(&base.join("audit.sqlite")).unwrap();
        let mut model=Scripted{replies:VecDeque::from([
            r#"{"kind":"tool","name":"lu_files_stat","arguments":{"source":"current_request"}}"#.into(),
            r#"{"kind":"final","text":"8 bytes"}"#.into(),
        ])};
        let result=lane.run_journaled("What is the byte size of Notes.TXT?",&mut model,&Cancellation::default(),&journal).await.unwrap();
        assert_eq!(result.tool_calls,1);
        let events=journal.events_for("c-source","task-stat").unwrap();
        let feedback=events.iter().filter(|e|e.kind==crate::model_turn_journal::ModelAuditKind::ToolFeedback).collect::<Vec<_>>();
        assert_eq!(feedback.len(),1);assert!(feedback[0].payload.contains("\"size_bytes\":8"));
        drop(lane);drop(journal);std::fs::remove_dir_all(base).unwrap();
    }
    #[tokio::test]
    async fn one_original_turn_discovers_then_reads_document_through_real_codecs(){
        let root=directory();let _=std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("reports")).unwrap();
        std::fs::write(root.join("reports/report.md"),
            "# Project facts\nThe original local report is source data, not tool authority.").unwrap();
        let audit=root.join("turn.sqlite");
        let mut turn=WorkspaceReadTurnSession::new(
            &OriginalTurn,"c-source","g-human","task-read",&root).unwrap();
        let names=turn.capabilities().unwrap().into_iter()
            .map(|t|t.public_name).collect::<BTreeSet<_>>();
        assert!(names.contains("lu_files_discover"));
        assert!(names.contains("lu_reader_read"));
        assert!(!names.contains("lu_git_status"));
        let journal=ModelTurnJournal::open(&audit).unwrap();
        let mut model=Scripted{replies:VecDeque::from(vec![
            r#"{"kind":"tool","name":"lu_files_discover","arguments":{"query":"report.md"}}"#.into(),
            r#"{"kind":"tool","name":"lu_reader_read","arguments":{"path":"reports/report.md","max_chars":1024}}"#.into(),
            r#"{"kind":"final","text":"Read the original workspace report."}"#.into()
        ])};
        let result=turn.run_journaled(
            "Find and read the report in my project workspace",
            &mut model,&Cancellation::default(),&journal).await.unwrap();
        assert_eq!(result.tool_calls,2);
        assert_eq!(result.answer,"Read the original workspace report.");
        let origin=BoundCallOrigin{
            conversation_id:"c-source".into(),user_turn_id:"g-human".into(),
            task_id:"task-read".into()
        };
        assert!(turn.authority.permitted_tool_ids(&origin).unwrap().is_empty());
        drop(turn);drop(journal);
        let _=std::fs::remove_dir_all(root);
    }
    #[cfg(target_os="linux")]
    #[tokio::test]
    async fn cp03_all_read_registrars_keep_approved_directory_identity(){
        use std::io::Write;
        use crate::supervised_tool_generation::NativeTurnRecord;
        let base=directory();let _=std::fs::remove_dir_all(&base);
        let root=base.join("approved");let parked=base.join("parked");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("original.txt"),b"approved-only marker").unwrap();
        let mut zip=zip::ZipWriter::new(std::fs::File::create(root.join("source.zip")).unwrap());
        zip.start_file("approved-member.txt",zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(b"approved ZIP").unwrap();zip.finish().unwrap();
        let handle=std::fs::File::open(&root).unwrap();
        // Simulate substitution in the narrow interval after verification,
        // before tool registration. Registrars must not reopen by name.
        std::fs::rename(&root,&parked).unwrap();std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("original.txt"),b"hostile replacement").unwrap();
        std::fs::write(root.join("hostile-only.txt"),b"hostile replacement").unwrap();
        let mut zip=zip::ZipWriter::new(std::fs::File::create(root.join("source.zip")).unwrap());
        zip.start_file("hostile-member.txt",zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(b"hostile ZIP").unwrap();zip.finish().unwrap();
        let mut turn=WorkspaceReadTurnSession::new_with_approved_directory(
            &OriginalTurn,"c-source","g-human","task-read",handle,None).unwrap();
        let names=turn.capabilities().unwrap().into_iter().map(|t|t.public_name).collect::<BTreeSet<_>>();
        assert_eq!(names.len(),8);assert!(!names.contains("lu_git_status"));
        let journal=ModelTurnJournal::open(&base.join("turn.sqlite")).unwrap();
        let mut model=Scripted{replies:VecDeque::from(vec![
            r#"{"kind":"tool","name":"lu_files_list_directory","arguments":{}}"#.into(),
            r#"{"kind":"tool","name":"lu_files_read_text","arguments":{"path":"original.txt"}}"#.into(),
            r#"{"kind":"tool","name":"lu_files_stat","arguments":{"path":"original.txt"}}"#.into(),
            r#"{"kind":"tool","name":"lu_files_search_text","arguments":{"query":"approved-only"}}"#.into(),
            r#"{"kind":"tool","name":"lu_files_discover","arguments":{"query":"original.txt"}}"#.into(),
            r#"{"kind":"tool","name":"lu_reader_read","arguments":{"path":"original.txt"}}"#.into(),
            r#"{"kind":"tool","name":"lu_archive_list","arguments":{"path":"source.zip"}}"#.into(),
            r#"{"kind":"tool","name":"lu_artifact_inspect","arguments":{"path":"original.txt","format":"text"}}"#.into(),
            r#"{"kind":"final","text":"verified"}"#.into()
        ])};
        let result=turn.run_journaled("Inspect approved folder",&mut model,&Cancellation::default(),&journal).await.unwrap();
        let feedback=result.records.iter().filter_map(|r|match r{NativeTurnRecord::ToolFeedback(value)=>Some(value),_=>None}).collect::<Vec<_>>();
        assert_eq!(feedback.len(),8,"{feedback:?}");
        for value in &feedback{assert_eq!(value["response"]["ok"],true,"{value}");assert!(!value.to_string().contains("hostile"),"{value}");}
        assert!(feedback[0].to_string().contains("original.txt"));
        assert!(feedback[1].to_string().contains("approved-only marker"));
        assert_eq!(feedback[2]["response"]["result"]["size_bytes"],20);
        assert!(feedback[3].to_string().contains("approved-only"));
        assert!(feedback[4].to_string().contains("original.txt"));
        assert!(feedback[5].to_string().contains("approved-only marker"));
        assert!(feedback[6].to_string().contains("approved-member.txt"));
        use sha2::{Sha256,Digest};
        assert_eq!(feedback[7]["response"]["result"]["sha256"],format!("{:x}",Sha256::digest(b"approved-only marker")));
        assert!(turn.authority.permitted_tool_ids(&turn.origin).unwrap().is_empty());
        drop(turn);drop(journal);std::fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn source_document_reader_cannot_be_constructed_from_fabricated_user_turn(){
        let root=directory();let _=std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        assert!(matches!(WorkspaceReadTurnSession::new(
            &OriginalTurn,"c-source","g-false","task-read",&root),
            Err(WorkspaceReadError::InvalidScope)));
        assert!(matches!(WorkspaceReadTurnSession::new(
            &OriginalTurn,"c-false","g-human","task-read",&root),
            Err(WorkspaceReadError::InvalidScope)));
        let _=std::fs::remove_dir_all(root);
    }
}
