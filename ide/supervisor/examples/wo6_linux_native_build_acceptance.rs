//! WO6 real Linux native Cargo check/test/cancellation under cgroup-v2 + bwrap.
//! Fixed test-only source, no project-provided executable or shell. This
//! acceptance fixture cannot satisfy installed-app/release approval.
#[cfg(not(all(
    target_os = "linux",
    feature = "linux-worker",
    feature = "dap-transport"
)))]
fn main() {
    eprintln!("DENIED: WO6 native Cargo acceptance needs protected Linux worker");
    std::process::exit(2);
}
#[cfg(all(
    target_os = "linux",
    feature = "linux-worker",
    feature = "dap-transport"
))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use lu_ide_execution_supervisor::{
        debug_binary_witness::audit_debug_binary_candidate,
        debug_symbol_verification::verify_build_debug_symbols,
        linux_worker::lsp_native::{native_lsp_bundle_digest, start_linux_project_dap},
        linux_worker::{
            execute_linux_worker_with_output, installed_toolchain_digest, RunCancellation,
        },
        live_output::LiveOutput,
        project_debug_target::{seal_project_debug_target, CompletedBuildLineage},
        snapshot::publish_input,
        wo6_output_evidence::audit_published_output,
        JobPhase, JobPlan, Supervisor,
    };
    use lu_ide_workspace::dap_session::{DapEvent, DapPhase};
    use std::os::unix::fs::PermissionsExt;
    use std::{
        env, fs,
        path::PathBuf,
        sync::Arc,
        thread,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };
    let args: Vec<String> = env::args().skip(1).collect();
    let mode = args.as_slice();
    let project_debug = mode == ["--project-debug"];
    let (recipe, cancel) = match mode {
        [one] if one == "--check" => ("rust_check", false),
        [one] if one == "--test" => ("rust_tests", false),
        [one] if one == "--build" => ("rust_build", false),
        [one] if one == "--project-debug" => ("rust_build", false),
        [one] if one == "--cancel" => ("rust_tests", true),
        _ => return Err("DENIED: only fixed check/test/build/project-debug/cancel modes".into()),
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
        debug_target_binding_sha256: None,
        runtime_limit_ms: 90_000,
        memory_limit_bytes: 512 * 1024 * 1024,
        process_limit: 12,
        log_limit_bytes: 64 * 1024,
        effect_class: "test_build_isolated".into(),
    };
    let journal = root.join("receipts.jsonl");
    let supervisor = Arc::new(Supervisor::open(journal.clone()).map_err(|e| e.code())?);
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
    let live = Arc::new(LiveOutput::default());
    let actual = execute_linux_worker_with_output(
        &supervisor,
        &prepared.job_id,
        &plan,
        &input,
        &output,
        &cancellation,
        Some(Arc::clone(&live)),
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
    // Real cgroup/bwrap worker bytes, not a synthetic Cursor unit test.
    // The Cargo test recipe emits fixed compiler/test-harness diagnostics.
    if !cancel && recipe == "rust_tests" {
        let page = live.page(0);
        if page.chunks.is_empty()
            || !page.live_unverified
            || !page
                .chunks
                .iter()
                .all(|entry| matches!(entry.stream, "stdout" | "stderr"))
        {
            return Err(
                "DENIED: protected Linux Cargo produced no bounded real native stream".into(),
            );
        }
        if page
            .chunks
            .iter()
            .map(|entry| entry.text.len())
            .sum::<usize>()
            > 65536
        {
            return Err("DENIED: live Linux Cargo stream exceeded host memory cap".into());
        }
        eprintln!(
            "DEV03B_LINUX_REAL_CGROUP_STDIO=PASS streams={} truncated={}",
            page.chunks.len(),
            page.truncated
        );
    }
    if !cancel && actual.exit_code != Some(0) {
        return Err("DENIED: native contained Cargo recipe returned failed result".into());
    }
    if !cancel && recipe == "rust_build" {
        // This is a real kernel-confined, offline Cargo build, not a mocked
        // file or an arbitrary project-supplied executable selector.
        let output_witness = audit_published_output(&actual.output_directory)
            .map_err(|_| "DENIED: real completed Cargo output audit failed")?;
        let candidate = audit_debug_binary_candidate(&actual.output_directory, &output_witness)
            .map_err(|_| "DENIED: real completed Cargo binary readback changed")?
            .ok_or("DENIED: real Cargo build had no unique executable")?;
        let receipt = supervisor
            .review_build_test_evidence(
                &prepared.job_id,
                &plan.project_id,
                &prepared.plan_sha256,
                &plan.source_head,
            )
            .map_err(|_| "DENIED: completed rust_build receipt review rejected")?;
        if receipt.recipe_id != "rust_build"
            || receipt.requires_reconciliation
            || receipt.terminal_receipt_hash.len() != 64
            || candidate.output_tree_sha256 != output_witness.tree_sha256
            || candidate.debugger_execution_authorized
        {
            return Err("DENIED: real Cargo binary not bound to terminal supervisor".into());
        }
        eprintln!(
            "DEV042_LINUX_CGROUP_REAL_BUILD_BINARY=PASS hash={} receipt={}",
            candidate.binary_sha256, receipt.terminal_receipt_hash
        );
        let symbols =
            verify_build_debug_symbols(&actual.output_directory, &output_witness, &candidate)
                .map_err(|_| "DENIED: real contained Linux debug sections were altered")?
                .ok_or("DENIED: real contained Linux executable lacked valid DWARF")?;
        if !symbols.debug_symbols_attested
            || symbols.source_line_mapping_verified
            || symbols.debugger_execution_authorized
            || symbols.binary_sha256 != candidate.binary_sha256
            || symbols.output_tree_sha256 != output_witness.tree_sha256
        {
            return Err("DENIED: real Linux ELF/DWARF identity not bound".into());
        }
        eprintln!(
            "DEV043_LINUX_REAL_ELF_DWARF=PASS symbols_sha={}",
            symbols.symbols_sha256
        );
        if project_debug {
            let private = root.join("sealed-debug-targets");
            fs::create_dir_all(&private)?;
            fs::set_permissions(&private, fs::Permissions::from_mode(0o700))?;
            let lineage = CompletedBuildLineage {
                project_id: plan.project_id.clone(),
                build_job_id: prepared.job_id.clone(),
                build_plan_sha256: prepared.plan_sha256.clone(),
                terminal_receipt_sha256: receipt.terminal_receipt_hash.clone(),
                source_head: plan.source_head.clone(),
                source_snapshot_sha256: plan.input_snapshot_sha256.clone(),
                manifest_sha256: plan.manifest_sha256.clone(),
                build_toolchain_sha256: plan.toolchain_sha256.clone(),
                output_tree_sha256: output_witness.tree_sha256.clone(),
                build_successfully_completed: true,
                requires_reconciliation: false,
            };
            let target = seal_project_debug_target(
                &actual.output_directory,
                &private,
                lineage,
                &candidate,
                &symbols,
            )
            .map_err(|_| "DENIED: native Cargo output could not be privately sealed")?;
            target
                .verify()
                .map_err(|_| "DENIED: private debug target changed")?;
            let bundle = PathBuf::from("/opt/lu-ide-wo6-dap-test");
            let adapter_sha = native_lsp_bundle_digest(&bundle, "dap_lldb_project")
                .map_err(|_| "DENIED: root-owned LLDB adapter not trusted")?;
            let debugger_plan = JobPlan {
                project_id: plan.project_id.clone(),
                recipe_id: "dap_lldb_project".into(),
                source_head: plan.source_head.clone(),
                input_snapshot_sha256: plan.input_snapshot_sha256.clone(),
                manifest_sha256: plan.manifest_sha256.clone(),
                toolchain_sha256: adapter_sha,
                debug_target_binding_sha256: Some(target.binding_sha256.clone()),
                runtime_limit_ms: 90_000,
                memory_limit_bytes: 512 * 1024 * 1024,
                process_limit: 16,
                log_limit_bytes: 64 * 1024,
                effect_class: "debug_inspection_isolated".into(),
            };
            let debug_job = supervisor
                .prepare(debugger_plan.clone())
                .map_err(|e| e.code())?;
            // CI fixed-scope native approval; not a user desktop approval.
            supervisor
                .record_human_approval(
                    &debug_job.job_id,
                    &debug_job.plan_sha256,
                    &debugger_plan.source_head,
                    true,
                )
                .map_err(|e| e.code())?;
            let mut dap = start_linux_project_dap(
                Arc::clone(&supervisor),
                &debug_job.job_id,
                &debugger_plan,
                &debugger_plan.source_head,
                &input,
                &bundle,
                &target,
            )
            .map_err(|e| e.code())?;
            dap.initialize().map_err(|e| e.code())?;
            let began = Instant::now();
            while dap.phase() != DapPhase::Ready && began.elapsed() < Duration::from_secs(20) {
                dap.poll().map_err(|e| e.code())?;
                thread::sleep(Duration::from_millis(30));
            }
            if dap.phase() != DapPhase::Ready {
                return Err("DENIED: protected project LLDB initialize timed out".into());
            }
            target
                .verify()
                .map_err(|_| "DENIED: target changed before launch")?;
            dap.launch_project_target().map_err(|e| e.code())?;
            if dap.configuration_needed() {
                dap.configuration_done().map_err(|e| e.code())?;
            }
            let began = Instant::now();
            let (mut stopped, mut threads, mut stack) = (None, false, false);
            let (mut requested_threads, mut requested_stack) = (false, false);
            while began.elapsed() < Duration::from_secs(35) {
                for event in dap.poll().map_err(|e| e.code())? {
                    match event {
                        DapEvent::ConfigurationRequested => {
                            dap.configuration_done().map_err(|e| e.code())?
                        }
                        DapEvent::Stopped { thread_id } => stopped = Some(thread_id),
                        DapEvent::ThreadsReceived => threads = true,
                        DapEvent::StackTraceReceived => stack = true,
                        DapEvent::Terminated => {
                            return Err("DENIED: project debuggee exited before stack".into())
                        }
                        _ => {}
                    }
                }
                if stopped.is_some() && !requested_threads && dap.phase() == DapPhase::Stopped {
                    dap.threads().map_err(|e| e.code())?;
                    requested_threads = true;
                }
                if threads && !requested_stack {
                    dap.stack_trace(stopped.ok_or("DENIED: missing stopped thread")?)
                        .map_err(|e| e.code())?;
                    requested_stack = true;
                }
                if stack {
                    break;
                }
                thread::sleep(Duration::from_millis(30));
            }
            if stopped.is_none() || !threads || !stack {
                return Err("DENIED: real project debug stop/stack absent".into());
            }
            dap.close().map_err(|e| e.code())?;
            let summary = supervisor
                .inspect()
                .map_err(|e| e.code())?
                .into_iter()
                .find(|j| j.job_id == debug_job.job_id)
                .ok_or("DENIED: project debug terminal receipt absent")?;
            if summary.phase != JobPhase::Completed || summary.requires_reconciliation {
                return Err("DENIED: project debug kernel descendants not reconciled".into());
            }
            eprintln!(
                "DEV044_LINUX_REAL_BUILD_TO_PROTECTED_LLDB=PASS bind={}",
                target.binding_sha256
            );
        }
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
    if jobs.len() != (if project_debug { 2 } else { 1 })
        || jobs
            .iter()
            .any(|job| job.phase != expected || job.requires_reconciliation)
    {
        return Err("DENIED: native Cargo journal restart/reconciliation invalid".into());
    }
    println!("PASS: genuine delegated Linux Cargo {recipe} OS worker, kernel cleanup and restart receipt");
    Ok(())
}
