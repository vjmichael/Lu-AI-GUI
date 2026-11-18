//! TOOL-INTEGRATION-READ-01: a host-selected directory and one authenticated
//! human turn share a bounded, read-only multi-tool model session.
//!
//! This is a reusable Lu Core bridge, NOT a desktop opt-in UI. The native host
//! must choose the directory through a genuine user grant (never model text or
//! a WebView-supplied path) and supply a real persisted-user-turn verifier.
//! Tool registration is not itself authorization.
use std::{collections::BTreeSet, path::Path, time::{Duration,SystemTime}};
use crate::{
    agent_tool_loop::{NativeToolTurn,ToolTurnError,ToolDispatchCheckpoint},
    contracts::{AuthorizationError,ToolAuthorizer,ToolDescriptor,ToolEffect,ToolInvocation},
    conversation_authority::{
        AuthorizedCapability,BoundCallOrigin,ConversationAuthority,TaskAuthority,
        TrustedUserTurnVerifier,
    },
    native_capability_inventory::{discover_turn_capabilities,NativeCapabilityView},
    inference_host::Cancellation,
    model::ModelToolCatalog,
    model_turn_journal::ModelTurnJournal,
    native_archive_inspect::register_archive_inspection_tool,
    native_artifact_inspect::register_artifact_inspection,
    native_file_extra::register_inspection_tools,
    native_file_discovery::register_discovery_tool,
    native_files::register_bundled_file_tools,
    native_git_inspect::register_git_inspection_tools,
    native_reader_tool::{
        register_reader,register_reader_with_host_observer,ReaderSourceObserver
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
struct ReadOnlyPolicy { ids:BTreeSet<String> }
impl ToolAuthorizer for ReadOnlyPolicy{
    fn authorize(&self,call:&ToolInvocation,desc:&ToolDescriptor)
        ->Result<(),AuthorizationError>{
        if desc.effect!=ToolEffect::ReadWorkspace ||
            call.tool_id!=desc.id || !self.ids.contains(&desc.id) ||
            call.input.type_url!=desc.input_type_url{
            return Err(AuthorizationError::new("Not in the approved read-only workspace tool set"));
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
            host_approved_root,None,None)
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
            host_approved_root,None,Some(observer)
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
            host_approved_repository,Some(host_trusted_git),None)
    }
    #[allow(clippy::too_many_arguments)]
    fn new_with_capability(
        verifier:&dyn TrustedUserTurnVerifier,
        conversation_id:&str,user_turn_id:&str,task_id:&str,
        host_approved_root:&Path,git_executable:Option<&Path>,
        reader_observer:Option<ReaderSourceObserver>,
    )->Result<Self,WorkspaceReadError>{
        if [conversation_id,user_turn_id,task_id].iter().any(|s|
             s.is_empty()||s.len()>128||
             !s.bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'-'|b'_'))
        ) || !verifier.verify_user_turn(conversation_id,user_turn_id){
            return Err(WorkspaceReadError::InvalidScope);
        }
        let root=host_approved_root.canonicalize()
            .map_err(|_|WorkspaceReadError::RootUnavailable)?;
        if !root.is_dir(){return Err(WorkspaceReadError::RootUnavailable)}
        let meta=std::fs::symlink_metadata(host_approved_root)
            .map_err(|_|WorkspaceReadError::RootUnavailable)?;
        if meta.file_type().is_symlink() || !meta.is_dir(){
            return Err(WorkspaceReadError::RootUnavailable);
        }
        let registry=NativeToolRegistry::new();
        let mut catalog=if let Some(executable)=git_executable{
            let mut catalog=ModelToolCatalog::new();
            register_git_inspection_tools(&registry,&mut catalog,&root,executable)
                .map_err(|e|{eprintln!("Workspace read tool registration failed: {e:?}");WorkspaceReadError::RegistrationFailed})?;
            catalog
        }else{
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
        let descriptors=registry.tools().map_err(|e|{eprintln!("Workspace read tool registration failed: {e:?}");WorkspaceReadError::RegistrationFailed})?;
        if descriptors.is_empty() || descriptors.iter().any(|d|d.effect!=ToolEffect::ReadWorkspace){
            return Err(WorkspaceReadError::RegistrationFailed);
        }
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
        let turn=NativeToolTurn::new_read_only(
            &catalog,&registry,&authority,origin.clone(),task_id.into(),&names
        ).map_err(|_:ToolTurnError|WorkspaceReadError::CatalogUnavailable)?;
        Ok(Self {
            registry,catalog,authority,turn,
            policy:ReadOnlyPolicy{ids},task_id:task_id.into(),
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
