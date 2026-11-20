//! Fixed Linux laboratory cache fixture, not installed or human approval.
#[cfg(not(all(
    target_os = "linux",
    feature = "linux-worker",
    feature = "package-vcpkg-worker"
)))]
fn main() {
    std::process::exit(2);
}
#[cfg(all(
    target_os = "linux",
    feature = "linux-worker",
    feature = "package-vcpkg-worker"
))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use lu_ide_execution_supervisor::{
        linux_worker::{
            execute_linux_worker_with_output, installed_multilang_toolchain_digest,
            native_delegated_host_readiness, reconcile_linux_interrupted, RunCancellation,
        },
        live_output::LiveOutput,
        snapshot::{publish_input, verify_input},
        JobPhase, JobPlan, Supervisor,
    };
    use std::{
        env, fs,
        os::unix::{fs::DirBuilderExt, process::ExitStatusExt},
        path::Path,
        process::Command,
        sync::Arc,
        thread,
        time::{Duration, Instant},
    };
    let mode = env::args().nth(1).ok_or("fixed mode required")?;
    if env::args().count() != 2
        || !matches!(
            mode.as_str(),
            "positive" | "miss" | "cancel" | "recover" | "fixed-child"
        )
    {
        return Err("fixed mode denied".into());
    }
    native_delegated_host_readiness().map_err(|e| e.code())?;
    let root = std::path::PathBuf::from(format!(
        "/tmp/ide08-vcpkg-{}",
        if mode == "fixed-child" {
            "recover"
        } else {
            &mode
        }
    ));
    let port = if mode == "miss" {
        "ide08-miss"
    } else {
        "ide08-demo"
    };
    let manifest=format!("{{\"name\":\"ide08-fixture\",\"version-string\":\"1.0.0\",\"dependencies\":[\"{port}\"]}}\n");
    let source = root.join("source");
    let output = root.join("output");
    let journal = root.join("receipts.jsonl");
    if mode != "fixed-child" {
        fs::DirBuilder::new().mode(0o700).create(&root)?;
        for d in ["source", "stage", "output"] {
            fs::DirBuilder::new().mode(0o700).create(root.join(d))?;
        }
        fs::write(source.join("vcpkg.json"), &manifest)?;
    }
    if mode == "recover" {
        let path = fs::read_to_string("/proc/self/cgroup")?;
        let rel = path
            .lines()
            .find_map(|l| l.strip_prefix("0::/"))
            .ok_or("missing cgroup")?;
        let host = Path::new("/sys/fs/cgroup").join(rel);
        if host.file_name().and_then(|n| n.to_str()) != Some("lu-ide-host") {
            return Err("unexpected cgroup".into());
        }
        let parent = host.parent().ok_or("missing parent")?;
        let mut child = Command::new(env::current_exe()?)
            .arg("fixed-child")
            .spawn()?;
        let observed = (|| -> Result<_, Box<dyn std::error::Error>> {
            let end = Instant::now() + Duration::from_secs(60);
            while Instant::now() < end {
                if child.try_wait()?.is_some() {
                    return Err("child completed before interruption".into());
                }
                if let Ok(s) = Supervisor::open(journal.clone()) {
                    if let Ok(rows) = s.inspect() {
                        if let [j] = rows.as_slice() {
                            let group = parent.join(format!("lu-ide-{}", j.job_id));
                            if j.phase == JobPhase::Running
                                && j.requires_reconciliation
                                && output
                                    .join(&j.job_id)
                                    .join("buildtrees/vcpkg-running.lock")
                                    .is_file()
                                && fs::read_to_string(group.join("cgroup.events"))
                                    .is_ok_and(|x| x.lines().any(|l| l == "populated 1"))
                            {
                                return Ok((j.clone(), group));
                            }
                        }
                    }
                }
                thread::sleep(Duration::from_millis(10));
            }
            Err("actual vcpkg launch not observed".into())
        })();
        child.kill()?;
        if child.wait()?.signal() != Some(9) {
            return Err("no actual interruption".into());
        }
        let (old, group) = observed?;
        let s = Supervisor::open(journal.clone()).map_err(|e| e.code())?;
        let rows = s.inspect().map_err(|e| e.code())?;
        if rows.len() != 1 || rows[0].phase != JobPhase::Running || !rows[0].requires_reconciliation
        {
            return Err("quarantine mismatch".into());
        }
        let clean = reconcile_linux_interrupted(&s, &old.job_id).map_err(|e| e.code())?;
        if clean.phase != JobPhase::Cancelled
            || clean.execution_verified
            || clean.requires_reconciliation
        {
            return Err("recovery mismatch".into());
        }
        let s = Supervisor::open(journal.clone()).map_err(|e| e.code())?;
        let rows = s.inspect().map_err(|e| e.code())?;
        let history = s.receipt_history(&old.job_id).map_err(|e| e.code())?;
        if rows.len() != 1
            || rows[0].phase != JobPhase::Cancelled
            || rows[0].requires_reconciliation
            || rows[0].execution_verified
            || !history.iter().any(|v| {
                v.body.phase == JobPhase::RecoveryRequired
                    && v.body.outcome_code == "recovery_before_os_reconcile"
            })
            || !history
                .last()
                .is_some_and(|v| v.body.outcome_code == "os_orphan_cleanup_verified")
            || reconcile_linux_interrupted(&s, &old.job_id).is_ok()
        {
            return Err("durable recovery/replay mismatch".into());
        }
        if group.exists()
            && !fs::read_to_string(group.join("cgroup.events"))?
                .lines()
                .any(|l| l == "populated 0")
        {
            return Err("orphan remains".into());
        }
    } else {
        let recipe = "vcpkg_install_offline";
        let input = publish_input(&source, &root.join("stage"), recipe).map_err(|e| e.code())?;
        let plan = JobPlan {
            project_id: format!("ide-{}", "8".repeat(32)),
            recipe_id: recipe.into(),
            source_head: "a".repeat(40),
            input_snapshot_sha256: input.seal.source_sha256.clone(),
            manifest_sha256: input.seal.manifest_sha256.clone(),
            toolchain_sha256: installed_multilang_toolchain_digest(recipe).map_err(|e| e.code())?,
            debug_target_binding_sha256: None,
            runtime_limit_ms: 90000,
            memory_limit_bytes: 512 * 1024 * 1024,
            process_limit: 12,
            log_limit_bytes: 128 * 1024,
            effect_class: format!("package_vcpkg_install_offline:{}", "b".repeat(64)),
        };
        let s = Supervisor::open(journal.clone()).map_err(|e| e.code())?;
        let j = s.prepare(plan.clone()).map_err(|e| e.code())?;
        s.record_human_approval(&j.job_id, &j.plan_sha256, &plan.source_head, true)
            .map_err(|e| e.code())?;
        let cancel = RunCancellation::default();
        let marker = output.join(&j.job_id).join("buildtrees/vcpkg-running.lock");
        let monitor = if mode == "cancel" {
            let flag = cancel.clone_handle();
            Some(thread::spawn(move || {
                let end = Instant::now() + Duration::from_secs(40);
                while Instant::now() < end {
                    if marker.is_file() {
                        flag.request_cancel();
                        return true;
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                flag.request_cancel();
                false
            }))
        } else {
            None
        };
        let live = Arc::new(LiveOutput::default());
        let result = execute_linux_worker_with_output(
            &s,
            &j.job_id,
            &plan,
            &input,
            &output,
            &cancel,
            Some(Arc::clone(&live)),
        )
        .map_err(|e| e.code())?;
        if mode == "fixed-child" {
            return Err("completed before interruption".into());
        }
        if let Some(t) = monitor {
            if !t.join().map_err(|_| "cancel witness panic")? {
                return Err("no actual launch observed".into());
            }
        }
        let expected = if mode == "cancel" {
            JobPhase::Cancelled
        } else if mode == "miss" {
            JobPhase::Failed
        } else {
            JobPhase::Completed
        };
        let text = live
            .page(0)
            .chunks
            .iter()
            .map(|c| c.text.as_str())
            .collect::<String>();
        eprintln!(
            "IDE08_VCPKG_RESULT mode={mode} phase={:?} exit={:?} category={} timeout={} budget={}",
            result.summary.phase,
            result.exit_code,
            result.stderr_category,
            result.timed_out,
            result.output_limit_hit
        );
        if result.summary.phase != expected
            || result.summary.requires_reconciliation
            || result.timed_out
            || result.output_limit_hit
        {
            eprintln!(
                "FIXED_FIXTURE_DIAGNOSTIC={}",
                text.chars().take(1200).collect::<String>()
            );
            return Err("terminal mismatch".into());
        }
        if mode == "positive" {
            if result.exit_code != Some(0)
                || !text.contains("Restored 1 package(s)")
                || fs::read(
                    result
                        .output_directory
                        .join("installed/x64-linux/include/ide08-demo.h"),
                )? != b"#pragma once\n#define IDE08_DEMO_VALUE 42\n"
            {
                return Err("cache receipt/header mismatch".into());
            }
        }
        if mode == "miss" {
            if result.exit_code == Some(0)
                || !text.contains("Restored 0 package(s)")
                || result
                    .output_directory
                    .join("installed/x64-linux/include/ide08-demo.h")
                    .exists()
            {
                return Err("cache miss not proven".into());
            }
        }
        verify_input(&input).map_err(|e| e.code())?;
        let reopened = Supervisor::open(journal.clone()).map_err(|e| e.code())?;
        let rows = reopened.inspect().map_err(|e| e.code())?;
        if rows.len() != 1
            || rows[0].phase != expected
            || rows[0].requires_reconciliation
            || rows[0].execution_verified
        {
            return Err("terminal restart mismatch".into());
        }
    }
    if fs::read_to_string(source.join("vcpkg.json"))? != manifest {
        return Err("source mutated".into());
    }
    println!("IDE08_REAL_VCPKG_MODE_PASS={mode}; source_unchanged=true; laboratory_only=true");
    Ok(())
}
