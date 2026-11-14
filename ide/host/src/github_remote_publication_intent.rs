//! FIN-03 / Group 6E-05C-01 — native-only, NON-PUBLISHING exact-ref intent.
//!
//! A 6E-05B authenticated read is a time-bounded observation, not a lease.
//! Here, the actual registered Git object graph must prove the observed
//! remote commit is an ancestor of an exact clean local HEAD. A host-only
//! journal can persist one prepared intent and deny restart/replay.
//! Nothing here may execute Git, use a credential, send HTTP, approve an OS
//! dialog, mutate the remote ref, or signal publication completion.
//! 6E-05C-02 native OS consent and actual atomic remote compare-and-swap
//! are independent, still unimplemented acceptance gates.
use crate::{
    git_branch_operations::exact_repo,
    github_remote_live_policy::LiveBranchPolicyEvidence,
    github_remote_publication::RemotePublicationPreflight,
    IdeWorkspaceRegistry,
};
use git2::{Oid,Repository};
use serde::{Deserialize,Serialize};
use sha2::{Digest,Sha256};
use std::{
    fs::{self,OpenOptions},
    io::{Read,Write},
    path::Path,
    time::{SystemTime,UNIX_EPOCH},
};
const MAX_REMOTE_AGE_SECONDS:u64=120;
const MAX_JOURNAL_BYTES:u64=8192;
fn valid_sha(v:&str,n:usize)->bool{
    v.len()==n&&v.bytes().all(|c|c.is_ascii_hexdigit())
}
fn now()->Result<u64,&'static str>{
    SystemTime::now().duration_since(UNIX_EPOCH)
        .map(|t|t.as_secs()).map_err(|_|"FIN03_C_NATIVE_CLOCK_INVALID")
}
fn digest(fields:&[&str])->String{
    let mut hasher=Sha256::new();
    for v in fields{
        hasher.update((v.len() as u64).to_le_bytes());
        hasher.update(v.as_bytes());
    }
    format!("{:x}",hasher.finalize())
}
/// This is an offline preparation record, NOT a token or an authorization.
/// The remote may change between a live GET and a future legitimate CAS.
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct RemotePublicationIntent{
    pub schema_version:u32,
    pub project_id:String,
    pub repository:String,
    pub branch:String,
    pub source_preview_sha256:String,
    pub observed_policy_sha256:String,
    pub expected_remote_commit:String,
    pub proposed_local_commit:String,
    pub observed_at_unix_seconds:u64,
    pub generated_at_unix_seconds:u64,
    pub intent_sha256:String,
    pub remote_commit_local_ancestor_verified:bool,
    pub os_human_approval:false_marker::MustRemainFalse,
    pub atomic_remote_cas_authorized:false_marker::MustRemainFalse,
    pub publication_performed:false_marker::MustRemainFalse,
    pub restart_auto_retry_authorized:false_marker::MustRemainFalse,
}
/// A strong typed denial: these fields cannot silently become true through
/// serde input, a forged UI JSON value or a network response.
mod false_marker{
    use serde::{Deserialize,Deserializer,Serialize};
    #[derive(Debug,Clone,Copy,Default)]
    pub struct MustRemainFalse;
    impl Serialize for MustRemainFalse{
        fn serialize<S:serde::Serializer>(&self,s:S)->Result<S::Ok,S::Error>{
            s.serialize_bool(false)
        }
    }
    impl<'de> Deserialize<'de> for MustRemainFalse{
        fn deserialize<D:Deserializer<'de>>(d:D)->Result<Self,D::Error>{
            if bool::deserialize(d)?{
                return Err(serde::de::Error::custom("remote grant must be false"));
            }
            Ok(Self)
        }
    }
}
impl IdeWorkspaceRegistry{
    /// This is NOT exposed through Tauri, WebView, LLM or a model tool.
    /// Live evidence is deliberately revalidated against original native
    /// repository state and real local ancestry. It still cannot grant CAS.
    pub(crate) fn prepare_github_remote_publication_intent(
        &self,project_id:&str,evidence:&LiveBranchPolicyEvidence,
    )->Result<RemotePublicationIntent,&'static str>{
        let preview=self.review_github_remote_publication(project_id)?;
        let repo=exact_repo(self,project_id)?;
        prepare_from_native(&repo,&preview,evidence,now()?)
    }
}
fn prepare_from_native(
    repo:&Repository,pre:&RemotePublicationPreflight,
    live:&LiveBranchPolicyEvidence,at:u64,
)->Result<RemotePublicationIntent,&'static str>{
    if !pre.source_rechecked||pre.dirty_entries!=0||!pre.git_state_clean
        ||pre.foreign_git_lock_present
        ||pre.blockers.iter().any(|b|matches!(*b,
            "origin_push_url_override_denied"|"repository_in_git_operation"
            |"worktree_or_index_dirty"|"dirty_status_too_large"
            |"foreign_git_lock_present"
        ))
        ||pre.local_relationship!="local_descendant_of_stored_tracking"
        ||pre.schema_version!=1||live.schema_version!=1
        ||pre.project_id!=live.project_id||pre.repository!=live.repository
        ||pre.local_branch!=live.branch||pre.local_head!=live.exact_local_head
        ||pre.preview_evidence_sha256!=live.exact_local_preview_sha256
        ||!live.authenticated_host_read||!live.source_rechecked
        ||!live.remote_ref_same_on_recheck||!live.branch_protection_observed
        ||!live.branch_rules_observed||!live.policy_same_on_recheck
        ||!live.local_is_descendant_of_live_head||live.error_code.is_some()
        ||live.checked_unix_seconds==0||at<live.checked_unix_seconds
        ||at-live.checked_unix_seconds>MAX_REMOTE_AGE_SECONDS
        ||!valid_sha(&pre.preview_evidence_sha256,64)
        ||!valid_sha(&pre.local_head,40)
        ||!valid_sha(&pre.local_index_sha256,64)
        ||live.remote_head_sha256.as_ref().is_none_or(|s|!valid_sha(s,40))
        ||live.policy_response_sha256.as_ref().is_none_or(|s|!valid_sha(s,64))
        ||live.native_approval_verified||live.remote_mutation_performed
        ||live.current_remote_write_lease||live.live_ref_compare_and_swap_authorized
        ||live.remote_write_route_available||live.installed_accepted
        ||pre.native_one_use_approval_granted||pre.remote_write_route_available
        ||pre.remote_mutation_performed||pre.fsynced_remote_action_journal
    {return Err("FIN03_C_STALE_OR_UNTRUSTED_PUBLICATION_EVIDENCE");}
    let expected=live.remote_head_sha256.as_ref().unwrap();
    if pre.cached_tracking_head.as_deref()!=Some(expected.as_str()){
        // No local fetch is permitted: we cannot claim ancestry against
        // a remote commit absent from the registered object database.
        return Err("FIN03_C_EXPECTED_REMOTE_NOT_PINNED_LOCALLY");
    }
    let from=Oid::from_str(expected).map_err(|_|"FIN03_C_INVALID_COMMIT")?;
    let to=Oid::from_str(&pre.local_head).map_err(|_|"FIN03_C_INVALID_COMMIT")?;
    if from==to
        ||repo.find_commit(from).is_err()||repo.find_commit(to).is_err()
        ||repo.graph_descendant_of(to,from).ok()!=Some(true)
        ||repo.head().ok().and_then(|h|h.target())!=Some(to)
    {return Err("FIN03_C_NO_VERIFIED_FORWARD_COMMIT");}
    let policy=live.policy_response_sha256.as_ref().unwrap().clone();
    let id=digest(&[
        &pre.project_id,&pre.repository,&pre.local_branch,
        expected,&pre.local_head,&pre.preview_evidence_sha256,
        &policy,&live.checked_unix_seconds.to_string(),&at.to_string(),
    ]);
    Ok(RemotePublicationIntent{
        schema_version:1,project_id:pre.project_id.clone(),
        repository:pre.repository.clone(),branch:pre.local_branch.clone(),
        source_preview_sha256:pre.preview_evidence_sha256.clone(),
        observed_policy_sha256:policy,expected_remote_commit:expected.clone(),
        proposed_local_commit:pre.local_head.clone(),
        observed_at_unix_seconds:live.checked_unix_seconds,
        generated_at_unix_seconds:at,intent_sha256:id,
        remote_commit_local_ancestor_verified:true,
        os_human_approval:Default::default(),
        atomic_remote_cas_authorized:Default::default(),
        publication_performed:Default::default(),
        restart_auto_retry_authorized:Default::default(),
    })
}
#[derive(Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct PreparedJournal{
    schema_version:u32,
    phase:String,
    intent_sha256:String,
    expected_remote_commit:String,
    proposed_local_commit:String,
    record_sha256:String,
}
fn journal_record(intent:&RemotePublicationIntent)->PreparedJournal{
    let record=digest(&[
        "fin03-6e05c-prepared-only-v1",&intent.intent_sha256,
        &intent.expected_remote_commit,&intent.proposed_local_commit,
    ]);
    PreparedJournal{
        schema_version:1,phase:"prepared_never_approved".into(),
        intent_sha256:intent.intent_sha256.clone(),
        expected_remote_commit:intent.expected_remote_commit.clone(),
        proposed_local_commit:intent.proposed_local_commit.clone(),
        record_sha256:record,
    }
}
fn verify_record(v:&PreparedJournal)->Result<(),&'static str>{
    if v.schema_version!=1||v.phase!="prepared_never_approved"
        ||!valid_sha(&v.intent_sha256,64)
        ||!valid_sha(&v.expected_remote_commit,40)
        ||!valid_sha(&v.proposed_local_commit,40)
        ||v.record_sha256!=digest(&[
            "fin03-6e05c-prepared-only-v1",&v.intent_sha256,
            &v.expected_remote_commit,&v.proposed_local_commit,
        ])
    {return Err("FIN03_C_RESTART_JOURNAL_TAMPERED");}
    Ok(())
}
/// Protected journal directory is selected/ACL-attested by the trusted OS
/// host; nothing in a workspace, WebView, or remote response can select it.
/// Uses create_new, no overwrite, file fsync, bounded replay. Full directory
/// durability is OS-specific and remains an installed recovery gate.
pub(crate) fn fsync_prepared_intent_once(
    host_private_dir:&Path,intent:&RemotePublicationIntent,
)->Result<(),&'static str>{
    // The installed caller must independently prove this private directory
    // is owner-only, non-reparse and kernel-backed before using this helper.
    // Filesystem metadata checks here DO NOT constitute that OS ACL proof.
    if !host_private_dir.is_absolute()
        ||!valid_sha(&intent.intent_sha256,64)
        ||fs::symlink_metadata(host_private_dir).ok().is_none_or(|m|
            !m.is_dir()||m.file_type().is_symlink()
        ){
        return Err("FIN03_C_PRIVATE_JOURNAL_DIRECTORY_NOT_ATTESTED");
    }
    let record=journal_record(intent);
    let bytes=serde_json::to_vec(&record)
        .map_err(|_|"FIN03_C_JOURNAL_ENCODING_DENIED")?;
    if bytes.len() as u64>MAX_JOURNAL_BYTES{return Err("FIN03_C_JOURNAL_TOO_LARGE");}
    let path=host_private_dir.join(format!("{}.prepared.json",intent.intent_sha256));
    let mut file=OpenOptions::new().create_new(true).write(true)
        .open(&path).map_err(|_|"FIN03_C_DUPLICATE_OR_INSECURE_JOURNAL")?;
    file.write_all(&bytes).map_err(|_|"FIN03_C_JOURNAL_WRITE_FAILED")?;
    file.sync_all().map_err(|_|"FIN03_C_JOURNAL_SYNC_FAILED")?;
    Ok(())
}
/// Any prepared-only record on restart is a blocked uncertain transaction.
/// Never replay an approval, generate a push, or automatically retry.
pub(crate) fn recover_prepared_intent(
    host_private_dir:&Path,intent_sha256:&str,
)->Result<&'static str,&'static str>{
    if !host_private_dir.is_absolute()||!valid_sha(intent_sha256,64){
        return Err("FIN03_C_RECOVERY_SCOPE_INVALID");
    }
    let path=host_private_dir.join(format!("{intent_sha256}.prepared.json"));
    let m=fs::symlink_metadata(&path)
        .map_err(|_|"FIN03_C_PREPARED_JOURNAL_MISSING")?;
    if !m.is_file()||m.file_type().is_symlink()
        ||m.len()>MAX_JOURNAL_BYTES {
        return Err("FIN03_C_RESTART_JOURNAL_TAMPERED");
    }
    let mut bytes=Vec::new();
    fs::File::open(&path).map_err(|_|"FIN03_C_RECOVERY_READ_FAILED")?
        .take(MAX_JOURNAL_BYTES+1)
        .read_to_end(&mut bytes)
        .map_err(|_|"FIN03_C_RECOVERY_READ_FAILED")?;
    let record:PreparedJournal=serde_json::from_slice(&bytes)
        .map_err(|_|"FIN03_C_RESTART_JOURNAL_TAMPERED")?;
    verify_record(&record)?;
    if record.intent_sha256!=intent_sha256 {
        return Err("FIN03_C_RECOVERY_SCOPE_CHANGED");
    }
    Ok("reconciliation_required_no_remote_retry")
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::{IdeError,NativeDirectoryPicker};
    use git2::{Repository,Signature};
    use std::{fs,path::{Path,PathBuf}};
    struct Picker(PathBuf);
    impl NativeDirectoryPicker for Picker{
        fn choose_project_directory(&mut self)->Result<Option<PathBuf>,IdeError>{
            Ok(Some(self.0.clone()))
        }
    }
    struct Fixture{
        dir:PathBuf,root:PathBuf,registry:IdeWorkspaceRegistry,id:String,
        old:String,new:String,
    }
    impl Drop for Fixture{
        fn drop(&mut self){let _=fs::remove_dir_all(&self.dir);}
    }
    fn fixture()->Fixture{
        let dir=std::env::temp_dir().join(format!("fin03-6e05c-{:032x}",rand::random::<u128>()));
        let root=dir.join("repo");
        fs::create_dir_all(&root).unwrap();
        let repo=Repository::init(&root).unwrap();
        fs::write(root.join("p.txt"),"initial\n").unwrap();
        let mut index=repo.index().unwrap();
        index.add_path(Path::new("p.txt")).unwrap();index.write().unwrap();
        let sig=Signature::now("CI","ci@example.invalid").unwrap();
        let tree=repo.find_tree(index.write_tree().unwrap()).unwrap();
        let old=repo.commit(Some("HEAD"),&sig,&sig,"first",&tree,&[]).unwrap();
        drop(tree);drop(index);
        repo.remote("origin","https://github.com/example/fixture.git").unwrap();
        let branch=repo.head().unwrap().shorthand().unwrap().to_owned();
        repo.reference(&format!("refs/remotes/origin/{branch}"),old,true,"fixture").unwrap();
        fs::write(root.join("p.txt"),"second\n").unwrap();
        let mut index=repo.index().unwrap();
        index.add_path(Path::new("p.txt")).unwrap();index.write().unwrap();
        let tree=repo.find_tree(index.write_tree().unwrap()).unwrap();
        let parent=repo.find_commit(old).unwrap();
        let new=repo.commit(Some("HEAD"),&sig,&sig,"second",&tree,&[&parent]).unwrap();
        drop(tree);drop(parent);drop(index);drop(repo);
        let registry=IdeWorkspaceRegistry::open(dir.join("catalog.json")).unwrap();
        let id=registry.register_from_native_picker(&mut Picker(root.clone()))
            .unwrap().unwrap().project_id;
        Fixture{dir,root,registry,id,old:old.to_string(),new:new.to_string()}
    }
    fn observed(f:&Fixture)->LiveBranchPolicyEvidence{
        let p=f.registry.review_github_remote_publication(&f.id).unwrap();
        LiveBranchPolicyEvidence{
            schema_version:1,project_id:f.id.clone(),
            repository:p.repository,branch:p.local_branch,
            exact_local_head:f.new.clone(),
            exact_local_preview_sha256:p.preview_evidence_sha256,
            remote_head_sha256:Some(f.old.clone()),
            remote_ref_same_on_recheck:true,
            local_is_descendant_of_live_head:true,
            branch_protection_observed:true,branch_rules_observed:true,
            policy_same_on_recheck:true,policy_response_sha256:Some("a".repeat(64)),
            checked_unix_seconds:now().unwrap(),
            error_code:None,authenticated_host_read:true,source_rechecked:true,
            live_ref_compare_and_swap_authorized:false,current_remote_write_lease:false,
            native_approval_verified:false,remote_mutation_performed:false,
            remote_write_route_available:false,installed_accepted:false,
        }
    }
    #[test]fn offline_intent_requires_real_local_ancestry_without_write_authority(){
        let f=fixture();let policy=observed(&f);
        let intent=f.registry.prepare_github_remote_publication_intent(&f.id,&policy).unwrap();
        assert_eq!(intent.expected_remote_commit,f.old);
        assert_eq!(intent.proposed_local_commit,f.new);
        assert!(intent.remote_commit_local_ancestor_verified);
        let j=serde_json::to_value(&intent).unwrap();
        for key in ["osHumanApproval","atomicRemoteCasAuthorized",
            "publicationPerformed","restartAutoRetryAuthorized"]{
            assert_eq!(j[key],false);
        }
    }
    #[test]fn stale_or_moved_snapshot_denies_even_with_claimed_policy(){
        let f=fixture();let mut p=observed(&f);
        p.checked_unix_seconds=now().unwrap()-121;
        assert!(f.registry.prepare_github_remote_publication_intent(&f.id,&p).is_err());
        p=observed(&f);
        p.remote_head_sha256=Some("b".repeat(40));
        assert!(f.registry.prepare_github_remote_publication_intent(&f.id,&p).is_err());
        p=observed(&f);p.native_approval_verified=true;
        assert!(f.registry.prepare_github_remote_publication_intent(&f.id,&p).is_err());
    }
    #[test]fn journal_one_use_fsync_then_restart_never_retries(){
        let f=fixture();let p=observed(&f);
        let i=f.registry.prepare_github_remote_publication_intent(&f.id,&p).unwrap();
        let journal=f.dir.join("private");
        fs::create_dir(&journal).unwrap();
        fsync_prepared_intent_once(&journal,&i).unwrap();
        assert!(fsync_prepared_intent_once(&journal,&i).is_err());
        assert_eq!(recover_prepared_intent(&journal,&i.intent_sha256).unwrap(),
            "reconciliation_required_no_remote_retry");
        let path=journal.join(format!("{}.prepared.json",i.intent_sha256));
        fs::write(&path,b"{\"phase\":\"approved\"}").unwrap();
        assert!(recover_prepared_intent(&journal,&i.intent_sha256).is_err());
    }
    #[test]fn foreign_remote_or_dirty_project_never_prepares_intent(){
        let f=fixture();
        let p=observed(&f);
        fs::write(f.root.join("p.txt"),"dirty").unwrap();
        assert!(f.registry.prepare_github_remote_publication_intent(&f.id,&p).is_err());
    }
}
