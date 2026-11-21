//! Disposable fixed native Cargo recovery witness. Never installed or launched by the IDE.
#[cfg(not(all(target_os = "linux", feature = "linux-worker")))]
fn main() {
    std::process::exit(2);
}
#[cfg(all(target_os = "linux", feature = "linux-worker"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use lu_ide_execution_supervisor::{
        linux_worker::{
            execute_linux_worker_with_output, installed_toolchain_digest,
            native_delegated_host_readiness, reconcile_linux_interrupted, RunCancellation,
        },
        snapshot::publish_input,
        JobPhase, JobPlan, Supervisor,
    };
    use std::{
        env, fs,
        os::unix::{fs::DirBuilderExt, process::ExitStatusExt},
        path::Path,
        process::Command,
        thread,
        time::{Duration, Instant},
    };
    let root = Path::new("/tmp/lu-ide-wo6-r3-interrupted");
    let journal = root.join("receipts.jsonl");
    let args: Vec<_> = env::args().skip(1).collect();
    native_delegated_host_readiness().map_err(|e| e.code())?;
    if args.as_slice() == ["--fixed-worker"] {
        let source = root.join("source");
        let stage = root.join("stage");
        let output = root.join("output");
        let input = publish_input(&source, &stage, "rust_tests").map_err(|e| e.code())?;
        let plan = JobPlan {
            project_id: format!("ide-{}", "7".repeat(32)),
            recipe_id: "rust_tests".into(),
            source_head: "a".repeat(40),
            input_snapshot_sha256: input.seal.source_sha256.clone(),
            manifest_sha256: input.seal.manifest_sha256.clone(),
            toolchain_sha256: installed_toolchain_digest().map_err(|e| e.code())?,
            debug_target_binding_sha256: None,
            runtime_limit_ms: 90_000,
            memory_limit_bytes: 512 * 1024 * 1024,
            process_limit: 12,
            log_limit_bytes: 64 * 1024,
            effect_class: "test_build_isolated".into(),
        };
        let supervisor = Supervisor::open(journal).map_err(|e| e.code())?;
        let prepared = supervisor.prepare(plan.clone()).map_err(|e| e.code())?;
        // This is an isolated, fixed CI test grant; it is not human or release approval.
        supervisor
            .record_human_approval(
                &prepared.job_id,
                &prepared.plan_sha256,
                &plan.source_head,
                true,
            )
            .map_err(|e| e.code())?;
        let _ = execute_linux_worker_with_output(
            &supervisor,
            &prepared.job_id,
            &plan,
            &input,
            &output,
            &RunCancellation::default(),
            None,
        )
        .map_err(|e| e.code())?;
        return Err("fixture completed before interruption".into());
    }
    if !args.is_empty() {
        return Err("fixed recovery fixture accepts no user arguments".into());
    }
    fs::DirBuilder::new().mode(0o700).create(root)?;
    for dir in ["source", "source/src", "stage", "output"] {
        fs::DirBuilder::new().mode(0o700).create(root.join(dir))?;
    }
    let manifest = "[package]\nname=\"wo6_interrupted\"\nversion=\"0.1.0\"\nedition=\"2021\"\n";
    let lock = "version = 3\n\n[[package]]\nname = \"wo6_interrupted\"\nversion = \"0.1.0\"\n";
    let source = "#[cfg(test)] mod tests { #[test] fn fixed_wait() { std::thread::sleep(std::time::Duration::from_secs(30)); } }\n";
    for (name, content) in [
        ("Cargo.toml", manifest),
        ("Cargo.lock", lock),
        ("src/lib.rs", source),
    ] {
        fs::write(root.join("source").join(name), content)?;
    }
    let self_group = fs::read_to_string("/proc/self/cgroup")?;
    let rel = self_group
        .lines()
        .find_map(|line| line.strip_prefix("0::/"))
        .ok_or("missing kernel cgroup")?;
    let host = Path::new("/sys/fs/cgroup").join(rel);
    if host.file_name().and_then(|x| x.to_str()) != Some("lu-ide-host") {
        return Err("unexpected host leaf".into());
    }
    let parent = host.parent().ok_or("missing delegated parent")?;
    let mut child = Command::new(env::current_exe()?)
        .arg("--fixed-worker")
        .env_clear()
        .env("HOME", "/tmp")
        .env("LANG", "C")
        .spawn()?;
    let observed = (|| -> Result<_, Box<dyn std::error::Error>> {
        let deadline = Instant::now() + Duration::from_secs(45);
        while Instant::now() < deadline {
            if child.try_wait()?.is_some() {
                return Err("fixture exited before verified running".into());
            }
            if let Ok(sup) = Supervisor::open(journal.clone()) {
                if let Ok(rows) = sup.inspect() {
                    if let [job] = rows.as_slice() {
                        if job.phase == JobPhase::Running && job.requires_reconciliation {
                            let group = parent.join(format!("lu-ide-{}", job.job_id));
                            if fs::read_to_string(group.join("cgroup.events"))
                                .is_ok_and(|s| s.lines().any(|v| v == "populated 1"))
                            {
                                return Ok((job.clone(), group));
                            }
                        }
                    }
                }
            }
            thread::sleep(Duration::from_millis(30));
        }
        Err("no real running populated group before deadline".into())
    })();
    // Only terminate the fixture child spawned above; never accept a caller-supplied PID.
    child.kill()?;
    if child.wait()?.signal() != Some(9) {
        return Err("fixture host interruption was not SIGKILL".into());
    }
    let (old, group) = observed?;
    let reopened = Supervisor::open(journal.clone()).map_err(|e| e.code())?;
    let rows = reopened.inspect().map_err(|e| e.code())?;
    if rows.len() != 1
        || rows[0].job_id != old.job_id
        || rows[0].phase != JobPhase::Running
        || !rows[0].requires_reconciliation
    {
        return Err("original interrupted job not quarantined".into());
    }
    let clean = reconcile_linux_interrupted(&reopened, &old.job_id).map_err(|e| e.code())?;
    if clean.phase != JobPhase::Cancelled
        || clean.requires_reconciliation
        || clean.execution_verified
    {
        return Err("false recovery or execution evidence".into());
    }
    let after = Supervisor::open(journal).map_err(|e| e.code())?;
    let rows = after.inspect().map_err(|e| e.code())?;
    let history = after.receipt_history(&old.job_id).map_err(|e| e.code())?;
    if rows.len() != 1
        || rows[0].phase != JobPhase::Cancelled
        || rows[0].requires_reconciliation
        || rows[0].execution_verified
        || !history.iter().any(|r| {
            r.body.phase == JobPhase::RecoveryRequired
                && r.body.outcome_code == "recovery_before_os_reconcile"
        })
        || !history.last().is_some_and(|r| {
            r.body.phase == JobPhase::Cancelled
                && r.body.outcome_code == "os_orphan_cleanup_verified"
        })
        || reconcile_linux_interrupted(&after, &old.job_id).is_ok()
    {
        return Err("durable recovery or replay denial failed".into());
    }
    if group.exists()
        && !fs::read_to_string(group.join("cgroup.events"))?
            .lines()
            .any(|v| v == "populated 0")
    {
        return Err("worker group still populated".into());
    }
    for (name, content) in [
        ("Cargo.toml", manifest),
        ("Cargo.lock", lock),
        ("src/lib.rs", source),
    ] {
        if fs::read_to_string(root.join("source").join(name))? != content {
            return Err("source changed".into());
        }
    }
    println!("IDE_LINUX05_INTERRUPTED_RECOVERY_PASS: durable Running, kernel population, host death, quarantined replay, verified cleanup, durable Cancelled, second recovery denied, source preserved");
    Ok(())
}
