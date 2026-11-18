//! P0-TOOLS-04: two real, *read-only* host-reviewed specialist applications.
//!
//! PDFInfo reads PDF page/version metadata; ExifTool reads PNG/JPEG image
//! dimensions/type. The original human chooses the file through an OS picker.
//! No model paths, extra argv, script switches, export, writes or networking.
//! Native process custody is the EXISTING BackgroundProcessSupervisor and
//! EffectJournal. An independent original-byte oracle runs in the separate
//! reviewed_tool layer before any result is marked VERIFIED for the model.
use std::{collections::BTreeMap,ffi::OsString,path::{Path,PathBuf},
    sync::{Arc,Mutex}};
use serde_json::{json,Value};
use crate::{
    contracts::{ToolAuthorizer,ToolDescriptor,ToolEffect,ToolInvocation},
    conversation_authority::{BoundCallOrigin,ConversationAuthority},
    journal::{EffectJournal,EffectState,PrepareOutcome},
    native_app_admission::{HostReviewedExecutable,inspect_host_reviewed_code_application},
    native_background_process::{
        BackgroundProcessSupervisor,TrustedWorkerPlan,verified_media_input,
    },
    tasks::{TaskStore,TaskState},
};
pub const REQUEST_TYPE:&str="type.googleapis.com/lu.p0.SpecialistReadRequest";
pub const RESULT_TYPE:&str="type.googleapis.com/lu.p0.SpecialistReadReceipt";
pub const READ_TYPE:&str="type.googleapis.com/lu.p0.SpecialistSourceRead";
pub const EXEC_BYTES:&[u8]=b"lu.p0.reviewed.specialist.fixed.read.v1";
pub const READ_BYTES:&[u8]=b"lu.p0.reviewed.specialist.source.read.v1";

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum SpecialistKind{PdfInfo,ImageExif}
impl SpecialistKind{
    pub fn name(self)->&'static str{
        match self{Self::PdfInfo=>"pdfinfo",Self::ImageExif=>"exiftool"}
    }
    pub fn format(self)->&'static str{
        match self{Self::PdfInfo=>"pdf",Self::ImageExif=>"image"}
    }
    pub fn extension_ok(self,path:&Path)->bool{
        let ext=path.extension().and_then(|x|x.to_str())
            .unwrap_or("").to_ascii_lowercase();
        match self{
            Self::PdfInfo=>ext=="pdf",
            Self::ImageExif=>matches!(ext.as_str(),"png"|"jpg"|"jpeg"),
        }
    }
    pub fn command(self,path:&Path)->Vec<OsString>{
        let args:&[&str]=match self{
            Self::PdfInfo=>&["-isodates"],
            Self::ImageExif=>&["-j","-n","-ImageWidth","-ImageHeight",
                "-FileType","-FileSize#","--"],
        };
        args.iter().map(|arg|OsString::from(*arg))
            .chain(std::iter::once(path.as_os_str().to_os_string()))
            .collect()
    }
    pub fn schema_kind(self)->&'static str{
        match self{Self::PdfInfo=>"pdf_info",Self::ImageExif=>"image_info"}
    }
}
#[derive(Clone)]
pub struct HostReviewedSpecialistProbe{
    pub profile_id:String,
    pub executable:HostReviewedExecutable,
    pub approved_input:PathBuf,
    pub input_sha256:String,
    pub kind:SpecialistKind,
    pub execute_tool_id:String,
    pub read_tool_id:String,
    pub stdout_limit:usize,
    pub timeout_seconds:u64,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum SpecialistError{
    InvalidProfile,Duplicate,Unavailable,WrongOperation,
    Unauthorized,AlreadyRunning,EffectExists,Journal,
    Spawn,Poll,Lock,CancelUncertain,
}
#[derive(Default)]
pub struct HostSpecialistCatalog{data:BTreeMap<String,HostReviewedSpecialistProbe>}
fn valid_id(s:&str)->bool{
    !s.is_empty()&&s.len()<=128&&s.bytes().all(|b|
        b.is_ascii_alphanumeric()||matches!(b,b'.'|b'-'|b'_'))
}
fn valid_sha(s:&str)->bool{
    s.len()==64&&s.bytes().all(|b|
        b.is_ascii_digit()||(b'a'..=b'f').contains(&b))
}
impl HostSpecialistCatalog{
    pub fn new()->Self{Self::default()}
    pub fn register_host_reviewed(&mut self,entry:HostReviewedSpecialistProbe)
        ->Result<(),SpecialistError>{
        let binary=entry.executable.executable.file_name()
            .and_then(|x|x.to_str()).unwrap_or("").to_ascii_lowercase();
        let accepted_binary=match entry.kind{
            SpecialistKind::PdfInfo=>matches!(binary.as_str(),"pdfinfo"|"pdfinfo.exe"),
            SpecialistKind::ImageExif=>matches!(binary.as_str(),"exiftool"|"exiftool.exe"),
        };
        if !valid_id(&entry.profile_id)||
            !valid_id(&entry.executable.app_id)||
            !valid_id(&entry.executable.workspace_id)||
            !valid_id(&entry.execute_tool_id)||
            !valid_id(&entry.read_tool_id)||
            entry.read_tool_id==entry.execute_tool_id||
            !entry.execute_tool_id.starts_with("lu.reviewed.")||
            !valid_sha(&entry.executable.expected_sha256)||
            !valid_sha(&entry.input_sha256)||
            !accepted_binary||
            !entry.executable.executable.is_absolute()||
            !entry.approved_input.is_absolute()||
            !entry.kind.extension_ok(&entry.approved_input)||
            !(1..=8192).contains(&entry.stdout_limit)||
            !(1..=30).contains(&entry.timeout_seconds)||
            !verified_media_input(&entry.approved_input,&entry.input_sha256)
                .is_ok_and(|n|n<=32*1024*1024){
            return Err(SpecialistError::InvalidProfile)
        }
        if self.data.contains_key(&entry.profile_id){
            return Err(SpecialistError::Duplicate)
        }
        if self.data.len()>=16{return Err(SpecialistError::InvalidProfile)}
        self.data.insert(entry.profile_id.clone(),entry);
        Ok(())
    }
    fn reviewed(&self,id:&str)->Option<HostReviewedSpecialistProbe>{
        self.data.get(id).cloned()
    }
}
fn parse_pdfinfo(bytes:&[u8])->Option<Value>{
    let text=std::str::from_utf8(bytes).ok()?;
    if text.len()>8192||text.contains('\0'){return None}
    let mut pages=None;let mut version=None;
    for line in text.lines(){
        if let Some(value)=line.strip_prefix("Pages:"){
            if pages.is_some(){return None}
            let raw=value.trim();
            if raw.is_empty()||raw.len()>5||
                !raw.bytes().all(|b|b.is_ascii_digit()){return None}
            let n=raw.parse::<u64>().ok()?;
            if !(1..=4096).contains(&n){return None}
            pages=Some(n);
        }else if let Some(value)=line.strip_prefix("PDF version:"){
            if version.is_some(){return None}
            let raw=value.trim();
            if !matches!(raw,"1.0"|"1.1"|"1.2"|"1.3"|"1.4"|
                "1.5"|"1.6"|"1.7"|"2.0"){return None}
            version=Some(raw.to_owned());
        }
    }
    Some(json!({
        "kind":"pdf_info","pages":pages?,"pdfVersion":version?
    }))
}
fn parse_exif(bytes:&[u8])->Option<Value>{
    if bytes.len()>8192{return None}
    let raw:Value=serde_json::from_slice(bytes).ok()?;
    let root=raw.as_array()?;
    if root.len()!=1{return None}
    let data=root[0].as_object()?;
    if data.len()>7||data.keys().any(|key|!matches!(key.as_str(),
        "SourceFile"|"FileType"|"ImageWidth"|"ImageHeight"|"FileSize"|"Warning"|"Error")){
        return None
    }
    if data.contains_key("Warning")||data.contains_key("Error"){return None}
    let format=data.get("FileType")?.as_str()?;
    if !matches!(format,"PNG"|"JPEG"){return None}
    let width=data.get("ImageWidth")?.as_u64()?;
    let height=data.get("ImageHeight")?.as_u64()?;
    if width==0||width>16384||height==0||height>16384||
       width.checked_mul(height)?>32_000_000{
        return None
    }
    Some(json!({
        "kind":"image_info","imageType":format,"width":width,"height":height
    }))
}
pub fn parse_specialist_stdout(kind:SpecialistKind,bytes:&[u8])->Option<Value>{
    match kind{
        SpecialistKind::PdfInfo=>parse_pdfinfo(bytes),
        SpecialistKind::ImageExif=>parse_exif(bytes),
    }
}
#[derive(Clone)]
struct Active{
    call:String,workspace:String,origin:BoundCallOrigin,
    input:PathBuf,sha:String,kind:SpecialistKind,max_stdout:usize,
    revoked:bool,
}
#[derive(Default)]
struct Gate{running:BTreeMap<String,Active>}
pub struct TypedSpecialistBroker{
    authority:ConversationAuthority,
    supervisor:Arc<BackgroundProcessSupervisor>,
    tasks:Arc<TaskStore>,journal:Arc<EffectJournal>,
    gate:Arc<Mutex<Gate>>,
}
impl TypedSpecialistBroker{
    pub fn new(authority:ConversationAuthority,
        supervisor:Arc<BackgroundProcessSupervisor>,
        tasks:Arc<TaskStore>,journal:Arc<EffectJournal>
    )->Result<Self,SpecialistError>{
        let gate=Arc::new(Mutex::new(Gate::default()));
        let watch=Arc::clone(&gate);
        let workers=Arc::clone(&supervisor);
        let effects=Arc::clone(&journal);
        authority.on_task_revoked(Arc::new(move|task_id|{
            if let Ok(mut lock)=watch.lock(){
                if let Some(active)=lock.running.get_mut(task_id){
                    active.revoked=true;
                    let _=effects.mark_uncertain(&active.call);
                    let call=active.call.clone();let workspace=active.workspace.clone();
                    if workers.cancel_owned(&call,&workspace).is_ok(){
                        lock.running.remove(task_id);
                    }
                }
            }
        })).map_err(|_|SpecialistError::Lock)?;
        Ok(Self{authority,supervisor,tasks,journal,gate})
    }
    #[allow(clippy::too_many_arguments)]
    pub fn launch(&self,catalog:&HostSpecialistCatalog,profile:&str,
        origin:&BoundCallOrigin,execute:&ToolInvocation,
        descriptor:&ToolDescriptor,read:&ToolInvocation,
        read_desc:&ToolDescriptor,policy:&dyn ToolAuthorizer
    )->Result<(),SpecialistError>{
        let mut gate=self.gate.lock().map_err(|_|SpecialistError::Lock)?;
        if gate.running.contains_key(&origin.task_id){
            return Err(SpecialistError::AlreadyRunning)
        }
        let approved=catalog.reviewed(profile).ok_or(SpecialistError::Unavailable)?;
        let scope=approved.executable.workspace_id.as_str();
        if execute.tool_id!=approved.execute_tool_id||
            descriptor.id!=approved.execute_tool_id||
            descriptor.effect!=ToolEffect::ExecuteCode||
            descriptor.input_type_url!=REQUEST_TYPE||
            descriptor.output_type_url!=RESULT_TYPE||
            execute.input.type_url!=REQUEST_TYPE||
            execute.input.bytes.as_slice()!=EXEC_BYTES||
            execute.workspace_id.as_deref()!=Some(scope)||
            read.tool_id!=approved.read_tool_id||
            read_desc.id!=approved.read_tool_id||
            read_desc.effect!=ToolEffect::ReadWorkspace||
            read_desc.input_type_url!=READ_TYPE||
            read.input.type_url!=READ_TYPE||
            read.input.bytes.as_slice()!=READ_BYTES||
            read.workspace_id.as_deref()!=Some(scope)||
            !valid_id(&execute.call_id)||!valid_id(&read.call_id)||
            read.call_id==execute.call_id{
            return Err(SpecialistError::WrongOperation)
        }
        self.authority.authorize_worker_call(
            origin,read,read_desc,policy
        ).map_err(|_|SpecialistError::Unauthorized)?;
        let mut reviewed=approved.executable.clone();
        reviewed.tool_id=approved.execute_tool_id.clone();
        inspect_host_reviewed_code_application(
            &reviewed,origin,execute,descriptor,&self.authority,policy
        ).map_err(|_|SpecialistError::Unauthorized)?;
        if !verified_media_input(&approved.approved_input,&approved.input_sha256)
            .is_ok_and(|n|n<=32*1024*1024){
            return Err(SpecialistError::Unauthorized)
        }
        let plan=TrustedWorkerPlan{
            task_id:execute.call_id.clone(),workspace_id:scope.to_owned(),
            executable:approved.executable.executable.clone(),
            expected_sha256:approved.executable.expected_sha256.clone(),
            args:approved.kind.command(&approved.approved_input),
            progress_protocol:false,
            max_runtime_seconds:Some(approved.timeout_seconds),
        };
        // P0-AUTH: source+executable preflight and BOTH independent native
        // capabilities are followed by one atomic host launch fence.
        // Revocation cannot run its OS kill callback before the child is
        // registered. Nothing inside the fence waits for process completion.
        self.authority.with_authorized_worker_launch(
            origin,read,read_desc,execute,descriptor,policy,||{
                match self.journal.prepare(execute)
                    .map_err(|_|SpecialistError::Journal)?{
                    PrepareOutcome::New=>(),
                    PrepareOutcome::Existing(_)=>return Err(SpecialistError::EffectExists),
                }
                self.journal.mark_executing(&execute.call_id)
                    .map_err(|_|SpecialistError::Journal)?;
                if self.supervisor.start_host_approved_specialist_probe(
                    plan,approved.stdout_limit,&approved.input_sha256,
                    approved.kind.name()
                ).is_err(){
                    let _=self.journal.mark_uncertain(&execute.call_id);
                    return Err(SpecialistError::Spawn)
                }
                gate.running.insert(origin.task_id.clone(),Active{
                    call:execute.call_id.clone(),workspace:scope.into(),
                    origin:origin.clone(),input:approved.approved_input,
                    sha:approved.input_sha256,kind:approved.kind,
                    max_stdout:approved.stdout_limit,revoked:false
                });
                Ok(())
            }
        ).map_err(|_|SpecialistError::Unauthorized)?
    }
    pub fn poll(&self)->Result<usize,SpecialistError>{
        let mut gate=self.gate.lock().map_err(|_|SpecialistError::Lock)?;
        let finished=self.supervisor.poll().map_err(|_|SpecialistError::Poll)?;
        let mut done=Vec::new();
        for (task,active) in &gate.running{
            let state=self.tasks.get(&active.call).map_err(|_|SpecialistError::Poll)?
                .ok_or(SpecialistError::Poll)?.state;
            if matches!(state,TaskState::Pending|TaskState::Running|TaskState::Paused){
                continue
            }
            if active.revoked{
                self.journal.mark_uncertain(&active.call)
                    .map_err(|_|SpecialistError::Journal)?;
                done.push(task.clone());continue
            }
            if state==TaskState::Completed{
                let original=self.supervisor.take_local_version_stdout(
                    &active.call,&active.workspace
                ).map_err(|_|SpecialistError::Poll)?;
                if let Some(output)=original{
                    let same=verified_media_input(&active.input,&active.sha)
                        .is_ok_and(|n|n<=32*1024*1024);
                    if same && output.bytes.len()<=active.max_stdout{
                        if let Some(facts)=parse_specialist_stdout(
                            active.kind,&output.bytes
                        ){
                            let mut receipt=facts.as_object()
                                .ok_or(SpecialistError::Journal)?.clone();
                            receipt.insert("schema".into(),json!("lu-p0-tools04-specialist-v1"));
                            receipt.insert("callId".into(),json!(active.call));
                            receipt.insert("sourceSha256".into(),json!(active.sha));
                            receipt.insert("stdoutSha256".into(),json!(output.sha256));
                            receipt.insert("osChildReaped".into(),json!(true));
                            receipt.insert("sourceDigestReverified".into(),json!(true));
                            receipt.insert("rawSourcePathPersisted".into(),json!(false));
                            receipt.insert("rawOutputPersisted".into(),json!(false));
                            receipt.insert("luCoreSandboxed".into(),json!(false));
                            let bytes=serde_json::to_vec(&Value::Object(receipt))
                                .map_err(|_|SpecialistError::Journal)?;
                            self.journal.mark_applied(&active.call,&bytes)
                                .map_err(|_|SpecialistError::Journal)?;
                            done.push(task.clone());continue
                        }
                    }
                }
            }
            self.journal.mark_uncertain(&active.call)
                .map_err(|_|SpecialistError::Journal)?;
            done.push(task.clone());
        }
        for task in done{gate.running.remove(&task);}
        Ok(finished)
    }
    pub fn cancel_current_task(&self,origin:&BoundCallOrigin)
        ->Result<(),SpecialistError>{
        let mut gate=self.gate.lock().map_err(|_|SpecialistError::Lock)?;
        let active=gate.running.get(&origin.task_id)
            .ok_or(SpecialistError::Unauthorized)?;
        if &active.origin!=origin{return Err(SpecialistError::Unauthorized)}
        let active=active.clone();
        if let Some(v)=gate.running.get_mut(&origin.task_id){v.revoked=true}
        let _=self.journal.mark_uncertain(&active.call);
        if self.supervisor.cancel_owned(&active.call,&active.workspace).is_err(){
            return Err(SpecialistError::CancelUncertain)
        }
        gate.running.remove(&origin.task_id);
        Ok(())
    }
    pub fn active_count(&self)->Result<usize,SpecialistError>{
        Ok(self.gate.lock().map_err(|_|SpecialistError::Lock)?
            .running.len())
    }
}
impl Drop for TypedSpecialistBroker{
    fn drop(&mut self){
        if let Ok(mut gate)=self.gate.lock(){
            for (_,active) in std::mem::take(&mut gate.running){
                let _=self.journal.mark_uncertain(&active.call);
                let _=self.supervisor.cancel_owned(&active.call,&active.workspace);
            }
        }
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn only_bounded_specialist_output_admitted(){
        let pdf=b"Pages: 2\nPDF version: 1.7\nTitle: untrusted\n";
        assert_eq!(parse_specialist_stdout(SpecialistKind::PdfInfo,pdf)
            .unwrap()["pages"],2);
        assert!(parse_specialist_stdout(SpecialistKind::PdfInfo,
            b"Pages: 2\nPages: 3\nPDF version: 1.7").is_none());
        let image=br#"[{"SourceFile":"not-a-grant",
            "FileType":"PNG","ImageWidth":800,"ImageHeight":600}]"#;
        assert_eq!(parse_specialist_stdout(SpecialistKind::ImageExif,image)
            .unwrap()["width"],800);
        assert!(parse_specialist_stdout(SpecialistKind::ImageExif,
            br#"[{"FileType":"PNG","ImageWidth":800,"ImageHeight":600,
                "Warning":"unsafe"}]"#).is_none());
    }
    #[test]
    fn model_has_no_cli_arguments_or_source_path(){
        assert_eq!(SpecialistKind::PdfInfo.command(Path::new("/test.pdf")).len(),2);
        assert_eq!(SpecialistKind::ImageExif.command(Path::new("/test.png")).len(),8);
        assert_eq!(EXEC_BYTES,b"lu.p0.reviewed.specialist.fixed.read.v1");
    }
}
