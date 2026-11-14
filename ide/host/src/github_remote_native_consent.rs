//! FIN-03 / Group 6E-05C-02: host-only EXACT native consent REVIEW.
//!
//! It forms the bytes that a future independently installed native OS dialog
//! must display before a one-use GitHub remote mutation. It DOES NOT
//! implement a dialog, issue an approval, provide credentials or perform a
//! ref update. Every result remains NO AUTHORITY. An authenticated six-GET
//! 6E-05B snapshot is still immediately stale and cannot be a remote CAS.
use crate::{
    github_remote_publication_intent::RemotePublicationIntent,
    github_remote_live_policy::LiveBranchPolicyEvidence,
    IdeWorkspaceRegistry,
};
use serde::Serialize;
use sha2::{Digest,Sha256};
use std::time::{SystemTime,UNIX_EPOCH};
const MAX_AGE_SECONDS:u64=120;
fn sha_parts(parts:&[&str])->String{
    let mut h=Sha256::new();
    for p in parts{
        h.update((p.len() as u64).to_le_bytes());
        h.update(p.as_bytes());
    }
    format!("{:x}",h.finalize())
}
fn valid(v:&str,size:usize)->bool{
    v.len()==size&&v.bytes().all(|b|b.is_ascii_hexdigit())
}
#[derive(Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct RemotePublicationConsentReview{
    pub schema_version:u32,
    pub project_id:String,
    pub repository:String,
    pub branch:String,
    pub previous_remote_commit:String,
    pub proposed_local_commit:String,
    pub original_intent_sha256:String,
    pub policy_sha256:String,
    pub displayed_review_sha256:String,
    pub effect:&'static str,
    pub warning:&'static str,
    pub original_native_os_dialog_required:bool,
    pub independent_remote_cas_required:bool,
    pub original_restart_recovery_required:bool,
    pub native_operator_approval_granted:bool,
    pub remote_write_route_available:bool,
    pub remote_mutation_performed:bool,
    pub release_accepted:bool,
}
impl IdeWorkspaceRegistry{
    /// Native host only; no exposed UI action and no OS authorization token.
    /// This verifies that a preview can be DISPLAYED later, not that it
    /// can be executed or re-used after process restart or ref movement.
    pub(crate) fn review_exact_remote_publication_consent(
        &self,project_id:&str,
        intent:&RemotePublicationIntent,
        recent:&LiveBranchPolicyEvidence,
    )->Result<RemotePublicationConsentReview,&'static str>{
        let local=self.inspect_group6f_git_recovery(project_id)?;
        if !local.local_new_operation_preflight_clear
            ||local.worktree_dirty
            ||local.exact_source_head!=intent.proposed_local_commit
            ||local.project_id!=intent.project_id
            ||intent.project_id!=project_id
            ||intent.schema_version!=1
            ||intent.expected_remote_commit==intent.proposed_local_commit
            ||!valid(&intent.intent_sha256,64)
            ||!valid(&intent.source_preview_sha256,64)
            ||!valid(&intent.observed_policy_sha256,64)
            ||!valid(&intent.expected_remote_commit,40)
            ||!valid(&intent.proposed_local_commit,40)
        {return Err("FIN03_C_NATIVE_REVIEW_SOURCE_OR_PENDING_GIT_DENIED");}
        let supposed=sha_parts(&[
            &intent.project_id,&intent.repository,&intent.branch,
            &intent.expected_remote_commit,&intent.proposed_local_commit,
            &intent.source_preview_sha256,&intent.observed_policy_sha256,
            &intent.observed_at_unix_seconds.to_string(),
            &intent.generated_at_unix_seconds.to_string(),
        ]);
        if supposed!=intent.intent_sha256{
            return Err("FIN03_C_NATIVE_REVIEW_INTENT_FORGED");
        }
        let now=SystemTime::now().duration_since(UNIX_EPOCH)
            .map_err(|_|"FIN03_C_NATIVE_REVIEW_CLOCK_INVALID")?.as_secs();
        if !recent.authenticated_host_read||!recent.source_rechecked
            ||!recent.remote_ref_same_on_recheck
            ||!recent.policy_same_on_recheck
            ||!recent.branch_protection_observed
            ||!recent.branch_rules_observed
            ||recent.error_code.is_some()
            ||recent.schema_version!=1
            ||recent.project_id!=project_id
            ||recent.repository!=intent.repository
            ||recent.branch!=intent.branch
            ||recent.exact_local_head!=intent.proposed_local_commit
            ||recent.exact_local_preview_sha256!=intent.source_preview_sha256
            ||recent.remote_head_sha256.as_deref()
                !=Some(intent.expected_remote_commit.as_str())
            ||recent.policy_response_sha256.as_deref()
                !=Some(intent.observed_policy_sha256.as_str())
            ||recent.checked_unix_seconds<intent.generated_at_unix_seconds
            ||recent.checked_unix_seconds>now
            ||now-recent.checked_unix_seconds>MAX_AGE_SECONDS
            ||recent.native_approval_verified||recent.remote_mutation_performed
            ||recent.live_ref_compare_and_swap_authorized
            ||recent.current_remote_write_lease||recent.remote_write_route_available
        {return Err("FIN03_C_NATIVE_REVIEW_REMOTE_REF_OR_POLICY_NOT_CURRENT");}
        // Stable, bounded display digest. It is NOT a grant or CAS lease.
        let shown=sha_parts(&[
            "native-os-dialog-review-only-v1",&intent.intent_sha256,
            &intent.repository,&intent.branch,&intent.expected_remote_commit,
            &intent.proposed_local_commit,&intent.observed_policy_sha256,
            &recent.checked_unix_seconds.to_string(),
        ]);
        Ok(RemotePublicationConsentReview{
            schema_version:1,project_id:project_id.to_owned(),
            repository:intent.repository.clone(),branch:intent.branch.clone(),
            previous_remote_commit:intent.expected_remote_commit.clone(),
            proposed_local_commit:intent.proposed_local_commit.clone(),
            original_intent_sha256:intent.intent_sha256.clone(),
            policy_sha256:intent.observed_policy_sha256.clone(),
            displayed_review_sha256:shown,
            effect:"publish_exact_local_git_commit_to_existing_remote_branch",
            warning:"A separately approved native OS dialog, atomic remote ref compare-and-swap and durable after-write readback are REQUIRED; this review does not authorize or perform any publication.",
            original_native_os_dialog_required:true,
            independent_remote_cas_required:true,
            original_restart_recovery_required:true,
            native_operator_approval_granted:false,
            remote_write_route_available:false,
            remote_mutation_performed:false,
            release_accepted:false,
        })
    }
}
