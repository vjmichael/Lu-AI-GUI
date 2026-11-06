//! Opt-in D7 native acceptance. Not the installed IDE and not a product API.
//! Set LU_IDE_D7_NATIVE_TOOLCHAIN to a host-provisioned complete offline
//! Windows toolchain bundle, then run:
//! cargo run --manifest-path ide/supervisor/Cargo.toml --example d7_windows_acceptance --features windows-worker
//! This fixture supplies its own isolated test-only approval, never a user job.
#[cfg(not(windows))]
fn main() {
    eprintln!("DENIED: Windows-only native acceptance");
    std::process::exit(2);
}

#[cfg(all(windows, not(feature = "protected-recovery-service")))]
fn require_independent_broker_release(
    root: &std::path::Path,
    job_id: &str,
    hash: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use lu_ide_execution_supervisor::windows_broker::inspect_untrusted_witness;
    use std::{
        thread,
        time::{Duration, Instant},
    };
    let start = Instant::now();
    loop {
        if let Ok(states) = inspect_untrusted_witness(root, job_id, hash) {
            if states.first().map(String::as_str) == Some("ready")
                && states.last().map(String::as_str) == Some("released_empty")
            {
                return Ok(());
            }
        }
        if start.elapsed() >= Duration::from_secs(8) {
            return Err("DENIED: independent broker did not fsync normal release".into());
        }
        thread::sleep(Duration::from_millis(40));
    }
}

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use lu_ide_execution_supervisor::{
        snapshot::publish_input,
        windows_broker::TrustedBroker,
        windows_worker::{
            execute_windows_worker_after_staging_approval, trusted_offline_toolchain_digest,
            WindowsCancellation,
        },
        JobPhase, JobPlan, Supervisor,
    };
    use sha2::{Digest, Sha256};
    use std::{env, fs, io::Read, path::PathBuf};

    // Fixed acceptance modes only. No caller-selected project executable,
    // arguments, environment, source or toolchain plan can be injected.
    let args: Vec<String> = env::args().skip(1).collect();
    let cancel_mode = args.len() == 1 && args[0] == "--cancel-native-worker";
    if !args.is_empty() && !cancel_mode {
        return Err("DENIED: unsupported native worker acceptance mode".into());
    }
    let bundle = PathBuf::from(
        env::var_os("LU_IDE_D7_NATIVE_TOOLCHAIN")
            .ok_or("DENIED: explicit native acceptance toolchain is required")?,
    );
    if !bundle.is_absolute() {
        return Err("DENIED: bundle path must be absolute".into());
    }
    let bundle_digest = trusted_offline_toolchain_digest(&bundle)
        .map_err(|e| format!("DENIED: offline toolchain digest: {}", e.code()))?;
    let nonce = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    );
    let root = env::temp_dir().join(format!("lu-ide-d7-native-acceptance-{nonce}"));
    let project = root.join("project");
    let stage_parent = root.join("stage");
    let native_outputs = root.join("native-output");
    let broker_receipts = root.join("independent-broker-receipts");
    // Protected D7D uses the independently installed SCM executable, not
    // the legacy D7C same-user recovery broker. The exact service binary
    // is authenticated against the kernel service PID and its SHA-256 by
    // windows_service_client before any child resumes.
    #[cfg(feature = "protected-recovery-service")]
    let expected_exe_env = "LU_IDE_D7_PROTECTED_SERVICE_EXE";
    #[cfg(not(feature = "protected-recovery-service"))]
    let expected_exe_env = "LU_IDE_D7_BROKER_EXE";
    let broker_bin = PathBuf::from(
        env::var_os(expected_exe_env)
            .ok_or("DENIED: exact independently provisioned native recovery executable required")?,
    );
    if !broker_bin.is_absolute() {
        return Err("DENIED: broker path must be absolute".into());
    }
    // The acceptance harness self-measures this test binary. Production
    // host must instead supply an independently reviewed installation pin.
    let mut reader = fs::File::open(&broker_bin)?;
    let mut hasher = Sha256::new();
    let mut bytes = [0_u8; 64 * 1024];
    loop {
        let n = reader.read(&mut bytes)?;
        if n == 0 {
            break;
        }
        hasher.update(&bytes[..n]);
    }
    let broker_pin = format!("{:x}", hasher.finalize());
    let native_broker = TrustedBroker {
        executable: broker_bin,
        expected_sha256: broker_pin,
        private_witness_directory: broker_receipts.clone(),
    };
    fs::create_dir_all(project.join("src"))?;
    fs::create_dir(&stage_parent)?;
    fs::create_dir(&native_outputs)?;
    fs::create_dir(&broker_receipts)?;
    fs::write(
        project.join("Cargo.toml"),
        "[package]\nname = \"d7_acceptance\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )?;
    fs::write(project.join("Cargo.lock"),
        "# This file is automatically @generated by Cargo.\nversion = 4\n\n[[package]]\nname = \"d7_acceptance\"\nversion = \"0.1.0\"\n")?;
    fs::write(
        project.join("src/main.rs"),
        "fn main() { println!(\"hello isolated worker\"); }\n",
    )?;

    let input = publish_input(&project, &stage_parent, "rust_check")
        .map_err(|e| format!("DENIED: snapshot publication: {}", e.code()))?;
    let supervisor = Supervisor::open(root.join("supervisor-receipts.jsonl"))
        .map_err(|e| format!("DENIED: supervisor journal: {}", e.code()))?;
    let plan = JobPlan {
        project_id: format!("ide-{}", "7".repeat(32)),
        recipe_id: "rust_check".into(),
        source_head: "a".repeat(40),
        input_snapshot_sha256: input.seal.source_sha256.clone(),
        manifest_sha256: input.seal.manifest_sha256.clone(),
        toolchain_sha256: bundle_digest,
        runtime_limit_ms: 90_000,
        memory_limit_bytes: 512 * 1024 * 1024,
        process_limit: 12,
        log_limit_bytes: 65536,
        effect_class: "test_build_isolated".into(),
    };
    let prepared = supervisor
        .prepare(plan.clone())
        .map_err(|e| format!("DENIED: prepare: {}", e.code()))?;
    // The source fixture cannot start a child until a fresh one-use grant
    // exists. The large pinned offline MSVC/SDK bundle is staged first:
    // creating a grant here previously let it expire during the copy.
    let cancellation = WindowsCancellation::new();
    if cancel_mode {
        cancellation.request_cancel();
    }
    let result = execute_windows_worker_after_staging_approval(
        &supervisor,
        &prepared.job_id,
        &plan,
        &plan.source_head,
        &input,
        &bundle,
        &native_outputs,
        &native_broker,
        &cancellation,
        || {
            // Test-only simulated approval, after trusted stage + digest and
            // before admission. No production desktop grant is inferred.
            supervisor
                .record_human_approval(
                    &prepared.job_id,
                    &prepared.plan_sha256,
                    &plan.source_head,
                    true,
                )
                .map(|_| ())
        },
    )
    .map_err(|e| format!("DENIED: OS worker: {}", e.code()))?;
    // D7C test mode can only audit its same-user journal as diagnostic data.
    // D7D protected mode cannot read the service's ACL-isolated journal:
    // execute_windows_worker instead waits for an authenticated independent
    // SCM service ACK after its protected fsynced "released_empty" receipt.
    #[cfg(not(feature = "protected-recovery-service"))]
    require_independent_broker_release(&broker_receipts, &prepared.job_id, &prepared.plan_sha256)?;
    if cancel_mode {
        if result.summary.phase != JobPhase::Cancelled
            || result.output_root.is_dir()
            || !result.output_sha256.is_empty()
        {
            return Err("DENIED: cancellation leaked published outputs or false completion".into());
        }
        let phases: Vec<JobPhase> = supervisor
            .receipt_history(&prepared.job_id)
            .map_err(|e| format!("DENIED: cancellation receipts: {}", e.code()))?
            .into_iter()
            .map(|entry| entry.body.phase)
            .collect();
        if !phases
            .windows(2)
            .any(|pair| pair == [JobPhase::CancelRequested, JobPhase::Cancelled])
        {
            return Err("DENIED: cancellation was not durably recorded before kernel kill".into());
        }
        drop(supervisor);
        let reopened = Supervisor::open(root.join("supervisor-receipts.jsonl"))
            .map_err(|e| format!("DENIED: cancelled journal replay: {}", e.code()))?;
        let state = reopened
            .inspect()
            .map_err(|e| format!("DENIED: cancelled replay: {}", e.code()))?;
        if state[0].phase != JobPhase::Cancelled || state[0].requires_reconciliation {
            return Err("DENIED: cancelled Job did not survive journal reopen".into());
        }
        println!("PASS: real native Windows Cargo Job cancelled with verified tree kill, no outputs, and durable restart receipt");
        return Ok(());
    }
    // Debug values are bounded status/receipt metadata only. No project
    // paths, model output, service credentials or source bytes are logged.
    eprintln!(
        "NATIVE_CARGO_ACCEPTANCE phase={:?} exit_code={:?} timed_out={} output_bytes={} digest_present={}",
        result.summary.phase, result.exit_code, result.timed_out,
        result.output_bytes, result.output_sha256.len() == 64,
    );
    if result.exit_code != Some(0)
        && cfg!(debug_assertions)
        && std::env::var_os("LU_IDE_NATIVE_ACCEPTANCE_TRACE").as_deref()
            == Some(std::ffi::OsStr::new("1"))
    {
        // The acceptance source is synthetic, not a user's project. Only
        // report a small sanitized prefix of the OS-contained Cargo log,
        // after the kernel tree is empty and the output seal is published.
        let path = result.output_root.join("cargo-diagnostics.log");
        if let Ok(metadata) = fs::symlink_metadata(&path) {
            if metadata.is_file()
                && !metadata.file_type().is_symlink()
                && metadata.len() <= 64 * 1024
            {
                if let Ok(bytes) = fs::read(&path) {
                    let diagnostic = String::from_utf8_lossy(&bytes[..bytes.len().min(4096)])
                        .chars()
                        .filter(|c| c.is_ascii_graphic() || *c == '\n' || *c == '\t' || *c == '\r')
                        .collect::<String>();
                    eprintln!("NATIVE_CARGO_DIAGNOSTICS_BEGIN");
                    eprintln!("{diagnostic}");
                    eprintln!("NATIVE_CARGO_DIAGNOSTICS_END");
                }
            }
        }
    }
    if result.summary.phase != JobPhase::Completed
        || result.exit_code != Some(0)
        || result.output_sha256.len() != 64
        || !result.output_root.is_dir()
    {
        return Err(format!(
            "DENIED: incomplete native acceptance: {:?}",
            result.summary.phase
        )
        .into());
    }
    let last = supervisor
        .receipt_history(&prepared.job_id)
        .map_err(|e| format!("DENIED: receipt replay: {}", e.code()))?
        .pop()
        .ok_or("DENIED: missing completion receipt")?;
    if last.body.phase != JobPhase::Completed
        || !last.body.evidence_code.contains(&result.output_sha256)
    {
        return Err("DENIED: publication not bound to durable receipt".into());
    }
    drop(supervisor);
    let reopened = Supervisor::open(root.join("supervisor-receipts.jsonl"))
        .map_err(|e| format!("DENIED: journal reopen: {}", e.code()))?;
    if reopened
        .inspect()
        .map_err(|e| format!("DENIED: replay: {}", e.code()))?
        .iter()
        .any(|x| x.requires_reconciliation)
    {
        return Err("DENIED: unexpected incomplete journal after restart".into());
    }
    #[cfg(feature = "protected-recovery-service")]
    println!("PASS: native protected SCM service + real AppContainer Cargo check, Job quotas, signed-by-service release ACK, published output digest, durable reopen");
    #[cfg(not(feature = "protected-recovery-service"))]
    println!("PASS: native AppContainer Cargo check, Job quotas, published output digest, durable reopen");
    println!("OUTPUT: {}", result.output_root.display());
    println!(
        "NOTE: test fixture only; untested hostile crashes and installed desktop remain separate"
    );
    Ok(())
}
