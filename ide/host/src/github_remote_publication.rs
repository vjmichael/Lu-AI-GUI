//! FIN-03 / Group 6E-05A: exact local source preview for protected remote publication.
//!
//! This is intentionally NON-EXECUTING. A cached origin tracking ref is
//! neither a live remote ref nor an authorization to force/fast-forward
//! GitHub. Never use these observations to issue a push, update a remote
//! branch, write credentials, or mark a pull request approved.
//! Next gates: independent live remote ref CAS, verified host-only credential
//! custody, separate one-use native OS human approval, fsynced journal,
//! independent remote readback and negative interruption/restart evidence.
use crate::{
    git_branch_operations::{exact_repo,safe_local_name},
    github_live::github_repo,IdeWorkspaceRegistry,
};
use git2::{Oid,Repository,RepositoryState,Status,StatusOptions};
use serde::Serialize;
use sha2::{Digest,Sha256};
use std::{fs,io::Read,path::Path};

const MAX_INDEX_BYTES:u64=8*1024*1024;
const MAX_DIRTY:usize=512;
#[derive(Debug,Clone,Eq,PartialEq)]
struct Snapshot {
    origin:String,
    branch:String,
    head:Oid,
    cached:Option<Oid>,
    index_sha:String,
    dirty:usize,
    index_lock:bool,
    other_lock:bool,
    state:RepositoryState,
    push_url_override:bool,
}
#[derive(Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct RemotePublicationPreflight {
    pub schema_version:u32,
    pub project_id:String,
    pub effect:&'static str,
    pub repository:String,
    pub local_branch:String,
    pub local_head:String,
    pub cached_tracking_head:Option<String>,
    pub tracking_source:&'static str,
    pub local_relationship:&'static str,
    pub local_index_sha256:String,
    pub dirty_entries:usize,
    pub git_state_clean:bool,
    pub foreign_git_lock_present:bool,
    pub source_rechecked:bool,
    pub preview_evidence_sha256:String,
    pub blockers:Vec<&'static str>,
    pub live_remote_ref_verified:bool,
    pub branch_rules_complete:bool,
    pub remote_credentials_verified:bool,
    pub native_one_use_approval_granted:bool,
    pub fsynced_remote_action_journal:bool,
    pub remote_write_route_available:bool,
    pub remote_mutation_performed:bool,
    pub git_process_started:bool,
    pub network_queried:bool,
    pub release_accepted:bool,
}
fn index_hash(repo:&Repository)->Result<String,&'static str>{
    let path=repo.path().join("index");
    let meta=fs::symlink_metadata(&path)
        .map_err(|_|"FIN03_REMOTE_INDEX_NOT_VERIFIED")?;
    if !meta.is_file()||meta.file_type().is_symlink()
        ||meta.len()>MAX_INDEX_BYTES {
        return Err("FIN03_REMOTE_INDEX_NOT_VERIFIED");
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0{
            return Err("FIN03_REMOTE_INDEX_REPARSE_DENIED");
        }
    }
    let mut file=fs::File::open(path)
        .map_err(|_|"FIN03_REMOTE_INDEX_UNREADABLE")?;
    let mut bytes=Vec::new();
    file.take(MAX_INDEX_BYTES+1)
        .read_to_end(&mut bytes).map_err(|_|"FIN03_REMOTE_INDEX_UNREADABLE")?;
    if bytes.len() as u64>MAX_INDEX_BYTES {
        return Err("FIN03_REMOTE_INDEX_TOO_LARGE");
    }
    Ok(format!("{:x}",Sha256::digest(&bytes)))
}
fn inspect(repo:&Repository)->Result<Snapshot,&'static str>{
    let head=repo.head().map_err(|_|"FIN03_REMOTE_HEAD_UNAVAILABLE")?;
    let name=head.name().and_then(|s|s.strip_prefix("refs/heads/"))
        .filter(|s|safe_local_name(s))
        .ok_or("FIN03_REMOTE_LOCAL_BRANCH_INVALID")?;
    let oid=head.target().ok_or("FIN03_REMOTE_HEAD_INDIRECT")?;
    repo.find_commit(oid).map_err(|_|"FIN03_REMOTE_HEAD_NOT_COMMIT")?;
    let remote=repo.find_remote("origin")
        .map_err(|_|"FIN03_REMOTE_ORIGIN_UNAVAILABLE")?;
    let (owner,project)=remote.url().and_then(github_repo)
        .ok_or("FIN03_REMOTE_ORIGIN_NOT_CANONICAL_GITHUB")?;
    let origin=format!("{owner}/{project}");
    let push_url_override=remote.pushurl().is_some()
        ||repo.config().map_err(|_|"FIN03_REMOTE_CONFIG_UNVERIFIED")?
            .get_string("remote.origin.pushurl").is_ok();
    let tracking=format!("refs/remotes/origin/{name}");
    let cached=match repo.find_reference(&tracking){
        Ok(reference)=>{
            let id=reference.target()
                .ok_or("FIN03_REMOTE_TRACKING_REF_INDIRECT")?;
            repo.find_commit(id)
                .map_err(|_|"FIN03_REMOTE_TRACKING_NOT_COMMIT")?;
            Some(id)
        },
        Err(e) if e.code()==git2::ErrorCode::NotFound=>None,
        Err(_)=>return Err("FIN03_REMOTE_TRACKING_UNVERIFIED"),
    };
    let mut options=StatusOptions::new();
    options.include_untracked(true).recurse_untracked_dirs(true)
        .include_ignored(false).renames_head_to_index(false)
        .renames_index_to_workdir(false);
    let entries=repo.statuses(Some(&mut options))
        .map_err(|_|"FIN03_REMOTE_STATUS_UNVERIFIABLE")?;
    let dirty=entries.iter().filter(|row|row.status()!=Status::CURRENT)
        .count();
    let dotgit=repo.path();
    let index_lock=dotgit.join("index.lock").exists();
    let other_lock=dotgit.join("packed-refs.lock").exists()
        ||dotgit.join("config.lock").exists()
        ||dotgit.join("HEAD.lock").exists()
        ||dotgit.join("refs").join("heads").join(format!("{name}.lock")).exists();
    Ok(Snapshot{
        origin,branch:name.to_owned(),head:oid,cached,
        index_sha:index_hash(repo)?,dirty,index_lock,other_lock,
        state:repo.state(),push_url_override,
    })
}
fn relationship(repo:&Repository,local:Oid,cached:Option<Oid>)
    ->Result<&'static str,&'static str>{
    match cached{
        None=>Ok("stored_tracking_missing"),
        Some(old) if old==local=>Ok("same_local_commit"),
        Some(old)=>match repo.graph_descendant_of(local,old){
            Ok(true)=>Ok("local_descendant_of_stored_tracking"),
            Ok(false)=>Ok("divergent_or_behind_stored_tracking"),
            Err(_)=>Err("FIN03_REMOTE_ANCESTRY_UNVERIFIABLE"),
        },
    }
}
fn digest(parts:&[&str])->String{
    let mut hash=Sha256::new();
    hash.update(b"lu-ide-fin03-6e05a-nonexecuting-preflight-v1\n");
    for part in parts{
        hash.update((part.len() as u64).to_le_bytes());
        hash.update(part.as_bytes());
    }
    format!("{:x}",hash.finalize())
}
impl IdeWorkspaceRegistry{
    /// Native host only, project-ID input only. This creates no lasting
    /// permission and makes NO network or credential request. Even a clean
    /// local fast-forward remains blocked until an independently verified
    /// live remote-ref compare-and-swap and separate OS approval exist.
    pub fn review_github_remote_publication(
        &self,project_id:&str,
    )->Result<RemotePublicationPreflight,&'static str>{
        let repo=exact_repo(self,project_id)?;
        let before=inspect(&repo)?;
        let relationship=relationship(&repo,before.head,before.cached)?;
        // An immutable cached ref is NOT the actual remote branch tip.
        // The host must never infer publish safety from it.
        let mut blockers=vec![
            "live_remote_ref_compare_and_swap_missing",
            "remote_branch_and_ruleset_policy_unverified",
            "host_only_credentials_not_attested",
            "separate_native_one_use_human_approval_missing",
            "durable_remote_action_journal_and_readback_missing",
            "remote_publication_disabled",
        ];
        if before.push_url_override{
            blockers.push("origin_push_url_override_denied");
        }
        if before.state!=RepositoryState::Clean{
            blockers.push("repository_in_git_operation");
        }
        if before.dirty>0{
            blockers.push("worktree_or_index_dirty");
        }
        if before.dirty>MAX_DIRTY{
            blockers.push("dirty_status_too_large");
        }
        if before.index_lock||before.other_lock{
            blockers.push("foreign_git_lock_present");
        }
        match relationship{
            "stored_tracking_missing"=>blockers.push("no_stored_tracking_snapshot"),
            "same_local_commit"=>blockers.push("no_new_local_commits"),
            "divergent_or_behind_stored_tracking"=>
                blockers.push("stored_tracking_not_ancestor"),
            _=>(),
        }
        let after=inspect(&repo)?;
        if before!=after{
            return Err("FIN03_REMOTE_SOURCE_DRIFT_DURING_PREVIEW");
        }
        let checked=relationship(&repo,after.head,after.cached)?;
        if checked!=relationship{
            return Err("FIN03_REMOTE_ANCESTRY_DRIFT");
        }
        // Proof of a consistent local observation, never a token or
        // executable grant. No source bytes or host-private paths exposed.
        let head=before.head.to_string();
        let tracking=before.cached.map(|c|c.to_string());
        let preview=digest(&[
            project_id,&before.origin,&before.branch,&head,
            tracking.as_deref().unwrap_or("unobserved"),
            &before.index_sha,&before.dirty.to_string(),
            if before.index_lock{"locked"}else{"unlocked"},
            if before.other_lock{"other_lock"}else{"no_other_lock"},
            if before.push_url_override{"push_url_override"}else{"same_origin"},
            relationship,
        ]);
        Ok(RemotePublicationPreflight{
            schema_version:1,project_id:project_id.to_owned(),
            effect:"remote_branch_publication_requires_separate_native_approval",
            repository:before.origin,local_branch:before.branch,
            local_head:head,cached_tracking_head:tracking,
            tracking_source:"local_stored_ref_only_not_live_remote",
            local_relationship:relationship,local_index_sha256:before.index_sha,
            dirty_entries:before.dirty.min(MAX_DIRTY),
            git_state_clean:before.state==RepositoryState::Clean,
            foreign_git_lock_present:before.index_lock||before.other_lock,
            source_rechecked:true,preview_evidence_sha256:preview,
            blockers,live_remote_ref_verified:false,
            branch_rules_complete:false,remote_credentials_verified:false,
            native_one_use_approval_granted:false,
            fsynced_remote_action_journal:false,
            remote_write_route_available:false,remote_mutation_performed:false,
            git_process_started:false,network_queried:false,release_accepted:false,
        })
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    use crate::{NativeDirectoryPicker,IdeError};
    use git2::{Repository,Signature};
    use std::{path::PathBuf,fs};
    struct Picker(PathBuf);
    impl NativeDirectoryPicker for Picker{
        fn choose_project_directory(&mut self)->Result<Option<PathBuf>,IdeError>{
            Ok(Some(self.0.clone()))
        }
    }
    struct Fixture{
        home:PathBuf,root:PathBuf,registry:IdeWorkspaceRegistry,id:String,
    }
    impl Drop for Fixture{
        fn drop(&mut self){let _=fs::remove_dir_all(&self.home);}
    }
    fn fixture()->Fixture{
        let home=std::env::temp_dir().join(format!(
            "fin03-6e05a-{:032x}",rand::random::<u128>()
        ));
        let root=home.join("project");
        fs::create_dir_all(&root).unwrap();
        let repo=Repository::init(&root).unwrap();
        fs::write(root.join("tracked.txt"),"base\n").unwrap();
        let mut index=repo.index().unwrap();
        index.add_path(Path::new("tracked.txt")).unwrap();
        index.write().unwrap();
        let tree=repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig=Signature::now("CI","noreply@example.invalid").unwrap();
        repo.commit(Some("HEAD"),&sig,&sig,"base",&tree,&[]).unwrap();
        drop(tree);drop(index);
        repo.remote("origin","https://github.com/example/fixture.git").unwrap();
        let head=repo.head().unwrap().target().unwrap();
        let branch=repo.head().unwrap().shorthand().unwrap().to_owned();
        repo.reference(&format!("refs/remotes/origin/{branch}"),head,true,"fixture").unwrap();
        drop(repo);
        let registry=IdeWorkspaceRegistry::open(home.join("catalog.json")).unwrap();
        let id=registry.register_from_native_picker(&mut Picker(root.clone()))
            .unwrap().unwrap().project_id;
        Fixture{home,root,registry,id}
    }
    fn head(f:&Fixture)->String{
        Repository::open(&f.root).unwrap().head().unwrap()
            .target().unwrap().to_string()
    }
    #[test]
    fn immutable_local_publication_preview_cannot_mint_remote_authority(){
        let fixture=fixture();
        let before=head(&fixture);
        let review=fixture.registry.review_github_remote_publication(&fixture.id).unwrap();
        assert_eq!(review.repository,"example/fixture");
        assert_eq!(review.local_head,before);
        assert_eq!(review.local_relationship,"same_local_commit");
        assert!(review.source_rechecked);
        assert_eq!(review.preview_evidence_sha256.len(),64);
        assert!(review.blockers.contains(&"live_remote_ref_compare_and_swap_missing"));
        assert!(!review.remote_write_route_available);
        assert!(!review.native_one_use_approval_granted);
        assert!(!review.network_queried && !review.remote_mutation_performed);
        assert_eq!(before,head(&fixture));
        let again=fixture.registry.review_github_remote_publication(&fixture.id).unwrap();
        assert_eq!(review.preview_evidence_sha256,again.preview_evidence_sha256);
    }
    #[test]
    fn uncommitted_and_foreign_lock_are_never_publishable(){
        let fixture=fixture();
        let root=&fixture.root;
        fs::write(root.join("tracked.txt"),"dirty\n").unwrap();
        fs::write(root.join(".git/index.lock"),"foreign").unwrap();
        let review=fixture.registry.review_github_remote_publication(&fixture.id).unwrap();
        assert!(review.blockers.contains(&"worktree_or_index_dirty"));
        assert!(review.blockers.contains(&"foreign_git_lock_present"));
        assert!(!review.remote_write_route_available);
        assert_eq!(fs::read(root.join("tracked.txt")).unwrap(),b"dirty\n");
        assert_eq!(fs::read(root.join(".git/index.lock")).unwrap(),b"foreign");
    }
    #[test]
    fn different_local_commit_is_only_cached_fast_forward_not_live_cas(){
        let fixture=fixture();
        let repo=Repository::open(&fixture.root).unwrap();
        fs::write(fixture.root.join("tracked.txt"),"next\n").unwrap();
        let mut index=repo.index().unwrap();
        index.add_path(Path::new("tracked.txt")).unwrap();
        index.write().unwrap();
        let tree=repo.find_tree(index.write_tree().unwrap()).unwrap();
        let parent=repo.head().unwrap().peel_to_commit().unwrap();
        let sig=Signature::now("CI","noreply@example.invalid").unwrap();
        repo.commit(Some("HEAD"),&sig,&sig,"next",&tree,&[&parent]).unwrap();
        drop(tree);drop(parent);drop(index);drop(repo);
        let review=fixture.registry.review_github_remote_publication(&fixture.id).unwrap();
        assert_eq!(review.local_relationship,
            "local_descendant_of_stored_tracking");
        assert!(review.blockers.contains(&"live_remote_ref_compare_and_swap_missing"));
        assert!(!review.remote_write_route_available);
    }
    #[test]
    fn spoofed_origin_and_push_url_override_are_denied(){
        let fixture=fixture();
        let repo=Repository::open(&fixture.root).unwrap();
        repo.remote_set_url("origin","https://github.com.evil.invalid/org/repo").unwrap();
        assert!(fixture.registry.review_github_remote_publication(&fixture.id).is_err());
        repo.remote_set_url("origin","https://github.com/example/fixture.git").unwrap();
        repo.remote_set_pushurl("origin",Some("https://example.invalid/push")).unwrap();
        let review=fixture.registry.review_github_remote_publication(&fixture.id).unwrap();
        assert!(review.blockers.contains(&"origin_push_url_override_denied"));
        assert!(!review.remote_write_route_available);
    }
}
