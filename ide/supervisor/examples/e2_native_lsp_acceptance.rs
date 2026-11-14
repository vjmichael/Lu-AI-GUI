//! Native E2 integration acceptance: actual pinned clangd under OS-specific
//! protected worker and durable D3 supervisor. Acceptance fixture ONLY:
//! OS provisioned bundle and service pins are *self-measured* in this test,
//! NOT release-approved installer provenance.
#[cfg(not(any(
    all(
        windows,
        feature = "lsp-transport",
        feature = "protected-recovery-service"
    ),
    all(
        target_os = "linux",
        feature = "lsp-transport",
        feature = "linux-worker"
    )
)))]
fn main() {
    eprintln!("DENIED: E2 requires native protected OS LSP worker feature");
    std::process::exit(2);
}
#[cfg(any(
    all(
        windows,
        feature = "lsp-transport",
        feature = "protected-recovery-service"
    ),
    all(
        target_os = "linux",
        feature = "lsp-transport",
        feature = "linux-worker"
    )
))]
mod enabled {
    #[cfg(target_os = "linux")]
    use lu_ide_execution_supervisor::linux_worker::lsp_native::{
        native_lsp_bundle_digest, start_linux_lsp, LinuxLspSession,
    };
    use lu_ide_execution_supervisor::{snapshot::publish_lsp_input, JobPhase, JobPlan, Supervisor};
    #[cfg(windows)]
    use lu_ide_execution_supervisor::{
        windows_broker::TrustedBroker,
        windows_worker::lsp_native::{
            native_lsp_bundle_digest, start_windows_lsp, WindowsLspSession,
        },
    };
    use lu_ide_workspace::lsp::DiagnosticSource;
    use lu_ide_workspace::lsp_session::{LspEvent, Position};
    use std::{
        env, fs,
        io::Read,
        path::{Path, PathBuf},
        sync::Arc,
        thread,
        time::{Duration, Instant},
    };
    #[cfg(windows)]
    type NativeSession = WindowsLspSession;
    #[cfg(target_os = "linux")]
    type NativeSession = LinuxLspSession;

    fn sha256(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
        use sha2::{Digest, Sha256};
        let mut file = fs::File::open(path)?;
        let mut sha = Sha256::new();
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            sha.update(&buffer[..n]);
        }
        Ok(format!("{:x}", sha.finalize()))
    }
    fn wait_event(
        session: &mut NativeSession,
        label: &str,
        matched: fn(&LspEvent) -> bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(15) {
            for event in session.poll().map_err(|e| e.code())? {
                if matched(&event) {
                    println!("PASS: real contained clangd {label} response");
                    return Ok(());
                }
            }
            thread::sleep(Duration::from_millis(40));
        }
        Err(format!("DENIED: real contained clangd {label} response timed out").into())
    }
    #[cfg(windows)]
    fn start(
        supervisor: Arc<Supervisor>,
        id: &str,
        plan: &JobPlan,
        head: &str,
        input: &lu_ide_execution_supervisor::snapshot::PublishedInput,
        bundle: &Path,
    ) -> Result<NativeSession, Box<dyn std::error::Error>> {
        let service = PathBuf::from(
            env::var_os("LU_IDE_E2_TRUSTED_SERVICE")
                .ok_or("DENIED: explicit separately installed restricted SCM service required")?,
        );
        if !service.is_absolute() {
            return Err("DENIED: service path not absolute".into());
        }
        let config = TrustedBroker {
            executable: service.clone(),
            expected_sha256: sha256(&service)?,
            private_witness_directory: PathBuf::from(
                env::var_os("RUNNER_TEMP").unwrap_or_else(|| env::temp_dir().into_os_string()),
            )
            .join("e2-protected-fixture"),
        };
        Ok(
            start_windows_lsp(supervisor, id, plan, head, input, bundle, &config)
                .map_err(|e| e.code())?,
        )
    }
    #[cfg(target_os = "linux")]
    fn start(
        supervisor: Arc<Supervisor>,
        id: &str,
        plan: &JobPlan,
        head: &str,
        input: &lu_ide_execution_supervisor::snapshot::PublishedInput,
        bundle: &Path,
    ) -> Result<NativeSession, Box<dyn std::error::Error>> {
        Ok(start_linux_lsp(supervisor, id, plan, head, input, bundle).map_err(|e| e.code())?)
    }
    fn native_fixture_trace(stage: &'static str) {
        #[cfg(target_os = "linux")]
        if cfg!(debug_assertions)
            && env::var_os("LU_IDE_E2_NATIVE_TRACE").as_deref() == Some(std::ffi::OsStr::new("1"))
        {
            eprintln!("PROTECTED_LINUX_FIXTURE_STAGE={stage}");
        }
        #[cfg(not(target_os = "linux"))]
        let _ = stage;
    }

    pub fn main() -> Result<(), Box<dyn std::error::Error>> {
        native_fixture_trace("fixture_entry");
        let args: Vec<String> = env::args().skip(1).collect();
        let cancel = args.as_slice() == ["--cancel"];
        if !args.is_empty() && !cancel {
            return Err("DENIED: unsupported fixture mode".into());
        }
        let bundle = PathBuf::from(env::var_os("LU_IDE_E2_PINNED_LSP_BUNDLE").ok_or(
            "DENIED: explicit independently provisioned test-only clangd bundle required",
        )?);
        if !bundle.is_absolute() {
            return Err("DENIED: bundle must be absolute".into());
        }
        native_fixture_trace("bundle_digest_begin");
        let bundle_sha = native_lsp_bundle_digest(&bundle, "lsp_clangd").map_err(|e| e.code())?;
        native_fixture_trace("bundle_digest_verified");
        // A unique private acceptance project, never user-selected code.
        let id = format!(
            "e2-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        );
        let root = env::temp_dir().join(format!("lu-ide-{id}"));
        let project = root.join("source");
        let stage = root.join("staging");
        fs::create_dir_all(&project)?;
        fs::create_dir_all(&stage)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&stage, fs::Permissions::from_mode(0o700))?;
        }
        let text="int addition(int a, int b) { return a + b; }\nint main(void) { return addition(1,2); }\n";
        fs::write(project.join("main.c"), text)?;
        native_fixture_trace("fixed_source_project_created");
        let input =
            publish_lsp_input(&project, &stage, "main.c", "clangd").map_err(|e| e.code())?;
        native_fixture_trace("fixed_source_snapshot_published");
        let supervisor =
            Arc::new(Supervisor::open(root.join("receipts.jsonl")).map_err(|e| e.code())?);
        native_fixture_trace("durable_supervisor_open");
        let head = "a".repeat(40);
        let plan = JobPlan {
            project_id: format!("ide-{}", "b".repeat(32)),
            recipe_id: "lsp_clangd".into(),
            source_head: head.clone(),
            input_snapshot_sha256: input.seal.source_sha256.clone(),
            manifest_sha256: input.seal.manifest_sha256.clone(),
            toolchain_sha256: bundle_sha,
            debug_target_binding_sha256: None,
            runtime_limit_ms: 60_000,
            memory_limit_bytes: 512 * 1024 * 1024,
            process_limit: 16, // Kernel cgroup counts clangd threads; still globally capped.
            log_limit_bytes: 65_536,
            effect_class: "language_analysis_isolated".into(),
        };
        let prepared = supervisor.prepare(plan.clone()).map_err(|e| e.code())?;
        native_fixture_trace("job_plan_prepared");
        supervisor
            .record_human_approval(&prepared.job_id, &prepared.plan_sha256, &head, true)
            .map_err(|e| e.code())?;
        native_fixture_trace("one_use_approval_recorded");
        native_fixture_trace("native_worker_start_requested");
        let mut session = start(
            Arc::clone(&supervisor),
            &prepared.job_id,
            &plan,
            &head,
            &input,
            &bundle,
        )?;
        session.initialize().map_err(|e| e.code())?;
        let began = Instant::now();
        while !session.is_ready() && began.elapsed() < Duration::from_secs(15) {
            let _ = session.poll().map_err(|e| e.code())?;
            thread::sleep(Duration::from_millis(50));
        }
        if !session.is_ready() {
            return Err("DENIED: native clangd initialization".into());
        }
        println!("PASS: native contained clangd initialized");
        let uri = session.native_document_uri().to_owned();
        let mut source = DiagnosticSource {
            project_id: plan.project_id.clone(),
            file_id: "file_main".into(),
            disk_sha256: input.seal.manifest_sha256.clone(),
            buffer_revision: 1,
            language_id: "c".into(),
        };
        session
            .open(source.clone(), &uri, text)
            .map_err(|e| e.code())?;
        session
            .query(
                "hover",
                &source,
                Position {
                    line: 1,
                    character: 27,
                },
            )
            .map_err(|e| e.code())?;
        wait_event(&mut session, "hover", |x| matches!(x, LspEvent::Hover(_)))?;
        session
            .query(
                "definition",
                &source,
                Position {
                    line: 1,
                    character: 27,
                },
            )
            .map_err(|e| e.code())?;
        wait_event(&mut session, "same-file definition", |x| {
            matches!(x, LspEvent::Definition(Some(_)))
        })?;
        let changed =
            "int addition(int a, int b) { return a + b; }\nint main(void) { return addi; }\n";
        let previous = source.clone();
        source.buffer_revision = 2;
        session
            .changed(&previous, source.clone(), changed)
            .map_err(|e| e.code())?;
        // Native test must observe actual revision-scoped diagnostics from
        // clangd, not just protocol compilation or a fabricated diagnostic.
        // The intentionally undeclared 'addi' identifier produces at least
        // one diagnostic on both clangd 17 (Debian direct protocol witness)
        // and supported contemporary clangd releases.
        wait_event(
            &mut session,
            "changed-source diagnostics",
            |x| matches!(x, LspEvent::Diagnostics(items) if !items.is_empty()),
        )?;
        session
            .query(
                "completion",
                &source,
                Position {
                    line: 1,
                    character: 28,
                },
            )
            .map_err(|e| e.code())?;
        wait_event(
            &mut session,
            "completion",
            |x| matches!(x,LspEvent::Completions(items) if !items.is_empty()),
        )?;
        if cancel {
            drop(session); // D3 CancelRequested must precede kernel process-tree kill.
        } else {
            session.close().map_err(|e| e.code())?;
        }
        drop(supervisor);
        let reopened = Supervisor::open(root.join("receipts.jsonl")).map_err(|e| e.code())?;
        let summary = reopened.inspect().map_err(|e| e.code())?;
        let expected = if cancel {
            JobPhase::Cancelled
        } else {
            JobPhase::Completed
        };
        if summary.len() != 1 || summary[0].phase != expected || summary[0].requires_reconciliation
        {
            return Err(
                "DENIED: terminated clangd did not leave durable exact terminal receipt".into(),
            );
        }
        println!(
            "PASS: native E2 supervised real clangd; OS tree reaped; restart receipt verified"
        );
        Ok(())
    }
}
#[cfg(any(
    all(
        windows,
        feature = "lsp-transport",
        feature = "protected-recovery-service"
    ),
    all(
        target_os = "linux",
        feature = "lsp-transport",
        feature = "linux-worker"
    )
))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    enabled::main()
}
