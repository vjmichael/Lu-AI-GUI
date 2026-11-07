//! P03 AI-OPS-01: host-owned, inert optimization of *already admitted* tools
//! and reusable source identity. No permissions, executable handles, model
//! downloads or tool execution are created by recommendations/cache hits.
use crate::native_capability_inventory::NativeCapabilityView;
use sha2::{Digest,Sha256};
use std::{collections::{BTreeMap,BTreeSet},sync::Mutex};

const MAX_SOURCES:usize=64;
const MAX_SUGGESTIONS:usize=6;
const MAX_REF_BYTES:usize=96;
const MAX_HOST_PROMPT:usize=16*1024;

fn valid_id(s:&str)->bool{
    !s.is_empty()&&s.len()<=MAX_REF_BYTES&&s.bytes().all(|b|
        b.is_ascii_alphanumeric()||matches!(b,b'-'|b'_'))
}
fn valid_sha(s:&str)->bool{
    s.len()==64&&s.bytes().all(|b|b.is_ascii_hexdigit())
}

/// A suggestion is a name in the CURRENT, already authorized per-turn
/// catalog; it can never be used as an execution grant or an expanded schema.
/// Only the actual current user prompt (not scraped documents, transcripts or
/// model output) may determine the preference.
pub fn recommended_read_tools(
    current_user:&str,authorized:&[NativeCapabilityView]
)->Vec<String>{
    let prompt=current_user.split("\n\n[Attached text file:").next()
        .unwrap_or(current_user).trim();
    if prompt.is_empty()||prompt.len()>MAX_HOST_PROMPT||
       crate::capability_router::user_declines_tools(prompt){return Vec::new()}
    let lower=prompt.to_ascii_lowercase();
    let file_find=["find ","where ","locate ","search ","which file",
        "files named","list all","discover ","file name","filename"];
    let git=["git ","commit","tracked diff","repository status","branch"];
    let content=["read ","summarize ","contents","search for text",
        "contains ","inspect ","extract text","what does"];
    let archive=["archive","zip ","compressed","tar file"];
    let directory=["directory","folder","list files","show files"];
    let mut preferred=Vec::new();
    if file_find.iter().any(|w|lower.contains(w)){
        preferred.push("lu_files_discover");
    }
    if git.iter().any(|w|lower.contains(w)){
        preferred.extend(["lu_git_status","lu_git_log","lu_git_diff"]);
    }
    if content.iter().any(|w|lower.contains(w)){
        preferred.extend(["lu_files_read_text","lu_reader_read"]);
    }
    if archive.iter().any(|w|lower.contains(w)){
        preferred.push("lu_archive_list");
    }
    if directory.iter().any(|w|lower.contains(w)){
        preferred.push("lu_files_list_directory");
    }
    // When no task-specific match exists, preserve the original tool loop's
    // full catalog. Do not create misleading guessed preferences.
    let allowed=authorized.iter().filter(|c|c.effect=="read_workspace"&&
        !c.native_tool_id.is_empty()).map(|c|c.public_name.as_str())
        .collect::<BTreeSet<_>>();
    let mut out=Vec::new();
    for name in preferred{
        if allowed.contains(name)&&!out.iter().any(|v:&String|v==name){
            out.push(name.to_owned());
        }
        if out.len()==MAX_SUGGESTIONS{break}
    }
    out
}

/// Source reuse can only refer to bytes whose digest a trusted native Reader
/// or file tool has verified AGAIN for this exact chat/session. No text, paths,
/// credentials or privileged tool outputs are cached here.
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct SourceReference{
    pub source_id:String,
    pub source_sha256:String,
    pub source_epoch:u64,
}
#[derive(Debug,Clone)]
struct Saved{
    fingerprint:SourceReference,
    counter:u64,
}
#[derive(Default)]
struct Entries{
    by_scope:BTreeMap<(String,String,String),Saved>,
    seq:u64,
}
#[derive(Default)]
pub struct SourceReuseIndex{inner:Mutex<Entries>}
impl SourceReuseIndex{
    pub fn new()->Self{Self::default()}
    pub fn remember_verified(
        &self,chat_id:&str,session_id:&str,ref_id:&str,
        source_sha256:&str,source_epoch:u64
    )->bool{
        if !valid_id(chat_id)||!valid_id(session_id)||!valid_id(ref_id)||
            !valid_sha(source_sha256)||source_epoch==0{return false}
        let Ok(mut inner)=self.inner.lock()else{return false};
        inner.seq=inner.seq.saturating_add(1);
        let seq=inner.seq;
        inner.by_scope.insert(
            (chat_id.into(),session_id.into(),ref_id.into()),
            Saved{
                fingerprint:SourceReference{
                    source_id:ref_id.into(),source_sha256:source_sha256.to_ascii_lowercase(),
                    source_epoch,
                },counter:seq,
            }
        );
        while inner.by_scope.len()>MAX_SOURCES{
            let Some(key)=inner.by_scope.iter()
                .min_by_key(|(_,entry)|entry.counter).map(|(key,_)|key.clone())
            else{break};
            inner.by_scope.remove(&key);
        }
        true
    }
    pub fn retrieve_if_fresh(
        &self,chat_id:&str,session_id:&str,ref_id:&str,
        freshly_verified_sha256:&str,fresh_epoch:u64
    )->Option<SourceReference>{
        if !valid_id(chat_id)||!valid_id(session_id)||!valid_id(ref_id)||
            !valid_sha(freshly_verified_sha256){return None}
        let mut inner=self.inner.lock().ok()?;
        let key=(chat_id.into(),session_id.into(),ref_id.into());
        let current=inner.by_scope.get(&key)?.fingerprint.clone();
        if current.source_epoch!=fresh_epoch||
            current.source_sha256!=freshly_verified_sha256.to_ascii_lowercase(){
            // A changed source never leaves a reusable stale hit.
            inner.by_scope.remove(&key);
            return None
        }
        Some(current)
    }
    pub fn discard_session(&self,chat_id:&str,session_id:&str){
        if let Ok(mut g)=self.inner.lock(){
            g.by_scope.retain(|(chat,session,_),_|chat!=chat_id||session!=session_id);
        }
    }
    pub fn len(&self)->usize{
        self.inner.lock().map_or(0,|x|x.by_scope.len())
    }
}

/// Review-only provider metadata gate. Even valid metadata does NOT grant a
/// download, a license, a filesystem destination or a compatible GPU backend.
/// This is a prerequisite for a future P05 reviewed Transfer Manager.
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct ProviderCandidate<'a>{
    pub provider:&'a str,
    pub repo_id:&'a str,
    pub revision:&'a str,
    pub filename:&'a str,
    pub sha256:&'a str,
    pub bytes:u64,
    pub license_accepted:bool,
    pub access_approved:bool,
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum ProviderReviewError{
    UnknownProvider,UnsafeRepository,UnpinnedRevision,UnsafeFilename,
    MissingDigest,InvalidSize,MissingLicenseApproval,MissingAccessApproval,
}
pub fn review_provider_candidate(
    c:&ProviderCandidate<'_>
)->Result<(),ProviderReviewError>{
    // Provider cannot be silently inferred from an ambiguous "gf" nickname.
    if c.provider!="hugging_face"{return Err(ProviderReviewError::UnknownProvider)}
    // The same strict path component rule applies to discovery, review and
    // P05 handoff. No dot-segments or disguised folder references.
    if !crate::model_provider_catalog::valid_repo(c.repo_id){
        return Err(ProviderReviewError::UnsafeRepository)
    }
    if c.revision.len()!=40||!c.revision.bytes().all(|b|b.is_ascii_hexdigit()){
        return Err(ProviderReviewError::UnpinnedRevision)
    }
    if c.filename.is_empty()||c.filename.len()>200||
       c.filename.starts_with('.')||c.filename.contains("..")||
       !c.filename.to_ascii_lowercase().ends_with(".gguf")||
       !c.filename.bytes().all(|b|b.is_ascii_alphanumeric()||
           matches!(b,b'-'|b'_'|b'.')){
        return Err(ProviderReviewError::UnsafeFilename)
    }
    if !valid_sha(c.sha256){return Err(ProviderReviewError::MissingDigest)}
    if !(24..=128*1024*1024*1024u64).contains(&c.bytes){
        return Err(ProviderReviewError::InvalidSize)
    }
    if !c.license_accepted{return Err(ProviderReviewError::MissingLicenseApproval)}
    if !c.access_approved{return Err(ProviderReviewError::MissingAccessApproval)}
    Ok(())
}
/// Metadata-only, non-executable handoff to the future P05 Transfer
/// Manager. It intentionally cannot enqueue a network job, reserve capacity,
/// write a destination, or assert a file was downloaded. P05 must independently
/// revalidate source, redirect, ETag/range, license grant and final SHA/length.
#[derive(Debug,Clone,PartialEq,Eq,serde::Serialize)]
#[serde(rename_all="camelCase")]
pub struct ReviewedModelTransferIntent{
    pub provider:String,
    pub repo_id:String,
    pub revision:String,
    pub filename:String,
    pub sha256:String,
    pub bytes:u64,
    pub reported_license:String,
    pub exact_source_url:String,
    pub state:&'static str,
    pub downloaded:bool,
}
pub fn make_reviewed_transfer_intent(
    candidate:&ProviderCandidate<'_>,reported_license:&str
)->Result<ReviewedModelTransferIntent,ProviderReviewError>{
    review_provider_candidate(candidate)?;
    if reported_license.is_empty()||reported_license.len()>80||
        !reported_license.bytes().all(|b|b.is_ascii_alphanumeric()||
            matches!(b,b'-'|b'_'|b'.')){
        return Err(ProviderReviewError::MissingLicenseApproval)
    }
    Ok(ReviewedModelTransferIntent{
        provider:candidate.provider.into(),
        repo_id:candidate.repo_id.into(),revision:candidate.revision.into(),
        filename:candidate.filename.into(),sha256:candidate.sha256.to_ascii_lowercase(),
        bytes:candidate.bytes,reported_license:reported_license.into(),
        exact_source_url:format!(
            "https://huggingface.co/{}/resolve/{}/{}",
            candidate.repo_id,candidate.revision,candidate.filename,
        ),
        state:"awaiting_p05_transfer",downloaded:false,
    })
}

/// A capability catalog is optionally bound to the exact current USER turn.
/// This is a witness only: it cannot be replayed as a policy grant.
pub fn current_turn_digest(current_user:&str)->Option<String>{
    if current_user.trim().is_empty()||current_user.len()>MAX_HOST_PROMPT{
        return None
    }
    Some(format!("{:x}",Sha256::digest(current_user.as_bytes())))
}

#[cfg(test)]
mod tests{
    use super::*;
    fn view(name:&str,effect:&str)->NativeCapabilityView{
        NativeCapabilityView{native_tool_id:format!("id-{name}"),
            public_name:name.into(),display_name:name.into(),effect:effect.into(),
            input_type_url:"type.test/input".into(),output_type_url:"type.test/out".into()}
    }
    #[test] fn finder_uses_only_per_turn_read_grants(){
        let list=vec![view("lu_files_discover","read_workspace"),
            view("lu_files_read_text","read_workspace"),
            view("lu_files_list_directory","write_workspace")];
        assert_eq!(recommended_read_tools("Find config files",&list),vec!["lu_files_discover"]);
        assert!(recommended_read_tools("Find files without tools",&list).is_empty());
        assert!(recommended_read_tools("List files",&list).is_empty());
        assert!(recommended_read_tools("Find config", &[]).is_empty());
    }
    #[test] fn source_reuse_rechecks_chat_session_epoch_digest(){
        let index=SourceReuseIndex::new();let hash="a".repeat(64);
        assert!(index.remember_verified("chat1","session1","source1",&hash,7));
        assert!(index.retrieve_if_fresh("chat1","session1","source1",&hash,7).is_some());
        assert!(index.retrieve_if_fresh("chat2","session1","source1",&hash,7).is_none());
        assert!(index.retrieve_if_fresh("chat1","session2","source1",&hash,7).is_none());
        assert!(index.retrieve_if_fresh("chat1","session1","source1",&hash,8).is_none());
        assert_eq!(index.len(),0);
    }
    #[test] fn reviewed_model_transfer_is_inert_and_pinned(){
        let candidate=ProviderCandidate{
            provider:"hugging_face",repo_id:"models/local-gguf",
            revision:&"a".repeat(40),filename:"model.gguf",
            sha256:&"b".repeat(64),bytes:8192,
            license_accepted:true,access_approved:true,
        };
        let r=make_reviewed_transfer_intent(&candidate,"apache-2.0").unwrap();
        assert_eq!(r.state,"awaiting_p05_transfer");
        assert!(!r.downloaded);
        assert!(r.exact_source_url.contains("/resolve/"));
        assert!(make_reviewed_transfer_intent(
            &ProviderCandidate{access_approved:false,..candidate.clone()},
            "apache-2.0").is_err());
    }
    #[test] fn provider_review_does_not_accept_name_only_or_mutable_download(){
        let good=ProviderCandidate{provider:"hugging_face",repo_id:"owner/model",
            revision:&"a".repeat(40),filename:"weights.Q4.gguf",
            sha256:&"b".repeat(64),bytes:4096,
            license_accepted:true,access_approved:true};
        assert_eq!(review_provider_candidate(&good),Ok(()));
        assert_eq!(review_provider_candidate(&ProviderCandidate{
            revision:"main",..good.clone()
        }),Err(ProviderReviewError::UnpinnedRevision));
        assert_eq!(review_provider_candidate(&ProviderCandidate{
            filename:"../escape.gguf",..good.clone()
        }),Err(ProviderReviewError::UnsafeFilename));
        assert_eq!(review_provider_candidate(&ProviderCandidate{
            repo_id:"../other",..good.clone()
        }),Err(ProviderReviewError::UnsafeRepository));
        assert_eq!(review_provider_candidate(&ProviderCandidate{
            filename:"..hidden.gguf",..good.clone()
        }),Err(ProviderReviewError::UnsafeFilename));
        assert_eq!(review_provider_candidate(&ProviderCandidate{
            license_accepted:false,..good.clone()
        }),Err(ProviderReviewError::MissingLicenseApproval));
        assert_eq!(review_provider_candidate(&ProviderCandidate{
            provider:"gf",..good
        }),Err(ProviderReviewError::UnknownProvider));
    }
}
