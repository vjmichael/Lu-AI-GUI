//! WO6: truthful host-only projection of *existing* protected Cargo receipts.
//!
//! A D1 historical review is not an OS execution grant. Completed receipt
//! alone is not build/test acceptance: native kernel cleanup, toolchain
//! provenance and actual result evidence are separately required. This
//! projection never starts a worker, dispatches shell commands, opens user
//! paths, or releases a quarantined job.
use super::{fingerprint, JobPhase, Supervisor, SupervisorError};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildTestStage {
    PreparedNoExecution,
    ReviewedNoExecution,
    NativeAdmissionPending,
    RunningUnverified,
    ReleasePending,
    CancellationPending,
    Cancelled,
    Failed,
    Denied,
    CompletionRequiresNativeEvidence,
    RecoveryRequired,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildTestEvidence {
    pub project_id: String,
    pub job_id: String,
    pub recipe_id: String,
    pub source_head: String,
    pub input_snapshot_sha256: String,
    pub manifest_sha256: String,
    pub toolchain_sha256: String,
    pub plan_sha256: String,
    pub terminal_receipt_hash: String,
    pub stage: BuildTestStage,
    pub requires_reconciliation: bool,
    pub os_execution_accepted: bool,
    pub installed_app_accepted: bool,
    pub desktop_run_authorized: bool,
}

impl Supervisor {
    /// Read-only host query tied to an exact project/source/plan. Do not
    /// expose the journal path, internal grant ID, process handle or logs.
    /// Must be called from authenticated native host state, never with a
    /// WebView supplied Supervisor instance or user-selected journal.
    pub fn review_build_test_evidence(
        &self,
        job_id: &str,
        expected_project_id: &str,
        expected_plan_sha256: &str,
        expected_source_head: &str,
    ) -> Result<BuildTestEvidence, SupervisorError> {
        let state = self
            .state
            .lock()
            .map_err(|_| SupervisorError::JournalUnavailable)?;
        let job = state.jobs.get(job_id).ok_or(SupervisorError::NoSuchJob)?;
        if !matches!(
            job.plan.recipe_id.as_str(),
            "rust_check" | "rust_build" | "rust_tests" | "lu_ide_host_tests"
        ) || job.plan.effect_class != "test_build_isolated"
        {
            return Err(SupervisorError::InvalidPlan);
        }
        if job.plan.project_id != expected_project_id
            || job.plan.source_head != expected_source_head
            || job.plan_hash != expected_plan_sha256
            || fingerprint(&job.plan)? != job.plan_hash
        {
            return Err(SupervisorError::WrongProjectOrSource);
        }
        let reconciled = state.quarantine.contains(job_id);
        let stage = if reconciled {
            BuildTestStage::RecoveryRequired
        } else {
            match job.phase {
                JobPhase::Prepared => BuildTestStage::PreparedNoExecution,
                JobPhase::Authorized => BuildTestStage::ReviewedNoExecution,
                JobPhase::AdmissionPending => BuildTestStage::NativeAdmissionPending,
                JobPhase::Running => BuildTestStage::RunningUnverified,
                JobPhase::ReleasePending => BuildTestStage::ReleasePending,
                JobPhase::CancelRequested => BuildTestStage::CancellationPending,
                JobPhase::Cancelled => BuildTestStage::Cancelled,
                JobPhase::Failed => BuildTestStage::Failed,
                JobPhase::Denied => BuildTestStage::Denied,
                JobPhase::Completed => BuildTestStage::CompletionRequiresNativeEvidence,
                JobPhase::RecoveryRequired | JobPhase::Uncertain => {
                    BuildTestStage::RecoveryRequired
                }
            }
        };
        let receipt = state
            .receipts
            .iter()
            .rev()
            .find(|record| record.body.job_id == job_id)
            .ok_or(SupervisorError::JournalCorrupt)?;
        if receipt.body.project_id != expected_project_id
            || receipt.body.plan_sha256 != expected_plan_sha256
            || receipt.body.phase != job.phase
        {
            return Err(SupervisorError::JournalCorrupt);
        }
        Ok(BuildTestEvidence {
            project_id: job.plan.project_id.clone(),
            job_id: job_id.to_owned(),
            recipe_id: job.plan.recipe_id.clone(),
            source_head: job.plan.source_head.clone(),
            input_snapshot_sha256: job.plan.input_snapshot_sha256.clone(),
            manifest_sha256: job.plan.manifest_sha256.clone(),
            toolchain_sha256: job.plan.toolchain_sha256.clone(),
            plan_sha256: job.plan_hash.clone(),
            terminal_receipt_hash: receipt.entry_hash.clone(),
            stage,
            requires_reconciliation: reconciled,
            // Deliberately FALSE until a *separately verified* native kernel
            // job/result and installed-app attestation joins the receipt.
            // No source-only CI, D1 review, or JobPhase::Completed can flip it.
            os_execution_accepted: false,
            installed_app_accepted: false,
            desktop_run_authorized: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{JobPlan, Supervisor};
    fn fresh() -> (Supervisor, std::path::PathBuf) {
        let name = format!("lu-wo6-evidence-{:032x}.jsonl", rand::random::<u128>());
        let path = std::env::temp_dir().join(name);
        (Supervisor::open(path.clone()).unwrap(), path)
    }
    fn plan() -> JobPlan {
        JobPlan {
            project_id: format!("ide-{}", "a".repeat(32)),
            recipe_id: "rust_tests".to_owned(),
            source_head: "b".repeat(40),
            input_snapshot_sha256: "c".repeat(64),
            manifest_sha256: "d".repeat(64),
            toolchain_sha256: "e".repeat(64),
            debug_target_binding_sha256: None,
            runtime_limit_ms: 30_000,
            memory_limit_bytes: 128 * 1024 * 1024,
            process_limit: 4,
            log_limit_bytes: 32 * 1024,
            effect_class: "test_build_isolated".to_owned(),
        }
    }
    #[test]
    fn prepared_and_reviewed_d1_like_states_never_expose_execution_authority() {
        let (sup, path) = fresh();
        let plan = plan();
        let job = sup.prepare(plan.clone()).unwrap();
        let a = sup
            .review_build_test_evidence(
                &job.job_id,
                &plan.project_id,
                &job.plan_sha256,
                &plan.source_head,
            )
            .unwrap();
        assert_eq!(a.stage, BuildTestStage::PreparedNoExecution);
        assert!(!a.os_execution_accepted);
        assert!(!a.installed_app_accepted);
        assert!(!a.desktop_run_authorized);
        assert_eq!(a.input_snapshot_sha256, plan.input_snapshot_sha256);
        sup.record_human_approval(&job.job_id, &job.plan_sha256, &plan.source_head, true)
            .unwrap();
        let b = sup
            .review_build_test_evidence(
                &job.job_id,
                &plan.project_id,
                &job.plan_sha256,
                &plan.source_head,
            )
            .unwrap();
        assert_eq!(b.stage, BuildTestStage::ReviewedNoExecution);
        assert!(!b.desktop_run_authorized);
        assert_ne!(a.terminal_receipt_hash, b.terminal_receipt_hash);
        drop(sup);
        let reopened = Supervisor::open(path.clone()).unwrap();
        // D3 replay intentionally drops the full executable plan. Do not
        // reconstruct or trust a stale plan from the receipt chain, and do
        // not turn historical review into a post-restart run capability.
        assert_eq!(
            reopened
                .review_build_test_evidence(
                    &job.job_id,
                    &plan.project_id,
                    &job.plan_sha256,
                    &plan.source_head
                )
                .unwrap_err(),
            SupervisorError::InvalidPlan
        );
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn wrong_source_and_plan_hash_are_never_cross_project_evidence() {
        let (sup, path) = fresh();
        let p = plan();
        let job = sup.prepare(p.clone()).unwrap();
        for (project, hash, source) in [
            (
                "ide-anotherproject",
                job.plan_sha256.as_str(),
                p.source_head.as_str(),
            ),
            (p.project_id.as_str(), "f", p.source_head.as_str()),
            (p.project_id.as_str(), job.plan_sha256.as_str(), "0"),
        ] {
            assert_eq!(
                sup.review_build_test_evidence(&job.job_id, project, hash, source)
                    .unwrap_err(),
                SupervisorError::WrongProjectOrSource
            );
        }
        drop(sup);
        std::fs::remove_file(path).unwrap();
    }
}
