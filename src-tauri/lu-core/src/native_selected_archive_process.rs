//! Selected-member worker. Only a host-created plan can register a callable
//! tool; the model cannot choose a path, program, worker or digest.
use std::{ffi::OsString,fs,io::{Read,Write},path::{Path,PathBuf},sync::Arc,thread,time::Duration};
use cap_std::{ambient_authority,fs::Dir};
use prost::Message;
use serde::{Serialize,Deserialize};
use serde_json::json;
use sha2::{Digest,Sha256};
use crate::{contracts::{ModuleManifest,ModuleKind,NativeTool,ToolDescriptor,ToolEffect,ToolFailure,ToolFuture,ToolInvocation,ToolOutput,ProtoPayload,CORE_CONTRACT_VERSION},model::{ModelToolCatalog,ModelToolDefinition,ModelToolCodec},registry::NativeToolRegistry,native_archive_process::opened_directory_identity,native_archive_tree::ArchiveCancellation,native_background_process::{BackgroundProcessSupervisor,TrustedWorkerPlan},native_file_transfer::{source_digest,MAX_TRANSFER_BYTES},tasks::{TaskState,TaskStore},wire::proto,native_zip_extract_tool as zip,native_iso_extract_tool as iso,native_linux_archive_tool as linux};
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum Kind{Zip,Iso,Linux}
impl Kind{
 pub fn for_name(name:&str)->Option<Self>{let n=name.to_ascii_lowercase();if n.ends_with(".zip"){Some(Self::Zip)}else if n.ends_with(".iso"){Some(Self::Iso)}else if [".tar",".tar.gz",".tgz",".gz"].iter().any(|x|n.ends_with(x)){Some(Self::Linux)}else{None}}
 pub fn id(self)->&'static str{match self{Self::Zip=>zip::TOOL_ID,Self::Iso=>iso::TOOL_ID,Self::Linux=>linux::EXTRACT_ID}}
 pub fn input(self)->&'static str{match self{Self::Zip=>zip::INPUT,Self::Iso=>iso::INPUT,Self::Linux=>linux::EXTRACT_INPUT}}
 pub fn output(self)->&'static str{match self{Self::Zip=>zip::OUTPUT,Self::Iso=>iso::OUTPUT,Self::Linux=>linux::OUTPUT}}
 fn model(self)->&'static str{match self{Self::Zip=>zip::MODEL_NAME,Self::Iso=>iso::MODEL_NAME,Self::Linux=>linux::EXTRACT_MODEL}}
 pub fn payload(self,name:&str,member:&str,target:&str,w:&Witness)->(ProtoPayload,ProtoPayload){
  let (input,output)=match self{
   Self::Zip=>(proto::ZipExtractRequest{archive_name:name.into(),member_path:member.into(),target_name:target.into(),expected_sha256:w.member_sha256.clone()}.encode_to_vec(),proto::ZipExtractResult{target_name:target.into(),member_path:member.into(),bytes_written:w.member_bytes,sha256:w.member_sha256.clone()}.encode_to_vec()),
   Self::Iso=>(proto::IsoExtractRequest{source_name:name.into(),member_path:member.into(),target_name:target.into(),expected_sha256:w.member_sha256.clone()}.encode_to_vec(),proto::IsoExtractResult{target_name:target.into(),bytes_written:w.member_bytes,sha256:w.member_sha256.clone()}.encode_to_vec()),
   Self::Linux=>(proto::LinuxArchiveExtractRequest{archive_name:name.into(),member_path:member.into(),target_name:target.into(),expected_sha256:w.member_sha256.clone()}.encode_to_vec(),proto::LinuxArchiveWriteResult{target_name:target.into(),bytes_written:w.member_bytes,sha256:w.member_sha256.clone(),member_count:1}.encode_to_vec())};
  (ProtoPayload{type_url:self.input().into(),bytes:input},ProtoPayload{type_url:self.output().into(),bytes:output})
 }
}
pub fn leaf(s:&str)->bool{!s.is_empty()&&s.len()<=128&&!s.starts_with('.')&&!s.starts_with('-')&&!s.ends_with('.')&&!s.contains("..")&&s.bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'.'|b'-'|b'_'))}
pub fn member_ok(s:&str)->bool{!s.is_empty()&&s.len()<=512&&s.split('/').all(leaf)}
fn stage_ok(s:&str)->bool{(16..=116).contains(&s.len())&&s.bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'-'|b'_'))}
fn digest_ok(s:&str)->bool{s.len()==64&&s.bytes().all(|b|b.is_ascii_hexdigit()&&!b.is_ascii_uppercase())}
fn absent(dir:&Dir,name:&str)->bool{matches!(dir.symlink_metadata(name),Err(e) if e.kind()==std::io::ErrorKind::NotFound)}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Witness{pub source_bytes:u64,pub source_sha256:String,pub member_bytes:u64,pub member_sha256:String,pub source:String,pub member:String}
impl Witness{fn valid(&self,name:&str,member:&str)->bool{self.source==name&&self.member==member&&self.source_bytes<=MAX_TRANSFER_BYTES&&self.member_bytes<=MAX_TRANSFER_BYTES&&digest_ok(&self.source_sha256)&&digest_ok(&self.member_sha256)}}
fn checked_preview(root:&Dir,name:&str,member:&str)->Result<Witness,()>{
 if !leaf(name)||!member_ok(member){return Err(())}let kind=Kind::for_name(name).ok_or(())?;let (n,h)=source_digest(root,name).map_err(|_|())?;
 let (size,sha)=match kind{Kind::Zip=>crate::native_zip_extract::zip_member_preview(root,name,member).map_err(|_|())?,Kind::Iso=>crate::native_iso9660::iso_member_preview(root,name,member).map_err(|_|())?,Kind::Linux=>crate::native_linux_archive::linux_archive_member_preview(root,name,member).map_err(|_|())?};
 let w=Witness{source_bytes:n,source_sha256:h,member_bytes:size,member_sha256:sha,source:name.into(),member:member.into()};if !w.valid(name,member)||!source_matches(root,&w){return Err(())}Ok(w)
}
fn source_matches(root:&Dir,w:&Witness)->bool{source_digest(root,&w.source).ok()==Some((w.source_bytes,w.source_sha256.clone()))}
fn output_matches(root:&Dir,target:&str,w:&Witness)->bool{source_digest(root,target).ok()==Some((w.member_bytes,w.member_sha256.clone()))}
fn hash_exe(p:&Path)->Result<String,String>{let mut f=fs::File::open(p).map_err(|_|"Worker unavailable")?;let mut h=Sha256::new();let mut buf=[0;65536];loop{let n=f.read(&mut buf).map_err(|_|"Worker unreadable")?;if n==0{break}h.update(&buf[..n])}Ok(format!("{:x}",h.finalize()))}
/// Private scratch only for the bounded preview receipt; no extraction into
/// the destination may occur before a separately approved typed invocation.
struct Scratch(PathBuf);
impl Drop for Scratch{fn drop(&mut self){let _=fs::remove_dir_all(&self.0);}}
#[derive(Clone)]
pub struct WorkerPlan{processes:Arc<BackgroundProcessSupervisor>,tasks:Arc<TaskStore>,executable:PathBuf,exe_sha:String,source_parent:PathBuf,dest_parent:PathBuf,source_id:String,dest_id:String,task_id:String,owner:String,cancel:ArchiveCancellation}
impl WorkerPlan{
 pub fn new(processes:Arc<BackgroundProcessSupervisor>,tasks:Arc<TaskStore>,executable:PathBuf,source_parent:PathBuf,dest_parent:PathBuf,source:&Dir,dest:&Dir,task_id:&str,owner:&str,cancel:ArchiveCancellation)->Result<Self,String>{
  if !stage_ok(task_id)||!source_parent.is_absolute()||!dest_parent.is_absolute(){return Err("Invalid selected archive plan".into())}let executable=executable.canonicalize().map_err(|_|"Worker unavailable")?;let exe_sha=hash_exe(&executable)?;
  Ok(Self{processes,tasks,executable,exe_sha,source_parent,dest_parent,source_id:opened_directory_identity(source).map_err(|_|"Source identity unavailable")?,dest_id:opened_directory_identity(dest).map_err(|_|"Destination identity unavailable")?,task_id:task_id.into(),owner:owner.into(),cancel})
 }
 fn check(&self,source:&Dir,dest:&Dir)->bool{!self.cancel.is_cancelled()&&opened_directory_identity(source).ok().as_ref()==Some(&self.source_id)&&opened_directory_identity(dest).ok().as_ref()==Some(&self.dest_id)}
 fn run(&self,phase:&str,args:Vec<OsString>)->Result<(),String>{
  if self.cancel.is_cancelled(){return Err("Archive operation cancelled".into())}let id=format!("{}-{phase}",self.task_id);if self.tasks.get(&id).map_err(|_|"Task ledger unavailable")?.is_some(){return Err("Worker already attempted".into())}
  self.processes.start_host_approved(TrustedWorkerPlan{task_id:id.clone(),workspace_id:self.owner.clone(),executable:self.executable.clone(),expected_sha256:self.exe_sha.clone(),args,progress_protocol:false,max_runtime_seconds:Some(180)}).map_err(|_|"Selected archive worker unavailable")?;
  struct Lease<'a>{p:&'a WorkerPlan,id:String,active:bool}impl Drop for Lease<'_>{fn drop(&mut self){if self.active{let _=self.p.processes.cancel_owned(&self.id,&self.p.owner);}}}
  let mut lease=Lease{p:self,id:id.clone(),active:true};loop{self.processes.poll().map_err(|_|"Worker state unavailable")?;let state=self.tasks.get(&id).map_err(|_|"Worker state unavailable")?.ok_or("Worker missing")?.state;
   match state{TaskState::Completed=>{lease.active=false;return if self.cancel.is_cancelled(){Err("Worker cancelled".into())}else{Ok(())}},TaskState::Failed|TaskState::Interrupted|TaskState::Cancelled|TaskState::TimedOut=>{lease.active=false;return Err("Selected archive worker did not complete".into())},_=>{}}
   if self.cancel.is_cancelled(){return Err("Selected archive worker cancelled".into())}thread::sleep(Duration::from_millis(40));
  }
 }
 pub fn preview(&self,source:&Dir,dest:&Dir,name:&str,member:&str)->Result<Witness,String>{
  if !self.check(source,dest)||!leaf(name)||!member_ok(member)||Kind::for_name(name).is_none(){return Err("Unsafe selected archive".into())}let before=source_digest(source,name).map_err(|_|"Source unavailable or too large")?;
  let scratch=std::env::temp_dir().join(format!("lu-member-{}-{}",std::process::id(),self.task_id));let mut builder=fs::DirBuilder::new();#[cfg(unix)]{use std::os::unix::fs::DirBuilderExt;builder.mode(0o700);}builder.create(&scratch).map_err(|_|"Private preview unavailable")?;let scratch=Scratch(scratch);let out=scratch.0.join("receipt.json");
  self.run("preview",vec!["--lu-member-preview-worker".into(),self.source_parent.as_os_str().into(),name.into(),member.into(),out.as_os_str().into(),self.source_id.clone().into(),std::process::id().to_string().into()])?;
  let m=fs::symlink_metadata(&out).map_err(|_|"Missing preview")?;if !m.is_file()||m.len()>4096{return Err("Invalid preview result".into())}let mut bytes=Vec::new();fs::File::open(&out).map_err(|_|"Preview unavailable")?.take(4097).read_to_end(&mut bytes).map_err(|_|"Preview unreadable")?;let w:Witness=serde_json::from_slice(&bytes).map_err(|_|"Malformed preview")?;
  if bytes.len()>4096||!w.valid(name,member)||before!=(w.source_bytes,w.source_sha256.clone())||!source_matches(source,&w)||!self.check(source,dest){return Err("Preview source changed".into())}Ok(w)
 }
 pub fn verify(&self,source:&Dir,dest:&Dir,target:&str,w:&Witness)->bool{opened_directory_identity(source).ok().as_ref()==Some(&self.source_id)&&opened_directory_identity(dest).ok().as_ref()==Some(&self.dest_id)&&source_matches(source,w)&&output_matches(dest,target,w)}
 fn extract(&self,source:&Dir,dest:&Dir,target:&str,w:&Witness,id:&str)->Result<(),String>{
  if !self.check(source,dest)||!source_matches(source,w)||!leaf(target)||!absent(dest,target)||!stage_ok(id){return Err("Selected archive changed or target occupied".into())}
  self.run("extract",vec!["--lu-member-extract-worker".into(),self.source_parent.as_os_str().into(),w.source.clone().into(),w.member.clone().into(),self.dest_parent.as_os_str().into(),target.into(),w.source_sha256.clone().into(),w.member_sha256.clone().into(),self.source_id.clone().into(),self.dest_id.clone().into(),id.into(),std::process::id().to_string().into()])?;
  if !self.verify(source,dest,target,w){return Err("Selected member output conflict".into())}Ok(())
 }
 pub fn task_id(&self)->&str{&self.task_id}
 pub fn source_identity(&self)->&str{&self.source_id}
}
/// Internal worker entry; called only before desktop initialization. The
/// caller is checked by the same parent-death fence as the full-tree worker.
/// Macro-free, bounded parsers; no mount, shell, external compressor or egress.
pub fn worker(args:&[OsString],check_parent:impl Fn(&OsString)->Result<(),()>)->Result<(),()>{
 let text=|i:usize|args.get(i).and_then(|s|s.to_str()).ok_or(());let mode=text(0)?;let preview=mode=="--lu-member-preview-worker";if (preview&&args.len()!=7)||(!preview&&(mode!="--lu-member-extract-worker"||args.len()!=12)){return Err(())}check_parent(args.last().ok_or(())?)?;
 let path=Path::new(&args[1]);if !path.is_absolute(){return Err(())}let source=Dir::open_ambient_dir(path,ambient_authority()).map_err(|_|())?;let name=text(2)?;let member=text(3)?;if opened_directory_identity(&source).map_err(|_|())?!=text(if preview{5}else{8})?{return Err(())}let w=checked_preview(&source,name,member)?;
 if preview{let out=Path::new(&args[4]);if !out.is_absolute(){return Err(())}let mut f=fs::OpenOptions::new().write(true).create_new(true).open(out).map_err(|_|())?;f.write_all(&serde_json::to_vec(&w).map_err(|_|())?).map_err(|_|())?;return f.sync_all().map_err(|_|())}
 let dest_path=Path::new(&args[4]);let target=text(5)?;let id=text(10)?;if !dest_path.is_absolute()||!leaf(target)||!stage_ok(id)||w.source_sha256!=text(6)?||w.member_sha256!=text(7)?{return Err(())}let dest=Dir::open_ambient_dir(dest_path,ambient_authority()).map_err(|_|())?;if opened_directory_identity(&dest).map_err(|_|())?!=text(9)?||!absent(&dest,target){return Err(())}
 match Kind::for_name(name).ok_or(())?{Kind::Zip=>{crate::native_zip_extract::extract_zip_member_new_checked(&source,name,member,&dest,target,id,&w.member_sha256).map_err(|_|())?;},Kind::Iso=>{crate::native_iso9660::extract_iso_file_new_checked(&source,name,member,&dest,target,id,&w.member_sha256).map_err(|_|())?;},Kind::Linux=>{crate::native_linux_archive::extract_linux_archive_member(&source,name,member,&dest,target,id,&w.member_sha256).map_err(|_|())?;}}
 if !source_matches(&source,&w)||!output_matches(&dest,target,&w){return Err(())}Ok(())
}
struct Extractor{descriptor:ToolDescriptor,plan:WorkerPlan,source:Arc<Dir>,dest:Arc<Dir>,target:String,w:Witness,input:ProtoPayload,expected:ProtoPayload}
impl NativeTool for Extractor{fn descriptor(&self)->&ToolDescriptor{&self.descriptor}fn invoke(&self,call:ToolInvocation)->ToolFuture<'_>{let plan=self.plan.clone();let source=self.source.clone();let dest=self.dest.clone();let target=self.target.clone();let w=self.w.clone();let expected=self.expected.clone();let valid=call.input==self.input&&call.tool_id==self.descriptor.id;Box::pin(async move{if !valid{return Err(ToolFailure::new("BAD_ARGUMENTS","Selected member differs from review",false))}tokio::task::spawn_blocking(move||{plan.extract(&source,&dest,&target,&w,&call.call_id).map_err(|_|ToolFailure::new("WRITE_UNCERTAIN","Selected extraction not verified; inspect recovery",false))?;Ok(ToolOutput{payload:expected})}).await.map_err(|_|ToolFailure::new("WRITE_UNCERTAIN","Selected worker completion unknown",false))?})}}
pub(crate) fn register(registry:&NativeToolRegistry,catalog:&mut ModelToolCatalog,plan:WorkerPlan,source:Arc<Dir>,dest:Arc<Dir>,target:&str,w:&Witness)->Result<(),String>{
 let kind=Kind::for_name(&w.source).ok_or("Unknown selected archive")?;if !w.valid(&w.source,&w.member)||!plan.check(&source,&dest)||!leaf(target){return Err("Invalid selected review".into())}let (input,expected)=kind.payload(&w.source,&w.member,target,w);let codec:Arc<dyn ModelToolCodec>=match kind{Kind::Zip=>Arc::new(zip::Codec),Kind::Iso=>Arc::new(iso::Codec),Kind::Linux=>Arc::new(linux::ExtractCodec)};
 let descriptor=ToolDescriptor{id:kind.id().into(),display_name:"Extract exactly one reviewed archive member".into(),effect:ToolEffect::WriteWorkspace,input_type_url:kind.input().into(),output_type_url:kind.output().into()};registry.register(ModuleManifest{id:kind.id().rsplit_once('.').ok_or("Invalid selected tool identity")?.0.into(),display_name:"Supervised archive member extraction".into(),contract_version:CORE_CONTRACT_VERSION,kind:ModuleKind::Core,tools:vec![descriptor.clone()],panels:vec![],workflows:vec![]},vec![Arc::new(Extractor{descriptor,plan,source,dest,target:target.into(),w:w.clone(),input,expected})]).map_err(|_|"Selected registry unavailable")?;
 let key=if kind==Kind::Iso{"source_name"}else{"archive_name"};let parameters=json!({"type":"object","properties":{(key):{"type":"string"},"member_path":{"type":"string"},"target_name":{"type":"string"},"expected_sha256":{"type":"string"}},"required":[key,"member_path","target_name","expected_sha256"],"additionalProperties":false});catalog.insert(registry,ModelToolDefinition{name:kind.model().into(),native_tool_id:kind.id().into(),description:"Extract one exactly approved member via supervised worker".into(),parameters,codec}).map_err(|_|"Selected codec unavailable")?;Ok(())
}

#[cfg(all(test,target_os="linux"))]
mod tests{
 use super::*;
 #[test]fn reviewed_host_does_not_fall_back_when_original_kernel_broker_is_missing(){
  if Path::new("/usr/libexec/lu/lu-sec06c-launch-native").exists(){return}
  let root=std::env::temp_dir().join(format!("lu-selected-kernel-denied-{}",std::process::id()));let _=fs::remove_dir_all(&root);fs::create_dir(&root).unwrap();let root=root.canonicalize().unwrap();let dir=Dir::open_ambient_dir(&root,ambient_authority()).unwrap();fs::write(root.join("dummy.zip"),b"not accessed").unwrap();
  let tasks=Arc::new(TaskStore::open(&root.join("tasks.sqlite")).unwrap());let chat=format!("c-{:032x}",1);let turn=format!("g-{:032x}",2);tasks.bind_reviewed_human_turn(&chat,&turn).unwrap();let supervisor=Arc::new(BackgroundProcessSupervisor::new_for_reviewed_host(tasks.clone(),root.clone()));let plan=WorkerPlan::new(supervisor,tasks.clone(),PathBuf::from("/bin/true"),root.clone(),root.clone(),&dir,&dir,"selected-denied-0001",&turn,ArchiveCancellation::default()).unwrap();
  assert!(plan.preview(&dir,&dir,"dummy.zip","a.txt").is_err());assert_eq!(tasks.get("selected-denied-0001-preview").unwrap().unwrap().state,TaskState::Failed);assert!(tasks.get("selected-denied-0001-extract").unwrap().is_none());drop(plan);drop(tasks);drop(dir);fs::remove_dir_all(root).unwrap();
 }
}
