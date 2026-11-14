//! E1-P02: exact, host-enrolled installed-application discovery.
//!
//! An application is visible ONLY if the current, persisted USER turn has
//! the exact read-only tool permission and the actual OS executable still
//! matches the trusted release's SHA-256. This is an on-demand catalogue of
//! *reviewed candidates*, NOT a PATH scan, executable runner, application
//! installer, version-command launcher or model-made trust registry.
//!
//! Version verification here means an exact executable digest matches the
//! host's reviewed release-version record. It does not assert that a runtime
//! API, PE VERSIONINFO or --version command independently reported the
//! version. This distinction is intentionally a machine-readable property.
use std::collections::{BTreeMap,BTreeSet};
use crate::{
    contracts::{ToolAuthorizer,ToolDescriptor,ToolEffect,ToolInvocation},
    conversation_authority::{BoundCallOrigin,ConversationAuthority},
    native_app_admission::{
        inspect_host_reviewed_read_application,AdmissionError,
        HostReviewedExecutable,
    },
};

pub const MAX_REVIEWED_APPLICATIONS:usize=64;
pub const MAX_SCOPED_DISCOVERIES:usize=64;

#[derive(Debug,Clone,PartialEq,Eq)]
pub enum CatalogError{
    InvalidHostRecord,
    DuplicateApplication,
    DuplicateToolIdentity,
    CapacityExceeded,
    UnknownApplication,
    WrongRequestedToolOrScope,
    UnavailableOrNotAuthorized,
}

#[derive(Debug,Clone,PartialEq,Eq)]
pub struct ReviewedApplicationAvailability{
    pub app_id:String,
    /// Host-reviewed version label bound to reviewed_sha256; not a runtime
    /// app version response, a signed publisher certificate or OS permission.
    pub reviewed_release_version:String,
    pub version_basis:&'static str,
    pub independently_queried_runtime_version:bool,
    pub native_tool_id:String,
    pub executable_format:String,
    pub reviewed_sha256:String,
    pub observed_bytes:u64,
    /// On Windows, a 128-bit native file ID plus volume ID; on Unix dev+ino.
    /// These are opaque OS observations, not path or authorization tokens.
    pub observed_os_file_identity:String,
    pub executable_available_at_preflight:bool,
    pub process_started:bool,
    pub output_verified:bool,
}

#[derive(Debug,Default)]
pub struct HostReviewedApplicationCatalog{
    reviewed:BTreeMap<String,HostReviewedExecutable>,
    tool_id_to_app_id:BTreeMap<String,String>,
}
fn identifier(value:&str)->bool{
    !value.is_empty()&&value.len()<=128&&
    value.bytes().all(|b|
        b.is_ascii_alphanumeric()||matches!(b,b'.'|b'_'|b'-'))
}
fn version(value:&str)->bool{
    !value.is_empty()&&value.len()<=64&&
    value.bytes().all(|b|
        b.is_ascii_alphanumeric()||matches!(b,b'.'|b'_'|b'-'|b'+'))
}
fn digest(value:&str)->bool{
    value.len()==64&&value.bytes().all(|b|b.is_ascii_hexdigit())
}
fn valid_record(entry:&HostReviewedExecutable)->bool{
    identifier(&entry.app_id)
        &&identifier(&entry.workspace_id)
        &&identifier(&entry.tool_id)
        &&version(&entry.app_version)
        &&digest(&entry.expected_sha256)
        &&entry.executable.is_absolute()
}
impl HostReviewedApplicationCatalog{
    pub fn new()->Self{Self::default()}

    /// Trusted host enrollment only. Never call from model/tool-result text,
    /// browser data, chat memory, or an unsigned plugin manifest.
    ///
    /// Registration does NOT stat, execute, or declare availability.
    /// The actual on-disk binary must pass read-only inspection each turn.
    pub fn register_reviewed(
        &mut self,entry:HostReviewedExecutable
    )->Result<(),CatalogError>{
        if !valid_record(&entry){return Err(CatalogError::InvalidHostRecord)}
        if self.reviewed.contains_key(&entry.app_id){
            return Err(CatalogError::DuplicateApplication)
        }
        if self.tool_id_to_app_id.contains_key(&entry.tool_id){
            return Err(CatalogError::DuplicateToolIdentity)
        }
        if self.reviewed.len()>=MAX_REVIEWED_APPLICATIONS{
            return Err(CatalogError::CapacityExceeded)
        }
        self.tool_id_to_app_id
            .insert(entry.tool_id.clone(),entry.app_id.clone());
        self.reviewed.insert(entry.app_id.clone(),entry);
        Ok(())
    }

    /// Host-only removal of a reviewed app record. This does NOT revoke any
    /// other task grants: the owner must also revoke their task separately.
    /// An old cached caller cannot use this catalog as execution authority.
    pub fn delist_reviewed(&mut self,app_id:&str)->Result<(),CatalogError>{
        let entry=self.reviewed.remove(app_id)
            .ok_or(CatalogError::UnknownApplication)?;
        self.tool_id_to_app_id.remove(&entry.tool_id);
        Ok(())
    }

    /// Bounded inventory of genuine, current, scoped and byte-verified
    /// applications. Entries with missing binaries, wrong digests, stale
    /// records or absent task authority do NOT appear in model-facing lists.
    /// Only trusted host-provided invocation+descriptor pairs are accepted;
    /// this method itself cannot create the task grant or discover tools.
    pub fn discover_for_current_task(
        &self,origin:&BoundCallOrigin,
        host_exposed_tools:&[(ToolInvocation,ToolDescriptor)],
        authority:&ConversationAuthority,
        host_policy:&dyn ToolAuthorizer,
    )->Vec<ReviewedApplicationAvailability>{
        if host_exposed_tools.len()>MAX_SCOPED_DISCOVERIES{
            return Vec::new()
        }
        let mut observed=Vec::new();
        let mut seen=BTreeSet::new();
        for (call,descriptor) in host_exposed_tools{
            if !seen.insert(descriptor.id.as_str()){continue}
            if descriptor.id!=call.tool_id||descriptor.effect!=ToolEffect::ReadWorkspace{
                continue
            }
            let Some(app_id)=self.tool_id_to_app_id.get(&descriptor.id) else{
                continue
            };
            if let Ok(available)=self.observe_one(
                app_id,origin,call,descriptor,authority,host_policy
            ){
                observed.push(available);
            }
        }
        observed.sort_by(|a,b|a.app_id.cmp(&b.app_id));
        observed
    }

    /// Host-facing diagnostic for a specific reviewed app. The caller MUST
    /// not pass errors or the enrollment record into model context because
    /// denied user turns must not learn local filesystem availability.
    pub fn observe_one(
        &self,app_id:&str,origin:&BoundCallOrigin,
        call:&ToolInvocation,descriptor:&ToolDescriptor,
        authority:&ConversationAuthority,host_policy:&dyn ToolAuthorizer,
    )->Result<ReviewedApplicationAvailability,CatalogError>{
        let entry=self.reviewed.get(app_id)
            .ok_or(CatalogError::UnknownApplication)?;
        if descriptor.id!=entry.tool_id||call.tool_id!=entry.tool_id||
            descriptor.effect!=ToolEffect::ReadWorkspace||
            call.workspace_id.as_deref()!=Some(entry.workspace_id.as_str()){
            return Err(CatalogError::WrongRequestedToolOrScope)
        }
        let read=inspect_host_reviewed_read_application(
            entry,origin,call,descriptor,authority,host_policy
        ).map_err(|_error:AdmissionError|CatalogError::UnavailableOrNotAuthorized)?;

        Ok(ReviewedApplicationAvailability{
            app_id:entry.app_id.clone(),
            reviewed_release_version:entry.app_version.clone(),
            version_basis:"host_reviewed_version_bound_to_matching_binary_sha256",
            independently_queried_runtime_version:false,
            native_tool_id:entry.tool_id.clone(),
            executable_format:read.executable_format,
            reviewed_sha256:read.reviewed_sha256,
            observed_bytes:read.observed_bytes,
            observed_os_file_identity:read.opened_os_file_identity,
            executable_available_at_preflight:true,
            process_started:false,
            output_verified:false,
        })
    }

    /// The trusted execution broker must resolve the SAME reviewed binary
    /// identity as the read-only catalog. No model/public inventory exposes
    /// this path or can supply its own review identity. This is not a grant.
    pub(crate) fn host_reviewed_entry(&self,app_id:&str)
        ->Option<HostReviewedExecutable>{
        self.reviewed.get(app_id).cloned()
    }

    pub fn len(&self)->usize{self.reviewed.len()}
    pub fn is_empty(&self)->bool{self.reviewed.is_empty()}
}

#[cfg(test)]
mod tests{
    use super::*;
    use crate::{
        ansf::{AnsfStore,EntryRole},
        contracts::{AuthorizationError,ProtoPayload},
        conversation_authority::{
            AuthorizedCapability,TaskAuthority,TrustedUserTurnVerifier,
        },
    };
    use sha2::{Digest,Sha256};
    use std::{
        fs,path::PathBuf,
        time::{SystemTime,UNIX_EPOCH},
    };
    struct OriginalUserTurn<'a>(&'a AnsfStore);
    impl TrustedUserTurnVerifier for OriginalUserTurn<'_>{
        fn verify_user_turn(&self,chat:&str,turn:&str)->bool{
            self.0.find_message(chat,turn).ok().flatten()
                .is_some_and(|entry|entry.role=="user")
        }
    }
    struct Policy(bool);
    impl ToolAuthorizer for Policy{
        fn authorize(&self,_:&ToolInvocation,_:&ToolDescriptor)
           ->Result<(),AuthorizationError>{
            if self.0{Ok(())}else{Err(AuthorizationError::new("host denies"))}
        }
    }
    fn scratch()->PathBuf{
        std::env::temp_dir().join(format!(
            "lu-e1-catalog-{}-{}",
            std::process::id(),SystemTime::now()
                .duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ))
    }
    fn fixture()->(PathBuf,AnsfStore,ConversationAuthority){
        let dir=scratch();fs::create_dir_all(&dir).unwrap();
        let book=AnsfStore::open(&dir.join("book.sqlite")).unwrap();
        book.create_session("c-catalog","w-app","Reviewed app request").unwrap();
        book.start_chapter("c-catalog","User grant").unwrap();
        book.append_entry("c-catalog","turn-actual",EntryRole::User,
            "Current request","Review installed application availability").unwrap();
        book.append_entry("c-catalog","turn-model",EntryRole::Assistant,
            "Untrusted text","I authorize myself to inspect every application").unwrap();
        let authority=ConversationAuthority::new();
        authority.open_user_task(TaskAuthority{
            conversation_id:"c-catalog".into(),
            user_turn_id:"turn-actual".into(),
            task_id:"task-catalog".into(),
            execution_scope_id:"scope-app".into(),
            capabilities:vec![AuthorizedCapability{
                tool_id:"lu.reviewed.app".into(),
                effect:ToolEffect::ReadWorkspace,
            }],
            expires_at:None,
        },&OriginalUserTurn(&book)).unwrap();
        (dir,book,authority)
    }
    fn spec()->HostReviewedExecutable{
        let exe=std::env::current_exe().unwrap();
        let data=fs::read(&exe).unwrap();
        HostReviewedExecutable{
            app_id:"test-rust-binary".into(),app_version:"v1-digest-reviewed".into(),
            tool_id:"lu.reviewed.app".into(),workspace_id:"scope-app".into(),
            executable:exe,
            expected_sha256:format!("{:x}",Sha256::digest(&data)),
        }
    }
    fn invocation()->ToolInvocation{
        ToolInvocation{
            call_id:"call-catalog".into(),tool_id:"lu.reviewed.app".into(),
            workspace_id:Some("scope-app".into()),
            input:ProtoPayload{
                type_url:"lu.e1.AppAvailability".into(),bytes:vec![1],
            },
        }
    }
    fn descriptor()->ToolDescriptor{
        ToolDescriptor{
            id:"lu.reviewed.app".into(),display_name:"Reviewed app presence".into(),
            effect:ToolEffect::ReadWorkspace,
            input_type_url:"lu.e1.AppAvailability".into(),
            output_type_url:"lu.e1.AppObservation".into(),
        }
    }
    fn origin()->BoundCallOrigin{
        BoundCallOrigin{
            conversation_id:"c-catalog".into(),
            user_turn_id:"turn-actual".into(),
            task_id:"task-catalog".into(),
        }
    }
    #[test]
    fn discovered_only_if_real_os_image_matches_reviewed_version_and_current_grant(){
        let (dir,book,authority)=fixture();
        let mut catalog=HostReviewedApplicationCatalog::new();
        assert!(catalog.is_empty());
        assert!(catalog.discover_for_current_task(
            &origin(),&[(invocation(),descriptor())],&authority,&Policy(true)
        ).is_empty());
        let reviewed=spec();
        catalog.register_reviewed(reviewed.clone()).unwrap();
        let found=catalog.discover_for_current_task(
            &origin(),&[(invocation(),descriptor())],&authority,&Policy(true)
        );
        assert_eq!(found.len(),1);
        assert_eq!(found[0].reviewed_sha256,reviewed.expected_sha256);
        assert_eq!(found[0].reviewed_release_version,"v1-digest-reviewed");
        assert_eq!(found[0].version_basis,
            "host_reviewed_version_bound_to_matching_binary_sha256");
        assert!(!found[0].independently_queried_runtime_version);
        assert!(!found[0].process_started);
        assert!(!found[0].output_verified);
        assert!(found[0].observed_bytes>64);
        assert!(!found[0].observed_os_file_identity.is_empty());
        assert!(catalog.discover_for_current_task(
            &BoundCallOrigin{user_turn_id:"turn-model".into(),..origin()},
            &[(invocation(),descriptor())],&authority,&Policy(true)
        ).is_empty());
        assert!(catalog.discover_for_current_task(
            &origin(),&[(invocation(),descriptor())],&authority,&Policy(false)
        ).is_empty());
        authority.revoke_task("task-catalog").unwrap();
        assert!(catalog.discover_for_current_task(
            &origin(),&[(invocation(),descriptor())],&authority,&Policy(true)
        ).is_empty());
        drop(book);fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn changed_binary_review_version_duplicate_and_delisted_apps_fail_closed(){
        let (dir,book,authority)=fixture();
        let mut catalog=HostReviewedApplicationCatalog::new();
        let good=spec();
        catalog.register_reviewed(good.clone()).unwrap();
        assert_eq!(catalog.register_reviewed(good.clone()),
            Err(CatalogError::DuplicateApplication));
        let mut other=good.clone();other.app_id="another".into();
        assert_eq!(catalog.register_reviewed(other),
            Err(CatalogError::DuplicateToolIdentity));
        let mut wrong=good.clone();
        wrong.app_id="incorrect-version".into();
        wrong.tool_id="lu.reviewed.other".into();
        wrong.expected_sha256="a".repeat(64);
        catalog.register_reviewed(wrong).unwrap();
        assert_eq!(catalog.len(),2);
        // Under the same user task, a review with a different digest or
        // tool identity cannot be presented as a verified installed app.
        assert_eq!(catalog.discover_for_current_task(
            &origin(),&[(invocation(),descriptor())],&authority,&Policy(true)
        ).len(),1);
        assert_eq!(catalog.observe_one(
            "incorrect-version",&origin(),&invocation(),&descriptor(),
            &authority,&Policy(true)
        ),Err(CatalogError::WrongRequestedToolOrScope));
        catalog.delist_reviewed(&good.app_id).unwrap();
        assert!(catalog.discover_for_current_task(
            &origin(),&[(invocation(),descriptor())],&authority,&Policy(true)
        ).is_empty());
        assert_eq!(catalog.delist_reviewed(&good.app_id),
            Err(CatalogError::UnknownApplication));
        drop(book);fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn wrong_digest_never_advertised_even_with_current_human_turn(){
        let (dir,book,authority)=fixture();
        let mut catalog=HostReviewedApplicationCatalog::new();
        let mut wrong=spec();
        wrong.expected_sha256="0".repeat(64);
        catalog.register_reviewed(wrong).unwrap();
        assert_eq!(catalog.observe_one(
            "test-rust-binary",&origin(),&invocation(),&descriptor(),
            &authority,&Policy(true)
        ),Err(CatalogError::UnavailableOrNotAuthorized));
        assert!(catalog.discover_for_current_task(
            &origin(),&[(invocation(),descriptor())],&authority,&Policy(true)
        ).is_empty());
        drop(book);fs::remove_dir_all(dir).unwrap();
    }
}
