//! WO6 genuine OS acceptance for the fixed LLDB-DAP adapter plus self-test.
//! The host-provisioned bundle is SELF-MEASURED for CI only, not a redistributable
//! release signature. Never executes arbitrary caller-selected processes.
#[cfg(not(any(
    all(
        windows,
        feature = "dap-transport",
        feature = "protected-recovery-service"
    ),
    all(
        target_os = "linux",
        feature = "dap-transport",
        feature = "linux-worker"
    )
)))]
fn main() {
    eprintln!("DENIED: real native protected LLDB-DAP and D3 features required");
    std::process::exit(2);
}
#[cfg(any(
    all(
        windows,
        feature = "dap-transport",
        feature = "protected-recovery-service"
    ),
    all(
        target_os = "linux",
        feature = "dap-transport",
        feature = "linux-worker"
    )
))]
mod protected {
    #[cfg(target_os = "linux")]
    use lu_ide_execution_supervisor::linux_worker::lsp_native::{
        native_lsp_bundle_digest, start_linux_dap, LinuxDapSession,
    };
    use lu_ide_execution_supervisor::{snapshot::publish_lsp_input, JobPhase, JobPlan, Supervisor};
    #[cfg(windows)]
    use lu_ide_execution_supervisor::{
        windows_broker::TrustedBroker,
        windows_worker::lsp_native::{
            native_lsp_bundle_digest, start_windows_dap, WindowsDapSession,
        },
    };
    use lu_ide_workspace::dap_session::{DapEvent, DapPhase};
    use std::{
        env, fs,
        io::Read,
        path::{Path, PathBuf},
        sync::Arc,
        thread,
        time::{Duration, Instant},
    };
    #[cfg(windows)]
    type Dap = WindowsDapSession;
    #[cfg(target_os = "linux")]
    type Dap = LinuxDapSession;
    #[cfg(windows)]
    fn sha(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
        use sha2::{Digest, Sha256};
        let mut input = fs::File::open(path)?;
        let mut hash = Sha256::new();
        let mut chunk = [0_u8; 64 * 1024];
        loop {
            let n = input.read(&mut chunk)?;
            if n == 0 {
                break;
            }
            hash.update(&chunk[..n]);
        }
        Ok(format!("{:x}", hash.finalize()))
    }
    #[cfg(windows)]
    fn start(
        supervisor: Arc<Supervisor>,
        id: &str,
        plan: &JobPlan,
        input: &lu_ide_execution_supervisor::snapshot::PublishedInput,
        bundle: &Path,
    ) -> Result<Dap, Box<dyn std::error::Error>> {
        let service = PathBuf::from(
            env::var_os("LU_IDE_WO6_DAP_TRUSTED_SERVICE")
                .ok_or("DENIED: no pinned independently installed SCM service")?,
        );
        if !service.is_absolute() {
            return Err("DENIED: SCM path is not absolute".into());
        }
        let broker = TrustedBroker {
            executable: service.clone(),
            expected_sha256: sha(&service)?,
            private_witness_directory: env::temp_dir().join("wo6-testonly-service"),
        };
        Ok(start_windows_dap(
            supervisor,
            id,
            plan,
            &plan.source_head,
            input,
            bundle,
            &broker,
        )
        .map_err(|e| e.code())?)
    }
    #[cfg(target_os = "linux")]
    fn start(
        supervisor: Arc<Supervisor>,
        id: &str,
        plan: &JobPlan,
        input: &lu_ide_execution_supervisor::snapshot::PublishedInput,
        bundle: &Path,
    ) -> Result<Dap, Box<dyn std::error::Error>> {
        Ok(
            start_linux_dap(supervisor, id, plan, &plan.source_head, input, bundle)
                .map_err(|e| e.code())?,
        )
    }
    fn poll_until_ready(dap: &mut Dap, cancel: bool) -> Result<(), Box<dyn std::error::Error>> {
        let begin = Instant::now();
        while begin.elapsed() < Duration::from_secs(20) {
            for event in dap.poll().map_err(|e| e.code())? {
                if matches!(event, DapEvent::Terminated) {
                    return Err("DENIED: adapter terminated during initialize".into());
                }
            }
            if dap.phase() == DapPhase::Ready {
                return Ok(());
            }
            if cancel && begin.elapsed() > Duration::from_secs(5) {
                return Err("DENIED: debugger initialize did not complete".into());
            }
            thread::sleep(Duration::from_millis(40));
        }
        Err("DENIED: actual native DAP handshake timed out".into())
    }
    // Genuine native host-crash witness, available ONLY in the debug test
    // executable and enabled by its fixed trusted systemd CI host. No model,
    // project or WebView is given an option to exit the supervisor.
    #[cfg(target_os = "linux")]
    fn witness_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
        let root = PathBuf::from(
            env::var_os("LU_IDE_P03_CRASH_WITNESS_ROOT")
                .ok_or("DENIED: trusted P03 crash witness root missing")?,
        );
        if !cfg!(debug_assertions)
            || root.parent() != Some(env::temp_dir().as_path())
            || !root
                .file_name()
                .and_then(|x| x.to_str())
                .is_some_and(|x| x.starts_with("lu-ide-p03-crash-"))
            || !root.is_dir()
        {
            return Err("DENIED: only fixed CI-owned crash witness root is permitted".into());
        }
        Ok(root)
    }
    #[cfg(target_os = "linux")]
    fn verify_crash_replay() -> Result<(), Box<dyn std::error::Error>> {
        let root = witness_root()?;
        let id = fs::read_to_string(root.join("crash-job-id"))?;
        let id = id.trim();
        if !id.starts_with("job-")
            || id.len() != 36
            || !id[4..].bytes().all(|c| c.is_ascii_hexdigit())
        {
            return Err("DENIED: invalid native crash journal identity".into());
        }
        let replay = Supervisor::open(root.join("receipts.jsonl")).map_err(|e| e.code())?;
        let observed = replay.inspect().map_err(|e| e.code())?;
        if observed.len() != 1
            || observed[0].job_id != id
            || !observed[0].requires_reconciliation
            || matches!(observed[0].phase, JobPhase::Completed | JobPhase::Cancelled)
        {
            return Err(
                "DENIED: host crash did not quarantine unfinished OS work on restart".into(),
            );
        }
        // A cgroup witness must use actual kernel populated evidence, not
        // assumptions from journal flags or a mocked process table.
        let scope = fs::read_to_string("/proc/self/cgroup")?;
        let relative = scope
            .lines()
            .find_map(|line| line.strip_prefix("0::"))
            .ok_or("DENIED: host has no real cgroup v2 membership")?;
        let leaf = PathBuf::from("/sys/fs/cgroup").join(relative.trim_start_matches('/'));
        if leaf.file_name().and_then(|x| x.to_str()) != Some("lu-ide-host") {
            return Err("DENIED: crash verifier was not independently delegated".into());
        }
        let parent = leaf
            .parent()
            .ok_or("DENIED: delegated parent unavailable")?;
        if !matches!(
            parent.file_name().and_then(|x| x.to_str()),
            Some("lu-ide-delegated" | "lu-ide-delegated.service")
        ) {
            return Err("DENIED: unexpected cgroup recovery parent".into());
        }
        let group = parent.join(format!("lu-ide-{id}"));
        let began = Instant::now();
        let empty = loop {
            let events = fs::read_to_string(group.join("cgroup.events"))?;
            let clean = events.lines().any(|line| line == "populated 0")
                && fs::read_to_string(group.join("cgroup.procs"))?
                    .trim()
                    .is_empty();
            if clean || began.elapsed() >= Duration::from_secs(8) {
                break clean;
            }
            thread::sleep(Duration::from_millis(40));
        };
        if !empty {
            return Err("DENIED: real kernel cgroup retained child after supervisor crash".into());
        }
        println!(
            "PASS: real host crash -> kernel empty worker cgroup -> durable replay quarantined"
        );
        Ok(())
    }
    // DEV-04.5B native Windows proof: the executable is first built by a
    // protected Cargo AppContainer, not copied from the host or fixed fixture.
    // The separate project LLDB job obtains its OWN one-use, plan-bound CI
    // approval after an independently verified PE/PDB and durable build receipt.
    // Test-only native SCM acceptance; never a user grant or release approval.
    #[cfg(windows)]
    fn run_windows_project_debug(
        cancel_project: bool,
        verify_job_module: bool,
        sample_stopped_thread: bool,
        verify_independent_pdb: bool,
        verify_owner_event: bool,
        crash_owner: bool,
        ci_control: Option<&'static str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use lu_ide_execution_supervisor::{
            debug_binary_witness::audit_debug_binary_candidate,
            debug_symbol_verification::verify_build_debug_symbols,
            live_output::LiveOutput,
            project_debug_target::{seal_project_debug_target, CompletedBuildLineage},
            snapshot::publish_input,
            windows_worker::{
                execute_windows_worker_after_staging_approval_with_output,
                trusted_offline_toolchain_digest, WindowsCancellation,
            },
            wo6_output_evidence::audit_published_output,
        };
        use lu_ide_execution_supervisor::windows_worker::lsp_native::{
            native_lsp_bundle_digest, start_windows_project_dap,
        };
        if ci_control.is_some() && (!verify_owner_event || crash_owner || cancel_project) {
            return Err("DENIED: D6C native command requires verified owner and its own one-use normal Job".into());
        }
        if !matches!(ci_control, None | Some("step") | Some("breakpoint")) {
            return Err("DENIED: unknown native CI control mode".into());
        }
        let rust_bundle = PathBuf::from(
            env::var_os("LU_IDE_D7_NATIVE_TOOLCHAIN")
                .ok_or("DENIED: explicit CI-only pinned offline Cargo bundle missing")?,
        );
        let dap_bundle = PathBuf::from(
            env::var_os("LU_IDE_WO6_DAP_BUNDLE")
                .ok_or("DENIED: explicit CI-only pinned LLDB-DAP bundle missing")?,
        );
        let service = PathBuf::from(
            env::var_os("LU_IDE_WO6_DAP_TRUSTED_SERVICE")
                .ok_or("DENIED: independent SCM recovery service missing")?,
        );
        if !rust_bundle.is_absolute() || !dap_bundle.is_absolute() || !service.is_absolute() {
            return Err("DENIED: only absolute native-authority bundle paths".into());
        }
        let nonce = format!(
            "{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        );
        let root = env::temp_dir().join(format!("lu-ide-dev045b-project-{nonce}"));
        let source = root.join("source");
        let stage = root.join("stage");
        let output = root.join("output");
        let private = root.join("sealed-debug-targets");
        for dir in [&source.join("src"), &stage, &output, &private] {
            fs::create_dir_all(dir)?;
        }
        fs::write(
            source.join("Cargo.toml"),
            "[package]\nname = \"dev045b_acceptance\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )?;
        fs::write(
            source.join("Cargo.lock"),
            "# This file is automatically @generated by Cargo.\nversion = 4\n\n[[package]]\nname = \"dev045b_acceptance\"\nversion = \"0.1.0\"\n",
        )?;
        let source_file = source.join("src/main.rs");
        if ci_control == Some("step") {
            // A multi-line sealed original Cargo project allows a real
            // line step while preserving the immutable source snapshot.
            fs::write(&source_file, concat!(
                "fn main() {\n",
                "    let first = 2u64;\n",
                "    let second = first + 3;\n",
                "    println!(\"protected step result {}\", second);\n",
                "}\n"
            ))?;
        } else {
            fs::write(&source_file,
                "fn main() { println!(\"protected project DAP\"); }\n"
            )?;
        }
        let original_source_sha = sha(&source_file)?;
        let input = publish_input(&source, &stage, "rust_build").map_err(|e| e.code())?;
        let toolchain_sha = trusted_offline_toolchain_digest(&rust_bundle)
            .map_err(|e| e.code())?;
        let broker = TrustedBroker {
            executable: service.clone(),
            expected_sha256: sha(&service)?,
            private_witness_directory: root.join("private-scm-receipts"),
        };
        fs::create_dir_all(&broker.private_witness_directory)?;
        let journal = root.join("receipts.jsonl");
        let supervisor = Arc::new(Supervisor::open(journal.clone()).map_err(|e| e.code())?);
        let build_plan = JobPlan {
            project_id: format!("ide-{}", "7".repeat(32)),
            recipe_id: "rust_build".into(),
            source_head: "c".repeat(40),
            input_snapshot_sha256: input.seal.source_sha256.clone(),
            manifest_sha256: input.seal.manifest_sha256.clone(),
            toolchain_sha256: toolchain_sha,
            debug_target_binding_sha256: None,
            runtime_limit_ms: 90_000,
            memory_limit_bytes: 512 * 1024 * 1024,
            process_limit: 12,
            log_limit_bytes: 65536,
            effect_class: "test_build_isolated".into(),
        };
        let build_job = supervisor.prepare(build_plan.clone()).map_err(|e| e.code())?;
        let live = Arc::new(LiveOutput::default());
        let built = execute_windows_worker_after_staging_approval_with_output(
            &supervisor,
            &build_job.job_id,
            &build_plan,
            &build_plan.source_head,
            &input,
            &rust_bundle,
            &output,
            &broker,
            &WindowsCancellation::new(),
            || {
                // CI-owned explicit consent, after pinned staging; never
                // carried over into the second, different debugger plan.
                supervisor
                    .record_human_approval(
                        &build_job.job_id,
                        &build_job.plan_sha256,
                        &build_plan.source_head,
                        true,
                    )
                    .map(|_| ())
            },
            Some(Arc::clone(&live)),
        )
        .map_err(|e| e.code())?;
        if built.summary.phase != JobPhase::Completed
            || built.exit_code != Some(0)
            || built.summary.requires_reconciliation
        {
            return Err("DENIED: native Cargo build not terminally verified".into());
        }
        let output_witness = audit_published_output(&built.output_root)
            .map_err(|e| e.code())?;
        let candidate = audit_debug_binary_candidate(&built.output_root, &output_witness)
            .map_err(|e| e.code())?
            .ok_or("DENIED: no unique executable from protected build")?;
        let symbols = verify_build_debug_symbols(&built.output_root, &output_witness, &candidate)
            .map_err(|e| e.code())?
            .ok_or("DENIED: no matching PE/RSDS/PDB from protected build")?;
        let receipt = supervisor
            .review_build_test_evidence(
                &build_job.job_id,
                &build_plan.project_id,
                &build_job.plan_sha256,
                &build_plan.source_head,
            )
            .map_err(|e| e.code())?;
        if receipt.recipe_id != "rust_build"
            || receipt.requires_reconciliation
            || receipt.terminal_receipt_hash.len() != 64
            || candidate.output_tree_sha256 != output_witness.tree_sha256
            || !symbols.debug_symbols_attested
            || symbols.symbols_kind != "external_pdb_guid_age_dbi_match"
            || symbols.source_line_mapping_verified
            || symbols.debugger_execution_authorized
            || symbols.binary_sha256 != candidate.binary_sha256
        {
            return Err("DENIED: independent native PE/PDB build identity mismatch".into());
        }
        println!("DEV045B_WINDOWS_PROTECTED_CARGO_PE_PDB=PASS");
        let lineage = CompletedBuildLineage {
            project_id: build_plan.project_id.clone(),
            build_job_id: build_job.job_id.clone(),
            build_plan_sha256: build_job.plan_sha256.clone(),
            terminal_receipt_sha256: receipt.terminal_receipt_hash.clone(),
            source_head: build_plan.source_head.clone(),
            source_snapshot_sha256: build_plan.input_snapshot_sha256.clone(),
            manifest_sha256: build_plan.manifest_sha256.clone(),
            build_toolchain_sha256: build_plan.toolchain_sha256.clone(),
            output_tree_sha256: output_witness.tree_sha256.clone(),
            build_successfully_completed: true,
            requires_reconciliation: false,
        };
        let target = seal_project_debug_target(
            &built.output_root, &private, lineage, &candidate, &symbols,
        )
        .map_err(|e| e.code())?;
        target.verify().map_err(|e| e.code())?;
        let debug_plan = JobPlan {
            project_id: build_plan.project_id.clone(),
            recipe_id: "dap_lldb_project".into(),
            source_head: build_plan.source_head.clone(),
            input_snapshot_sha256: build_plan.input_snapshot_sha256.clone(),
            manifest_sha256: build_plan.manifest_sha256.clone(),
            toolchain_sha256: native_lsp_bundle_digest(&dap_bundle, "dap_lldb_project")
                .map_err(|e| e.code())?,
            debug_target_binding_sha256: Some(target.binding_sha256.clone()),
            runtime_limit_ms: 90_000,
            memory_limit_bytes: 512 * 1024 * 1024,
            process_limit: 16,
            log_limit_bytes: 65536,
            effect_class: "debug_inspection_isolated".into(),
        };
        let debug_job = supervisor.prepare(debug_plan.clone()).map_err(|e| e.code())?;
        if supervisor
            .record_human_approval(
                &build_job.job_id, &build_job.plan_sha256, &build_plan.source_head, true,
            )
            .is_ok()
        {
            return Err("DENIED: consumed Cargo build approval could be replayed".into());
        }
        supervisor
            .record_human_approval(
                &debug_job.job_id, &debug_job.plan_sha256, &debug_plan.source_head, true,
            )
            .map_err(|e| e.code())?;
        let mut dap = start_windows_project_dap(
            Arc::clone(&supervisor),
            &debug_job.job_id,
            &debug_plan,
            &debug_plan.source_head,
            &input,
            &dap_bundle,
            &broker,
            &target,
        )
        .map_err(|e| e.code())?;
        dap.initialize().map_err(|e| e.code())?;
        poll_until_ready(&mut dap, false)?;
        target.verify().map_err(|e| e.code())?;
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
                        dap.configuration_done().map_err(|e| e.code())?;
                    }
                    DapEvent::Stopped { thread_id } => stopped = Some(thread_id),
                    DapEvent::ThreadsReceived => threads = true,
                    DapEvent::StackTraceReceived => stack = true,
                    DapEvent::Terminated => {
                        return Err("DENIED: protected project exited before stack".into());
                    }
                    _ => {}
                }
            }
            if stopped.is_some() && !requested_threads && dap.phase() == DapPhase::Stopped {
                dap.threads().map_err(|e| e.code())?;
                requested_threads = true;
            }
            if threads && !requested_stack {
                dap.stack_trace(stopped.ok_or("DENIED: project stopped thread absent")?)
                    .map_err(|e| e.code())?;
                requested_stack = true;
            }
            if stack {
                break;
            }
            thread::sleep(Duration::from_millis(35));
        }
        if stopped.is_none() || !threads || !stack {
            return Err("DENIED: native Windows project LLDB stop/threads/stack absent".into());
        }
        // Adapter-only stopped PC candidate. This cannot attest the loaded
        // PE base, process identity or PDB line table without independent
        // SCM/Job kernel module mapping and a separate symbol parser.
        if let Some(candidate) = dap.source_candidate() {
            if let Some(pc) = candidate.instruction_address {
                println!(
                    "DEV045D2_WINDOWS_REAL_DAP_STOP_PC_CLAIM=0x{:x};line={};os_module_slide_verified=false;binary_sha={}",
                    pc, candidate.line, target.binary.binary_sha256
                );
            } else {
                println!("DEV045D2_WINDOWS_REAL_DAP_STOP_PC_CLAIM=absent");
            }
        } else {
            println!("DEV045D2_WINDOWS_REAL_DAP_SOURCE_CLAIM=unavailable");
        }
        if crash_owner && !verify_owner_event {
            return Err("DENIED: fixed owner crash test requires original verified event".into());
        }
        if (sample_stopped_thread || verify_independent_pdb || verify_owner_event)
            && !verify_job_module {
            return Err("DENIED: thread evidence cannot bypass PE module and journal witness".into());
        }
        if verify_job_module {
            // An authentic Job-owned PE image and preserved original journal
            // can now be proved, but this is NOT an independent thread RIP
            // nor PDB source-line pass. A different dedicated gate is needed
            // before any setBreakpoints/step/variable-control request.
            let observed = dap.verify_project_loaded_module(
                &supervisor, &debug_job.job_id, &debug_plan, &target,
            ).map_err(|_| "DENIED: live Windows AppContainer/SCM Job module or durable build/debug receipts not verified")?;
            if !observed.module.job_member_verified
                || !observed.module.process_exe_path_verified
                || !observed.module.os_loaded_pe_image_verified
                || observed.module.os_thread_context_ip_verified
                || observed.module.independent_pdb_line_verified
                || observed.source_line_mapping_verified
                || observed.stopped_thread_os_verified
                || observed.installed_app_accepted
                || observed.debugger_job_id != debug_job.job_id
                || observed.debugger_plan_sha256 != debug_job.plan_sha256
                || observed.build_job_id != build_job.job_id
                || observed.build_terminal_receipt_sha256 != receipt.terminal_receipt_hash
                || observed.source_snapshot_sha256 != input.seal.source_sha256
                || observed.target_binding_sha256 != target.binding_sha256
            {
                return Err("DENIED: loaded PE observation does not belong to the protected debugger/build pair".into());
            }
            println!(
                "DEV045D4_WINDOWS_REAL_JOB_PE_MODULE=PASS;observed_base=0x{:x};preferred_pc=0x{:x};build_receipt=bound;running_receipt=bound;thread_pc_verified=false;pdb_line_verified=false",
                observed.module.loaded_base, observed.module.preferred_pc
            );
        }
        if sample_stopped_thread {
            let sample = dap.sample_stopped_project_thread_for_ci(
                &supervisor, &debug_job.job_id, &debug_plan, &target,
            ).map_err(|_| "DENIED: query-only Job-owned stopped thread context unavailable or disagrees with DAP PC")?;
            if !sample.two_matching_samples || !sample.job_membership_rechecked
                || sample.kernel_debug_event_independently_verified
                || sample.pdb_line_independently_verified
                || sample.debugger_control_authorized
                || sample.observed_rip == 0 {
                return Err("DENIED: inconsistent or overclaimed Windows context observation".into());
            }
            println!(
                "DEV045D4B_WINDOWS_JOB_THREAD_RIP_SAMPLED=PASS;twice=true;independent_debug_event_verified=false;pdb_line_verified=false"
            );
        }
        // This is the decisive *real Lu* test path, NOT a fixed fake adapter
        // or a separate standalone debuggee: same one-use SCM/Job session,
        // actual LLDB-DAP stop, OS-owned thread RIP and distinct PE/PDB
        // line reader; every native receipt must be preserved on restart.
        #[cfg(all(windows, debug_assertions, feature = "dev045d4c-ci-oracle"))]
        let mut bound_line_receipts: Option<(String, String)> = None;
        if verify_independent_pdb && !verify_owner_event {
            #[cfg(all(windows, debug_assertions, feature = "dev045d4c-ci-oracle"))]
            {
                let evidence = dap.verify_project_os_thread_pdb_receipts_for_ci(
                    &supervisor, &debug_job.job_id, &debug_plan, &target,
                ).map_err(|_| "DENIED: real LLDB Job-owned stopped RIP/PDB/source/receipt chain mismatch")?;
                if !evidence.os_owned_thread_context_matches_lldb
                    || !evidence.independent_pdb_source_line_verified
                    || !evidence.kernel_debug_port_present
                    || !evidence.debuggee_parent_is_native_lldb
                    || evidence.lldb_debug_event_owner_attested
                    || evidence.debugger_commands_authorized
                    || evidence.installed_app_accepted
                    || evidence.observed_rip == 0
                    || evidence.source_line == 0
                    || evidence.source_file_sha256 != original_source_sha
                    || evidence.debugger_job_id != debug_job.job_id
                    || evidence.debugger_plan_sha256 != debug_job.plan_sha256
                    || evidence.build_job_id != build_job.job_id
                    || evidence.build_terminal_receipt_sha256 != receipt.terminal_receipt_hash
                    || evidence.source_snapshot_sha256 != input.seal.source_sha256
                    || evidence.target_binding_sha256 != target.binding_sha256
                {
                    return Err("DENIED: source line/OS/Journals cannot be upgraded into debugger authority".into());
                }
                bound_line_receipts = Some((
                    evidence.debug_running_receipt_sha256.clone(),
                    evidence.build_terminal_receipt_sha256.clone()
                ));
                println!(
                    "DEV045D4C_REAL_LU_JOB_RIP_PDB_AND_RUNNING_RECEIPTS=PASS;source_line={};os_pe_base=0x{:x};os_thread_context_matches_lldb=true;kernel_debug_port=true;parent_is_owned_lldb=true;pdb_line=independent;debug_event_owner_attested=false;source_commands_authorized=false",
                    evidence.source_line, evidence.loaded_pe_base,
                );
            }
            #[cfg(not(all(windows, debug_assertions, feature = "dev045d4c-ci-oracle")))]
            {
                return Err("DENIED: real LLDB/PDB mapping is CI-only and never enabled in release".into());
            }
        }
        #[cfg(all(windows, debug_assertions, feature = "dev045d5-ci-event-owner"))]
        let mut owner_receipt: Option<(u64, String)> = None;
        #[cfg(all(windows, debug_assertions, feature = "dev045d5-ci-event-owner"))]
        let mut action_audit: Option<(String, String, String, String)> = None;
        #[cfg(all(windows, debug_assertions, feature = "dev046c-ci-control"))]
        let mut control_audit: Option<(String, String, String, String)> = None;
        if verify_owner_event {
            #[cfg(all(windows, debug_assertions, feature = "dev045d5-ci-event-owner"))]
            {
                let owner = dap.verify_real_lldb_event_owner_for_ci(
                    &supervisor, &debug_job.job_id, &debug_plan, &target,
                ).map_err(|_| "DENIED: actual pinned LLDB owner DEBUG_EVENT/sideband/PDB/journal not verified")?;
                if !owner.lldb_event_loop_owner_attested
                    || !owner.protected_pe_pdb_verified
                    || !owner.durable_receipt_verified
                    || owner.debugger_commands_authorized
                    || owner.installed_app_accepted
                    || owner.debugger_job_id != debug_job.job_id
                    || owner.debugger_plan_sha256 != debug_job.plan_sha256
                    || owner.build_terminal_receipt_sha256 != receipt.terminal_receipt_hash
                    || owner.owner_sequence == 0 || owner.stopped_rip == 0
                    || owner.verified_source_line == 0 {
                    return Err("DENIED: owner event did not bind exact approved debug journal".into());
                }
                let audit = dap.verify_bounded_debug_reviews_for_ci(
                    &supervisor, &owner, &input.seal.source_sha256,
                ).map_err(|_| "DENIED: DEV-04.6A/B native operator review/one-use journal admission failed")?;
                if audit.0.len() != 64 || audit.1.len() != 64
                    || audit.2.len() != 64 || audit.3 != "ResultUncertain" {
                    return Err("DENIED: D6B per-operation CI receipt incorrectly asserted success".into());
                }
                action_audit = Some(audit);
                println!("DEV046B_NATIVE_OWNER_ACTION_ONE_USE=PASS;human_consent=CI_SIMULATED_ONLY;consumed_before_dispatch=true;no_dap_control=true;result=UNCERTAIN");
                println!("DEV046A_NATIVE_REVIEW_BOUND_TO_REAL_OWNER=PASS;breakpoint=preview_only;step=preview_only;variables=denied;control_authorized=false");
                bound_line_receipts = Some((
                    owner.debug_running_receipt_sha256,
                    owner.build_terminal_receipt_sha256,
                ));
                owner_receipt = Some((
                    owner.owner_sequence, owner.native_owner_receipt_sha256,
                ));
                if let Some(mode) = ci_control {
                    #[cfg(all(windows, debug_assertions, feature = "dev046c-ci-control"))]
                    {
                        match mode {
                            "step" => {
                                let (after, receipt) = dap.execute_one_protected_ci_step(
                                    &supervisor, &debug_job.job_id, &debug_plan, &target, &owner,
                                ).map_err(|_| "DENIED: D6C new native LLDB stop/OS RIP/PDB and step result not independently verified")?;
                                if receipt.3 != "ResultVerified"
                                    || after.owner_sequence != owner.owner_sequence + 1
                                    || after.native_owner_receipt_sha256 == owner.native_owner_receipt_sha256 {
                                    return Err("DENIED: D6C adapter acknowledgement substituted for a new owner event".into());
                                }
                                owner_receipt = Some((
                                    after.owner_sequence, after.native_owner_receipt_sha256,
                                ));
                                control_audit = Some(receipt);
                                println!("DEV046C_NATIVE_CI_STEP_INDEPENDENT_OS_PDB=PASS;consent=CI_ONLY;new_event=true;result=verified");
                            }
                            "breakpoint" => {
                                let receipt = dap.attempt_one_protected_ci_breakpoint(
                                    &supervisor, &owner,
                                ).map_err(|_| "DENIED: D6C protected one-line native LLDB breakpoint attempt failed")?;
                                if receipt.3 != "ResultUncertain" {
                                    return Err("DENIED: D6C DAP breakpoint ACK was falsely promoted to OS verified".into());
                                }
                                control_audit = Some(receipt);
                                println!("DEV046C_CI_BREAKPOINT_SUBMITTED=PASS;independent_hit=NOT_PROVEN;result=UNCERTAIN");
                            }
                            _ => return Err("DENIED: unrecognized D6C action".into()),
                        }
                    }
                    #[cfg(not(all(windows, debug_assertions, feature = "dev046c-ci-control")))]
                    {
                        let _ = mode;
                        return Err("DENIED: D6C control dispatch disabled outside disposable native CI feature".into());
                    }
                }
                if ci_control.is_none() {
                    println!(
                        "DEV045D5_REAL_LLDB_KERNEL_OWNER_STOP_PDB_JOURNALS=PASS;source_line={};native_owner_event=true;new_debugger_commands=false",
                        owner.verified_source_line,
                    );
                }
            }
            #[cfg(not(all(windows, debug_assertions, feature = "dev045d5-ci-event-owner")))]
            {
                return Err("DENIED: first-party LLDB owner-event proof unavailable outside native CI".into());
            }
        }
        if crash_owner {
            #[cfg(all(windows, debug_assertions, feature = "dev045d5-ci-event-owner"))]
            {
                dap.crash_real_lldb_after_owner_receipt_for_ci()
                    .map_err(|_| "DENIED: native CI failed to kill the actual protected LLDB process")?;
                if dap.verify_real_lldb_event_owner_for_ci(
                    &supervisor, &debug_job.job_id, &debug_plan, &target,
                ).is_ok() {
                    return Err("DENIED: dead LLDB process replayed active kernel event evidence".into());
                }
                println!("DEV045D5_REAL_LLDB_WORKER_CRASH_REVOKES_OWNER=PASS");
            }
            #[cfg(not(all(windows, debug_assertions, feature = "dev045d5-ci-event-owner")))]
            return Err("DENIED: worker crash injection available only in reviewed CI".into());
        }
        if cancel_project || crash_owner {
            // The debugger reached a real stopped Cargo-built project and
            // returned actual nonempty thread/stack evidence. Dropping the
            // native gateway must journal CancelRequested BEFORE the Job
            // termination and verify the entire kernel descendant tree.
            drop(dap);
            let history = supervisor
                .receipt_history(&debug_job.job_id)
                .map_err(|e| e.code())?;
            if !history.windows(2).any(|pair| {
                pair[0].body.phase == JobPhase::CancelRequested
                    && pair[1].body.phase == JobPhase::Cancelled
            }) {
                return Err("DENIED: project LLDB cancel receipts not fsynced before tree kill".into());
            }
            println!("DEV045C_WINDOWS_PROJECT_LLDB_CANCEL_JOB_TREE_AND_RECEIPTS=PASS");
        } else if verify_owner_event {
            #[cfg(all(windows, debug_assertions, feature = "dev045d5-ci-event-owner"))]
            {
                let seq = owner_receipt.as_ref()
                    .ok_or("DENIED: owner event receipt disappeared before DAP disconnect")?.0;
                dap.close_with_real_owner_continue_for_ci(seq)
                    .map_err(|_| "DENIED: original LLDB ContinueDebugEvent revocation missing before Job terminal")?;
                println!("DEV045D5_NATIVE_LLDB_EVENT_CONTINUED_AND_REVOKED=PASS");
            }
            #[cfg(not(all(windows, debug_assertions, feature = "dev045d5-ci-event-owner")))]
            return Err("DENIED: unreviewed source cannot continue owner event".into());
        } else {
            dap.close().map_err(|e| e.code())?;
        }
        if supervisor.record_human_approval(
            &debug_job.job_id, &debug_job.plan_sha256, &debug_plan.source_head, true,
        ).is_ok() {
            return Err("DENIED: consumed debugger approval could be replayed".into());
        }
        if sha(&source_file)? != original_source_sha {
            return Err("DENIED: debugger changed original project source".into());
        }
        let expected_debug = if cancel_project || crash_owner {
            JobPhase::Cancelled
        } else { JobPhase::Completed };
        let jobs = supervisor.inspect().map_err(|e| e.code())?;
        if jobs.len() != 2 || jobs.iter().any(|j| {
            j.requires_reconciliation
                || j.phase != (if j.job_id == debug_job.job_id {
                    expected_debug
                } else {
                    JobPhase::Completed
                })
        }) {
            return Err("DENIED: native protected build/debug terminals incomplete".into());
        }
        #[cfg(all(windows, debug_assertions, feature = "dev045d4c-ci-oracle"))]
        if let Some((running_sha, completed_build_sha)) = bound_line_receipts {
            let debug_chain = supervisor.receipt_history(&debug_job.job_id)
                .map_err(|e| e.code())?;
            let original_running = debug_chain.iter().any(|r| {
                r.body.phase == JobPhase::Running
                    && r.entry_hash == running_sha
                    && r.body.plan_sha256 == debug_job.plan_sha256
            });
            let terminal = debug_chain.last()
                .ok_or("DENIED: protected Windows line proof missing terminal receipt")?;
            let build_chain = supervisor.receipt_history(&build_job.job_id)
                .map_err(|e| e.code())?;
            let original_build = build_chain.last().is_some_and(|r| {
                r.body.phase == JobPhase::Completed
                    && r.entry_hash == completed_build_sha
                    && r.body.plan_sha256 == build_job.plan_sha256
            });
            if !original_running || !original_build
                || terminal.body.phase != expected_debug
                || terminal.body.plan_sha256 != debug_job.plan_sha256
                || terminal.entry_hash == running_sha {
                return Err("DENIED: protected Windows PDB stop did not survive independent fsynced terminal".into());
            }
            println!("DEV045D4C_LU_JOB_PDB_TERMINAL_JOURNAL=PASS");
        }
        let target_root = target.root.clone();
        drop(target);
        if target_root.exists() {
            return Err("DENIED: native project target stage survived owner".into());
        }
        drop(supervisor);
        let replay = Supervisor::open(journal).map_err(|e| e.code())?;
        let jobs = replay.inspect().map_err(|e| e.code())?;
        if jobs.len() != 2 || jobs.iter().any(|j| {
            j.requires_reconciliation
                || j.phase != (if j.job_id == debug_job.job_id {
                    expected_debug
                } else {
                    JobPhase::Completed
                })
        }) {
            return Err("DENIED: native SCM debug receipts failed journal restart".into());
        }
        #[cfg(all(windows, debug_assertions, feature = "dev045d5-ci-event-owner"))]
        if let Some((sequence, owner_sha)) = owner_receipt {
            replay.verify_native_owner_receipt_after_restart(
                &debug_job.job_id, sequence, &owner_sha,
            ).map_err(|e| e.code())?;
            println!("DEV045D5_OWNER_EVENT_FSYNC_RESTART_RECOVERY=PASS");
        }
        #[cfg(all(windows, debug_assertions, feature = "dev045d5-ci-event-owner"))]
        if let Some((action_id, preview, last, phase)) = action_audit {
            replay.verify_native_debug_action_after_restart(
                &action_id, &preview, &last, &phase,
            ).map_err(|_| "DENIED: native D6B one-use action journal missing or tampered after restart")?;
            println!("DEV046B_ACTION_JOURNAL_REOPEN_AUDIT=PASS;grant_restored=false;operation_not_dispatched=true");
        }
        #[cfg(all(windows, debug_assertions, feature = "dev046c-ci-control"))]
        if let Some((action_id, preview, last, phase)) = control_audit {
            replay.verify_native_debug_action_after_restart(
                &action_id, &preview, &last, &phase,
            ).map_err(|_| "DENIED: D6C verified/uncertain control receipt not durable after restart")?;
            match ci_control {
                Some("step") if phase == "ResultVerified" => {},
                Some("breakpoint") if phase == "ResultUncertain" => {},
                _ => return Err("DENIED: native source control outcome asserted without matching OS witness".into()),
            }
            println!("DEV046C_NATIVE_STEP_OR_BP_ACTION_JOURNAL_RESTART=PASS;old_consent_replay=false");
        }
        // Opening a fresh supervisor session must not restore either prior
        // grant, even though both old plan hashes and receipts remain readable.
        if replay.record_human_approval(
            &build_job.job_id, &build_job.plan_sha256, &build_plan.source_head, true,
        ).is_ok() || replay.record_human_approval(
            &debug_job.job_id, &debug_job.plan_sha256, &debug_plan.source_head, true,
        ).is_ok() {
            return Err("DENIED: completed Windows project debug grant replayed after restart".into());
        }
        println!("DEV045B_WINDOWS_CARGO_TO_APPCONTAINER_PROJECT_LLDB=PASS");
        println!("DEV045B_WINDOWS_DISTINCT_GRANTS_RESTART_AND_STAGE_CLEANUP=PASS");
        Ok(())
    }

    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        let args: Vec<String> = env::args().skip(1).collect();
        #[cfg(windows)]
        if args.as_slice() == ["--project-debug"] {
            return run_windows_project_debug(false, false, false, false, false, false, None);
        }
        #[cfg(windows)]
        if args.as_slice() == ["--project-debug-cancel"] {
            return run_windows_project_debug(true, false, false, false, false, false, None);
        }
        #[cfg(windows)]
        if args.as_slice() == ["--project-debug-module"] {
            return run_windows_project_debug(false, true, false, false, false, false, None);
        }
        #[cfg(windows)]
        if args.as_slice() == ["--project-debug-thread"] {
            return run_windows_project_debug(false, true, true, false, false, false, None);
        }
        #[cfg(windows)]
        if args.as_slice() == ["--project-debug-line"] {
            return run_windows_project_debug(false, true, true, true, false, false, None);
        }
        #[cfg(windows)]
        if args.as_slice() == ["--project-debug-line-cancel"] {
            return run_windows_project_debug(true, true, true, true, false, false, None);
        }
        #[cfg(windows)]
        if args.as_slice() == ["--project-debug-owner"] {
            return run_windows_project_debug(false, true, true, true, true, false, None);
        }
        #[cfg(windows)]
        if args.as_slice() == ["--project-debug-owner-cancel"] {
            return run_windows_project_debug(true, true, true, true, true, false, None);
        }
        #[cfg(windows)]
        if args.as_slice() == ["--project-debug-owner-crash"] {
            return run_windows_project_debug(false, true, true, true, true, true, None);
        }
        #[cfg(windows)]
        if args.as_slice() == ["--project-debug-step"] {
            return run_windows_project_debug(false, true, true, true, true, false, Some("step"));
        }
        #[cfg(windows)]
        if args.as_slice() == ["--project-debug-breakpoint"] {
            return run_windows_project_debug(false, true, true, true, true, false, Some("breakpoint"));
        }
        let cancel = args.as_slice() == ["--cancel"];
        #[cfg(target_os = "linux")]
        let crash = args.as_slice() == ["--crash-after-initialize"];
        #[cfg(not(target_os = "linux"))]
        let crash = false;
        #[cfg(target_os = "linux")]
        if args.as_slice() == ["--verify-crash-replay"] {
            return verify_crash_replay();
        }
        if !args.is_empty() && !cancel && !crash {
            return Err("DENIED: only fixed native test modes accepted".into());
        }
        let bundle =
            PathBuf::from(env::var_os("LU_IDE_WO6_DAP_BUNDLE").ok_or(
                "DENIED: root-owned or host-sealed acceptance-only LLDB-DAP bundle absent",
            )?);
        if !bundle.is_absolute() {
            return Err("DENIED: bundle path must be absolute".into());
        }
        let digest = native_lsp_bundle_digest(&bundle, "dap_lldb_fixed").map_err(|e| e.code())?;
        let nonce = format!(
            "{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        );
        #[cfg(target_os = "linux")]
        let root = if crash {
            witness_root()?
        } else {
            env::temp_dir().join(format!("lu-ide-wo6-native-dap-{nonce}"))
        };
        #[cfg(not(target_os = "linux"))]
        let root = env::temp_dir().join(format!("lu-ide-wo6-native-dap-{nonce}"));
        let source = root.join("source");
        let staging = root.join("stage");
        fs::create_dir_all(&source)?;
        fs::create_dir_all(&staging)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&staging, fs::Permissions::from_mode(0o700))?;
        }
        fs::write(source.join("main.c"), "int main(void) { return 0; }\n")?;
        let input =
            publish_lsp_input(&source, &staging, "main.c", "clangd").map_err(|e| e.code())?;
        let journal = root.join("receipts.jsonl");
        let supervisor = Arc::new(Supervisor::open(journal.clone()).map_err(|e| e.code())?);
        let plan = JobPlan {
            project_id: format!("ide-{}", "a".repeat(32)),
            recipe_id: "dap_lldb_fixed".into(),
            source_head: "b".repeat(40),
            input_snapshot_sha256: input.seal.source_sha256.clone(),
            manifest_sha256: input.seal.manifest_sha256.clone(),
            toolchain_sha256: digest,
            debug_target_binding_sha256: None,
            runtime_limit_ms: 90_000,
            memory_limit_bytes: 512 * 1024 * 1024,
            process_limit: 16,
            log_limit_bytes: 64 * 1024,
            effect_class: "debug_inspection_isolated".into(),
        };
        let prepared = supervisor.prepare(plan.clone()).map_err(|e| e.code())?;
        supervisor
            .record_human_approval(
                &prepared.job_id,
                &prepared.plan_sha256,
                &plan.source_head,
                true,
            )
            .map_err(|e| e.code())?;
        let mut dap = start(
            Arc::clone(&supervisor),
            &prepared.job_id,
            &plan,
            &input,
            &bundle,
        )?;
        dap.initialize().map_err(|e| e.code())?;
        poll_until_ready(&mut dap, cancel)?;
        println!("PASS: native OS-restricted LLDB-DAP initialize");
        #[cfg(target_os = "linux")]
        if crash {
            // Persist independent identity before abrupt exit. process::exit
            // skips Rust Drop, so only the kernel PDEATHSIG, systemd process
            // scope, and later fresh-host journal replay can establish safety.
            let path = root.join("crash-job-id");
            fs::write(&path, &prepared.job_id)?;
            fs::File::open(path)?.sync_all()?;
            std::process::exit(73);
        }
        dap.launch_fixed_fixture().map_err(|e| e.code())?;
        // The adapter may have sent initialized in the same read that
        // completed initialize, before the native fixed launch was issued.
        if dap.configuration_needed() {
            dap.configuration_done().map_err(|e| e.code())?;
        }
        let began = Instant::now();
        let mut stopped = None;
        let mut threads = false;
        let mut stack = false;
        let mut queried_threads = false;
        let mut queried_stack = false;
        while began.elapsed() < Duration::from_secs(35) {
            for event in dap.poll().map_err(|e| e.code())? {
                match event {
                    DapEvent::ConfigurationRequested => {
                        // DAP permits initialized only after launch. Complete the
                        // fixed empty configuration; never run adapter requests.
                        dap.configuration_done().map_err(|e| e.code())?;
                    }
                    DapEvent::Stopped { thread_id } => {
                        stopped = Some(thread_id);
                    }
                    DapEvent::ThreadsReceived => threads = true,
                    DapEvent::StackTraceReceived => stack = true,
                    DapEvent::Terminated => {
                        return Err("DENIED: native debuggee exited before stop/stack".into())
                    }
                    _ => {}
                }
            }
            if stopped.is_some() && !queried_threads && dap.phase() == DapPhase::Stopped {
                dap.threads().map_err(|e| e.code())?;
                queried_threads = true;
            }
            if threads && !queried_stack {
                dap.stack_trace(stopped.ok_or("DENIED: no stopped thread")?)
                    .map_err(|e| e.code())?;
                queried_stack = true;
            }
            if stack {
                break;
            }
            thread::sleep(Duration::from_millis(35));
        }
        if stopped.is_none() || !threads || !stack {
            return Err("DENIED: native protected stop/thread/stack not observed".into());
        }
        println!("PASS: real native protected LLDB-DAP stopped, threads, stackTrace");
        if cancel {
            // This must record CancelRequested, kill/verify the real kernel
            // tree and journal Cancelled before host restart; no DAP reply
            // can substitute for OS descendant termination.
            drop(dap);
        } else {
            dap.close().map_err(|e| e.code())?;
        }
        drop(supervisor);
        let replay = Supervisor::open(journal).map_err(|e| e.code())?;
        let jobs = replay.inspect().map_err(|e| e.code())?;
        let expected = if cancel {
            JobPhase::Cancelled
        } else {
            JobPhase::Completed
        };
        if jobs.len() != 1 || jobs[0].phase != expected || jobs[0].requires_reconciliation {
            return Err("DENIED: real native debugger cleanup or D3 restart receipt failed".into());
        }
        println!("PASS: native LLDB-DAP whole-tree OS cleanup and durable restart receipt");
        Ok(())
    }
}
#[cfg(any(
    all(
        windows,
        feature = "dap-transport",
        feature = "protected-recovery-service"
    ),
    all(
        target_os = "linux",
        feature = "dap-transport",
        feature = "linux-worker"
    )
))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    protected::run()
}
