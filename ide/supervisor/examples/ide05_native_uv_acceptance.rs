//! IDE-05.5 native *negative* acceptance: execute a real, exact uv binary
//! inside the existing delegated Linux cgroup and unnetworked Bubblewrap.
//! This tests an intentional OFFLINE MISS; it is NOT a successful install.
//! Fixed CI review is NOT a human approval and cannot promote Lu releases.
#[cfg(not(all(
    target_os = "linux",
    feature = "package-uv-worker",
    feature = "linux-worker"
)))]
fn main() {
    eprintln!("IDE055_DENIED: real Linux protected uv worker features required");
    std::process::exit(2);
}
#[cfg(all(
    target_os = "linux",
    feature = "package-uv-worker",
    feature = "linux-worker"
))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use lu_ide_execution_supervisor::{
        linux_worker::{
            execute_linux_worker_with_output, installed_multilang_toolchain_digest, RunCancellation,
        },
        live_output::LiveOutput,
        package_output_evidence::verify_offline_registry_lock,
        snapshot::{publish_input, verify_input},
        JobPhase, JobPlan, Supervisor,
    };
    use sha2::{Digest, Sha256};
    use std::{
        env, fs,
        os::unix::fs::PermissionsExt,
        sync::Arc,
        time::{SystemTime, UNIX_EPOCH},
    };
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.as_slice() != ["--offline-miss"] && args.as_slice() != ["--cancel"] {
        return Err("IDE055_DENIED: only fixed --offline-miss/--cancel mode".into());
    }
    let cancel_before_launch = args.as_slice() == ["--cancel"];
    // A fixture-owned project/lock: deliberately no cached wheel.
    // This mode tests REAL native execution and denial, not package success.
    let root = env::temp_dir().join(format!(
        "ide05-uv-native-{:032x}",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    let source = root.join("source");
    let staged = root.join("staged");
    let output = root.join("output");
    fs::create_dir_all(&source)?;
    fs::create_dir_all(&staged)?;
    fs::create_dir_all(&output)?;
    fs::set_permissions(&staged, fs::Permissions::from_mode(0o700))?;
    fs::set_permissions(&output, fs::Permissions::from_mode(0o700))?;
    fs::write(
        source.join("pyproject.toml"),
        r#"[project]
name = "ide05-native-fixture"
version = "0.1.0"
requires-python = ">=3.12"
dependencies = ["unavailable-ide05-fixture==9999.0.0"]
"#,
    )?;
    // The locked registry wheel is deliberately unavailable locally.
    // Source policy must pass, but the native unnetworked uv process
    // must fail WITHOUT fetching it or executing any build hook.
    let lock = format!(
        r#"version = 1
requires-python = ">=3.12"

[[package]]
name = "ide05-native-fixture"
version = "0.1.0"
source = {{ virtual = "." }}
dependencies = [
    {{ name = "unavailable-ide05-fixture" }},
]

[[package]]
name = "unavailable-ide05-fixture"
version = "9999.0.0"
source = {{ registry = "https://pypi.org/simple" }}
wheels = [
    {{ url = "https://files.pythonhosted.org/packages/ide05/unavailable-ide05-fixture-9999.0.0-py3-none-any.whl", hash = "sha256:{}" }},
]
"#,
        "a".repeat(64)
    );
    fs::write(source.join("uv.lock"), lock)?;
    let input = publish_input(&source, &staged, "uv_sync_offline").map_err(|e| e.code())?;
    verify_input(&input).map_err(|e| e.code())?;
    let lock_sha = format!(
        "{:x}",
        Sha256::digest(fs::read(input.root.join("uv.lock"))?)
    );
    verify_offline_registry_lock(&input.root, &lock_sha)
        .map_err(|_| "IDE055_FIXED_LOCK_SOURCE_POLICY_DENIED")?;
    let uv_digest = installed_multilang_toolchain_digest("uv_sync_offline")
        .map_err(|_| "IDE055_PINNED_ROOT_OWNED_UV_RUNTIME_ABSENT")?;
    let plan = JobPlan {
        project_id: format!("ide-{}", "5".repeat(32)),
        recipe_id: "uv_sync_offline".into(),
        source_head: "a".repeat(40),
        input_snapshot_sha256: input.seal.source_sha256.clone(),
        manifest_sha256: input.seal.manifest_sha256.clone(),
        toolchain_sha256: uv_digest.clone(),
        debug_target_binding_sha256: None,
        runtime_limit_ms: 180_000,
        memory_limit_bytes: 512 * 1024 * 1024,
        process_limit: 8,
        log_limit_bytes: 128 * 1024,
        effect_class: format!("package_uv_sync_offline:{}", "b".repeat(64)),
    };
    let journal = root.join("supervisor-receipts.jsonl");
    let supervisor = Supervisor::open(journal.clone()).map_err(|e| e.code())?;
    let prepared = supervisor.prepare(plan.clone()).map_err(|e| e.code())?;
    // CI-only fixture admission, NOT a real human/desktop modal.
    supervisor
        .record_human_approval(
            &prepared.job_id,
            &prepared.plan_sha256,
            &plan.source_head,
            true,
        )
        .map_err(|e| e.code())?;
    let cancellation = RunCancellation::default();
    if cancel_before_launch {
        // Bounded, deterministic cancellation request before any possible
        // package download. This is not a human cancel button acceptance.
        cancellation.request_cancel();
    }
    let stream = Arc::new(LiveOutput::default());
    let result = execute_linux_worker_with_output(
        &supervisor,
        &prepared.job_id,
        &plan,
        &input,
        &output,
        &cancellation,
        Some(Arc::clone(&stream)),
    )
    .map_err(|e| e.code())?;
    if !cancel_before_launch {
        let page = stream.page(0);
        let diagnostic = page
            .chunks
            .iter()
            .map(|chunk| chunk.text.to_ascii_lowercase())
            .collect::<String>();
        if !(diagnostic.contains("offline")
            || diagnostic.contains("cache")
            || diagnostic.contains("network disabled"))
        {
            return Err("IDE055_OFFLINE_MISS_CAUSE_NOT_EVIDENCED".into());
        }
    }
    let expected = if cancel_before_launch {
        JobPhase::Cancelled
    } else {
        JobPhase::Failed
    };
    if result.summary.phase != expected
        || result.summary.requires_reconciliation
        || (!cancel_before_launch && result.exit_code == Some(0))
        || result.output_limit_hit
    {
        return Err("IDE055_REAL_UV_OS_TERMINATION_OR_CANCELLATION_NOT_VERIFIED".into());
    }
    verify_input(&input).map_err(|_| "IDE055_FROZEN_INPUT_MUTATED")?;
    let evidence = supervisor
        .review_protected_package_evidence(
            &prepared.job_id,
            &plan.project_id,
            &prepared.plan_sha256,
            &plan.source_head,
        )
        .map_err(|_| "IDE055_D3_ORIGINAL_FAILURE_RECEIPT_UNVERIFIED")?;
    if evidence.requires_reconciliation
        || evidence.recipe_id != "uv_sync_offline"
        || evidence.terminal_receipt_hash.len() != 64
    {
        return Err("IDE055_ORIGINAL_OS_FAILURE_RECEIPT_UNTRUSTED".into());
    }
    let replay = Supervisor::open(journal).map_err(|_| "IDE055_RESTART_JOURNAL_INVALID")?;
    let job = replay
        .inspect()
        .map_err(|_| "IDE055_RESTART_INSPECT_DENIED")?
        .into_iter()
        .find(|j| j.job_id == prepared.job_id)
        .ok_or("IDE055_RESTART_JOB_NOT_FOUND")?;
    if job.phase != expected || job.requires_reconciliation || job.execution_verified {
        return Err("IDE055_FALSE_RESTART_EXECUTION_AUTHORITY".into());
    }
    eprintln!(
        "IDE055_REAL_PROTECTED_UV_OS_MODE=PASS; mode={}; job={}; plan={}; \
        terminal={}; runtime={}; native_exit={:?}; source_preserved=true; \
        restart_not_authorized=true; CI_SYNTHETIC_CONSENT_NOT_HUMAN=true; \
        INSTALLED_DEPENDENCIES_UNVERIFIED=true",
        if cancel_before_launch {
            "cancel"
        } else {
            "offline_miss"
        },
        prepared.job_id,
        prepared.plan_sha256,
        evidence.terminal_receipt_hash,
        uv_digest,
        result.exit_code,
    );
    Ok(())
}
