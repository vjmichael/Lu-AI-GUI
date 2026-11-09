//! DEV-02.6B: REAL protected OS worker LSP protocols, three reviewed native
//! providers only. This fixture MUST run on an independently provisioned
//! Windows AppContainer/Job/SCM or Linux Bwrap/delegated-cgroup host.
//! A passing source test or uncontained stdio server is NEVER accepted here.
//! Synthetic fixture approval is NOT a genuine UI consent or release approval.
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
    eprintln!("DENIED: native protected Windows/Linux LSP capabilities required");
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
    use lu_ide_workspace::{
        lsp::DiagnosticSource,
        lsp_session::{LspEvent, Position},
    };
    use sha2::{Digest, Sha256};
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

    struct Case {
        id: &'static str,
        recipe: &'static str,
        filename: &'static str,
        language: &'static str,
        text: &'static str,
        symbol: &'static str,
        diagnostics: bool,
        semantics: bool,
    }
    fn case(id: &str) -> Result<Case, &'static str> {
        match id {
            "clangd"=>Ok(Case{
                id:"clangd",recipe:"lsp_clangd",filename:"main.c",language:"c",
                text:"int calculate(int x) { return x + 1; }\nint main(void) { int value = calculate(2); return missing_identifier; }\n",
                symbol:"calculate",diagnostics:true,semantics:true,
            }),
            "rust-analyzer"=>Ok(Case{
                id:"rust-analyzer",recipe:"lsp_rust_analyzer",filename:"main.rs",language:"rust",
                text:"fn calculate(x: i32) -> i32 { x + 1 }\nfn main() { let _value = calculate(2); let _missing = undefined_symbol; }\n",
                symbol:"calculate",diagnostics:true,semantics:true,
            }),
            "ruff"=>Ok(Case{
                id:"ruff",recipe:"lsp_ruff",filename:"main.py",language:"python",
                text:"def calculate(x: int) -> int:\n    return x + 1\n\nprint(undefined_symbol)\n",
                symbol:"calculate",diagnostics:true,semantics:false,
            }),
            _=>Err("DENIED: only three fixed provider IDs allowed")
        }
    }
    fn sha(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
        let mut digest = Sha256::new();
        let mut stream = fs::File::open(path)?;
        let mut block = [0u8; 65536];
        loop {
            let n = stream.read(&mut block)?;
            if n == 0 {
                break;
            }
            digest.update(&block[..n]);
        }
        Ok(format!("{:x}", digest.finalize()))
    }
    fn expected_hash(key: &str) -> Result<String, Box<dyn std::error::Error>> {
        let expected = env::var(key)?;
        if expected.len() != 64 || !expected.bytes().all(|x| x.is_ascii_hexdigit()) {
            return Err("DENIED: mandatory independent pinned SHA256 malformed".into());
        }
        Ok(expected.to_lowercase())
    }
    #[cfg(windows)]
    fn start(
        supervisor: Arc<Supervisor>,
        job: &str,
        plan: &JobPlan,
        head: &str,
        snapshot: &lu_ide_execution_supervisor::snapshot::PublishedInput,
        root: &Path,
    ) -> Result<NativeSession, Box<dyn std::error::Error>> {
        let service = PathBuf::from(env::var("LU_IDE_E2_TRUSTED_SERVICE")?);
        if !service.is_absolute()
            || sha(&service)? != expected_hash("LU_IDE_E2_TRUSTED_SERVICE_SHA256")?
        {
            return Err(
                "DENIED: independent service executable not present or hash mismatch".into(),
            );
        }
        let broker = TrustedBroker {
            executable: service,
            expected_sha256: expected_hash("LU_IDE_E2_TRUSTED_SERVICE_SHA256")?,
            private_witness_directory: PathBuf::from(
                env::var_os("RUNNER_TEMP").ok_or("DENIED: missing protected runner temp")?,
            )
            .join("dev026b-protected-fixture-witness"),
        };
        Ok(
            start_windows_lsp(supervisor, job, plan, head, snapshot, root, &broker)
                .map_err(|e| e.code())?,
        )
    }
    #[cfg(target_os = "linux")]
    fn start(
        supervisor: Arc<Supervisor>,
        job: &str,
        plan: &JobPlan,
        head: &str,
        snapshot: &lu_ide_execution_supervisor::snapshot::PublishedInput,
        root: &Path,
    ) -> Result<NativeSession, Box<dyn std::error::Error>> {
        Ok(start_linux_lsp(supervisor, job, plan, head, snapshot, root).map_err(|e| e.code())?)
    }
    fn wait_until<F>(
        session: &mut NativeSession,
        label: &str,
        predicate: F,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        F: Fn(&LspEvent) -> bool,
    {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(40) {
            for event in session.poll().map_err(|e| e.code())? {
                if predicate(&event) {
                    println!("DEV026B_REAL_PROTECTED_LSP_EVENT={label}");
                    return Ok(());
                }
            }
            thread::sleep(Duration::from_millis(50));
        }
        Err(format!("DENIED: actual contained LSP {label} response missing").into())
    }
    fn position_for(source: &str, symbol: &str) -> Result<Position, &'static str> {
        let line = source
            .lines()
            .nth(1)
            .ok_or("DENIED: malformed source fixture")?;
        let character = line.find(symbol).ok_or("DENIED: fixture symbol missing")?;
        Ok(Position {
            line: 1,
            character: character as u32,
        })
    }
    pub fn main() -> Result<(), Box<dyn std::error::Error>> {
        let mut args = env::args().skip(1);
        let id = args
            .next()
            .ok_or("DENIED: exactly one fixed provider ID required")?;
        let mode = args.next();
        if args.next().is_some() || !matches!(mode.as_deref(), None | Some("--cancel")) {
            return Err("DENIED: only optional --cancel allowed".into());
        }
        let cancel = mode.is_some();
        let case = case(&id)?;
        let bundle = PathBuf::from(env::var("LU_IDE_E2_PINNED_LSP_BUNDLE")?);
        if !bundle.is_absolute() {
            return Err("DENIED: only absolute reviewed native resource roots allowed".into());
        }
        let expected = expected_hash("LU_IDE_E2_EXPECTED_BUNDLE_SHA256")?;
        let measured = native_lsp_bundle_digest(&bundle, case.recipe).map_err(|e| e.code())?;
        if expected != measured {
            return Err("DENIED: independently pinned entire resource tree differs".into());
        }
        let root = PathBuf::from(
            env::var_os("RUNNER_TEMP").ok_or("DENIED: protected runner temp unavailable")?,
        )
        .join(format!("luide-dev026b-{}-{}", case.id, std::process::id()));
        if root.exists() {
            return Err("DENIED: repeated fixture root".into());
        }
        let project = root.join("project");
        let stage = root.join("snapshot");
        fs::create_dir_all(&project)?;
        fs::create_dir_all(&stage)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&stage, fs::Permissions::from_mode(0o700))?;
        }
        let user_source = project.join(case.filename);
        fs::write(&user_source, case.text)?;
        let source_hash = sha(&user_source)?;
        let snapshot =
            publish_lsp_input(&project, &stage, case.filename, case.id).map_err(|e| e.code())?;
        let supervisor =
            Arc::new(Supervisor::open(root.join("receipts.jsonl")).map_err(|e| e.code())?);
        let head = "a".repeat(40);
        let plan = JobPlan {
            project_id: format!("ide-{}", "b".repeat(32)),
            recipe_id: case.recipe.into(),
            source_head: head.clone(),
            input_snapshot_sha256: snapshot.seal.source_sha256.clone(),
            manifest_sha256: snapshot.seal.manifest_sha256.clone(),
            toolchain_sha256: measured,
            runtime_limit_ms: 180_000,
            memory_limit_bytes: 1024 * 1024 * 1024,
            process_limit: 32,
            log_limit_bytes: 65_536,
            effect_class: "language_analysis_isolated".into(),
        };
        let prepared = supervisor.prepare(plan.clone()).map_err(|e| e.code())?;
        // Test fixture authorizes its OWN synthetic file. No real user dialog
        // or installed product consent is claimed by this protocol fixture.
        supervisor
            .record_human_approval(&prepared.job_id, &prepared.plan_sha256, &head, true)
            .map_err(|e| e.code())?;
        let mut session = start(
            Arc::clone(&supervisor),
            &prepared.job_id,
            &plan,
            &head,
            &snapshot,
            &bundle,
        )?;
        session.initialize().map_err(|e| e.code())?;
        let start_time = Instant::now();
        while !session.is_ready() && start_time.elapsed() < Duration::from_secs(40) {
            let _ = session.poll().map_err(|e| e.code())?;
            thread::sleep(Duration::from_millis(50));
        }
        if !session.is_ready() {
            return Err("DENIED: real protected provider initialize timed out".into());
        }
        println!("DEV026B_REAL_PROTECTED_INITIALIZE={}", case.id);
        let uri = session.native_document_uri().to_owned();
        let source = DiagnosticSource {
            project_id: plan.project_id.clone(),
            file_id: format!("file_{}", case.id.replace('-', "_")),
            disk_sha256: snapshot.seal.manifest_sha256.clone(),
            buffer_revision: 1,
            language_id: case.language.into(),
        };
        session
            .open(source.clone(), &uri, case.text)
            .map_err(|e| e.code())?;
        if case.diagnostics {
            wait_until(
                &mut session,
                "real_diagnostics",
                |event| matches!(event,LspEvent::Diagnostics(items) if !items.is_empty()),
            )?;
        }
        if case.semantics {
            let at = position_for(case.text, case.symbol)?;
            session
                .query("hover", &source, at.clone())
                .map_err(|e| e.code())?;
            wait_until(
                &mut session,
                "hover",
                |event| matches!(event,LspEvent::Hover(text) if !text.is_empty()),
            )?;
            session
                .query("definition", &source, at.clone())
                .map_err(|e| e.code())?;
            wait_until(&mut session, "same_file_definition", |event| {
                matches!(event, LspEvent::Definition(Some(_)))
            })?;
            session
                .query("completion", &source, at)
                .map_err(|e| e.code())?;
            // Completion can legitimately be empty at a complete identifier:
            // a genuine non-empty response is still required for acceptance.
            wait_until(
                &mut session,
                "completion",
                |event| matches!(event,LspEvent::Completions(items) if !items.is_empty()),
            )?;
        }
        if sha(&user_source)? != source_hash {
            return Err("DENIED: real LSP modified its registered source".into());
        }
        if cancel {
            drop(session);
        } else {
            session.close().map_err(|e| e.code())?;
        }
        drop(supervisor);
        let restarted = Supervisor::open(root.join("receipts.jsonl")).map_err(|e| e.code())?;
        let state = restarted.inspect().map_err(|e| e.code())?;
        let phase = if cancel {
            JobPhase::Cancelled
        } else {
            JobPhase::Completed
        };
        if state.len() != 1 || state[0].phase != phase || state[0].requires_reconciliation {
            return Err("DENIED: real protected OS tree/journal restart gate failed".into());
        }
        println!(
            "DEV026B_REAL_PROTECTED_NATIVE_LSP_KERNEL_REAPED_AND_JOURNALED={}",
            case.id
        );
        println!("DEV026B_USER_OS_DIALOG_AND_INSTALLED_GUI_ACCEPTANCE=NOT_TESTED");
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
