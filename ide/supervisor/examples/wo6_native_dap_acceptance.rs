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
    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        let args: Vec<String> = env::args().skip(1).collect();
        let cancel = args.as_slice() == ["--cancel"];
        if !args.is_empty() && !cancel {
            return Err("DENIED: only normal or fixed cancellation acceptance".into());
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
            runtime_limit_ms: 90_000,
            memory_limit_bytes: 512 * 1024 * 1024,
            process_limit: 4,
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
        dap.launch_fixed_fixture().map_err(|e| e.code())?;
        let began = Instant::now();
        let mut stopped = None;
        let mut threads = false;
        let mut stack = false;
        let mut queried_threads = false;
        let mut queried_stack = false;
        while began.elapsed() < Duration::from_secs(35) {
            for event in dap.poll().map_err(|e| e.code())? {
                match event {
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
