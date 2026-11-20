//! Fixed test-only native multilang acceptance. No project path, command, or approval supplied by user.
#[cfg(not(all(
    target_os = "linux",
    feature = "linux-worker",
    feature = "multilang-worker"
)))]
fn main() {
    std::process::exit(2);
}
#[cfg(all(
    target_os = "linux",
    feature = "linux-worker",
    feature = "multilang-worker"
))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use lu_ide_execution_supervisor::{
        linux_worker::{
            execute_linux_worker_with_output, installed_multilang_toolchain_digest,
            native_delegated_host_readiness, reconcile_linux_interrupted, RunCancellation,
        },
        live_output::LiveOutput,
        snapshot::publish_input,
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
    let a: Vec<_> = env::args().skip(1).collect();
    let [r, mode] = a.as_slice() else {
        return Err("fixed recipe and mode required".into());
    };
    if !matches!(r.as_str(), "python_pytest" | "js_tests" | "cpp_cmake_build")
        || !matches!(
            mode.as_str(),
            "normal" | "cancel" | "recover" | "fixed-child"
        )
    {
        return Err("invalid fixed recipe/mode".into());
    }
    native_delegated_host_readiness().map_err(|e| e.code())?;
    let root = std::path::PathBuf::from(format!(
        "/tmp/lu-ide-dev03-{}-{}",
        r,
        if mode == "fixed-child" {
            "recover"
        } else {
            mode
        }
    ));
    let source = root.join("source");
    let journal = root.join("receipts.jsonl");
    let output = root.join("output");
    let slow = mode != "normal";
    let files:Vec<(&str,String)>=match r.as_str(){
  "js_tests"=>vec![("package.json","{\"name\":\"lu-fixture\",\"version\":\"1.0.0\",\"type\":\"module\"}\n".into()),("test.test.mjs",format!("import test from 'node:test'; import assert from 'node:assert/strict'; import fs from 'node:fs'; test('fixture', async()=>{{assert.equal(2+2,4); fs.writeFileSync('/output/started','yes'); if ({slow}) await new Promise(r=>setTimeout(r,30000)); console.log('DEV03_FIXED_TEST_PASSED');}});\n"))],
  "python_pytest"=>vec![("pyproject.toml","[project]\nname='lu-fixture'\nversion='1.0.0'\n".into()),("test_fixed.py",format!("import time\nfrom pathlib import Path\ndef test_fixture():\n    assert 2+2==4\n    Path('/output/started').write_text('yes')\n    {}\n",if slow{"time.sleep(30)"}else{"pass"}))],
  _=>vec![("CMakeLists.txt",format!("cmake_minimum_required(VERSION 3.25)\nproject(lu_fixture C)\nenable_testing()\nadd_executable(fixture main.c)\nadd_test(NAME fixture COMMAND fixture)\n")),("main.c",format!("#include <stdio.h>\n#include <unistd.h>\nint main(void){{FILE*f=fopen(\"/output/started\",\"w\");if(!f)return 1;fputs(\"yes\",f);fclose(f);{} return 0;}}\n",if slow{"sleep(30);"}else{""}))]
 };
    if mode != "fixed-child" {
        fs::DirBuilder::new().mode(0o700).create(&root)?;
        for d in ["source", "stage", "output"] {
            fs::DirBuilder::new().mode(0o700).create(root.join(d))?;
        }
        for (n, v) in &files {
            fs::write(source.join(n), v)?;
        }
    }
    let cg = fs::read_to_string("/proc/self/cgroup")?;
    let rel = cg
        .lines()
        .find_map(|l| l.strip_prefix("0::/"))
        .ok_or("missing cgroup")?;
    let host = Path::new("/sys/fs/cgroup").join(rel);
    if host.file_name().and_then(|v| v.to_str()) != Some("lu-ide-host") {
        return Err("unexpected delegated host".into());
    }
    let parent = host.parent().ok_or("missing cgroup parent")?.to_path_buf();
    if mode == "recover" {
        let mut child = Command::new(env::current_exe()?)
            .args([r.as_str(), "fixed-child"])
            .spawn()?;
        let observed = (|| -> Result<_, Box<dyn std::error::Error>> {
            let end = Instant::now() + Duration::from_secs(55);
            while Instant::now() < end {
                if child.try_wait()?.is_some() {
                    return Err("worker ended before interruption".into());
                }
                if let Ok(s) = Supervisor::open(journal.clone()) {
                    if let Ok(rows) = s.inspect() {
                        if let [j] = rows.as_slice() {
                            let group = parent.join(format!("lu-ide-{}", j.job_id));
                            if j.phase == JobPhase::Running
                                && j.requires_reconciliation
                                && fs::read_to_string(group.join("cgroup.events"))
                                    .is_ok_and(|v| v.lines().any(|v| v == "populated 1"))
                                && fs::read_to_string(output.join(&j.job_id).join("started"))
                                    .is_ok_and(|v| v == "yes")
                            {
                                return Ok((j.clone(), group));
                            }
                        }
                    }
                }
                thread::sleep(Duration::from_millis(40));
            }
            Err("no actual fixture start".into())
        })();
        child.kill()?;
        if child.wait()?.signal() != Some(9) {
            return Err("no fixed host SIGKILL".into());
        }
        let (old, group) = observed?;
        let s = Supervisor::open(journal.clone()).map_err(|e| e.code())?;
        let rows = s.inspect().map_err(|e| e.code())?;
        if rows.len() != 1
            || rows[0].job_id != old.job_id
            || rows[0].phase != JobPhase::Running
            || !rows[0].requires_reconciliation
        {
            return Err("not quarantined".into());
        }
        let clean = reconcile_linux_interrupted(&s, &old.job_id).map_err(|e| e.code())?;
        if clean.phase != JobPhase::Cancelled
            || clean.requires_reconciliation
            || clean.execution_verified
        {
            return Err("false recovery".into());
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
            || !history.last().is_some_and(|v| {
                v.body.phase == JobPhase::Cancelled
                    && v.body.outcome_code == "os_orphan_cleanup_verified"
            })
            || reconcile_linux_interrupted(&s, &old.job_id).is_ok()
        {
            return Err("recovery journal or replay denial".into());
        }
        if group.exists()
            && !fs::read_to_string(group.join("cgroup.events"))?
                .lines()
                .any(|v| v == "populated 0")
        {
            return Err("populated orphan".into());
        }
    } else {
        let input = publish_input(&source, &root.join("stage"), r).map_err(|e| e.code())?;
        let plan = JobPlan {
            project_id: format!("ide-{}", "7".repeat(32)),
            recipe_id: r.clone(),
            source_head: "a".repeat(40),
            input_snapshot_sha256: input.seal.source_sha256.clone(),
            manifest_sha256: input.seal.manifest_sha256.clone(),
            toolchain_sha256: installed_multilang_toolchain_digest(r).map_err(|e| e.code())?,
            debug_target_binding_sha256: None,
            runtime_limit_ms: 90_000,
            memory_limit_bytes: 512 * 1024 * 1024,
            process_limit: 16,
            log_limit_bytes: 64 * 1024,
            effect_class: "test_build_isolated".into(),
        };
        let s = Supervisor::open(journal.clone()).map_err(|e| e.code())?;
        let j = s.prepare(plan.clone()).map_err(|e| e.code())?;
        s.record_human_approval(&j.job_id, &j.plan_sha256, &plan.source_head, true)
            .map_err(|e| e.code())?;
        let cancellation = RunCancellation::default();
        let marker = output.join(&j.job_id).join("started");
        let cancel_thread = if mode == "cancel" {
            let flag = cancellation.clone_handle();
            let m = marker.clone();
            Some(thread::spawn(move || {
                let end = Instant::now() + Duration::from_secs(55);
                while Instant::now() < end {
                    if fs::read_to_string(&m).is_ok_and(|v| v == "yes") {
                        flag.request_cancel();
                        return true;
                    }
                    thread::sleep(Duration::from_millis(40));
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
            &cancellation,
            Some(Arc::clone(&live)),
        )
        .map_err(|e| e.code())?;
        if mode == "fixed-child" {
            return Err("fixture completed before interrupt".into());
        }
        let expected = if mode == "cancel" {
            JobPhase::Cancelled
        } else {
            JobPhase::Completed
        };
        if let Some(t) = cancel_thread {
            if !t.join().map_err(|_| "cancel thread failed")? {
                return Err("cancel before actual execution".into());
            }
        }
        eprintln!("DEV03_RESULT recipe={r} mode={mode} phase={:?} exit={:?} category={} timed_out={} output_limit={}",result.summary.phase,result.exit_code,result.stderr_category,result.timed_out,result.output_limit_hit);
        if result.summary.phase != expected
            || result.summary.requires_reconciliation
            || result.timed_out
            || result.output_limit_hit
            || !fs::read_to_string(&marker).is_ok_and(|v| v == "yes")
            || (mode == "normal" && (result.exit_code != Some(0) || live.page(0).chunks.is_empty()))
        {
            return Err("native test result mismatch".into());
        }
        let s2 = Supervisor::open(journal.clone()).map_err(|e| e.code())?;
        let rows = s2.inspect().map_err(|e| e.code())?;
        if rows.len() != 1 || rows[0].phase != expected || rows[0].requires_reconciliation {
            return Err("terminal replay mismatch".into());
        }
    }
    for (n, v) in &files {
        if fs::read_to_string(source.join(n))? != *v {
            return Err("source mutated".into());
        }
    }
    println!("IDE_LINUX07_PROTECTED_PASS recipe={r} mode={mode}");
    Ok(())
}
