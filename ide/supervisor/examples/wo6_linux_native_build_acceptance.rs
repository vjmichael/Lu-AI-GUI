//! WO6 real Linux native Cargo check/test/cancellation under cgroup-v2 + bwrap.
//! Fixed test-only source, no project-provided executable or shell. This
//! acceptance fixture cannot satisfy installed-app/release approval.
#[cfg(not(all(target_os = "linux", feature = "linux-worker")))]
fn main() {
    eprintln!("DENIED: WO6 native Cargo acceptance needs protected Linux worker");
    std::process::exit(2);
}
#[cfg(all(target_os = "linux", feature = "linux-worker"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use lu_ide_execution_supervisor::{
        linux_worker::{execute_linux_worker, installed_toolchain_digest, RunCancellation},
        snapshot::publish_input,
        JobPhase, JobPlan, Supervisor,
    };
    use std::os::unix::fs::PermissionsExt;
    use std::{
        env, fs,
        path::PathBuf,
        thread,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };
    let args: Vec<String> = env::args().skip(1).collect();
    let mode = args.as_slice();
    let (recipe, cancel) = match mode {
        [one] if one == "--check" => ("rust_check", false),
        [one] if one == "--test" => ("rust_tests", false),
        [one] if one == "--cancel" => ("rust_tests", true),
        _ => return Err("DENIED: only fixed --check / --test / --cancel allowed".into()),
    };
    let root = env::temp_dir().join(format!(
        "lu-ide-wo6-cargo-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    let source = root.join("source");
    let stage = root.join("stage");
    let output = root.join("output");
    fs::create_dir_all(source.join("src"))?;
    fs::create_dir_all(&stage)?;
    fs::create_dir_all(&output)?;
    fs::set_permissions(&stage, fs::Permissions::from_mode(0o700))?;
    fs::set_permissions(&output, fs::Permissions::from_mode(0o700))?;
    fs::write(
        source.join("Cargo.toml"),
        "[package]\nname=\"wo6_native_acceptance\"\nversion=\"0.1.0\"\nedition=\"2021\"\n",
    )?;
    // v3 lock remains compatible with independently installed OS Cargo.
    fs::write(
        source.join("Cargo.lock"),
        "version = 3\n\n[[package]]\nname = \"wo6_native_acceptance\"\nversion = \"0.1.0\"\n",
    )?;
    fs::write(
        source.join("src/lib.rs"),
        "#[cfg(test)] mod tests { #[test] fn fixed_pass() { assert_eq!(2+2,4); } }\n",
    )?;
    fs::write(
        source.join("src/main.rs"),
        "fn main(){ println!(\"fixed WO6 isolated job\"); }\n",
    )?;
    let input = publish_input(&source, &stage, recipe).map_err(|e| e.code())?;
    let digest = installed_toolchain_digest().map_err(|e| e.code())?;
    let plan = JobPlan {
        project_id: format!("ide-{}", "7".repeat(32)),
        recipe_id: recipe.into(),
        source_head: "a".repeat(40),
        input_snapshot_sha256: input.seal.source_sha256.clone(),
        manifest_sha256: input.seal.manifest_sha256.clone(),
        toolchain_sha256: digest,
        runtime_limit_ms: 90_000,
        memory_limit_bytes: 512 * 1024 * 1024,
        process_limit: 12,
        log_limit_bytes: 64 * 1024,
        effect_class: "test_build_isolated".into(),
    };
    let journal = root.join("receipts.jsonl");
    let supervisor = Supervisor::open(journal.clone()).map_err(|e| e.code())?;
    let prepared = supervisor.prepare(plan.clone()).map_err(|e| e.code())?;
    supervisor
        .record_human_approval(
            &prepared.job_id,
            &prepared.plan_sha256,
            &plan.source_head,
            true,
        )
        .map_err(|e| e.code())?;
    let cancellation = RunCancellation::default();
    let cancel_thread = if cancel {
        let cloned = cancellation.clone_handle();
        Some(thread::spawn(move || {
            thread::sleep(Duration::from_millis(300));
            cloned.request_cancel();
        }))
    } else {
        None
    };
    let actual = execute_linux_worker(
        &supervisor,
        &prepared.job_id,
        &plan,
        &input,
        &output,
        &cancellation,
    )
    .map_err(|e| e.code())?;
    if let Some(worker) = cancel_thread {
        worker
            .join()
            .map_err(|_| "DENIED: cancel thread panicked")?;
    }
    let expected = if cancel {
        JobPhase::Cancelled
    } else {
        JobPhase::Completed
    };
    eprintln!(
        "WO6_NATIVE_CARGO_FIXED_RESULT phase={:?} exit={:?} stderr_class={} timed_out={} output_limit={} truncated={}",
        actual.summary.phase, actual.exit_code, actual.stderr_category,
        actual.timed_out, actual.output_limit_hit, actual.output_truncated,
    );
    if actual.summary.phase != expected || actual.summary.requires_reconciliation {
        return Err(
            "DENIED: real native Cargo OS worker did not finish with expected terminal phase"
                .into(),
        );
    }
    if !cancel && actual.exit_code != Some(0) {
        return Err("DENIED: native contained Cargo recipe returned failed result".into());
    }
    if cancel && actual.summary.phase != JobPhase::Cancelled {
        return Err("DENIED: requested native process-tree kill missing".into());
    }
    let history = supervisor
        .receipt_history(&prepared.job_id)
        .map_err(|e| e.code())?;
    if cancel
        && !history.windows(2).any(|pair| {
            pair[0].body.phase == JobPhase::CancelRequested
                && pair[1].body.phase == JobPhase::Cancelled
        })
    {
        return Err("DENIED: cancellation receipt not durable before kernel kill".into());
    }
    drop(supervisor);
    let replay = Supervisor::open(journal).map_err(|e| e.code())?;
    let jobs = replay.inspect().map_err(|e| e.code())?;
    if jobs.len() != 1 || jobs[0].phase != expected || jobs[0].requires_reconciliation {
        return Err("DENIED: native Cargo journal restart/reconciliation invalid".into());
    }
    println!("PASS: genuine delegated Linux Cargo {recipe} OS worker, kernel cleanup and restart receipt");
    Ok(())
}
