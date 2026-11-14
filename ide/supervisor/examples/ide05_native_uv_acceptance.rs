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
    if args.as_slice() != ["--offline-miss"]
        && args.as_slice() != ["--cancel"]
        && args.as_slice() != ["--positive"]
        && args.as_slice() != ["--crash-after-running"]
        && args.as_slice() != ["--recover"]
    {
        return Err("IDE055_DENIED: fixed acceptance or recovery mode only".into());
    }
    let crash = args.as_slice() == ["--crash-after-running"];
    let recover = args.as_slice() == ["--recover"];
    let cancel_before_launch = args.as_slice() == ["--cancel"];
    let positive = args.as_slice() == ["--positive"];
    if (positive || crash || recover) && !cfg!(feature = "ide055-ci-wheelhouse") {
        return Err("IDE055_CI_WHEELHOUSE_FEATURE_REQUIRED".into());
    }
    let recovery_root = std::path::Path::new("/tmp/ide055-ci-hostile-recovery");
    if recover {
        use lu_ide_execution_supervisor::linux_worker::reconcile_linux_interrupted;
        let journal = recovery_root.join("supervisor-receipts.jsonl");
        let reopened = Supervisor::open(journal.clone()).map_err(|e| e.code())?;
        let mut pending = reopened.inspect().map_err(|e| e.code())?;
        // Replayed Running must be quarantined, NOT silently rewritten as
        // RecoveryRequired: that receipt is durably appended only by the
        // explicit kernel-backed reconcile operation below.
        if pending.len() != 1
            || pending[0].phase != JobPhase::Running
            || !pending[0].requires_reconciliation
        {
            return Err("IDE055_RESTART_RUNNING_NOT_QUARANTINED".into());
        }
        let old = pending.remove(0);
        let clean = reconcile_linux_interrupted(&reopened, &old.job_id).map_err(|e| e.code())?;
        if clean.phase != JobPhase::Cancelled
            || clean.requires_reconciliation
            || clean.execution_verified
        {
            return Err("IDE055_HOSTILE_CGROUP_TREE_NOT_INDEPENDENTLY_REAPED".into());
        }
        let after = Supervisor::open(journal).map_err(|e| e.code())?;
        let rows = after.inspect().map_err(|e| e.code())?;
        if rows.len() != 1
            || rows[0].phase != JobPhase::Cancelled
            || rows[0].requires_reconciliation
            || rows[0].execution_verified
        {
            return Err("IDE055_FALSE_EXECUTION_AUTHORITY_ON_RESTART".into());
        }
        let history = after.receipt_history(&old.job_id).map_err(|e| e.code())?;
        let recovery_written = history.iter().any(|item| {
            item.body.phase == JobPhase::RecoveryRequired
                && item.body.outcome_code == "recovery_before_os_reconcile"
        });
        let terminal_written = history.last().is_some_and(|item| {
            item.body.phase == JobPhase::Cancelled
                && item.body.outcome_code == "os_orphan_cleanup_verified"
        });
        if !recovery_written || !terminal_written {
            return Err("IDE055_RESTART_RECOVERY_RECEIPTS_UNVERIFIED".into());
        }
        let source = recovery_root.join("source/pyproject.toml");
        if !fs::read_to_string(source)?.contains("unavailable-ide05-fixture==9999.0.0") {
            return Err("IDE055_ORIGINAL_SOURCE_MUTATED_DURING_HOSTILE_EXIT".into());
        }
        eprintln!("IDE055_HOSTILE_RESTART=PASS; original_job={}; kernel_reaped=true; replay_grant=false; auto_retry=false; source_preserved=true", old.job_id);
        return Ok(());
    }
    // A fixture-owned project/lock: deliberately no cached wheel.
    // This mode tests REAL native execution and denial, not package success.
    let root = if crash {
        recovery_root.to_path_buf()
    } else {
        env::temp_dir().join(format!(
            "ide05-uv-native-{:032x}",
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
        ))
    };
    if crash {
        fs::create_dir(&root)?;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
    }
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
        if positive {
            "[project]\nname = \"ide05-native-fixture\"\nversion = \"0.1.0\"\nrequires-python = \">=3.12\"\ndependencies = [\"ide05-demo==1.0.0\"]\n"
        } else {
            "[project]\nname = \"ide05-native-fixture\"\nversion = \"0.1.0\"\nrequires-python = \">=3.12\"\ndependencies = [\"unavailable-ide05-fixture==9999.0.0\"]\n"
        },
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
    {{ url = "https://files.pythonhosted.org/packages/ide05/unavailable_ide05_fixture-9999.0.0-py3-none-any.whl", hash = "sha256:{}" }},
]
"#,
        "a".repeat(64)
    );
    let wheel_sha = option_env!("LU_IDE05_CI_WHEEL_SHA256").unwrap_or("");
    let positive_lock = format!(
        r#"version = 1
revision = 3
requires-python = ">=3.12"

[[package]]
name = "ide05-native-fixture"
version = "0.1.0"
source = {{ virtual = "." }}
dependencies = [
    {{ name = "ide05-demo" }},
]

[package.metadata]
requires-dist = [{{ name = "ide05-demo", specifier = "==1.0.0" }}]

[[package]]
name = "ide05-demo"
version = "1.0.0"
source = {{ registry = "../runtime/wheelhouse" }}
wheels = [
    {{ path = "ide05_demo-1.0.0-py3-none-any.whl", hash = "sha256:{}" }},
]
"#,
        wheel_sha
    );
    fs::write(
        source.join("uv.lock"),
        if positive { &positive_lock } else { &lock },
    )?;
    let input = publish_input(&source, &staged, "uv_sync_offline").map_err(|e| e.code())?;
    verify_input(&input).map_err(|e| e.code())?;
    let lock_sha = format!(
        "{:x}",
        Sha256::digest(fs::read(input.root.join("uv.lock"))?)
    );
    verify_offline_registry_lock(&input.root, &lock_sha)
        .map_err(|_| "IDE055_FIXED_LOCK_SOURCE_POLICY_DENIED")?;
    let uv_digest = installed_multilang_toolchain_digest("uv_sync_offline").map_err(|error| {
        // CI-only diagnostic: distinguish invalid immutable runtime/pin from
        // absent kernel containment without exposing a user path or a grant.
        eprintln!("IDE055_NATIVE_UV_RUNTIME_GATE_DENIED={}", error.code());
        "IDE055_PINNED_ROOT_OWNED_UV_RUNTIME_ABSENT"
    })?;
    let fixture_preview_sha = if positive {
        "c".repeat(64)
    } else {
        "b".repeat(64)
    };
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
        effect_class: format!("package_uv_sync_offline:{fixture_preview_sha}"),
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
    if !cancel_before_launch && !positive {
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
            // CI owns this fixed synthetic project; print only a bounded
            // diagnostic for a genuine native failure, never arbitrary
            // installed application stderr.
            eprintln!(
                "IDE055_OFFLINE_DENIAL_DIAGNOSTIC={}",
                diagnostic
                    .chars()
                    .take(400)
                    .collect::<String>()
                    .replace('\n', " ")
            );
            return Err("IDE055_OFFLINE_MISS_CAUSE_NOT_EVIDENCED".into());
        }
    }
    // Task 4 CI-only positive case: expose a bounded native diagnostic from
    // the *fixed synthetic* package when uv exits nonzero. Never turn this
    // diagnostic into an execution receipt or expose general project stderr.
    if positive && result.exit_code != Some(0) {
        let diagnostic = stream
            .page(0)
            .chunks
            .iter()
            .map(|chunk| chunk.text.as_str())
            .collect::<String>();
        eprintln!(
            "IDE055_CI_POSITIVE_UV_FAILURE=phase:{:?},exit:{:?},category:{},output_bounded:{},diagnostic:{}",
            result.summary.phase,
            result.exit_code,
            result.stderr_category,
            !result.output_limit_hit,
            diagnostic.chars().take(1000).collect::<String>().replace('\n', " ")
        );
    }
    let expected = if cancel_before_launch {
        JobPhase::Cancelled
    } else if positive {
        JobPhase::Completed
    } else {
        JobPhase::Failed
    };
    if result.summary.phase != expected
        || result.summary.requires_reconciliation
        || (positive && result.exit_code != Some(0))
        || (!positive && !cancel_before_launch && result.exit_code == Some(0))
        || result.output_limit_hit
        || !result.summary.execution_verified && !cancel_before_launch
    {
        return Err("IDE055_REAL_UV_OS_TERMINATION_OR_CANCELLATION_NOT_VERIFIED".into());
    }
    verify_input(&input).map_err(|_| "IDE055_FROZEN_INPUT_MUTATED")?;
    if positive {
        let wheel = std::path::Path::new(
            "/opt/lu-ide/runtimes/uv_sync_offline/wheelhouse/ide05_demo-1.0.0-py3-none-any.whl",
        );
        let verified = ci_positive_wheel_install(&result.output_directory, wheel, wheel_sha)?;
        let module = verified.join("ide05_demo/__init__.py");
        let original = fs::read(&module)?;
        fs::write(&module, b"VALUE = 777\\n")?;
        if ci_positive_wheel_install(&result.output_directory, wheel, wheel_sha).is_ok() {
            return Err("IDE055_HOSTILE_INSTALLED_BYTES_WERE_ACCEPTED".into());
        }
        fs::write(&module, original)?;
        if ci_positive_wheel_install(&result.output_directory, wheel, wheel_sha).is_err() {
            return Err("IDE055_RESTORED_PRIVATE_WHEEL_CANNOT_BE_VERIFIED".into());
        }
        eprintln!(
            "IDE055_REAL_POSITIVE_OFFLINE=PASS; wheel_sha256={}; receipt={}; source_unchanged=true; private_bytes_verified=true; tampered_bytes_denied=true; release_promotion=false",
            wheel_sha,
            prepared.plan_sha256
        );
    }
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
        } else if positive {
            "positive"
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

#[cfg(all(target_os = "linux", feature = "ide055-ci-wheelhouse"))]
fn ci_positive_wheel_install(
    output: &std::path::Path,
    wheel: &std::path::Path,
    expected_sha: &str,
) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
    use sha2::{Digest, Sha256};
    use std::{collections::BTreeSet, fs};
    let hash = |bytes: &[u8]| format!("{:x}", Sha256::digest(bytes));
    if expected_sha.len() != 64 || hash(&fs::read(wheel)?) != expected_sha {
        return Err("IDE055_WHEEL_ARCHIVE_HASH_MISMATCH".into());
    }
    let lib = output.join(".venv/lib");
    let dirs = fs::read_dir(&lib)?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("python3."))
        .collect::<Vec<_>>();
    if dirs.len() != 1 {
        return Err("IDE055_INSTALLED_INTERPRETER_IDENTITY_AMBIGUOUS".into());
    }
    let site = dirs[0].path().join("site-packages");
    let dist = site.join("ide05_demo-1.0.0.dist-info");
    let meta = fs::read(dist.join("METADATA"))?;
    if !meta
        .windows(b"Name: ide05-demo".len())
        .any(|w| w == b"Name: ide05-demo")
        || !meta
            .windows(b"Version: 1.0.0".len())
            .any(|w| w == b"Version: 1.0.0")
        || !meta
            .windows(b"License-Expression: MIT".len())
            .any(|w| w == b"License-Expression: MIT")
        || !meta
            .windows(b"License-File: LICENSE.txt".len())
            .any(|w| w == b"License-File: LICENSE.txt")
    {
        return Err("IDE055_INSTALLED_ORIGINAL_LICENSE_METADATA_DENIED".into());
    }
    if fs::read(site.join("ide05_demo/__init__.py"))? != b"VALUE = 42\n" {
        return Err("IDE055_INSTALLED_MODULE_BYTES_UNEXPECTED".into());
    }
    if fs::read(dist.join("licenses/LICENSE.txt"))?
        != b"MIT License\nCI-only first-party synthetic fixture; no upstream claim.\n"
    {
        return Err("IDE055_INSTALLED_LICENSE_BYTES_CHANGED".into());
    }
    let record = fs::read_to_string(dist.join("RECORD"))?;
    let mut declared = BTreeSet::new();
    for row in record.lines() {
        let fields: Vec<_> = row.split(',').collect();
        if fields.len() != 3 {
            return Err("IDE055_INVALID_REAL_WHEEL_RECORD_CSV".into());
        }
        let rel = fields[0];
        if rel.starts_with('/')
            || rel
                .split('/')
                .any(|segment| segment.is_empty() || segment == "." || segment == "..")
            || !declared.insert(rel.to_owned())
        {
            return Err("IDE055_UNTRUSTED_RECORD_PATH_OR_DUPLICATE".into());
        }
        let target = site.join(rel);
        if rel.ends_with("/RECORD") {
            if !fields[1].is_empty() || !fields[2].is_empty() {
                return Err("IDE055_RECORD_SELF_HASH_DENIED".into());
            }
            continue;
        }
        let meta = fs::symlink_metadata(&target)?;
        if !meta.is_file() || meta.file_type().is_symlink() {
            return Err("IDE055_RECORD_TARGET_NOT_REGULAR".into());
        }
        let bytes = fs::read(target)?;
        let expected = fields[1]
            .strip_prefix("sha256=")
            .ok_or("IDE055_RECORD_UNHASHED_FILE")?;
        if expected != URL_SAFE_NO_PAD.encode(Sha256::digest(&bytes))
            || fields[2].parse::<usize>().ok() != Some(bytes.len())
        {
            return Err("IDE055_REAL_RECORD_CONTENT_HASH_MISMATCH".into());
        }
    }
    for essential in [
        "ide05_demo/__init__.py",
        "ide05_demo-1.0.0.dist-info/WHEEL",
        "ide05_demo-1.0.0.dist-info/METADATA",
        "ide05_demo-1.0.0.dist-info/RECORD",
        "ide05_demo-1.0.0.dist-info/licenses/LICENSE.txt",
    ] {
        if !declared.contains(essential) {
            return Err("IDE055_REQUIRED_WHEEL_RECORD_MISSING".into());
        }
    }
    // Reject undeclared site-packages payload, except the two exact
    // host-runtime virtualenv bootstrap files that uv emits.
    fn walk(
        root: &std::path::Path,
        dir: &std::path::Path,
        observed: &mut BTreeSet<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let meta = fs::symlink_metadata(&path)?;
            if meta.file_type().is_symlink() {
                return Err("IDE055_HOSTILE_SITE_PACKAGES_LINK".into());
            }
            if meta.is_dir() {
                walk(root, &path, observed)?;
            } else if meta.is_file() {
                observed.insert(path.strip_prefix(root)?.to_string_lossy().to_string());
            } else {
                return Err("IDE055_HOSTILE_SITE_PACKAGES_NODE".into());
            }
        }
        Ok(())
    }
    let mut observed = BTreeSet::new();
    walk(&site, &site, &mut observed)?;
    for rel in &observed {
        if !declared.contains(rel) && rel != "_virtualenv.pth" && rel != "_virtualenv.py" {
            return Err("IDE055_UNDECLARED_INSTALLED_FILE_DENIED".into());
        }
    }
    if fs::read(site.join("_virtualenv.pth"))? != b"import _virtualenv" {
        return Err("IDE055_HOSTILE_BOOTSTRAP_PTH".into());
    }
    if !observed.contains("_virtualenv.py") || observed.len() != declared.len() + 2 {
        return Err("IDE055_SITE_PACKAGES_COVERAGE_UNVERIFIED".into());
    }
    Ok(site)
}

#[cfg(not(all(target_os = "linux", feature = "ide055-ci-wheelhouse")))]
fn ci_positive_wheel_install(
    _output: &std::path::Path,
    _wheel: &std::path::Path,
    _expected_sha: &str,
) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    Err("IDE055_CI_WHEELHOUSE_FEATURE_REQUIRED".into())
}
