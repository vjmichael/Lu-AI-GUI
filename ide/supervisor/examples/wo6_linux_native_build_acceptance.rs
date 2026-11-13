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
    let debug_cancel = mode == ["--project-debug-cancel"];
    let project_debug = mode == ["--project-debug"] || debug_cancel;
    let (recipe, cancel) = match mode {
        [one] if one == "--check" => ("rust_check", false),
        [one] if one == "--test" => ("rust_tests", false),
        [one] if one == "--build" => ("rust_build", false),
        [one] if one == "--project-debug" || one == "--project-debug-cancel" => {
            ("rust_build", false)
        }
        [one] if one == "--cancel" => ("rust_tests", true),
        _ => return Err(
            "DENIED: only fixed check/test/build/project-debug/project-debug-cancel/cancel modes"
                .into(),
        ),
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
        "fn main(){\n    #[cfg(target_arch=\"x86_64\")]\n    unsafe { core::arch::asm!(\"int3\", options(nomem, nostack, preserves_flags)); }\n    println!(\"fixed WO6 isolated job\");\n}\n",
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
    let mut project_debug_job_id: Option<String> = None;
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
            // This exact CI-reviewed source contains a real x86_64 INT3
            // instruction. A stop-on-entry in the dynamic loader has no
            // source line and cannot satisfy independent DWARF evidence.
            // Run without entry stop only in this disposable native fixture;
            // the first genuine SIGTRAP must have the original source line.
            dap.launch_project_source_trap_for_ci().map_err(|e| e.code())?;
            // LLDB 18 resumes on configurationDone(stopOnEntry=false).
            // First bind a single breakpoint to the fixed, sealed source
            // file/line, and await its verified adapter response. Only a
            // subsequent real stopped event, native PC and independent
            // DWARF line may satisfy the acceptance gate.
            if dap.ci_fixed_source_breakpoint_needed() {
                dap.ci_set_fixed_source_breakpoint().map_err(|e| e.code())?;
            }
            if dap.configuration_needed() {
                dap.configuration_done().map_err(|e| e.code())?;
            }
            let began = Instant::now();
            let (mut stopped, mut threads, mut stack) = (None, false, false);
            let (mut requested_threads, mut requested_stack) = (false, false);
            let mut launch_acknowledged = false;
            let mut config_requested = false;
            let mut native_continued = false;
            while began.elapsed() < Duration::from_secs(35) {
                for event in dap.poll().map_err(|e| e.code())? {
                    match event {
                        DapEvent::ConfigurationRequested => {
                            config_requested = true;
                            eprintln!("FIN04_D3_LINUX_DAP_EVENT=configuration_requested");
                            if dap.ci_fixed_source_breakpoint_needed() {
                                dap.ci_set_fixed_source_breakpoint().map_err(|e| e.code())?;
                            }
                        }
                        DapEvent::Stopped { thread_id } => {
                            eprintln!("FIN04_D3_LINUX_DAP_EVENT=real_stopped");
                            stopped = Some(thread_id);
                        },
                        DapEvent::ThreadsReceived => {
                            eprintln!("FIN04_D3_LINUX_DAP_EVENT=threads_received");
                            threads = true;
                        },
                        DapEvent::StackTraceReceived => {
                            eprintln!("FIN04_D3_LINUX_DAP_EVENT=stack_received");
                            stack = true;
                        },
                        DapEvent::LaunchAcknowledged => {
                            eprintln!("FIN04_D3_LINUX_DAP_EVENT=launch_acknowledged");
                            launch_acknowledged = true;
                        },
                        DapEvent::Continued => native_continued = true,
                        DapEvent::Terminated => {
                            return Err("DENIED: project debuggee exited before stack".into())
                        }
                        _ => {}
                    }
                }
                if dap.ci_fixed_source_breakpoint_needed() {
                    dap.ci_set_fixed_source_breakpoint().map_err(|e| e.code())?;
                }
                if dap.configuration_needed() {
                    dap.configuration_done().map_err(|e| e.code())?;
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
                // Fixed booleans only; never forward adapter output, process
                // identity, source bytes or an unverified OS stop.
                eprintln!(
                    "FIN04_D3_LINUX_DAP_STOP_REFUSAL launch_ack={};config_event={};bp_verified={};config_ack={};continued={};stopped={};threads={};stack={};phase={:?}",
                    launch_acknowledged, config_requested,
                    dap.ci_fixed_source_breakpoint_verified(),
                    dap.ci_configuration_done_acknowledged(), native_continued,
                    stopped.is_some(), threads, stack, dap.phase(),
                );
                return Err("DENIED: real project debug stop/stack absent".into());
            }
            // Unlike D2's adapter-only PC claim, this requires a genuine
            // traced-stop task in this exact dedicated cgroup, its /proc/PID
            // executable inode and RX VMA, ELF PT_LOAD relocation, the exact
            // native source snapshot and BOTH independently fsynced journals.
            // A *second*, fixed, compile-pinned LLVM oracle must then agree
            // with the stopped instruction's original source line.
            let stopped_witness = dap.verify_project_source_line_for_ci(
                &supervisor, &debug_job.job_id, &debugger_plan, &target,
            ).map_err(|_| "DENIED: protected Linux stopped-PC, cgroup, build/debug receipts or independent DWARF line mapping failed")?;
            if !stopped_witness.independent_line_table_verified
                || stopped_witness.installed_app_accepted
                || stopped_witness.debugger_job_id != debug_job.job_id
                || stopped_witness.debugger_plan_sha256 != debug_job.plan_sha256
                || stopped_witness.build_job_id != prepared.job_id
                || stopped_witness.build_terminal_receipt_sha256 != receipt.terminal_receipt_hash
                || stopped_witness.source_snapshot_sha256 != input.seal.source_sha256
                || stopped_witness.target_binding_sha256 != target.binding_sha256
            {
                return Err("DENIED: independent live stop lost its verified build lineage".into());
            }
            eprintln!(
                "DEV045D3_LINUX_CGROUP_REAL_STOP_ELF_DWARF=PASS;load_bias=0x{:x};source_line={};build_receipt=bound;debug_running_receipt=bound",
                stopped_witness.kernel.load_bias, stopped_witness.source_line,
            );
            if debug_cancel {
                // A real project, not the fixed debuggee, is stopped under
                // cgroup custody before native Drop cancels and reaps it.
                drop(dap);
                project_debug_job_id = Some(debug_job.job_id.clone());
            } else {
                dap.close().map_err(|e| e.code())?;
            }
            let summary = supervisor
                .inspect()
                .map_err(|e| e.code())?
                .into_iter()
                .find(|j| j.job_id == debug_job.job_id)
                .ok_or("DENIED: project debug terminal receipt absent")?;
            let debug_expected = if debug_cancel {
                JobPhase::Cancelled
            } else {
                JobPhase::Completed
            };
            if summary.phase != debug_expected || summary.requires_reconciliation {
                return Err("DENIED: project debug kernel descendants not reconciled".into());
            }
            let terminal_history = supervisor
                .receipt_history(&debug_job.job_id)
                .map_err(|e| e.code())?;
            let terminal = terminal_history
                .last()
                .ok_or("DENIED: missing fsynced stopped-PC terminal receipt")?;
            if terminal.body.phase != debug_expected
                || terminal.body.plan_sha256 != stopped_witness.debugger_plan_sha256
                || terminal.entry_hash == stopped_witness.debug_running_receipt_sha256
                || !terminal_history.iter().any(|r| {
                    r.entry_hash == stopped_witness.debug_running_receipt_sha256
                        && r.body.phase == JobPhase::Running
                })
            {
                return Err(
                    "DENIED: live kernel-PC proof not bound to durable debug terminal".into(),
                );
            }
            let build_terminal = supervisor
                .receipt_history(&stopped_witness.build_job_id)
                .map_err(|e| e.code())?
                .last()
                .cloned()
                .ok_or("DENIED: independent source build terminal missing")?;
            if build_terminal.body.phase != JobPhase::Completed
                || build_terminal.entry_hash != stopped_witness.build_terminal_receipt_sha256
            {
                return Err("DENIED: stopped-PC lineage lost completed native build".into());
            }
            eprintln!("DEV045D3_LINUX_REAL_STOP_LINE_TERMINAL_JOURNALS=PASS");
            if debug_cancel {
                let entries = supervisor
                    .receipt_history(&debug_job.job_id)
                    .map_err(|e| e.code())?;
                if !entries.windows(2).any(|pair| {
                    pair[0].body.phase == JobPhase::CancelRequested
                        && pair[1].body.phase == JobPhase::Cancelled
                }) {
                    return Err(
                        "DENIED: real project LLDB cancellation not durable before kernel kill"
                            .into(),
                    );
                }
                eprintln!("DEV045C_LINUX_REAL_PROJECT_LLDB_CANCEL_AND_CGROUP_REAP=PASS");
            }
            // A completed Cargo approval cannot authorize a second debugger;
            // neither the build grant nor the distinct DAP grant survives
            // its terminal native receipt.
            if supervisor
                .record_human_approval(
                    &prepared.job_id,
                    &prepared.plan_sha256,
                    &plan.source_head,
                    true,
                )
                .is_ok()
                || supervisor
                    .record_human_approval(
                        &debug_job.job_id,
                        &debug_job.plan_sha256,
                        &debugger_plan.source_head,
                        true,
                    )
                    .is_ok()
            {
                return Err("DENIED: native Linux build/debug authorization replay".into());
            }
            eprintln!(
                "DEV044_LINUX_REAL_BUILD_TO_PROTECTED_LLDB=PASS bind={}",
                target.binding_sha256
            );
            let retired_stage = target.root.clone();
            drop(target);
            if retired_stage.exists() {
                return Err("DENIED: old private Linux debug target stage retained".into());
            }
            // Reopening the journal creates a new owner/session; replayed
            // build/debug approval is still denied, not silently restored.
            let independently_restarted =
                Supervisor::open(journal.clone()).map_err(|e| e.code())?;
            if independently_restarted
                .record_human_approval(
                    &prepared.job_id,
                    &prepared.plan_sha256,
                    &plan.source_head,
                    true,
                )
                .is_ok()
                || independently_restarted
                    .record_human_approval(
                        &debug_job.job_id,
                        &debug_job.plan_sha256,
                        &debugger_plan.source_head,
                        true,
                    )
                    .is_ok()
            {
                return Err("DENIED: old build/debug approval replayed after Linux restart".into());
            }
            eprintln!("DEV045B_LINUX_DISTINCT_APPROVAL_REPLAY_AND_STAGE_CLEANUP=PASS");
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
        || jobs.iter().any(|job| {
            let phase = if project_debug_job_id.as_deref() == Some(job.job_id.as_str()) {
                JobPhase::Cancelled
            } else {
                expected
            };
            job.phase != phase || job.requires_reconciliation
        })
    {
        return Err("DENIED: native Cargo journal restart/reconciliation invalid".into());
    }
    println!("PASS: genuine delegated Linux Cargo {recipe} OS worker, kernel cleanup and restart receipt");
    Ok(())
}
