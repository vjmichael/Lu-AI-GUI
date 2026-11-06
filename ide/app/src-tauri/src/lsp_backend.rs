//! E2 native Windows-only Tauri host for reviewed installer-packaged clangd.
//! This module is compiled ONLY with explicit native-lsp feature. The default
//! IDE alpha exposes no language-server executable or Lu bridge.
//! An OS dialog (not a UI boolean) grants one fresh D3 job for an exact source
//! snapshot after release pins, registered project and disk revision checks.
#![cfg(all(windows, feature = "native-lsp"))]
use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
    thread,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest,Sha256};
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
use lu_ide_workspace::IdeWorkspaceRegistry;
use lu_ide_execution_supervisor::{
    JobPlan, Supervisor,
    snapshot::publish_lsp_input,
    windows_broker::TrustedBroker,
    windows_worker::lsp_native::{
        native_lsp_bundle_digest, start_windows_lsp, WindowsLspSession,
    },
};
use lu_ide_workspace::{
    lsp::{DiagnosticSource},
    lsp_session::{LspEvent, Position, Diagnostic, Completion, Definition},
};

// Pins are embedded at build time by the reviewed installer release job.
// Runtime environment variables, user selected paths, repo configs and
// language server replies cannot override these values.
const APPROVED_CLANGD_BUNDLE: Option<&str> =
    option_env!("LU_IDE_E2_CLANGD_BUNDLE_SHA256");
const APPROVED_SERVICE: Option<&str> =
    option_env!("LU_IDE_E2_D7D_SERVICE_SHA256");

fn pin(value: Option<&str>) -> Result<&str, String> {
    let text=value.ok_or("LSP_RELEASE_PINS_ABSENT")?;
    if text.len()!=64||!text.bytes().all(|c|c.is_ascii_hexdigit()){
        return Err("LSP_RELEASE_PINS_INVALID".into());
    }
    Ok(text)
}
fn hash_file(path:&Path)->Result<String,String>{
    let mut f=File::open(path).map_err(|_|"LSP_SERVICE_MISSING")?;
    let mut hasher=Sha256::new();
    let mut bytes=[0u8;64*1024];
    loop {
        let n=f.read(&mut bytes).map_err(|_|"LSP_SERVICE_UNREADABLE")?;
        if n==0 {break}
        hasher.update(&bytes[..n]);
    }
    Ok(format!("{:x}",hasher.finalize()))
}
#[derive(Clone)]
struct Installation {
    bundle: PathBuf,
    service: TrustedBroker,
    bundle_sha256: String,
}
fn installation(app:&AppHandle)->Result<Installation,String>{
    let root=app.path().resource_dir().map_err(|_|"LSP_RESOURCE_ROOT_UNAVAILABLE")?;
    let bundle=root.join("language-servers").join("clangd");
    let service=root.join("recovery").join("windows_recovery_service.exe");
    let expected_bundle=pin(APPROVED_CLANGD_BUNDLE)?;
    let expected_service=pin(APPROVED_SERVICE)?;
    let observed=native_lsp_bundle_digest(&bundle,"lsp_clangd")
        .map_err(|_|"LSP_UNTRUSTED_BUNDLE")?;
    if observed!=expected_bundle {return Err("LSP_BUNDLE_PIN_MISMATCH".into());}
    if hash_file(&service)?!=expected_service {
        return Err("LSP_SERVICE_PIN_MISMATCH".into());
    }
    let broker=TrustedBroker{
        executable:service,expected_sha256:expected_service.to_owned(),
        private_witness_directory:root.join("recovery"),
    };
    Ok(Installation{bundle,service:broker,bundle_sha256:observed})
}
pub fn installed_worker_ready(app:&AppHandle)->bool { installation(app).is_ok() }

#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct LspRequest {
    project_id:String,relative_path:String,expected_revision:String,
    buffer_revision:u64, language_id:String, text:String,
    kind:Option<String>,position:Option<Position>,
}
#[derive(Serialize,Clone)]
#[serde(rename_all="camelCase")]
pub struct SourceCoordinate {
    project_id:String,relative_path:String,disk_sha256:String,
    buffer_revision:u64,language_id:String,
}
#[derive(Serialize)]
#[serde(rename_all="camelCase")]
pub struct DefinitionCoordinate {
    relative_path:String,range:lu_ide_workspace::lsp_session::Span,
}
#[derive(Serialize)]
#[serde(rename_all="camelCase")]
pub struct LspReply {
    source:SourceCoordinate,
    diagnostics:Vec<Diagnostic>,completions:Vec<Completion>,
    hover:Option<String>,definition:Option<DefinitionCoordinate>,
}
struct Active {
    source:DiagnosticSource,
    project_id:String,
    relative_path:String,
    saved_revision:String,
    text:String,
    diagnostics:Vec<Diagnostic>,
    session:WindowsLspSession,
}
impl Active {
    fn output(&self)->LspReply {
        LspReply{
            source:SourceCoordinate{
                project_id:self.project_id.clone(),
                relative_path:self.relative_path.clone(),
                disk_sha256:self.saved_revision.clone(),
                buffer_revision:self.source.buffer_revision,
                language_id:self.source.language_id.clone(),
            },diagnostics:self.diagnostics.clone(),
            completions:vec![],hover:None,definition:None,
        }
    }
    fn receive(&mut self, kind:&str)->Result<LspReply,String>{
        let start=Instant::now();
        let mut result=self.output();
        let read_result=|event:&LspEvent,kind:&str|match (event,kind){
            (LspEvent::Completions(_),"completion") => true,
            (LspEvent::Hover(_),"hover") => true,
            (LspEvent::Definition(_),"definition") => true,
            _ => false,
        };
        while start.elapsed()<Duration::from_secs(if kind=="diagnostics" {1} else {8}){
            for event in self.session.poll().map_err(|e|e.code())? {
                match &event {
                    LspEvent::Diagnostics(value)=>{
                        self.diagnostics=value.clone();
                        result.diagnostics=value.clone();
                    },
                    LspEvent::Completions(value) if kind=="completion" =>
                        result.completions=value.clone(),
                    LspEvent::Hover(value) if kind=="hover" =>
                        result.hover=Some(value.clone()),
                    LspEvent::Definition(Some(value)) if kind=="definition" =>{
                        result.definition=Some(DefinitionCoordinate{
                            relative_path:self.relative_path.clone(),
                            range:value.range.clone(),
                        });
                    },
                    _=>{},
                }
                if read_result(&event,kind){return Ok(result);}
            }
            thread::sleep(Duration::from_millis(30));
        }
        if kind=="diagnostics" {Ok(result)} else {Err("LSP_REQUEST_TIMEOUT".into())}
    }
}
pub struct NativeLspRuntime {
    supervisor:Arc<Supervisor>,
    stage:PathBuf,
    active:Mutex<Option<Active>>,
}
impl NativeLspRuntime {
    pub fn new(app:&AppHandle)->Result<Self,String>{
        let root=app.path().app_data_dir().map_err(|_|"LSP_HOST_PROFILE_UNAVAILABLE")?;
        fs::create_dir_all(&root).map_err(|_|"LSP_PROFILE_UNWRITABLE")?;
        let stage=root.join("lsp-staged-inputs");
        fs::create_dir_all(&stage).map_err(|_|"LSP_PROFILE_UNWRITABLE")?;
        let supervisor=Supervisor::open(root.join("lsp-job-receipts.jsonl"))
            .map_err(|e|e.code())?;
        Ok(Self{supervisor:Arc::new(supervisor),stage,
            active:Mutex::new(None)})
    }
    fn validated_registered_text(
        registry:&IdeWorkspaceRegistry,request:&LspRequest,
    )->Result<(PathBuf,String),String>{
        if request.language_id!="c"||!request.relative_path.to_ascii_lowercase().ends_with(".c")
            ||request.text.len()>512*1024||request.text.contains('\0')
            ||request.expected_revision.len()!=64 {
            return Err("LSP_INVALID_ACTIVE_SOURCE".into())
        }
        let root=registry.trusted_native_worker_root(&request.project_id)
            .map_err(|_|"LSP_PROJECT_NOT_ACTIVE")?;
        let saved=registry.preview_source_file(&request.project_id,&request.relative_path)
            .map_err(|_|"LSP_SOURCE_UNAVAILABLE")?;
        if saved.revision!=request.expected_revision{
            return Err("LSP_FILE_CHANGED_ON_DISK".into());
        }
        Ok((root,saved.revision))
    }
    pub fn start(
        &self,app:&AppHandle,registry:&IdeWorkspaceRegistry,request:LspRequest,
    )->Result<Value,String>{
        if self.active.lock().map_err(|_|"LSP_HOST_UNAVAILABLE")?.is_some() {
            return Err("LSP_SESSION_ALREADY_ACTIVE".into())
        }
        let install=installation(app)?;
        let (root,saved_revision)=Self::validated_registered_text(registry,&request)?;
        // This is a genuine native OS confirmation, not a client boolean
        // or model-generated consent. It runs on Tauri's background thread.
        let choice=app.dialog()
            .message(format!("Allow the installed clangd language server to inspect the current file and the staged copy of this project?\n\nProject: {}\nFile: {}\n\nThe worker has no network privileges. It cannot edit source files.",
                request.project_id,request.relative_path))
            .title("Allow language analysis")
            .buttons(MessageDialogButtons::OkCancelCustom(
                "Allow this session".into(),"Cancel".into(),
            )).blocking_show();
        if !choice {return Err("LSP_HUMAN_APPROVAL_DENIED".into());}
        // Recheck after the human dialog: no stale source or replacement
        // can inherit approval to a different on-disk project.
        let (root_now,rev_now)=Self::validated_registered_text(registry,&request)?;
        if root_now!=root||rev_now!=saved_revision {
            return Err("LSP_SCOPE_CHANGED_DURING_APPROVAL".into());
        }
        let input=publish_lsp_input(&root,&self.stage,&request.relative_path,"clangd")
            .map_err(|e|e.code())?;
        if input.seal.manifest_sha256!=saved_revision{
            return Err("LSP_SNAPSHOT_CHANGED_DURING_ADMISSION".into())
        }
        let observed=registry.inspect_git_worktree(&request.project_id).local_head
            .filter(|sha|sha.len()==40&&sha.bytes().all(|b|b.is_ascii_hexdigit()))
            .unwrap_or_else(||"0".repeat(40));
        let plan=JobPlan{
            project_id:request.project_id.clone(),recipe_id:"lsp_clangd".into(),
            source_head:observed.clone(),
            input_snapshot_sha256:input.seal.source_sha256.clone(),
            manifest_sha256:input.seal.manifest_sha256.clone(),
            toolchain_sha256:install.bundle_sha256,
            runtime_limit_ms:5*60_000,
            memory_limit_bytes:512*1024*1024,process_limit:4,
            log_limit_bytes:64*1024,
            effect_class:"language_analysis_isolated".into(),
        };
        let prepared=self.supervisor.prepare(plan.clone()).map_err(|e|e.code())?;
        self.supervisor.record_human_approval(
            &prepared.job_id,&prepared.plan_sha256,&observed,true,
        ).map_err(|e|e.code())?;
        let mut session=start_windows_lsp(
            Arc::clone(&self.supervisor),&prepared.job_id,&plan,&observed,
            &input,&install.bundle,&install.service,
        ).map_err(|e|e.code())?;
        session.initialize().map_err(|e|e.code())?;
        let started=Instant::now();
        while !session.is_ready() && started.elapsed()<Duration::from_secs(12) {
            session.poll().map_err(|e|e.code())?;
            thread::sleep(Duration::from_millis(40));
        }
        if !session.is_ready(){return Err("LSP_INITIALIZE_TIMEOUT".into());}
        let native_uri=session.native_document_uri().to_owned();
        let file_id=format!("file_{:x}",Sha256::digest(request.relative_path.as_bytes()));
        let source=DiagnosticSource{
            project_id:request.project_id.clone(),
            file_id:file_id[..37].to_owned(),disk_sha256:saved_revision.clone(),
            buffer_revision:request.buffer_revision,language_id:"c".into(),
        };
        session.open(source.clone(),&native_uri,&request.text)
            .map_err(|e|e.code())?;
        let active=Active{
            project_id:request.project_id,
            relative_path:request.relative_path,
            source,saved_revision,text:request.text,
            diagnostics:vec![],session,
        };
        let result=serde_json::to_value(active.output())
            .map_err(|_|"LSP_RESPONSE_UNAVAILABLE")?;
        let mut holder=self.active.lock().map_err(|_|"LSP_HOST_UNAVAILABLE")?;
        if holder.is_some(){return Err("LSP_SESSION_ALREADY_ACTIVE".into())}
        *holder=Some(active);
        Ok(result)
    }
    fn use_active<F>(&self, registry:&IdeWorkspaceRegistry,request:LspRequest,mut action:F)
        ->Result<Value,String>
    where F:FnMut(&mut Active,&LspRequest)->Result<LspReply,String>{
        let mut lock=self.active.lock().map_err(|_|"LSP_HOST_UNAVAILABLE")?;
        let active=lock.as_mut().ok_or("LSP_SESSION_NOT_STARTED")?;
        if active.project_id!=request.project_id
            ||active.relative_path!=request.relative_path
            ||active.saved_revision!=request.expected_revision
            ||request.buffer_revision<active.source.buffer_revision {
            return Err("LSP_SOURCE_SCOPE_MISMATCH".into());
        }
        Self::validated_registered_text(registry,&request)?;
        if request.buffer_revision>active.source.buffer_revision {
            let mut next=active.source.clone();
            next.buffer_revision=request.buffer_revision;
            active.session.changed(&active.source,next.clone(),&request.text)
                .map_err(|e|e.code())?;
            active.source=next;
            active.text=request.text.clone();
            active.diagnostics.clear();
        } else if active.text!=request.text{
            return Err("LSP_BUFFER_REVISION_MISMATCH".into());
        }
        let result=action(active,&request)?;
        serde_json::to_value(result).map_err(|_|"LSP_RESPONSE_UNAVAILABLE".into())
    }
    pub fn feature(&self, registry:&IdeWorkspaceRegistry,request:LspRequest)
        ->Result<Value,String>{
        self.use_active(registry,request,|active,req|{
            let kind=req.kind.as_deref().ok_or("LSP_METHOD_REQUIRED")?;
            if !matches!(kind,"completion"|"hover"|"definition"){
                return Err("LSP_METHOD_DENIED".into());
            }
            let pos=req.position.clone().ok_or("LSP_POSITION_REQUIRED")?;
            active.session.query(kind,&active.source,pos).map_err(|e|e.code())?;
            active.receive(kind)
        })
    }
    pub fn diagnostics(&self, registry:&IdeWorkspaceRegistry,request:LspRequest)
        ->Result<Value,String>{
        self.use_active(registry,request,|active,_|active.receive("diagnostics"))
    }
    pub fn change(&self,registry:&IdeWorkspaceRegistry,request:LspRequest)
        ->Result<Value,String>{
        self.use_active(registry,request,|active,_|Ok(active.output()))
    }
    pub fn stop(&self)->Result<(),String>{
        let active=self.active.lock().map_err(|_|"LSP_HOST_UNAVAILABLE")?.take();
        if let Some(active)=active{
            active.session.close().map_err(|e|e.code())?;
        }
        Ok(())
    }
}
