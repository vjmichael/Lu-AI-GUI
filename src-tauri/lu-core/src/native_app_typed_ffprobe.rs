//! E1-P06E: typed local FFprobe metadata inspection using a reviewed native app.
//!
//! Lu Core is a *native, unsandboxed desktop host*. Child process control is
//! for cancellation/recovery, not a sandbox around Lu. The host picks the
//! executable and media file. Neither model text nor outside apps select
//! argv, file paths, authority or journal effects. Exact original-user-turn
//! ReadWorkspace and ExecuteCode grants are both required.
use std::{
    collections::{BTreeMap,BTreeSet},
    fs,path::{Path,PathBuf},ffi::OsString,
    sync::{Arc,Mutex},
};
use serde_json::{json,Value};
use crate::{
    contracts::{ToolAuthorizer,ToolDescriptor,ToolEffect,ToolInvocation},
    conversation_authority::{BoundCallOrigin,ConversationAuthority},
    journal::{EffectJournal,EffectState,PrepareOutcome},
    native_app_admission::{
        HostReviewedExecutable,inspect_host_reviewed_code_application,
    },
    native_background_process::{
        BackgroundProcessSupervisor,TrustedWorkerPlan,verified_media_input,
    },
    tasks::{TaskStore,TaskState},
};

pub const INSPECT_INPUT:&str="type.googleapis.com/lu.e1.HostMediaProbeRequest";
pub const INSPECT_OUTPUT:&str="type.googleapis.com/lu.e1.MediaProbeObservation";
pub const MEDIA_READ_INPUT:&str="type.googleapis.com/lu.e1.HostMediaReadPermission";
pub const SEALED_INSPECT_BYTES:&[u8]=b"lu.e1.ffprobe.basic-media-facts.v1";
pub const SEALED_READ_BYTES:&[u8]=b"lu.e1.ffprobe.host-approved-media-read.v1";
const FIELDS:&str="format=duration:stream=index,codec_type,codec_name,width,height,sample_rate,channels";
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum ReviewedContainer{Wave,Mp4}
impl ReviewedContainer{
    fn ffprobe_demuxer(self)->&'static str{
        match self{Self::Wave=>"wav",Self::Mp4=>"mov"}
    }
    fn accepted_path(self,path:&Path)->bool{
        path.extension().and_then(|s|s.to_str())
            .is_some_and(|extension|match self{
                Self::Wave=>extension.eq_ignore_ascii_case("wav"),
                Self::Mp4=>extension.eq_ignore_ascii_case("mp4"),
            })
    }
}

#[derive(Debug,Clone,PartialEq,Eq)]
pub struct HostReviewedMediaProbe{
    pub profile_id:String,
    pub executable:HostReviewedExecutable,
    pub approved_media_path:PathBuf,
    pub media_sha256:String,
    pub reviewed_container:ReviewedContainer,
    pub execute_tool_id:String,
    pub read_tool_id:String,
    pub stdout_limit:usize,
    pub timeout_seconds:u64,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum MediaProbeError{
    InvalidProfile,DuplicateProfile,NotReviewed,Capacity,
    InvalidOperation,UnauthorizedOrChanged,TaskAlreadyRunning,
    DuplicateEffect,JournalUnavailable,WorkerStartFailed,
    WorkerStatusUnavailable,LockUnavailable,WrongCancellationOrigin,
    WorkerCancellationUncertain,
}
#[derive(Default)]
pub struct HostMediaProbeCatalog{items:BTreeMap<String,HostReviewedMediaProbe>}
fn valid_id(id:&str)->bool{
    !id.is_empty()&&id.len()<=128&&id.bytes().all(|b|
        b.is_ascii_alphanumeric()||matches!(b,b'.'|b'_'|b'-'))
}
fn valid_sha(value:&str)->bool{
    value.len()==64&&value.bytes().all(|b|b.is_ascii_hexdigit())
}
impl HostMediaProbeCatalog{
    pub fn new()->Self{Self::default()}
    /// Called only by trusted host enrollment, not a model-visible tool.
    pub fn register_host_reviewed(&mut self,item:HostReviewedMediaProbe)
        ->Result<(),MediaProbeError>{
        if !valid_id(&item.profile_id)
            ||!valid_id(&item.executable.app_id)
            ||!valid_id(&item.executable.tool_id)
            ||!valid_id(&item.executable.workspace_id)
            ||!valid_id(&item.execute_tool_id)
            ||!valid_id(&item.read_tool_id)
            ||item.execute_tool_id==item.read_tool_id
            ||!valid_sha(&item.executable.expected_sha256)
            ||!valid_sha(&item.media_sha256)
            ||!item.executable.executable.is_absolute()
            ||!(1..=16_384).contains(&item.stdout_limit)
            ||!(1..=30).contains(&item.timeout_seconds)
            ||!item.reviewed_container.accepted_path(&item.approved_media_path)
            ||verified_media_input(&item.approved_media_path,&item.media_sha256)
                .is_err(){
            return Err(MediaProbeError::InvalidProfile)
        }
        if self.items.contains_key(&item.profile_id){
            return Err(MediaProbeError::DuplicateProfile)
        }
        if self.items.len()>=32{return Err(MediaProbeError::Capacity)}
        self.items.insert(item.profile_id.clone(),item);
        Ok(())
    }
    pub fn delist(&mut self,id:&str)->Result<(),MediaProbeError>{
        self.items.remove(id)
            .map(|_|()).ok_or(MediaProbeError::NotReviewed)
    }
    fn reviewed(&self,id:&str)->Option<HostReviewedMediaProbe>{
        self.items.get(id).cloned()
    }
    pub fn len(&self)->usize{self.items.len()}
}

fn ffprobe_args(input:&Path,container:ReviewedContainer)->Vec<OsString>{
    [
        OsString::from("-v"),OsString::from("error"),
        OsString::from("-protocol_whitelist"),OsString::from("file"),
        OsString::from("-f"),OsString::from(container.ffprobe_demuxer()),
        OsString::from("-show_entries"),OsString::from(FIELDS),
        OsString::from("-of"),OsString::from("json"),
        OsString::from("-i"),input.as_os_str().to_owned()
    ].into_iter().collect()
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct MediaFacts{
    pub duration_millis:u64,
    /// Normalized bounded stream facts; never raw FFprobe JSON or media tags.
    pub streams:Vec<MediaStreamFacts>,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct MediaStreamFacts{
    pub index:u64,
    pub kind:String,
    pub codec:String,
    pub width:Option<u64>,
    pub height:Option<u64>,
    pub sample_rate_hz:Option<u64>,
    pub channels:Option<u64>,
}
fn json_uint(value:Option<&Value>,min:u64,max:u64)->Option<u64>{
    let n=value?.as_u64()?;
    if (min..=max).contains(&n){Some(n)}else{None}
}
fn json_ascii(value:Option<&Value>,max:usize)->Option<String>{
    let value=value?.as_str()?;
    if value.is_empty()||value.len()>max
        ||!value.bytes().all(|b|
            b.is_ascii_alphanumeric()||matches!(b,b'.'|b'+'|b'_'|b'-')){
        return None
    }
    Some(value.to_owned())
}
pub(crate) fn parse_ffprobe_facts(bytes:&[u8])->Option<MediaFacts>{
    if bytes.len()<16||bytes.len()>16_384{return None}
    let value:Value=serde_json::from_slice(bytes).ok()?;
    let root=value.as_object()?;
    // Reject unexpected payload sections: no arbitrary process output,
    // file names, tags, chapters, third-party metadata or diagnostics.
    if root.keys().any(|key|
        !matches!(key.as_str(),"streams"|"format"|"programs"|"stream_groups"))
        ||["programs","stream_groups"].iter().any(|key|
            root.get(*key).is_some_and(|v|v.as_array().is_none_or(|a|!a.is_empty()))){
        return None
    }
    let info=root.get("format")?.as_object()?;
    if info.len()!=1||!info.contains_key("duration"){return None}
    let duration=info.get("duration")?.as_str()?;
    if duration.is_empty()||duration.len()>24||duration.matches('.').count()>1
        ||!duration.bytes().all(|b|b.is_ascii_digit()||b==b'.'){
        return None
    }
    let secs=duration.parse::<f64>().ok()?;
    if !secs.is_finite()||secs<=0.0||secs>7.0*24.0*3600.0{return None}
    let duration_millis=(secs*1000.0).round() as u64;
    if duration_millis==0{return None}
    let rows=root.get("streams")?.as_array()?;
    if rows.is_empty()||rows.len()>16{return None}
    let mut used=BTreeSet::new();
    let mut streams=Vec::with_capacity(rows.len());
    for row in rows{
        let obj=row.as_object()?;
        if obj.keys().any(|key|!matches!(key.as_str(),
            "index"|"codec_type"|"codec_name"|"width"|"height"|"sample_rate"|"channels")){
            return None
        }
        let index=json_uint(obj.get("index"),0,1024)?;
        if !used.insert(index){return None}
        let kind=obj.get("codec_type")?.as_str()?;
        if !matches!(kind,"video"|"audio"){return None}
        let codec=json_ascii(obj.get("codec_name"),48)?;
        let (width,height,sample_rate_hz,channels)=if kind=="video"{
            (
                Some(json_uint(obj.get("width"),1,16384)?),
                Some(json_uint(obj.get("height"),1,16384)?),
                None,None
            )
        }else{
            let rate=obj.get("sample_rate")?.as_str()?;
            if !rate.bytes().all(|b|b.is_ascii_digit()){return None}
            let hz=rate.parse::<u64>().ok()?;
            if !(1..=768000).contains(&hz){return None}
            (None,None,Some(hz),Some(json_uint(obj.get("channels"),1,64)?))
        };
        streams.push(MediaStreamFacts{
            index,kind:kind.into(),codec,width,height,sample_rate_hz,channels
        });
    }
    Some(MediaFacts{duration_millis,streams})
}

#[derive(Debug,Clone)]
struct ActiveMediaProbe{
    call_id:String,
    workspace:String,
    origin:BoundCallOrigin,
    media_path:PathBuf,
    media_sha256:String,
    max_stdout:usize,
    revoked:bool,
}
#[derive(Default)]
struct ActiveGate{tasks:BTreeMap<String,ActiveMediaProbe>}
pub struct TypedMediaProbeBroker{
    authority:ConversationAuthority,
    supervisor:Arc<BackgroundProcessSupervisor>,
    tasks:Arc<TaskStore>,
    journal:Arc<EffectJournal>,
    gate:Arc<Mutex<ActiveGate>>,
}
impl TypedMediaProbeBroker{
    pub fn new(
        authority:ConversationAuthority,
        supervisor:Arc<BackgroundProcessSupervisor>,
        tasks:Arc<TaskStore>,journal:Arc<EffectJournal>,
    )->Result<Self,MediaProbeError>{
        let gate=Arc::new(Mutex::new(ActiveGate::default()));
        let revoked_gate=Arc::clone(&gate);
        let revoked_workers=Arc::clone(&supervisor);
        let revoked_journal=Arc::clone(&journal);
        authority.on_task_revoked(Arc::new(move |task_id|{
            if let Ok(mut guard)=revoked_gate.lock(){
                if let Some(worker)=guard.tasks.get_mut(task_id){
                    worker.revoked=true;
                    let _=revoked_journal.mark_uncertain(&worker.call_id);
                    let call=worker.call_id.clone();
                    let workspace=worker.workspace.clone();
                    if revoked_workers.cancel_owned(&call,&workspace).is_ok(){
                        guard.tasks.remove(task_id);
                    }
                }
            }
        })).map_err(|_|MediaProbeError::LockUnavailable)?;
        Ok(Self{authority,supervisor,tasks,journal,gate})
    }
    pub fn launch(
        &self,catalog:&HostMediaProbeCatalog,profile_id:&str,
        origin:&BoundCallOrigin,
        execute:&ToolInvocation,execute_descriptor:&ToolDescriptor,
        read:&ToolInvocation,read_descriptor:&ToolDescriptor,
        host_policy:&dyn ToolAuthorizer,
    )->Result<(),MediaProbeError>{
        let mut gate=self.gate.lock().map_err(|_|MediaProbeError::LockUnavailable)?;
        if gate.tasks.contains_key(&origin.task_id){
            return Err(MediaProbeError::TaskAlreadyRunning)
        }
        let profile=catalog.reviewed(profile_id)
            .ok_or(MediaProbeError::NotReviewed)?;
        if execute.tool_id!=profile.execute_tool_id
            ||execute_descriptor.id!=profile.execute_tool_id
            ||execute_descriptor.effect!=ToolEffect::ExecuteCode
            ||execute_descriptor.input_type_url!=INSPECT_INPUT
            ||execute_descriptor.output_type_url!=INSPECT_OUTPUT
            ||execute.workspace_id.as_deref()
                !=Some(profile.executable.workspace_id.as_str())
            ||execute.input.type_url!=INSPECT_INPUT
            ||execute.input.bytes.as_slice()!=SEALED_INSPECT_BYTES
            ||read.tool_id!=profile.read_tool_id
            ||read_descriptor.id!=profile.read_tool_id
            ||read_descriptor.effect!=ToolEffect::ReadWorkspace
            ||read_descriptor.input_type_url!=MEDIA_READ_INPUT
            ||read_descriptor.output_type_url!=INSPECT_OUTPUT
            ||read.workspace_id.as_deref()
                !=Some(profile.executable.workspace_id.as_str())
            ||read.input.type_url!=MEDIA_READ_INPUT
            ||read.input.bytes.as_slice()!=SEALED_READ_BYTES
            ||!valid_id(&execute.call_id)
            ||!valid_id(&read.call_id)
            ||execute.call_id==read.call_id{
            return Err(MediaProbeError::InvalidOperation)
        }
        self.authority.authorize_worker_call(
            origin,read,read_descriptor,host_policy
        ).map_err(|_|MediaProbeError::UnauthorizedOrChanged)?;
        let mut exec_image=profile.executable.clone();
        exec_image.tool_id=profile.execute_tool_id.clone();
        inspect_host_reviewed_code_application(
            &exec_image,origin,execute,execute_descriptor,
            &self.authority,host_policy
        ).map_err(|_|MediaProbeError::UnauthorizedOrChanged)?;
        verified_media_input(&profile.approved_media_path,&profile.media_sha256)
            .map_err(|_|MediaProbeError::UnauthorizedOrChanged)?;
        let plan=TrustedWorkerPlan{
            task_id:execute.call_id.clone(),
            workspace_id:profile.executable.workspace_id.clone(),
            executable:profile.executable.executable,
            expected_sha256:profile.executable.expected_sha256,
            args:ffprobe_args(&profile.approved_media_path,profile.reviewed_container),
            progress_protocol:false,
            max_runtime_seconds:Some(profile.timeout_seconds),
        };
        // P0-AUTH: do not permit a revoked task to spawn after the
        // synchronous revocation listener has already scanned ownership.
        // This read+execute lease ends as soon as the OS child is registered.
        self.authority.with_authorized_worker_launch(
            origin,read,read_descriptor,execute,execute_descriptor,
            host_policy,||{
                match self.journal.prepare(execute)
                    .map_err(|_|MediaProbeError::JournalUnavailable)?{
                    PrepareOutcome::New=>{},
                    PrepareOutcome::Existing(_)=>return Err(MediaProbeError::DuplicateEffect),
                }
                self.journal.mark_executing(&execute.call_id)
                    .map_err(|_|MediaProbeError::JournalUnavailable)?;
                if self.supervisor.start_host_approved_media_probe(
                    plan,profile.stdout_limit,&profile.media_sha256
                ).is_err(){
                    let _=self.journal.mark_uncertain(&execute.call_id);
                    return Err(MediaProbeError::WorkerStartFailed)
                }
                gate.tasks.insert(origin.task_id.clone(),ActiveMediaProbe{
                    call_id:execute.call_id.clone(),
                    workspace:profile.executable.workspace_id,
                    origin:origin.clone(),
                    media_path:profile.approved_media_path,
                    media_sha256:profile.media_sha256,
                    max_stdout:profile.stdout_limit,
                    revoked:false,
                });
                Ok(())
            }
        ).map_err(|_|MediaProbeError::UnauthorizedOrChanged)?
    }
    pub fn poll(&self)->Result<usize,MediaProbeError>{
        let mut gate=self.gate.lock().map_err(|_|MediaProbeError::LockUnavailable)?;
        let finished=self.supervisor.poll()
            .map_err(|_|MediaProbeError::WorkerStatusUnavailable)?;
        let mut done=Vec::new();
        for (user_task,active) in &gate.tasks{
            let state=self.tasks.get(&active.call_id)
                .map_err(|_|MediaProbeError::WorkerStatusUnavailable)?
                .ok_or(MediaProbeError::WorkerStatusUnavailable)?.state;
            if matches!(state,TaskState::Pending|TaskState::Running|TaskState::Paused){
                continue
            }
            if active.revoked{
                if self.journal.get(&active.call_id)
                    .map_err(|_|MediaProbeError::JournalUnavailable)?
                    .is_some_and(|effect|effect.state==EffectState::Executing){
                    self.journal.mark_uncertain(&active.call_id)
                        .map_err(|_|MediaProbeError::JournalUnavailable)?;
                }
                done.push(user_task.clone());
                continue
            }
            if state==TaskState::Completed{
                let stdout=self.supervisor.take_local_version_stdout(
                    &active.call_id,&active.workspace
                ).map_err(|_|MediaProbeError::WorkerStatusUnavailable)?;
                if let Some(captured)=stdout{
                    let still_same=verified_media_input(
                        &active.media_path,&active.media_sha256
                    ).is_ok();
                    if still_same&&captured.bytes.len()<=active.max_stdout{
                        if let Some(facts)=parse_ffprobe_facts(&captured.bytes){
                            let streams=facts.streams.iter().map(|stream|json!({
                                "index":stream.index,"type":stream.kind,
                                "codec":stream.codec,"width":stream.width,
                                "height":stream.height,
                                "sampleRateHz":stream.sample_rate_hz,
                                "channels":stream.channels,
                            })).collect::<Vec<_>>();
                            let receipt=json!({
                                "schema":"lu-e1-p06e-media-facts-v1",
                                "callId":active.call_id,
                                "durationMilliseconds":facts.duration_millis,
                                "streams":streams,
                                "approvedMediaSha256":active.media_sha256,
                                "stdoutSha256":captured.sha256,
                                "stdoutBytes":captured.bytes.len(),
                                "mediaDigestReverifiedAfterProbe":true,
                                "osChildReaped":true,
                                "hostExecuteAuthorized":true,
                                "hostReadAuthorized":true,
                                "rawMediaPathPersisted":false,
                                "rawFFprobeOutputPersisted":false,
                                "luCoreSandboxed":false
                            });
                            let encoded=serde_json::to_vec(&receipt)
                                .map_err(|_|MediaProbeError::JournalUnavailable)?;
                            self.journal.mark_applied(&active.call_id,&encoded)
                                .map_err(|_|MediaProbeError::JournalUnavailable)?;
                            done.push(user_task.clone());
                            continue
                        }
                    }
                }
            }
            // Bad, changed or unreadable media; invalid JSON; child failure;
            // timeout/cancellation are uncertain and cannot be auto-replayed.
            self.journal.mark_uncertain(&active.call_id)
                .map_err(|_|MediaProbeError::JournalUnavailable)?;
            done.push(user_task.clone());
        }
        for task in done{gate.tasks.remove(&task);}
        Ok(finished)
    }
    pub fn cancel_current_task(
        &self,origin:&BoundCallOrigin
    )->Result<(),MediaProbeError>{
        let mut gate=self.gate.lock().map_err(|_|MediaProbeError::LockUnavailable)?;
        let active=gate.tasks.get(&origin.task_id)
            .ok_or(MediaProbeError::WrongCancellationOrigin)?;
        if active.origin!=*origin{return Err(MediaProbeError::WrongCancellationOrigin)}
        let active=active.clone();
        if let Some(owned)=gate.tasks.get_mut(&origin.task_id){
            owned.revoked=true;
        }
        let _=self.journal.mark_uncertain(&active.call_id);
        if self.supervisor.cancel_owned(
            &active.call_id,&active.workspace
        ).is_err(){
            return Err(MediaProbeError::WorkerCancellationUncertain)
        }
        gate.tasks.remove(&origin.task_id);
        Ok(())
    }
    pub fn active_count(&self)->Result<usize,MediaProbeError>{
        Ok(self.gate.lock().map_err(|_|MediaProbeError::LockUnavailable)?
            .tasks.len())
    }
}
impl Drop for TypedMediaProbeBroker{
    fn drop(&mut self){
        if let Ok(mut gate)=self.gate.lock(){
            for (_,active) in std::mem::take(&mut gate.tasks){
                let _=self.journal.mark_uncertain(&active.call_id);
                let _=self.supervisor.cancel_owned(&active.call_id,&active.workspace);
            }
        }
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    use sha2::{Digest,Sha256};
    use std::{
        time::{Duration,SystemTime,UNIX_EPOCH},
        process,
    };
    use crate::{
        ansf::{AnsfStore,EntryRole},
        contracts::{ProtoPayload,AuthorizationError},
        conversation_authority::{
            AuthorizedCapability,TaskAuthority,TrustedUserTurnVerifier,
        },
    };
    struct Human<'a>(&'a AnsfStore);
    impl TrustedUserTurnVerifier for Human<'_>{
        fn verify_user_turn(&self,session:&str,turn:&str)->bool{
            self.0.find_message(session,turn).ok().flatten()
                .is_some_and(|e|e.role=="user")
        }
    }
    struct Policy(bool);
    impl ToolAuthorizer for Policy{
        fn authorize(&self,_:&ToolInvocation,_:&ToolDescriptor)
            ->Result<(),AuthorizationError>{
            if self.0{Ok(())}else{Err(AuthorizationError::new("host veto"))}
        }
    }
    fn wav_one_second()->Vec<u8>{
        const SAMPLES:u32=8000;
        let mut v=Vec::with_capacity(44+SAMPLES as usize*2);
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&(36+SAMPLES*2).to_le_bytes());
        v.extend_from_slice(b"WAVEfmt ");
        v.extend_from_slice(&16u32.to_le_bytes()); // PCM header
        v.extend_from_slice(&1u16.to_le_bytes()); // PCM
        v.extend_from_slice(&1u16.to_le_bytes()); // mono
        v.extend_from_slice(&8000u32.to_le_bytes()); // sample rate
        v.extend_from_slice(&16000u32.to_le_bytes()); // byte rate
        v.extend_from_slice(&2u16.to_le_bytes()); // block align
        v.extend_from_slice(&16u16.to_le_bytes()); // bits/sample
        v.extend_from_slice(b"data");
        v.extend_from_slice(&(SAMPLES*2).to_le_bytes());
        v.extend(vec![0;SAMPLES as usize*2]);
        v
    }
    fn root()->PathBuf{
        std::env::temp_dir().join(format!(
            "lu-e1-p06e-{}-{}",process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ))
    }
    fn ffprobe()->PathBuf{
        let path=PathBuf::from(std::env::var_os("LU_E1_P06_FFPROBE")
            .expect("dedicated native CI needs genuine LU_E1_P06_FFPROBE"));
        let path=path.canonicalize().unwrap();
        assert!(path.is_absolute()&&path.is_file());
        path
    }
    fn origin()->BoundCallOrigin{
        BoundCallOrigin{
            conversation_id:"c-media".into(),
            user_turn_id:"u-media".into(),
            task_id:"t-media".into(),
        }
    }
    fn execute()->ToolInvocation{
        ToolInvocation{
            call_id:"e1-media-exec".into(),
            tool_id:"lu.media.ffprobe.execute".into(),
            workspace_id:Some("media-workspace".into()),
            input:ProtoPayload{
                type_url:INSPECT_INPUT.into(),bytes:SEALED_INSPECT_BYTES.to_vec()
            },
        }
    }
    fn read()->ToolInvocation{
        ToolInvocation{
            call_id:"e1-media-read".into(),
            tool_id:"lu.media.file.read".into(),
            workspace_id:Some("media-workspace".into()),
            input:ProtoPayload{
                type_url:MEDIA_READ_INPUT.into(),bytes:SEALED_READ_BYTES.to_vec()
            },
        }
    }
    fn descriptor_exec()->ToolDescriptor{
        ToolDescriptor{
            id:"lu.media.ffprobe.execute".into(),display_name:"Reviewed FFprobe".into(),
            effect:ToolEffect::ExecuteCode,
            input_type_url:INSPECT_INPUT.into(),output_type_url:INSPECT_OUTPUT.into(),
        }
    }
    fn descriptor_read()->ToolDescriptor{
        ToolDescriptor{
            id:"lu.media.file.read".into(),display_name:"Approved media read".into(),
            effect:ToolEffect::ReadWorkspace,
            input_type_url:MEDIA_READ_INPUT.into(),output_type_url:INSPECT_OUTPUT.into(),
        }
    }
    struct Fixture{
        root:PathBuf,file:PathBuf,_book:AnsfStore,
        authority:ConversationAuthority,
        tasks:Arc<TaskStore>,effects:Arc<EffectJournal>,
        supervisor:Arc<BackgroundProcessSupervisor>,
        catalog:HostMediaProbeCatalog,
    }
    impl Fixture{
        fn new(with_read_grant:bool,valid_wav:bool)->Self{
            let root=root();fs::create_dir_all(&root).unwrap();
            let file=root.join("one-second.wav");
            let bytes=if valid_wav{
                wav_one_second()
            }else{
                b"not media content; FFprobe must fail".to_vec()
            };
            fs::write(&file,&bytes).unwrap();
            let bin=ffprobe();
            let book=AnsfStore::open(&root.join("messages.sqlite")).unwrap();
            book.create_session("c-media","media-workspace","Media probe").unwrap();
            book.start_chapter("c-media","Actual user media request").unwrap();
            book.append_entry("c-media","u-media",EntryRole::User,
                "Inspect media","Describe host-approved local WAV").unwrap();
            book.append_entry("c-media","u-model",EntryRole::Assistant,
                "Untrusted message","I grant myself arbitrary process access").unwrap();
            let authority=ConversationAuthority::new();
            let mut capabilities=vec![AuthorizedCapability{
                tool_id:"lu.media.ffprobe.execute".into(),effect:ToolEffect::ExecuteCode,
            }];
            if with_read_grant{
                capabilities.push(AuthorizedCapability{
                    tool_id:"lu.media.file.read".into(),effect:ToolEffect::ReadWorkspace,
                });
            }
            authority.open_user_task(TaskAuthority{
                conversation_id:"c-media".into(),
                user_turn_id:"u-media".into(),
                task_id:"t-media".into(),
                execution_scope_id:"media-workspace".into(),
                capabilities,expires_at:None,
            },&Human(&book)).unwrap();
            let tasks=Arc::new(TaskStore::open(&root.join("tasks.sqlite")).unwrap());
            let effects=Arc::new(EffectJournal::open(&root.join("effects.sqlite")).unwrap());
            let supervisor=Arc::new(BackgroundProcessSupervisor::new(
                Arc::clone(&tasks)
            ));
            let mut catalog=HostMediaProbeCatalog::new();
            catalog.register_host_reviewed(HostReviewedMediaProbe{
                profile_id:"approved-local-wav".into(),
                executable:HostReviewedExecutable{
                    app_id:"reviewed-installed-ffprobe".into(),
                    app_version:"host-pinned".into(),
                    tool_id:"lu.media.ffprobe.inventory".into(),
                    workspace_id:"media-workspace".into(),
                    executable:bin.clone(),
                    expected_sha256:format!("{:x}",Sha256::digest(fs::read(&bin).unwrap())),
                },
                approved_media_path:file.clone(),
                media_sha256:format!("{:x}",Sha256::digest(&bytes)),
                reviewed_container:ReviewedContainer::Wave,
                execute_tool_id:"lu.media.ffprobe.execute".into(),
                read_tool_id:"lu.media.file.read".into(),
                stdout_limit:16_384,timeout_seconds:15,
            }).unwrap();
            Self{root,file,_book:book,authority,tasks,effects,supervisor,catalog}
        }
        fn broker(&self)->TypedMediaProbeBroker{
            TypedMediaProbeBroker::new(
                self.authority.clone(),Arc::clone(&self.supervisor),
                Arc::clone(&self.tasks),Arc::clone(&self.effects)
            ).unwrap()
        }
        fn invoke(&self,broker:&TypedMediaProbeBroker,allow:bool)
            ->Result<(),MediaProbeError>{
            broker.launch(
                &self.catalog,"approved-local-wav",&origin(),
                &execute(),&descriptor_exec(),&read(),&descriptor_read(),
                &Policy(allow)
            )
        }
        fn complete(&self,broker:&TypedMediaProbeBroker)->TaskState{
            let mut finished=false;
            for _ in 0..750{
                broker.poll().unwrap();
                if broker.active_count().unwrap()==0{
                    finished=true;break
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            assert!(finished,"actual OS FFprobe must exit and be reaped");
            assert!(!self.supervisor.can_cancel("e1-media-exec"));
            self.tasks.get("e1-media-exec").unwrap().unwrap().state
        }
    }
    #[test]
    fn strict_media_parser_accepts_realistic_pcm_and_rejects_injections(){
        let clean=br#"{
            "programs":[],"stream_groups":[],
            "streams":[{"index":0,"codec_name":"pcm_s16le",
                "codec_type":"audio","sample_rate":"8000","channels":1}],
            "format":{"duration":"1.000000"}
        }"#;
        let got=parse_ffprobe_facts(clean).unwrap();
        assert_eq!(got.duration_millis,1000);
        assert_eq!(got.streams[0].codec,"pcm_s16le");
        assert_eq!(got.streams[0].sample_rate_hz,Some(8000));
        for hostile in [
            br#"{"streams":[],"format":{"duration":"1"}}"#.as_slice(),
            br#"{"streams":[{"index":0,"codec_name":"aac","codec_type":"audio","sample_rate":"NaN","channels":2}],"format":{"duration":"1"}}"#.as_slice(),
            br#"{"streams":[{"index":0,"codec_name":"aac","codec_type":"audio","sample_rate":"48000","channels":2}],"format":{"duration":"Infinity"}}"#.as_slice(),
            br#"{"streams":[{"index":0,"codec_name":"aac","codec_type":"audio","sample_rate":"48000","channels":2}],"format":{"duration":"1"},"tags":{"title":"secret"}}"#.as_slice(),
            br#"{"streams":[{"index":0,"codec_name":"aac","codec_type":"audio","sample_rate":"48000","channels":2}],"format":{"duration":"1"},"programs":[{"secret":"tag"}]}"#.as_slice(),
        ]{
            assert!(parse_ffprobe_facts(hostile).is_none());
        }
    }
    #[test]
    #[ignore="real FFprobe executable required, native Windows/Linux CI"]
    fn actual_ffprobe_one_second_wav_is_authorized_parsed_reaped_and_journaled(){
        let f=Fixture::new(true,true);
        let broker=f.broker();
        assert_eq!(f.invoke(&broker,false),Err(MediaProbeError::UnauthorizedOrChanged));
        assert!(f.effects.get(&execute().call_id).unwrap().is_none());
        f.invoke(&broker,true).unwrap();
        assert_eq!(f.complete(&broker),TaskState::Completed);
        let effect=f.effects.get(&execute().call_id).unwrap().unwrap();
        assert_eq!(effect.state,EffectState::Applied);
        let encoded=effect.receipt.as_ref().unwrap();
        let receipt:Value=serde_json::from_slice(encoded).unwrap();
        assert_eq!(receipt["schema"],"lu-e1-p06e-media-facts-v1");
        assert_eq!(receipt["durationMilliseconds"],1000);
        assert_eq!(receipt["streams"][0]["codec"],"pcm_s16le");
        assert_eq!(receipt["streams"][0]["sampleRateHz"],8000);
        assert_eq!(receipt["streams"][0]["channels"],1);
        assert_eq!(receipt["osChildReaped"],true);
        assert_eq!(receipt["rawMediaPathPersisted"],false);
        assert_eq!(receipt["mediaDigestReverifiedAfterProbe"],true);
        assert!(!String::from_utf8_lossy(encoded)
            .contains(f.file.to_string_lossy().as_ref()));
        assert_eq!(f.invoke(&broker,true),Err(MediaProbeError::DuplicateEffect));
        let dir=f.root.clone();drop(broker);drop(f);
        let reopened=EffectJournal::open(&dir.join("effects.sqlite")).unwrap();
        assert_eq!(reopened.get("e1-media-exec").unwrap().unwrap().state,
            EffectState::Applied);
        drop(reopened);fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    #[ignore="actual FFmpeg fixture generator and reviewed FFprobe video consumer required"]
    fn real_mp4_video_dimensions_codec_and_duration_are_independently_verified(){
        let mut f=Fixture::new(true,true);
        let ffmpeg=PathBuf::from(std::env::var_os("LU_E1_P06_FFMPEG")
            .expect("native video fixture requires actual FFmpeg"));
        let ffmpeg=ffmpeg.canonicalize().unwrap();
        assert!(ffmpeg.is_file());
        let video=f.root.join("one-second.mp4");
        let status=process::Command::new(&ffmpeg)
            .args(["-hide_banner","-loglevel","error","-f","lavfi",
                   "-i","color=c=black:s=32x32:r=1","-t","1",
                   "-c:v","mpeg4","-y"])
            .arg(&video)
            .status().unwrap();
        assert!(status.success(),"real FFmpeg must generate actual MP4 bytes");
        f.catalog.delist("approved-local-wav").unwrap();
        let probe=ffprobe();
        f.catalog.register_host_reviewed(HostReviewedMediaProbe{
            profile_id:"approved-local-wav".into(),
            executable:HostReviewedExecutable{
                app_id:"reviewed-installed-ffprobe".into(),
                app_version:"host-pinned".into(),
                tool_id:"lu.media.ffprobe.inventory".into(),
                workspace_id:"media-workspace".into(),
                executable:probe.clone(),
                expected_sha256:format!("{:x}",Sha256::digest(fs::read(probe).unwrap())),
            },
            approved_media_path:video.clone(),
            media_sha256:format!("{:x}",Sha256::digest(fs::read(&video).unwrap())),
            reviewed_container:ReviewedContainer::Mp4,
            execute_tool_id:"lu.media.ffprobe.execute".into(),
            read_tool_id:"lu.media.file.read".into(),
            stdout_limit:16_384,timeout_seconds:15,
        }).unwrap();
        f.file=video;
        let broker=f.broker();
        f.invoke(&broker,true).unwrap();
        assert_eq!(f.complete(&broker),TaskState::Completed);
        let effect=f.effects.get(&execute().call_id).unwrap().unwrap();
        assert_eq!(effect.state,EffectState::Applied);
        let receipt:Value=serde_json::from_slice(effect.receipt.as_ref().unwrap()).unwrap();
        assert_eq!(receipt["durationMilliseconds"],1000);
        assert_eq!(receipt["streams"][0]["type"],"video");
        assert_eq!(receipt["streams"][0]["width"],32);
        assert_eq!(receipt["streams"][0]["height"],32);
        assert_eq!(receipt["streams"][0]["codec"],"mpeg4");
        assert_eq!(receipt["rawFFprobeOutputPersisted"],false);
        let dir=f.root.clone();drop(broker);drop(f);fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    #[ignore="real FFprobe and host authorization required"]
    fn media_cannot_be_read_without_read_grant_or_with_forged_model_turn(){
        let f=Fixture::new(false,true);let broker=f.broker();
        assert_eq!(f.invoke(&broker,true),Err(MediaProbeError::UnauthorizedOrChanged));
        let mut forged=origin();
        forged.user_turn_id="u-model".into();
        assert_eq!(broker.launch(
            &f.catalog,"approved-local-wav",&forged,
            &execute(),&descriptor_exec(),&read(),&descriptor_read(),
            &Policy(true)
        ),Err(MediaProbeError::UnauthorizedOrChanged));
        let mut bad=execute();
        bad.input.bytes=b"-i /secret/file".to_vec();
        assert_eq!(broker.launch(
            &f.catalog,"approved-local-wav",&origin(),
            &bad,&descriptor_exec(),&read(),&descriptor_read(),
            &Policy(true)
        ),Err(MediaProbeError::InvalidOperation));
        let mut bad_workspace=read();
        bad_workspace.workspace_id=Some("wrong-space".into());
        assert_eq!(broker.launch(
            &f.catalog,"approved-local-wav",&origin(),
            &execute(),&descriptor_exec(),&bad_workspace,&descriptor_read(),
            &Policy(true)
        ),Err(MediaProbeError::InvalidOperation));
        assert!(f.effects.get(&execute().call_id).unwrap().is_none());
        let dir=f.root.clone();drop(broker);drop(f);fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    #[ignore="real FFprobe child must reject content not valid as media"]
    fn actual_ffprobe_invalid_media_cannot_become_an_applied_receipt(){
        let f=Fixture::new(true,false);
        let broker=f.broker();
        f.invoke(&broker,true).unwrap();
        assert_eq!(f.complete(&broker),TaskState::Failed);
        let effect=f.effects.get(&execute().call_id).unwrap().unwrap();
        assert_eq!(effect.state,EffectState::Uncertain);
        assert!(effect.receipt.is_none());
        let dir=f.root.clone();drop(broker);drop(f);fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    #[ignore="real FFprobe reviewed media hash must be rechecked before spawn"]
    fn changed_media_is_denied_before_journal_and_cannot_autoreplay(){
        let f=Fixture::new(true,true);
        fs::write(&f.file,wav_one_second().into_iter()
            .chain([1u8]).collect::<Vec<_>>()).unwrap();
        let broker=f.broker();
        assert_eq!(f.invoke(&broker,true),Err(MediaProbeError::UnauthorizedOrChanged));
        assert!(f.effects.get(&execute().call_id).unwrap().is_none());
        let dir=f.root.clone();drop(broker);drop(f);fs::remove_dir_all(dir).unwrap();
    }
}
