//! FIN-04W2-C developer-CI fixed original-authority transaction.
//! This is an independently compiled native fixture, never an IDE command
//! or physical human confirmation. It cannot select a process to run,
//! control LLDB, attach, provide a DAP payload or forge an OS admission.
//! The FIRST-PARTY restricted SCM service must create the exact pinned
//! benign worker suspended under its original protected kernel Job.
//! All transaction receipts are written to the ORIGINAL Supervisor journal.
#![cfg_attr(not(all(windows,debug_assertions,feature="fin04w2-c2")),
    allow(unused))]
#[cfg(all(windows,debug_assertions,feature="fin04w2-c2"))]
fn main() {
    if let Err(error)=run() {
        eprintln!("FIN04W2_C_ORIGINAL_NATIVE_TRANSACTION=DENIED:{}",error.code());
        std::process::exit(1);
    }
}
#[cfg(not(all(windows,debug_assertions,feature="fin04w2-c2")))]
fn main() {
    eprintln!("FIN04W2_C_ORIGINAL_NATIVE_TRANSACTION=DISABLED");
    std::process::exit(2);
}
#[cfg(all(windows,debug_assertions,feature="fin04w2-c2"))]
fn run()->Result<(),lu_ide_execution_supervisor::SupervisorError>{
    use lu_ide_execution_supervisor::{
        JobPhase, JobPlan, Supervisor, SupervisorError,
        windows_fin04w2_c2_host,
    };
    use std::{
        fs::{self,File,OpenOptions},
        io::Write,path::{Path,PathBuf},
        time::{Duration,Instant},
    };
    use sha2::{Digest,Sha256};
    fn denied()->SupervisorError{SupervisorError::NoContainment}
    if std::env::args_os().len()!=1
        || std::env::var_os("GITHUB_ACTIONS").as_deref()
          !=Some(std::ffi::OsStr::new("true")){
        return Err(denied());
    }
    let root=PathBuf::from(std::env::var_os("LU_FIN04W2_NATIVE_FIXTURE_ROOT")
        .ok_or_else(denied)?);
    let meta=fs::symlink_metadata(&root).map_err(|_|denied())?;
    if !root.is_absolute()||!meta.is_dir()||meta.file_type().is_symlink(){
        return Err(denied());
    }
    use std::os::windows::fs::MetadataExt;
    if meta.file_attributes() & 0x400 != 0 {return Err(denied());}
    let original_journal=root.join("original-supervisor.jsonl");
    if original_journal.exists() || root.join("staged.json").exists()
        || root.join("allow-service").exists()
        || root.join("result.json").exists(){
        return Err(denied());
    }
    // The CI parent checked out an EXACT PR head and independently
    // supplies that native Git identity. Never assert a dummy source head
    // as verified current code; all other build/target hashes here remain
    // explicit benign-fixture coordinates, not a production PE/PDB seal.
    let source_head=std::env::var("LU_FIN04W2_NATIVE_SOURCE_HEAD")
        .map_err(|_|denied())?;
    if source_head.len()!=40 || !source_head.bytes().all(|b|b.is_ascii_hexdigit()){
        return Err(denied());
    }
    let plan=JobPlan {
        project_id:format!("ide-{}","a".repeat(32)),
        recipe_id:"dap_lldb_project".into(),
        source_head:source_head.clone(),
        input_snapshot_sha256:"c".repeat(64),
        manifest_sha256:"d".repeat(64),
        toolchain_sha256:"e".repeat(64),
        debug_target_binding_sha256:Some("f".repeat(64)),
        runtime_limit_ms:90_000,
        memory_limit_bytes:512*1024*1024,
        process_limit:16,log_limit_bytes:64*1024,
        effect_class:"debug_inspection_isolated".into(),
    };
    let supervisor=Supervisor::open(original_journal.clone())?;
    let prepared=supervisor.prepare(plan.clone())?;
    let host_exe=std::env::current_exe().map_err(|_|denied())?;
    let mut bytes=Vec::new();
    use std::io::Read;
    File::open(&host_exe).map_err(|_|denied())?
        .take(64*1024*1024).read_to_end(&mut bytes).map_err(|_|denied())?;
    let host_sha=format!("{:x}",Sha256::digest(&bytes));
    let manifest=serde_json::json!({
        "jobId":prepared.job_id,
        "planSha256":prepared.plan_sha256,
        "plan":plan,
        "hostExecutable":host_exe,
        "hostSha256":host_sha,
        "originalJournal":original_journal,
        "approvalKind":"CI_SIMULATED_NOT_PHYSICAL",
    });
    let staged=root.join("staged.json");
    let mut f=OpenOptions::new().write(true).create_new(true)
        .open(&staged).map_err(|_|denied())?;
    f.write_all(&serde_json::to_vec_pretty(&manifest)
        .map_err(|_|denied())?).map_err(|_|denied())?;
    f.sync_all().map_err(|_|denied())?;
    println!("FIN04W2_C_ORIGINAL_PREPARED_JOURNAL_FSYNC=PASS");

    // External CI provisioner reads the manifest, sets exact private ACLs
    // and native WFP rule, creates a restricted LocalService SCM service
    // with a service-only protected config, then creates allow-service.
    // The marker is orchestration only; never evidence of human approval.
    let until=Instant::now()+Duration::from_secs(45);
    while !root.join("allow-service").exists() {
        if Instant::now()>=until{return Err(denied());}
        std::thread::sleep(Duration::from_millis(75));
    }
    let service=PathBuf::from(std::env::var_os("LU_FIN04W2_SERVICE_BINARY")
        .ok_or_else(denied)?);
    let service_sha=std::env::var("LU_FIN04W2_SERVICE_SHA256")
        .map_err(|_|denied())?;
    let plan_sha=prepared.plan_sha256.clone();
    // Explicitly simulated original API event ONLY in disposable native CI.
    // Never claim this was a human clicking the desktop OS-native dialog.
    supervisor.record_human_approval(
        &prepared.job_id,&plan_sha,&plan.source_head,true,
    )?; // CI-SIMULATED approval, NEVER physical operator consent.
    windows_fin04w2_c2_host::run_fixed_after_original_approval(
        &supervisor,&prepared.job_id,&plan,&service,&service_sha,
    )?;
    let events=supervisor.receipt_history(&prepared.job_id)?;
    let expect=[
        (JobPhase::Prepared,"host_plan_prepared"),
        (JobPhase::Authorized,"fresh_human_approval"),
        (JobPhase::AdmissionPending,"grant_consumed_pending_os_admission"),
        (JobPhase::Running,"native_contained_worker_started"),
        (JobPhase::ReleasePending,"native_tree_empty_release_pending"),
        (JobPhase::Completed,"native_worker_completed"),
    ];
    if events.len()!=expect.len() {return Err(denied());}
    for (r,(phase,outcome)) in events.iter().zip(expect.iter()){
        if &r.body.phase!=phase || r.body.outcome_code!=*outcome
            || r.body.job_id!=prepared.job_id
            || r.body.plan_sha256!=prepared.plan_sha256
            || r.body.project_id!=plan.project_id
            || r.body.session_id!=events[0].body.session_id {
            return Err(denied());
        }
    }
    let replay=Supervisor::open(original_journal.clone())?;
    let jobs=replay.inspect()?;
    if jobs.len()!=1 || jobs[0].phase!=JobPhase::Completed
        || jobs[0].requires_reconciliation {
        return Err(denied());
    }
    if windows_fin04w2_c2_host::run_fixed_after_original_approval(
        &supervisor,&prepared.job_id,&plan,&service,&service_sha,
    ).is_ok(){
        return Err(denied());
    }
    let result=serde_json::json!({
        "schema":"lu.ide.fin04w2.c2.native.final.v1",
        "sourceHead":plan.source_head,
        "jobId":prepared.job_id,
        "planSha256":prepared.plan_sha256,
        "originalEventCount":events.len(),
        "originalPhases":expect.iter().map(|(p,_)|format!("{p:?}")).collect::<Vec<_>>(),
        "terminalEntrySha256":events.last().unwrap().entry_hash,
        "terminalOutcomeCode":events.last().unwrap().body.outcome_code,
        "restartReplay":"CompletedNoGrant",
        "secondUse":"Denied",
        "approvalKind":"CI_SIMULATED_NOT_PHYSICAL",
        "lldbExecution":"NOT_PERMITTED",
    });
    let mut out=OpenOptions::new().write(true).create_new(true)
        .open(root.join("result.json")).map_err(|_|denied())?;
    out.write_all(&serde_json::to_vec_pretty(&result)
        .map_err(|_|denied())?).map_err(|_|denied())?;
    out.sync_all().map_err(|_|denied())?;
    println!("FIN04W2_C_ORIGINAL_RECEIPT_CHAIN_AND_RESTART=PASS");
    println!("FIN04W2_C_ORIGINAL_SERVICE_ONE_USE_REPLAY_DENIED=PASS");
    println!("FIN04W2_C_ORIGINAL_NATIVE_TRANSACTION=CI_SIMULATED_APPROVAL_ONLY");
    Ok(())
}
