//! Host-owned native worker process lifecycle for TOOLS-11.
//! This is deliberately NOT a model-facing "run executable" tool. The Lu host
//! chooses a reviewed executable digest, argv, workspace and task identity;
//! untrusted model output never becomes a command, PID or process-kill target.
use std::{
    collections::BTreeMap,ffi::OsString,fs,io::{self,Read,BufReader},path::{Path,PathBuf},
    process::{Child,Command,Stdio},sync::{Arc,Mutex},
    time::{Duration,Instant},
};
use sha2::{Digest,Sha256};
use crate::tasks::{TaskState,TaskStore};

#[derive(Debug,Clone,PartialEq,Eq)]
pub enum BackgroundProcessError{
    InvalidPlan,AlreadyOwned,NotOwned,WrongWorkspace,SpawnFailed,
    ProcessUncertain,StoreUnavailable,Poisoned,
}
pub struct TrustedWorkerPlan {
    pub task_id:String,
    pub workspace_id:String,
    pub executable:PathBuf,
    pub expected_sha256:String,
    pub args:Vec<OsString>,
    /// Only reviewed workers with an authenticated progress contract may
    /// request stdout IPC; arbitrary subprocess output is discarded.
    pub progress_protocol:bool,
    /// Explicit host deadline, not a model generation or heartbeat timeout.
    /// None means there is no overall task deadline.
    pub max_runtime_seconds:Option<u64>,
}
struct OwnedWorker {
    workspace_id:String,
    child:Child,
    progress_reader:Option<std::thread::JoinHandle<Option<ScanProof>>>,
    version_reader:Option<std::thread::JoinHandle<Option<CapturedVersionOutput>>>,
    deadline:Option<Instant>,
    #[cfg(windows)]
    job:Option<lu_win_worker_job::Job>,
    #[cfg(windows)]
    job_empty:Option<lu_win_worker_job::WindowsJobEmptyProof>,
    #[cfg(target_os="linux")]
    guardian:Option<lu_win_worker_job::unix_owned::LinuxOwnedGroupGuardian>,
    #[cfg(target_os="linux")]
    cgroup:Option<lu_win_worker_job::unix_owned::LinuxCgroupCustodian>,
}
pub struct BackgroundProcessSupervisor {
    store:Arc<TaskStore>,
    /// Host-owned profile only; never WebView/model supplied. None preserves
    /// legacy lower-assurance PGID operation with no positive OS custody.
    profile:Option<PathBuf>,
    owned:Mutex<BTreeMap<String,OwnedWorker>>,
    // In-memory, one-use private WGC pixel transport. Never TaskStore,
    // WebView, disk, network or model tool output. Drop scrubs raw BGRA.
    observe_frames:Mutex<BTreeMap<String,(String,CapturedBgra)>>,
    // Host-owned stdout pipe custody for reviewed DOCX/XLSX workers. Never
    // exposes worker-chosen paths, raw bytes in SQLite or a model API.
    output_artifacts:Mutex<BTreeMap<String,(String,CapturedArtifact)>>,
    // E1-P06B: one-use, separately bounded stdout from a fixed, reviewed
    // external version command. Never model-selected CLI or generic stdio.
    version_outputs:Mutex<BTreeMap<String,(String,CapturedVersionOutput)>>,
}
fn valid_id(s:&str)->bool{
    !s.is_empty()&&s.len()<=128
       &&s.bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'-'|b'_'))
}
fn verified_executable(path:&Path,digest:&str)->Result<PathBuf,BackgroundProcessError>{
    if !path.is_absolute()||digest.len()!=64||
       !digest.bytes().all(|b|b.is_ascii_hexdigit()){
        return Err(BackgroundProcessError::InvalidPlan);
    }
    let meta=fs::symlink_metadata(path).map_err(|_|BackgroundProcessError::InvalidPlan)?;
    if !meta.file_type().is_file()||meta.len()==0{
        return Err(BackgroundProcessError::InvalidPlan);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes()&0x400!=0{
            return Err(BackgroundProcessError::InvalidPlan);
        }
    }
    let canonical=path.canonicalize().map_err(|_|BackgroundProcessError::InvalidPlan)?;
    let mut file=fs::File::open(&canonical).map_err(|_|BackgroundProcessError::InvalidPlan)?;
    let mut sha=Sha256::new();
    let mut buf=[0u8;65536];
    loop{
        let n=file.read(&mut buf).map_err(|_|BackgroundProcessError::InvalidPlan)?;
        if n==0{break;}
        sha.update(&buf[..n]);
    }
    if format!("{:x}",sha.finalize())!=digest.to_ascii_lowercase(){
        return Err(BackgroundProcessError::InvalidPlan);
    }
    Ok(canonical)
}
/// E1-P06E single local media input. This is *not* permission to access any
/// host path: the caller must independently authorize the original user
/// turn and host-enroll this exact file digest. An opened-handle OS directory
/// broker is a separate future stronger TOCTOU boundary.
pub(crate) fn verified_media_input(
    path:&Path,expected_sha256:&str
)->Result<u64,BackgroundProcessError>{
    if !path.is_absolute()||
        path.components().any(|component|matches!(component,
            std::path::Component::CurDir|std::path::Component::ParentDir))
        ||expected_sha256.len()!=64
        ||!expected_sha256.bytes().all(|b|b.is_ascii_hexdigit()){
        return Err(BackgroundProcessError::InvalidPlan)
    }
    for part in path.ancestors(){
        let meta=fs::symlink_metadata(part)
            .map_err(|_|BackgroundProcessError::InvalidPlan)?;
        if meta.file_type().is_symlink(){
            return Err(BackgroundProcessError::InvalidPlan)
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if meta.file_attributes()&0x400!=0{
                return Err(BackgroundProcessError::InvalidPlan)
            }
        }
    }
    let meta=fs::metadata(path).map_err(|_|BackgroundProcessError::InvalidPlan)?;
    if !meta.is_file()||meta.len()==0||meta.len()>256*1024*1024{
        return Err(BackgroundProcessError::InvalidPlan)
    }
    let mut input=fs::File::open(path)
        .map_err(|_|BackgroundProcessError::InvalidPlan)?;
    let mut h=Sha256::new();
    let mut count=0u64;
    let mut buf=[0u8;64*1024];
    loop{
        let n=input.read(&mut buf).map_err(|_|BackgroundProcessError::InvalidPlan)?;
        if n==0{break}
        count=count.checked_add(n as u64).ok_or(BackgroundProcessError::InvalidPlan)?;
        if count>256*1024*1024{
            return Err(BackgroundProcessError::InvalidPlan)
        }
        h.update(&buf[..n]);
    }
    if count!=meta.len()
        ||format!("{:x}",h.finalize())!=expected_sha256.to_ascii_lowercase(){
        return Err(BackgroundProcessError::InvalidPlan)
    }
    Ok(count)
}

/// Worker progress stream is a deliberately tiny, bounded native protocol.
/// The caller has already pinned the executable hash, so this is a receipt,
/// never an authorization grant or a model-provided status claim.
/// A bounded BGRA frame admitted only through the OS-identified, separately
/// user-consented local WGC image mode. No generic picker source is eligible.
pub struct CapturedBgra{
    pub width:u32,pub height:u32,pub row_pitch:u32,pub bytes:Vec<u8>
}
impl Drop for CapturedBgra{
    fn drop(&mut self){self.bytes.fill(0);}
}
/// A finite OS-identified child stdout payload. Not automatically a
/// verified *meaningful* document; independent P04 parsers decide that.
pub struct CapturedArtifact{
    pub format:String,
    pub bytes:Vec<u8>,
    pub sha256:String,
}
impl Drop for CapturedArtifact{
    fn drop(&mut self){self.bytes.fill(0);}
}
/// E1-P06B: actual OS pipe output. Untrusted until the host version
/// parser independently validates the expected application's exact format.
pub struct CapturedVersionOutput{
    pub bytes:Vec<u8>,
    pub sha256:String,
}
impl Drop for CapturedVersionOutput{
    fn drop(&mut self){self.bytes.fill(0);}
}
fn read_bounded_version_stdout(
    pipe:impl Read,limit:usize,empty_is_valid:bool
)->Option<CapturedVersionOutput>{
    if !(1..=16_384).contains(&limit){return None}
    let mut pipe=pipe;
    let mut bytes=Vec::with_capacity(limit.min(256));
    // Read at most limit+1 bytes. If the child produces more, reject and
    // drop its pipe instead of storing arbitrary unbounded process output.
    if pipe.take(limit as u64+1).read_to_end(&mut bytes).is_err()
        ||(!empty_is_valid&&bytes.is_empty())||bytes.len()>limit{
        bytes.fill(0);
        return None
    }
    let sha256=format!("{:x}",Sha256::digest(&bytes));
    Some(CapturedVersionOutput{bytes,sha256})
}
struct ScanProof{
    sha256:String,bytes:u64,frame:Option<CapturedBgra>,
    artifact:Option<CapturedArtifact>,
}
fn read_worker_progress(
    pipe:impl Read,store:&TaskStore,task_id:&str,
    expected_image_source:Option<&'static str>,
    expected_artifact_kind:Option<&'static str>,
)->Option<ScanProof>{
    let mut input=BufReader::new(pipe);
    let mut line=Vec::with_capacity(96);
    let mut final_result=None;
    let mut total_seen=None::<u64>;
    let mut last_bytes=0u64;
    loop{
        line.clear();
        // Limit each line before allocating an attacker-sized buffer.
        for _ in 0..128{
            let mut byte=[0u8;1];
            match input.read(&mut byte){
                Ok(0)=>return if !line.is_empty()
                    ||(expected_image_source.is_some()
                        &&final_result.as_ref().is_some_and(|p:&ScanProof|p.frame.is_none()))
                    ||(expected_artifact_kind.is_some()
                        &&final_result.as_ref().is_some_and(|p:&ScanProof|p.artifact.is_none())){
                    // A partial trailing line after a complete binary frame
                    // is never a valid extension of the reviewed protocol.
                    None
                }else{final_result},
                Err(_)=>return None,
                Ok(_)=>{
                    if byte[0]==b'\n'{break;}
                    line.push(byte[0]);
                }
            }
        }
        if line.len()>=128{return None;}
        let Ok(text)=std::str::from_utf8(&line) else{return None};
        if let Some(value)=text.strip_prefix("P\t"){
            if final_result.is_some(){return None;}
            let Some((read,total))=value.split_once('\t') else{return None};
            let (Ok(read),Ok(total))=(read.parse::<u64>(),total.parse::<u64>())
                else{return None};
            if total==0||read>total||read<last_bytes
                ||total_seen.is_some_and(|seen|seen!=total){return None;}
            last_bytes=read;
            total_seen=Some(total);
            let percent=((u128::from(read)*99)/u128::from(total)) as u8;
            if store.checkpoint(task_id,percent,
                &format!("Verified {} of {} bytes",read,total),&[]).is_err(){
                return None;
            }
        }else if let Some(digest)=text.strip_prefix("D\t"){
            if final_result.is_some()||total_seen!=Some(last_bytes)
                ||last_bytes==0||digest.len()!=64
                ||!digest.bytes().all(|b|b.is_ascii_hexdigit()
                    &&!b.is_ascii_uppercase()){
                return None;
            }
            if store.checkpoint(task_id,100,&format!("SHA-256: {digest}"),&[])
                .is_err(){return None;}
            final_result=Some(ScanProof{
                sha256:digest.into(),bytes:last_bytes,frame:None,
                artifact:None,
            });
        }else if let Some(meta)=text.strip_prefix("A\t"){
            // A single bounded artifact follows an independently checked
            // P/D progress receipt. ONLY an exact host-reviewed one-shot
            // document mode may send this protocol extension.
            let kind=expected_artifact_kind?;
            if final_result.as_ref().is_none_or(|p|p.artifact.is_some())
                ||expected_image_source.is_some(){return None}
            let fields:Vec<_>=meta.split('\t').collect();
            if fields.len()!=3||fields[0]!=kind{return None}
            let size=fields[1].parse::<usize>().ok()?;
            if !(16..=1024*1024).contains(&size){return None}
            let sha=fields[2];
            if sha.len()!=64||!sha.bytes().all(|b|
                b.is_ascii_hexdigit()&&!b.is_ascii_uppercase()){return None}
            let mut bytes=vec![0u8;size];
            if input.read_exact(&mut bytes).is_err(){bytes.fill(0);return None}
            if format!("{:x}",Sha256::digest(&bytes))!=sha{
                bytes.fill(0);
                return None
            }
            final_result.as_mut()?.artifact=Some(CapturedArtifact{
                format:kind.into(),bytes,sha256:sha.into()
            });
        }else if let Some(meta)=text.strip_prefix("B\t"){
            // A separate WGC OS-reviewed image mode may append ONE bounded
            // binary BGRA payload after the ordinary fully-blanked receipt.
            // This never calls TaskStore.checkpoint with raw frame bytes.
            if expected_image_source.is_none()
                ||final_result.as_ref().is_none_or(|v|v.frame.is_some()){
                return None;
            }
            let fields:Vec<_>=meta.split('\t').collect();
            if fields.len()!=4 ||Some(fields[0])!=expected_image_source{return None;}
            let width=fields[1].parse::<u32>().ok()?;
            let height=fields[2].parse::<u32>().ok()?;
            let raw_sha=fields[3];
            if width==0||height==0||width>1280||height>720||
                raw_sha.len()!=64||!raw_sha.bytes().all(|b|
                    b.is_ascii_hexdigit()&&!b.is_ascii_uppercase()){
                return None;
            }
            let pitch=width.checked_mul(4)?;
            let size=(pitch as usize).checked_mul(height as usize)?;
            if size>1280*720*4{return None;}
            let mut pixels=vec![0u8;size];
            if input.read_exact(&mut pixels).is_err(){
                pixels.fill(0);
                return None;
            }
            let actual=format!("{:x}",Sha256::digest(&pixels));
            if actual!=raw_sha{
                pixels.fill(0);
                return None;
            }
            final_result.as_mut()?.frame=Some(CapturedBgra{
                width,height,row_pitch:pitch,bytes:pixels,
            });
        }else{return None;}
    }
}

impl BackgroundProcessSupervisor {
    pub fn new(store:Arc<TaskStore>)->Self{
        Self{
            store,profile:None,owned:Mutex::new(BTreeMap::new()),
            observe_frames:Mutex::new(BTreeMap::new()),
            output_artifacts:Mutex::new(BTreeMap::new()),
            version_outputs:Mutex::new(BTreeMap::new()),
        }
    }
    /// Reviewed app execution captures the exact authenticated host profile
    /// for the independent crash-surviving cgroup custodian's receipt.
    /// The profile is selected only by the native Lu host, never by the model.
    pub fn new_for_reviewed_host(store:Arc<TaskStore>,profile:PathBuf)->Self{
        let mut supervisor=Self::new(store);
        supervisor.profile=Some(profile);
        supervisor
    }
    /// Host-only admission. The caller must obtain the user's task grant
    /// independently and build this plan from a trusted executable catalog.
    /// No Tauri command or ModelToolCatalog invokes this API directly.
    pub fn start_host_approved(&self,plan:TrustedWorkerPlan)
        ->Result<(),BackgroundProcessError>{
        self.start_host_approved_internal(plan,None,false)
    }
    /// Host-only external VERSION-ONLY observation. No generic executable
    /// stdout transport is available to tools or the WebView.
    pub(crate) fn start_host_approved_version(
        &self,plan:TrustedWorkerPlan,output_limit:usize
    )->Result<(),BackgroundProcessError>{
        if !(1..=4096).contains(&output_limit)
            ||plan.progress_protocol
            ||plan.max_runtime_seconds.is_none_or(|n|!(1..=15).contains(&n)){
            return Err(BackgroundProcessError::InvalidPlan)
        }
        // Production accepts only the host-compiled version flag, with no
        // model-selected options or script evaluation.
        let version_only=plan.args.len()==1
            &&matches!(plan.args[0].to_str(),Some("--version"|"-version"));
        #[cfg(test)]
        let version_only=version_only||(
            plan.args.len()==4
            &&plan.args[0].to_str()==Some("-I")
            &&plan.args[1].to_str()==Some("-S")
            &&plan.args[2].to_str()==Some("-c")
            &&matches!(plan.args[3].to_str(),
                Some("import time;time.sleep(20);print('Python 3.13.0')"
                    |"print('Python '+('9'*8192))"))
        );
        if !version_only{return Err(BackgroundProcessError::InvalidPlan)}
        self.start_host_approved_internal(plan,Some((output_limit,false)),false)
    }
    /// Host-owned, bounded read-only Git operations; this does NOT sandbox
    /// Lu and it is NOT a generic CLI bridge. The repository root is provided
    /// only by the trusted host broker and rechecked immediately before spawn.
    pub(crate) fn start_host_approved_git_read(
        &self,plan:TrustedWorkerPlan,output_limit:usize
    )->Result<(),BackgroundProcessError>{
        if !(1..=16_384).contains(&output_limit)
            ||plan.progress_protocol
            ||plan.max_runtime_seconds.is_none_or(|n|!(1..=30).contains(&n))
            ||plan.args.len()!=8
            ||plan.args[0].to_str()!=Some("-c")
            ||plan.args[1].to_str()!=Some("core.fsmonitor=false")
            ||plan.args[2].to_str()!=Some("--no-optional-locks")
            ||plan.args[3].to_str()!=Some("-C"){
            return Err(BackgroundProcessError::InvalidPlan)
        }
        let repo=Path::new(&plan.args[4]);
        if !repo.is_absolute()||repo.components().any(|part|
            matches!(part,std::path::Component::CurDir
                |std::path::Component::ParentDir)){
            return Err(BackgroundProcessError::InvalidPlan)
        }
        if !fs::canonicalize(repo)
            .map_err(|_|BackgroundProcessError::InvalidPlan)?.is_dir(){
            return Err(BackgroundProcessError::InvalidPlan)
        }
        // Refuse symlinks/junctions at any ancestor or .git boundary; the
        // reviewed source root is NOT a path selected by the model.
        for ancestor in repo.ancestors(){
            let meta=fs::symlink_metadata(ancestor)
                .map_err(|_|BackgroundProcessError::InvalidPlan)?;
            if meta.file_type().is_symlink(){return Err(BackgroundProcessError::InvalidPlan)}
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if meta.file_attributes()&0x400!=0{
                    return Err(BackgroundProcessError::InvalidPlan)
                }
            }
        }
        let dotgit=repo.join(".git");
        let meta=fs::symlink_metadata(&dotgit)
            .map_err(|_|BackgroundProcessError::InvalidPlan)?;
        if !meta.is_dir()||meta.file_type().is_symlink(){
            return Err(BackgroundProcessError::InvalidPlan)
        }
        let is_head=plan.args[5].to_str()==Some("rev-parse")
            &&plan.args[6].to_str()==Some("--verify")
            &&plan.args[7].to_str()==Some("HEAD");
        let is_status=plan.args[5].to_str()==Some("status")
            &&plan.args[6].to_str()==Some("--porcelain=v1")
            &&plan.args[7].to_str()==Some("--untracked-files=no");
        let is_branch=plan.args[5].to_str()==Some("branch")
            &&plan.args[6].to_str()==Some("--show-current")
            &&plan.args[7].to_str()==Some("--no-color");
        let is_count=plan.args[5].to_str()==Some("rev-list")
            &&plan.args[6].to_str()==Some("--count")
            &&plan.args[7].to_str()==Some("HEAD");
        if !is_head&&!is_status&&!is_branch&&!is_count{
            return Err(BackgroundProcessError::InvalidPlan)
        }
        self.start_host_approved_internal(plan,Some((output_limit,is_status||is_branch)),false)
    }

    /// E1-P06E: host-owned read-only FFprobe *single-file* inspection. This
    /// gates the exact compiled CLI shape; it does not offer a generic
    /// subprocess, a model-selected file path or sandbox native Lu Core.
    pub(crate) fn start_host_approved_media_probe(
        &self,plan:TrustedWorkerPlan,output_limit:usize,
        reviewed_media_sha256:&str
    )->Result<(),BackgroundProcessError>{
        if !(1..=16_384).contains(&output_limit)
            ||plan.progress_protocol
            ||plan.max_runtime_seconds.is_none_or(|n|!(1..=30).contains(&n))
            ||plan.args.len()!=12
            ||plan.args[0].to_str()!=Some("-v")
            ||plan.args[1].to_str()!=Some("error")
            ||plan.args[2].to_str()!=Some("-protocol_whitelist")
            ||plan.args[3].to_str()!=Some("file")
            ||plan.args[4].to_str()!=Some("-f")
            ||!matches!(plan.args[5].to_str(),Some("wav"|"mov"))
            ||plan.args[6].to_str()!=Some("-show_entries")
            ||plan.args[7].to_str()!=Some(
                "format=duration:stream=index,codec_type,codec_name,width,height,sample_rate,channels")
            ||plan.args[8].to_str()!=Some("-of")
            ||plan.args[9].to_str()!=Some("json")
            ||plan.args[10].to_str()!=Some("-i"){
            return Err(BackgroundProcessError::InvalidPlan)
        }
        // Check at the actual supervisor admission boundary, not just
        // during catalog enrollment or the separate permission preflight.
        let input=Path::new(&plan.args[11]);
        let extension=input.extension().and_then(|s|s.to_str());
        let correct_extension=match plan.args[5].to_str(){
            Some("wav")=>extension.is_some_and(|s|s.eq_ignore_ascii_case("wav")),
            Some("mov")=>extension.is_some_and(|s|s.eq_ignore_ascii_case("mp4")),
            _=>false
        };
        if !correct_extension{return Err(BackgroundProcessError::InvalidPlan)}
        let observed=verified_media_input(input,reviewed_media_sha256)?;
        if observed==0{return Err(BackgroundProcessError::InvalidPlan)}
        self.start_host_approved_internal(plan,Some((output_limit,false)),true)
    }
    /// P0-TOOLS-04: finite, host-only PDFInfo and ExifTool read operations.
    /// These exact argument shapes are compiled in Lu; no Tauri/model input
    /// can add flags, a different program, scripts, output paths or URLs.
    /// Executable SHA and original human ExecuteCode+ReadWorkspace authority
    /// are still separately enforced by the broker and OS supervisor.
    pub(crate) fn start_host_approved_specialist_probe(
        &self,plan:TrustedWorkerPlan,limit:usize,
        original_file_sha256:&str,kind:&str
    )->Result<(),BackgroundProcessError>{
        if !(1..=8192).contains(&limit)||plan.progress_protocol||
            plan.max_runtime_seconds.is_none_or(|t|!(1..=30).contains(&t)){
            return Err(BackgroundProcessError::InvalidPlan)
        }
        let file=match kind{
            "pdfinfo" if plan.args.len()==2 &&
                plan.args[0].to_str()==Some("-isodates")=>
                    Path::new(&plan.args[1]),
            "exiftool" if plan.args.len()==8 &&
                plan.args[0].to_str()==Some("-j") &&
                plan.args[1].to_str()==Some("-n") &&
                plan.args[2].to_str()==Some("-ImageWidth") &&
                plan.args[3].to_str()==Some("-ImageHeight") &&
                plan.args[4].to_str()==Some("-FileType") &&
                plan.args[5].to_str()==Some("-FileSize#") &&
                plan.args[6].to_str()==Some("--")=>
                    Path::new(&plan.args[7]),
            _=>return Err(BackgroundProcessError::InvalidPlan),
        };
        let name=plan.executable.file_name().and_then(|x|x.to_str())
            .ok_or(BackgroundProcessError::InvalidPlan)?
            .to_ascii_lowercase();
        let accepted_binary=match kind{
            "pdfinfo"=>matches!(name.as_str(),"pdfinfo"|"pdfinfo.exe"),
            "exiftool"=>matches!(name.as_str(),"exiftool"|"exiftool.exe"),
            _=>false
        };
        let ext=file.extension().and_then(|e|e.to_str()).unwrap_or("");
        let accepted_input=match kind{
            "pdfinfo"=>ext.eq_ignore_ascii_case("pdf"),
            "exiftool"=>matches!(ext.to_ascii_lowercase().as_str(),
                "png"|"jpg"|"jpeg"),
            _=>false
        };
        if !accepted_binary||!accepted_input{
            return Err(BackgroundProcessError::InvalidPlan)
        }
        let length=verified_media_input(file,original_file_sha256)?;
        if length>32*1024*1024{return Err(BackgroundProcessError::InvalidPlan)}
        self.start_host_approved_internal(plan,Some((limit,false)),true)
    }
    fn start_host_approved_internal(
        &self,plan:TrustedWorkerPlan,typed_capture:Option<(usize,bool)>,
        _linux_media_probe:bool
    )->Result<(),BackgroundProcessError>{
        if !valid_id(&plan.task_id)||!valid_id(&plan.workspace_id)
          ||plan.max_runtime_seconds.is_some_and(|n|n==0||n>86_400)
          ||plan.args.len()>64||plan.args.iter().any(|arg|
               arg.to_string_lossy().len()>4096
               ||arg.to_string_lossy().contains('\0')){
            return Err(BackgroundProcessError::InvalidPlan);
        }
        let expected_image_source=match plan.args.first().and_then(|arg|arg.to_str()){
            Some("--lu-observe-foreground-local-image-one-shot")=>
                Some("native-foreground-window-v1"),
            Some("--lu-observe-primary-monitor-local-image-one-shot")=>
                Some("native-primary-monitor-v1"),
            _=>None,
        };
        if expected_image_source.is_some()&&(!plan.progress_protocol||plan.args.len()!=2
            ||plan.max_runtime_seconds!=Some(90)){
            return Err(BackgroundProcessError::InvalidPlan);
        }
        let expected_artifact_kind=if plan.args.first().and_then(|arg|arg.to_str())
             ==Some("--lu-e1-artifact-one-shot"){
            if !plan.progress_protocol||plan.args.len()!=2
              ||plan.max_runtime_seconds!=Some(15){
                return Err(BackgroundProcessError::InvalidPlan)
            }
            match plan.args[1].to_str(){
                Some("docx")=>Some("docx"),
                Some("xlsx")=>Some("xlsx"),
                _=>return Err(BackgroundProcessError::InvalidPlan),
            }
        }else{None};
        let executable=verified_executable(&plan.executable,&plan.expected_sha256)?;
        let mut workers=self.owned.lock().map_err(|_|BackgroundProcessError::Poisoned)?;
        if workers.contains_key(&plan.task_id){
            return Err(BackgroundProcessError::AlreadyOwned);
        }
        // Reject an invalid reviewed origin before creating durable task state.
        #[cfg(target_os="linux")]
        let reviewed_origin=if let Some(profile)=self.profile.as_deref().filter(|_|typed_capture.is_some()){
            let (chat,turn)=self.store.reviewed_human_origin()
                .map_err(|_|BackgroundProcessError::StoreUnavailable)?
                .ok_or(BackgroundProcessError::InvalidPlan)?;
            if turn!=plan.workspace_id{return Err(BackgroundProcessError::WrongWorkspace)}
            Some((profile,chat,turn))
        }else{None};
        self.store.create(&plan.task_id,Some(&plan.workspace_id),None)
            .map_err(|_|BackgroundProcessError::StoreUnavailable)?;
        // Prefer a delegated kernel cgroup subtree for reviewed typed tools.
        // The independent helper lives outside it and kills the entire
        // subtree on EOF even if Lu is SIGKILLed. When OS delegation is
        // unavailable the reviewed tool may retain its existing PGID
        // guardian, but AGENT-01K/01L REFUSE full descendant custody.
        #[cfg(target_os="linux")]
        let mut cgroup=if let Some((profile,chat,turn))=reviewed_origin{
            lu_win_worker_job::unix_owned::LinuxCgroupCustodian::spawn_if_delegated(
                profile,&chat,&turn,&plan.task_id
            ).map_err(|_|{
                let _=self.store.fail_pending(&plan.task_id);
                BackgroundProcessError::SpawnFailed
            })?
        }else{None};
        #[cfg(target_os="linux")]
        let guardian=if typed_capture.is_some()&&cgroup.is_none(){
            lu_win_worker_job::unix_owned::LinuxOwnedGroupGuardian::spawn_if_available()
                .map_err(|_|{
                    let _=self.store.fail_pending(&plan.task_id);
                    BackgroundProcessError::SpawnFailed
                })?
        }else{None};
        let mut command=Command::new(&executable);
        command.args(&plan.args).env_clear().stdin(Stdio::null())
            .stdout(if plan.progress_protocol||typed_capture.is_some(){
                Stdio::piped()
            }else{Stdio::null()})
            .stderr(Stdio::null())
            .current_dir(executable.parent().ok_or(BackgroundProcessError::InvalidPlan)?);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000 | 0x0000_0004);
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            #[cfg(target_os="linux")]
            command.process_group(guardian.as_ref()
                .map_or(0,|w|w.group_id()));
            #[cfg(not(target_os="linux"))]
            command.process_group(0);
        }
        #[cfg(target_os="linux")]
        {
            if let Some(ref guard)=cgroup{
                lu_win_worker_job::unix_owned::bind_worker_to_cgroup_before_exec(
                    &mut command,guard
                ).map_err(|_|{
                    let _=self.store.fail_pending(&plan.task_id);
                    BackgroundProcessError::SpawnFailed
                })?;
            }
            let quota=match plan.args.first().and_then(|s|s.to_str()){
                Some("--lu-file-scan-worker")=>Some(1024*1024*1024u64),
                Some("--lu-e1-version-one-shot")|
                Some("--lu-e1-artifact-one-shot")=>Some(256*1024*1024u64),
                // Linux RLIMIT_AS includes mapped shared libraries. The
                // reviewed FFprobe path needs room for its codec libraries.
                _ if _linux_media_probe=>Some(512*1024*1024u64),
                _ if typed_capture.is_some()=>Some(256*1024*1024u64),
                _=>None
            };
            lu_win_worker_job::unix_owned::bind_owned_worker_pre_exec(
                &mut command,std::process::id(),quota,
            ).map_err(|_|{
                let _=self.store.fail_pending(&plan.task_id);
                BackgroundProcessError::SpawnFailed
            })?;
        }
        #[cfg(windows)]
        let job=lu_win_worker_job::Job::new().map_err(|_|{
            let _=self.store.fail_pending(&plan.task_id);
            BackgroundProcessError::SpawnFailed
        })?;
        #[cfg(windows)]
        if plan.args.first().and_then(|s|s.to_str())
            .is_some_and(|mode|matches!(mode,
                "--lu-archive-extract-worker"|"--lu-archive-preview-worker"))
            &&job.set_memory_limit(1024*1024*1024).is_err(){
            let _=self.store.fail_pending(&plan.task_id);
            return Err(BackgroundProcessError::SpawnFailed)
        }
        #[cfg(windows)]
        if plan.args.first().and_then(|s|s.to_str())==
            Some("--lu-file-scan-worker")
            &&job.set_memory_limit(512*1024*1024).is_err(){
            let _=self.store.fail_pending(&plan.task_id);
            return Err(BackgroundProcessError::SpawnFailed)
        }
        // E1-P03: a strictly one-shot reviewed version worker receives a
        // hard Windows Job Object memory ceiling. This does not make
        // unreviewed third-party executables, network or file I/O safe.
        #[cfg(windows)]
        if (matches!(plan.args.first().and_then(|s|s.to_str()),
            Some("--lu-e1-version-one-shot"|"--lu-e1-artifact-one-shot"))
            ||typed_capture.is_some())
            &&job.set_memory_limit(256*1024*1024).is_err(){
            let _=self.store.fail_pending(&plan.task_id);
            return Err(BackgroundProcessError::SpawnFailed)
        }
        // P06: admission alone is not an OS memory ceiling. Both approved
        // one-shot Windows.Graphics.Capture modes now run under the existing
        // Windows Job Object 512 MiB private-memory limit. GPU/driver VRAM
        // is separately unmeasured and never claimed as OS-enforced.
        #[cfg(windows)]
        if plan.args.first().and_then(|s|s.to_str()).is_some_and(|mode|
            matches!(mode,
                "--lu-observe-one-shot"|"--lu-observe-foreground-one-shot"
                |"--lu-observe-primary-monitor-one-shot"
                |"--lu-observe-foreground-local-image-one-shot"
                |"--lu-observe-primary-monitor-local-image-one-shot"))
            &&job.set_memory_limit(512*1024*1024).is_err(){
            let _=self.store.fail_pending(&plan.task_id);
            return Err(BackgroundProcessError::SpawnFailed)
        }
        let mut child=command.spawn().map_err(|_|{
            let _=self.store.fail_pending(&plan.task_id);
            BackgroundProcessError::SpawnFailed
        })?;
        #[cfg(windows)]
        if job.assign_and_resume_suspended(&child).is_err(){
            let _=child.kill();
            let _=child.wait();
            let _=self.store.fail_pending(&plan.task_id);
            return Err(BackgroundProcessError::SpawnFailed);
        }
        // The pre_exec kernel write is necessary but not sufficient proof:
        // independently read /proc for this exact still-owned Child, and
        // verify the helper remains alive outside the killable subtree.
        // No task is marked RUNNING when the guardian died or membership is
        // missing. This can conservatively reject an extremely fast worker.
        #[cfg(target_os="linux")]
        if let Some(guard)=cgroup.as_mut(){
            if guard.verify_admitted_worker(child.id()).is_err(){
                // The worker has executed; its side effects are uncertain.
                // Persist this original task's uncertainty BEFORE returning
                // even though the worker is still in PENDING transition.
                let _=child.kill();let _=child.wait();
                let recorded=self.store.record_process_uncertain(
                    &plan.task_id,"OS_CUSTODY_UNVERIFIED");
                let _=guard.stop_and_verify();
                // Do not mask a failed durable uncertainty write as an
                // ordinary failed admission after worker execution began.
                recorded.map_err(|_|BackgroundProcessError::StoreUnavailable)?;
                self.store.fail_pending(&plan.task_id)
                    .map_err(|_|BackgroundProcessError::StoreUnavailable)?;
                return Err(BackgroundProcessError::SpawnFailed)
            }
        }
        // Delay progress consumption until the SQLite RUNNING transition.
        // A fast worker must not lose its first checkpoint to a startup race.
        let progress_pipe=if plan.progress_protocol{child.stdout.take()}else{None};
        let version_pipe=if typed_capture.is_some(){child.stdout.take()}else{None};
        let owner=OwnedWorker{
            workspace_id:plan.workspace_id,
            child,
            progress_reader:None,
            version_reader:None,
            deadline:plan.max_runtime_seconds.map(|n|Instant::now()+Duration::from_secs(n)),
            #[cfg(windows)]
            job:Some(job),
            #[cfg(windows)]
            job_empty:None,
            #[cfg(target_os="linux")]
            guardian,
            #[cfg(target_os="linux")]
            cgroup,
        };
        workers.insert(plan.task_id.clone(),owner);
        if self.store.start(&plan.task_id).is_err(){
            let mut owner=workers.remove(&plan.task_id).expect("new worker registered");
            let _=terminate_owned(&mut owner);
            let _=self.store.fail_pending(&plan.task_id);
            return Err(BackgroundProcessError::StoreUnavailable);
        }
        if let (Some(pipe),Some((limit,empty_is_valid)))=(version_pipe,typed_capture){
            let reader=std::thread::spawn(move||
                read_bounded_version_stdout(pipe,limit,empty_is_valid)
            );
            workers.get_mut(&plan.task_id)
                .expect("just registered running version child")
                .version_reader=Some(reader);
        }
        if let Some(pipe)=progress_pipe{
            let store=Arc::clone(&self.store);
            let task_id=plan.task_id.clone();
            let reader=std::thread::spawn(move||
                read_worker_progress(
                    pipe,&store,&task_id,expected_image_source,expected_artifact_kind
                )
            );
            workers.get_mut(&plan.task_id)
                .expect("just registered running worker")
                .progress_reader=Some(reader);
        }
        Ok(())
    }
    /// Non-blocking native host observation. No model tokens are consumed.
    pub fn poll(&self)->Result<usize,BackgroundProcessError>{
        let mut workers=self.owned.lock().map_err(|_|BackgroundProcessError::Poisoned)?;
        let mut finished=Vec::new();
        for (id,worker) in workers.iter_mut(){
            #[cfg(target_os="linux")]
            if let Some(guard)=worker.cgroup.as_mut(){
                if guard.require_live_guardian().is_err(){
                    // The worker may still be running, or its descendants
                    // may already have escaped the leader. Kill the known
                    // leader, retain the owned entry, and REFUSE completion
                    // or independent cgroup-custody status.
                    let _=worker.child.kill();
                    let _=worker.child.wait();
                    self.store.record_process_uncertain(
                        id,"LINUX_GUARDIAN_LOST")
                        .map_err(|_|BackgroundProcessError::StoreUnavailable)?;
                    return Err(BackgroundProcessError::ProcessUncertain)
                }
            }
            match worker.child.try_wait(){
                Ok(Some(status))=>{
                    #[cfg(windows)]
                    {
                        // The owner still holds the original Job handle.
                        // Query the KERNEL after terminating any surviving
                        // descendants; a leader exit is not tree emptiness.
                        let job=worker.job.as_ref()
                            .ok_or(BackgroundProcessError::ProcessUncertain)?;
                        worker.job_empty=Some(
                            job.terminate_and_verify_empty()
                                .map_err(|_|BackgroundProcessError::ProcessUncertain)?
                        );
                    }
                    #[cfg(target_os="linux")]
                    {
                        // Never remove a worker from the owner map before
                        // native descendant cleanup has succeeded. Otherwise
                        // a failed cgroup stop would lose the only live
                        // cancellation handle and strand an uncertain effect.
                        if let Some(guard)=worker.cgroup.as_mut(){
                            if guard.stop_and_verify().is_err(){
                                self.store.record_process_uncertain(
                                    id,"OS_CUSTODY_UNVERIFIED")
                                    .map_err(|_|BackgroundProcessError::StoreUnavailable)?;
                                return Err(BackgroundProcessError::ProcessUncertain)
                            }
                        }
                        if let Some(guardian)=worker.guardian.as_mut(){
                            if guardian.stop_and_verify_signal().is_err(){
                                self.store.record_process_uncertain(
                                    id,"OS_CUSTODY_UNVERIFIED")
                                    .map_err(|_|BackgroundProcessError::StoreUnavailable)?;
                                return Err(BackgroundProcessError::ProcessUncertain)
                            }
                        }
                    }
                    finished.push((id.clone(),Some(status.success())));
                },
                Ok(None)=>{
                    if worker.deadline.is_some_and(|deadline|Instant::now()>=deadline){
                        // Host deadline, never a model decision or an inferred
                        // lost heartbeat. Kill and reap before publishing.
                        if terminate_owned(worker).is_err(){
                            self.store.record_process_uncertain(
                                id,"OS_CUSTODY_UNVERIFIED")
                                .map_err(|_|BackgroundProcessError::StoreUnavailable)?;
                            return Err(BackgroundProcessError::ProcessUncertain)
                        }
                        finished.push((id.clone(),None));
                    }
                },
                Err(_)=>return Err(BackgroundProcessError::ProcessUncertain),
            }
        }
        let count=finished.len();
        for (id,outcome) in finished{
            let mut owner=workers.remove(&id).expect("owned worker still registered");
            #[cfg(windows)]
            let empty_job_witness=owner.job_empty.take();
            #[cfg(windows)]
            drop(owner.job.take()); // The kernel empty-job check preceded CloseHandle.
            // Linux kernel/PGID cleanup was established BEFORE removal from
            // the owner map. For deadline outcomes terminate_owned did so.
            // Any failure must leave task ownership for explicit recovery.
            let had_reader=owner.progress_reader.is_some();
            let had_version_reader=owner.version_reader.is_some();
            let version=owner.version_reader.take()
                .and_then(|handle|handle.join().ok()).flatten();
            let mut proof=owner.progress_reader.take().and_then(|handle|handle.join().ok()).flatten();
            let workspace=owner.workspace_id.clone();
            drop(owner); // OS process and inherited stdout are now reaped.
            let result=match outcome{
                Some(true) if had_version_reader&&version.is_some()=>
                    self.store.complete(&id),
                Some(true) if had_version_reader=>self.store.fail(&id),
                Some(true) if !had_reader=>self.store.complete(&id),
                Some(true) if proof.is_some()=>{
                    let p=proof.as_ref().expect("checked verified proof");
                    self.store.complete_scan_with_receipt(&id,&p.sha256,p.bytes)
                },
                Some(_)=>self.store.fail(&id),
                None=>self.store.timed_out(&id),
            };
            result.map_err(|_|BackgroundProcessError::StoreUnavailable)?;
            #[cfg(windows)]
            if matches!(outcome,Some(true))&&had_version_reader&&version.is_some()
                &&["-gh","-gs","-gb","-gc","-fm"]
                    .iter().any(|suffix|id.ends_with(suffix)){
                // Publish the OS witness only AFTER successful task terminal
                // persistence. Any crash or mismatch leaves NO witness, so
                // recovery remains blocked; never fabricate from APPLIED.
                self.store.record_verified_windows_job_empty(
                    &id,&workspace,empty_job_witness
                        .ok_or(BackgroundProcessError::ProcessUncertain)?
                ).map_err(|_|BackgroundProcessError::StoreUnavailable)?;
            }
            if matches!(outcome,Some(true)){
                if let Some(version)=version{
                    let mut owned=self.version_outputs.lock()
                        .map_err(|_|BackgroundProcessError::Poisoned)?;
                    if owned.len()<4{
                        owned.insert(id.clone(),(workspace.clone(),version));
                    }
                }
                if let Some(artifact)=proof.as_mut().and_then(|p|p.artifact.take()){
                    let mut owned=self.output_artifacts.lock()
                        .map_err(|_|BackgroundProcessError::Poisoned)?;
                    if owned.len()<4{
                        owned.insert(id.clone(),(workspace.clone(),artifact));
                    }
                    // Otherwise drop and scrub bytes; no success is claimed
                    // until the authorized broker consumes the private slot.
                }
                if let Some(frame)=proof.as_mut().and_then(|p|p.frame.take()){
                    let mut pending=self.observe_frames.lock()
                        .map_err(|_|BackgroundProcessError::Poisoned)?;
                    if pending.len()<4{
                        pending.insert(id.clone(),(workspace,frame));
                    }
                    // If all four slots are occupied, drop/zero the frame.
                }
            }
        }
        Ok(count)
    }
    /// Exact host-owned one-use version output. The bytes are raw/untrusted;
    /// the caller MUST validate an independently reviewed version parser.
    pub(crate) fn take_local_version_stdout(
        &self,task_id:&str,workspace_id:&str
    )->Result<Option<CapturedVersionOutput>,BackgroundProcessError>{
        let mut owned=self.version_outputs.lock()
            .map_err(|_|BackgroundProcessError::Poisoned)?;
        if owned.get(task_id).is_some_and(|(owner,_)|owner!=workspace_id){
            return Err(BackgroundProcessError::WrongWorkspace)
        }
        Ok(owned.remove(task_id).map(|(_,version)|version))
    }
    /// Exact owner, one-use artifact custody from this supervisor's actual
    /// OS child stdout pipe. The caller must validate semantic document
    /// content, persist into a host-owned staging file, and audit the journal.
    pub fn take_local_output_artifact(
        &self,task_id:&str,workspace_id:&str
    )->Result<Option<CapturedArtifact>,BackgroundProcessError>{
        let mut owned=self.output_artifacts.lock()
            .map_err(|_|BackgroundProcessError::Poisoned)?;
        if owned.get(task_id).is_some_and(|(owner,_)|owner!=workspace_id){
            return Err(BackgroundProcessError::WrongWorkspace)
        }
        Ok(owned.remove(task_id).map(|(_,artifact)|artifact))
    }
    pub fn discard_local_output_artifact(
        &self,task_id:&str,workspace_id:&str
    )->Result<(),BackgroundProcessError>{
        let mut owned=self.output_artifacts.lock()
            .map_err(|_|BackgroundProcessError::Poisoned)?;
        if owned.get(task_id).is_some_and(|(owner,_)|owner!=workspace_id){
            return Err(BackgroundProcessError::WrongWorkspace)
        }
        owned.remove(task_id);
        Ok(())
    }
    /// Host-only, one-use, chat-owned local WGC frame retrieval. The caller
    /// MUST verify active Observe consent, keep the image tainted and admit
    /// its bytes with native_observe_image_admission. No model/tool API has
    /// access to this map.
    pub fn has_local_observe_frame(
        &self,task_id:&str,workspace_id:&str
    )->Result<bool,BackgroundProcessError>{
        let frames=self.observe_frames.lock()
            .map_err(|_|BackgroundProcessError::Poisoned)?;
        Ok(frames.get(task_id).is_some_and(|(owner,_)|owner==workspace_id))
    }
    pub fn take_local_observe_frame(
        &self,task_id:&str,workspace_id:&str
    )->Result<Option<CapturedBgra>,BackgroundProcessError>{
        let mut frames=self.observe_frames.lock()
            .map_err(|_|BackgroundProcessError::Poisoned)?;
        if let Some((owner,_))=frames.get(task_id){
            if owner!=workspace_id{return Err(BackgroundProcessError::WrongWorkspace);}
        }
        Ok(frames.remove(task_id).map(|(_,frame)|frame))
    }
    pub fn discard_local_observe_frame(
        &self,task_id:&str,workspace_id:&str
    )->Result<(),BackgroundProcessError>{
        let mut frames=self.observe_frames.lock()
            .map_err(|_|BackgroundProcessError::Poisoned)?;
        if let Some((owner,_))=frames.get(task_id){
            if owner!=workspace_id{return Err(BackgroundProcessError::WrongWorkspace);}
        }
        frames.remove(task_id);
        Ok(())
    }
    /// A cancel never targets a model-selected PID. It only affects a worker
    /// which remains owned by this manager and matches the host workspace.
    pub fn cancel_owned(&self,task_id:&str,workspace_id:&str)
        ->Result<(),BackgroundProcessError>{
        let mut workers=self.owned.lock().map_err(|_|BackgroundProcessError::Poisoned)?;
        let worker=workers.get_mut(task_id).ok_or(BackgroundProcessError::NotOwned)?;
        if worker.workspace_id!=workspace_id{
            return Err(BackgroundProcessError::WrongWorkspace);
        }
        if terminate_owned(worker).is_err(){
            self.store.record_process_uncertain(
                task_id,"OS_CUSTODY_UNVERIFIED")
                .map_err(|_|BackgroundProcessError::StoreUnavailable)?;
            return Err(BackgroundProcessError::ProcessUncertain)
        }
        let mut owner=workers.remove(task_id).expect("live worker owned");
        if let Some(reader)=owner.progress_reader.take(){let _=reader.join();}
        if let Some(reader)=owner.version_reader.take(){let _=reader.join();}
        drop(owner); // Release the Windows Job handle (kills descendants).
        self.store.cancelled(task_id)
            .map_err(|_|BackgroundProcessError::StoreUnavailable)
    }
    /// Local operator action: derive ownership from the host's active map,
    /// never from a model/WebView-supplied workspace or process identifier.
    pub fn cancel_operator_owned(&self,task_id:&str)
        ->Result<(),BackgroundProcessError>{
        let workspace={
            let workers=self.owned.lock().map_err(|_|BackgroundProcessError::Poisoned)?;
            workers.get(task_id).ok_or(BackgroundProcessError::NotOwned)?
                .workspace_id.clone()
        };
        self.cancel_owned(task_id,&workspace)
    }
    pub fn can_cancel(&self,task_id:&str)->bool{
        self.owned.lock().is_ok_and(|owned|owned.contains_key(task_id))
    }
}
fn terminate_owned(worker:&mut OwnedWorker)->Result<(),BackgroundProcessError>{
    #[cfg(target_os="linux")]
    {
        if let Some(guard)=worker.cgroup.as_mut(){
            guard.stop_and_verify()
                .map_err(|_|BackgroundProcessError::ProcessUncertain)?;
        }else if let Some(guardian)=worker.guardian.as_mut(){
            // The guardian is an independently-running PGID anchor. Closing
            // its Lu-owned pipe makes it SIGKILL its own group. We require
            // the exact kernel SIGKILL termination as an acknowledgement.
            guardian.stop_and_verify_signal()
                .map_err(|_|BackgroundProcessError::ProcessUncertain)?;
        }else{
            lu_win_worker_job::unix_owned::kill_owned_group(&worker.child)
                .map_err(|_|BackgroundProcessError::ProcessUncertain)?;
        }
    }
    #[cfg(all(unix,not(target_os="linux")))]
    {
        lu_win_worker_job::unix_owned::kill_owned_group(&worker.child)
            .map_err(|_|BackgroundProcessError::ProcessUncertain)?;
    }
    #[cfg(windows)]
    {
        // A cancelled Job must also be empty before claiming that the OS
        // stop completed. Cancellation does NOT create a reusable witness.
        worker.job.as_ref()
            .ok_or(BackgroundProcessError::ProcessUncertain)?
            .terminate_and_verify_empty()
            .map_err(|_|BackgroundProcessError::ProcessUncertain)?;
        drop(worker.job.take());
    }
    worker.child.wait().map_err(|_|BackgroundProcessError::ProcessUncertain)?;
    Ok(())
}
impl Drop for BackgroundProcessSupervisor{
    fn drop(&mut self){
        if let Ok(mut workers)=self.owned.lock(){
            for (_,mut worker) in std::mem::take(&mut *workers){
                let _=terminate_owned(&mut worker);
                if let Some(reader)=worker.progress_reader.take(){let _=reader.join();}
                if let Some(reader)=worker.version_reader.take(){let _=reader.join();}
            }
        }
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn media_input_hash_refuses_missing_symlinks_and_changed_bytes(){
        let root=std::env::temp_dir().join(format!(
            "lu-e1-p06e-media-{}-{:?}",std::process::id(),
            std::thread::current().id()
        ));
        fs::create_dir_all(&root).unwrap();
        let path=root.join("sample.wav");
        fs::write(&path,b"fixed tiny sample").unwrap();
        let expected=format!("{:x}",Sha256::digest(b"fixed tiny sample"));
        assert_eq!(verified_media_input(&path,&expected).unwrap(),17);
        fs::write(&path,b"different sample").unwrap();
        assert!(verified_media_input(&path,&expected).is_err());
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let link=root.join("linked.wav");
            symlink(&path,&link).unwrap();
            assert!(verified_media_input(&link,&expected).is_err());
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(target_os="linux")]
    #[test]
    fn mismatched_reviewed_origin_creates_no_pending_worker_task(){
        let store=Arc::new(TaskStore::open(Path::new(":memory:")).unwrap());
        let chat=format!("c-{}","1".repeat(32));
        let turn=format!("g-{}","2".repeat(32));
        store.bind_reviewed_human_turn(&chat,&turn).unwrap();
        let supervisor=BackgroundProcessSupervisor::new_for_reviewed_host(
            Arc::clone(&store),std::env::temp_dir()
        );
        let executable=std::env::current_exe().unwrap();
        let digest=format!("{:x}",Sha256::digest(fs::read(&executable).unwrap()));
        let result=supervisor.start_host_approved_internal(TrustedWorkerPlan{
            task_id:"rejected-origin-gh".into(),workspace_id:"wrong-turn".into(),
            executable,expected_sha256:digest,args:Vec::new(),
            progress_protocol:false,max_runtime_seconds:Some(10),
        },Some((1024,false)),false);
        assert!(matches!(result,Err(BackgroundProcessError::WrongWorkspace)));
        assert!(store.get("rejected-origin-gh").unwrap().is_none());
    }
    #[test]
    fn thirdparty_stdout_capture_is_bounded_and_one_use(){
        use std::io::Cursor;
        let bytes=b"git version 2.47.3\n";
        let out=read_bounded_version_stdout(Cursor::new(bytes),64,false).unwrap();
        assert_eq!(out.bytes,bytes);
        assert_eq!(out.sha256,format!("{:x}",Sha256::digest(bytes)));
        assert!(read_bounded_version_stdout(Cursor::new(bytes),4,false).is_none());
        assert!(read_bounded_version_stdout(Cursor::new(&b""[..]),64,false).is_none());
        let clean=read_bounded_version_stdout(Cursor::new(&b""[..]),64,true).unwrap();
        assert!(clean.bytes.is_empty());
        assert!(read_bounded_version_stdout(Cursor::new(bytes),16_385,false).is_none());
    }
    #[test]
    fn genuine_scanner_receipts_update_sqlite_without_model(){
        use std::io::Cursor;
        let store=TaskStore::open(Path::new(":memory:")).unwrap();
        store.create("media123",Some("media"),None).unwrap();
        store.start("media123").unwrap();
        let digest="a".repeat(64);
        let frames=format!("P\t0\t20\nP\t10\t20\nP\t20\t20\nD\t{digest}\n");
        assert!(read_worker_progress(Cursor::new(frames.into_bytes()),&store,"media123",None,None).is_some());
        let record=store.get("media123").unwrap().unwrap();
        assert_eq!(record.progress,100);
        assert_eq!(record.detail,format!("SHA-256: {digest}"));
        assert!(store.events_after("media123","other",0,12).unwrap().is_empty());
        let events=store.events_after("media123","media",0,12).unwrap();
        // One durable START event plus four worker checkpoints.
        assert_eq!(events.len(),5);
        assert_eq!(events.last().unwrap().progress,100);
    }
    #[test]
    fn invalid_and_truncated_worker_reports_cannot_complete(){
        use std::io::Cursor;
        for frames in [
            "P\t0\t100\nP\t100\t100\n",
            "P\t100\t100\nP\t50\t100\n",
            "P\t100\t100\nD\tnot-a-sha256\n",
            "P\t2\t1\n",
            "P\t0\t0\n",
            "P\t5\t10\nD\t",
        ]{
            let store=TaskStore::open(Path::new(":memory:")).unwrap();
            store.create("t",Some("w"),None).unwrap();
            store.start("t").unwrap();
            assert!(read_worker_progress(Cursor::new(frames.as_bytes()),&store,"t",None,None).is_none());
            assert!(store.get("t").unwrap().unwrap().progress<100);
        }
    }
    #[test]
    fn native_local_observe_image_pipe_is_private_one_shot_bounded_and_not_in_sqlite(){
        use std::io::Cursor;
        let store=TaskStore::open(Path::new(":memory:")).unwrap();
        store.create("obs-frame-1",Some("c-main"),None).unwrap();
        store.start("obs-frame-1").unwrap();
        let digest="a".repeat(64);
        let bgra=vec![42u8;8]; // two BGRA pixels, 2x1
        let hash=format!("{:x}",Sha256::digest(&bgra));
        let mut message=format!("P\t1\t1\nD\t{digest}\nB\tnative-foreground-window-v1\t2\t1\t{hash}\n").into_bytes();
        message.extend_from_slice(&bgra);
        let frame=read_worker_progress(Cursor::new(message.clone()),&store,
            "obs-frame-1",Some("native-foreground-window-v1"),None).unwrap().frame.unwrap();
        assert_eq!((frame.width,frame.height,frame.row_pitch),(2,1,8));
        assert_eq!(frame.bytes,bgra);
        let record=store.get("obs-frame-1").unwrap().unwrap();
        assert_eq!(record.detail,format!("SHA-256: {digest}"));
        // Without explicit host-owned image mode, even an otherwise valid
        // binary payload MUST be rejected.
        assert!(read_worker_progress(Cursor::new(message),&store,
            "obs-frame-1",None,None).is_none());
        // A foreground-host plan cannot accept a valid packet mislabeled
        // as a primary monitor, or a source without exact OS-backed mode.
        let mut wrong_source=format!("P\t1\t1\nD\t{digest}\nB\tnative-primary-monitor-v1\t2\t1\t{hash}\n").into_bytes();
        wrong_source.extend_from_slice(&bgra);
        assert!(read_worker_progress(Cursor::new(wrong_source),&store,
            "obs-frame-1",Some("native-foreground-window-v1"),None).is_none());
    }
    #[test]
    fn os_owned_image_pipe_enters_real_png_admission_and_remains_off_network(){
        use std::io::Cursor;
        use crate::native_observe_image_admission::admit_wgc_frame_locally;
        use crate::native_observe_egress::{DataProvenance,Destination};
        let store=TaskStore::open(Path::new(":memory:")).unwrap();
        store.create("obs-frame-2",Some("c-native"),None).unwrap();
        store.start("obs-frame-2").unwrap();
        let bgra=vec![1u8,2,3,255,4,5,6,255];
        let hash=format!("{:x}",Sha256::digest(&bgra));
        let digest="b".repeat(64);
        let mut packet=format!("P\t1\t1\nD\t{digest}\nB\tnative-foreground-window-v1\t2\t1\t{hash}\n").into_bytes();
        packet.extend_from_slice(&bgra);
        let scanned=read_worker_progress(Cursor::new(packet),&store,
            "obs-frame-2",Some("native-foreground-window-v1"),None).unwrap();
        let frame=scanned.frame.unwrap();
        let admitted=admit_wgc_frame_locally(
            &frame.bytes,frame.width,frame.height,frame.row_pitch,true
        ).unwrap();
        assert_eq!(admitted.provenance,DataProvenance::OBSERVE_LEARN);
        assert!(admitted.pinned.bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert!(admitted.provenance.gate(Destination::LocalInference).is_ok());
        assert!(admitted.provenance.gate(Destination::NetworkSocket).is_err());
        assert!(admitted.provenance.gate(Destination::RemoteModel).is_err());
        let sqlite=store.get("obs-frame-2").unwrap().unwrap();
        assert_eq!(sqlite.detail,format!("SHA-256: {digest}"));
        assert!(!sqlite.detail.contains(&hash));
    }
    #[test]
    fn malformed_or_truncated_observed_pixels_never_produce_image_proof(){
        use std::io::Cursor;
        let bgra=vec![1u8,2,3,255];
        let hash=format!("{:x}",Sha256::digest(&bgra));
        let digest="c".repeat(64);
        let mut valid=format!("P\t1\t1\nD\t{digest}\nB\tnative-foreground-window-v1\t1\t1\t{hash}\n").into_bytes();
        valid.extend_from_slice(&bgra);
        let mut wrong=valid.clone();
        *wrong.last_mut().unwrap()^=1;
        let truncated=valid[..valid.len()-1].to_vec();
        let mut extra=valid.clone();extra.push(b'X');
        for (i,packet) in [wrong,truncated,extra].into_iter().enumerate(){
            let id=format!("obs-bad-{i}");
            let store=TaskStore::open(Path::new(":memory:")).unwrap();
            store.create(&id,Some("c-native"),None).unwrap();
            store.start(&id).unwrap();
            assert!(read_worker_progress(Cursor::new(packet),&store,&id,Some("native-foreground-window-v1"),None).is_none());
        }
    }
    #[test]
    fn reviewed_artifact_stdout_protocol_requires_sha_kind_and_complete_bytes(){
        use std::io::Cursor;
        let content=b"PK\x03\x04-owned-reviewed-workbook-bytes";
        let content_hash=format!("{:x}",Sha256::digest(content));
        let progress_hash="b".repeat(64);
        let mut packet=format!(
            "P\t1\t1\nD\t{progress_hash}\nA\txlsx\t{}\t{content_hash}\n",
            content.len()
        ).into_bytes();
        packet.extend_from_slice(content);
        let store=TaskStore::open(Path::new(":memory:")).unwrap();
        store.create("a-owned",Some("w"),None).unwrap();
        store.start("a-owned").unwrap();
        let result=read_worker_progress(
            Cursor::new(packet.clone()),&store,"a-owned",None,Some("xlsx")
        ).unwrap();
        let artifact=result.artifact.unwrap();
        assert_eq!(artifact.format,"xlsx");
        assert_eq!(artifact.sha256,content_hash);
        assert_eq!(artifact.bytes,content);
        assert!(read_worker_progress(
            Cursor::new(packet.clone()),&store,"a-owned",None,Some("docx")
        ).is_none());
        assert!(read_worker_progress(
            Cursor::new(packet.clone()),&store,"a-owned",None,None
        ).is_none());
        let mut broken=packet.clone();
        *broken.last_mut().unwrap()^=1;
        assert!(read_worker_progress(
            Cursor::new(broken),&store,"a-owned",None,Some("xlsx")
        ).is_none());
        let truncated=packet[..packet.len()-1].to_vec();
        assert!(read_worker_progress(
            Cursor::new(truncated),&store,"a-owned",None,Some("xlsx")
        ).is_none());
        let mut extra=packet;extra.push(b'X');
        assert!(read_worker_progress(
            Cursor::new(extra),&store,"a-owned",None,Some("xlsx")
        ).is_none());
    }
    #[test]
    fn unreviewed_executable_never_creates_a_job(){
        let store=Arc::new(TaskStore::open(Path::new(":memory:")).unwrap());
        let manager=BackgroundProcessSupervisor::new(Arc::clone(&store));
        let plan=TrustedWorkerPlan{
            task_id:"video-1".into(),workspace_id:"media".into(),
            executable:PathBuf::from("relative-untrusted-executable"),
            expected_sha256:"0".repeat(64),args:vec![],progress_protocol:false,max_runtime_seconds:None,
        };
        assert_eq!(manager.start_host_approved(plan),
            Err(BackgroundProcessError::InvalidPlan));
        assert!(store.get("video-1").unwrap().is_none());
        assert_eq!(manager.cancel_owned("video-1","media"),
            Err(BackgroundProcessError::NotOwned));
    }
    #[cfg(target_os="linux")]
    #[test]
    fn sec03_real_os_cancel_poll_collision_has_one_terminal_receipt(){
        use std::sync::{Arc,Barrier};
        let binary=fs::canonicalize("/bin/sleep").unwrap();
        let digest=format!("{:x}",Sha256::digest(fs::read(&binary).unwrap()));
        let store=Arc::new(TaskStore::open(Path::new(":memory:")).unwrap());
        let supervisor=Arc::new(BackgroundProcessSupervisor::new(
            Arc::clone(&store)));
        supervisor.start_host_approved(TrustedWorkerPlan{
            task_id:"sec03-sleep".into(),workspace_id:"c-human".into(),
            executable:binary,expected_sha256:digest,
            args:vec!["3".into()],progress_protocol:false,
            max_runtime_seconds:Some(10),
        }).unwrap();
        assert_eq!(supervisor.cancel_owned("sec03-sleep","c-other"),
            Err(BackgroundProcessError::WrongWorkspace));
        let barrier=Arc::new(Barrier::new(3));
        let a=Arc::clone(&supervisor);
        let b=Arc::clone(&barrier);
        let poller=std::thread::spawn(move||{
            b.wait();
            a.poll()
        });
        let a=Arc::clone(&supervisor);
        let b=Arc::clone(&barrier);
        let stopper=std::thread::spawn(move||{
            b.wait();
            a.cancel_owned("sec03-sleep","c-human")
        });
        barrier.wait();
        let stopped=stopper.join().unwrap();
        let observed=poller.join().unwrap();
        assert!(stopped.is_ok()||matches!(stopped,
            Err(BackgroundProcessError::NotOwned)));
        assert!(observed.is_ok());
        let current=store.get("sec03-sleep").unwrap().unwrap().state;
        assert!(matches!(current,TaskState::Cancelled|TaskState::Completed));
        let terminal=store.events_after("sec03-sleep","c-human",0,32)
            .unwrap().into_iter().filter(|r|matches!(
                r.event.as_str(),"CANCELLED"|"COMPLETED"|"TIMED_OUT"))
            .collect::<Vec<_>>();
        assert_eq!(terminal.len(),1,
            "colliding host poll and stop must not publish two receipts");
        assert!(!supervisor.can_cancel("sec03-sleep"));
    }
    /// Actual Linux cgroup-v2 positive acceptance. Not a synthetic PGID
    /// assertion: the test intentionally fails as NOT_AVAILABLE if the
    /// runner has not delegated writable cgroup-v2 ancestry to this process.
    /// CI must run it explicitly with --ignored after staging the helper.
    #[cfg(target_os="linux")]
    #[test]
    #[ignore="requires genuine delegated cgroup v2 and staged first-party helper"]
    fn agent01l_linux_real_git_cgroup_custody_after_restart(){
        use crate::{
            agent_os_custody::HostOriginalJobCustody,
            agent_recovery_decision::IndependentProcessCustody,
        };
        use std::process::Command;
        let binary=fs::canonicalize("/usr/bin/git")
            .expect("Real native Git required");
        let root=std::env::temp_dir().join(format!(
            "lu-agent01l-linux-git-{}-{}",std::process::id(),
            std::time::SystemTime::now().duration_since(
                std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let repo=root.join("approved-repo");
        fs::create_dir_all(&repo).unwrap();
        assert!(Command::new(&binary).args(["init","-q"])
            .current_dir(&repo).status().unwrap().success());
        assert!(Command::new(&binary).args([
            "-c","user.name=Lu","-c","user.email=lu@example.test",
            "commit","--allow-empty","-q","-m","Native OS cgroup witness"
        ]).current_dir(&repo).status().unwrap().success());
        let sha=format!("{:x}",Sha256::digest(fs::read(&binary).unwrap()));
        let chat=format!("c-{}","3".repeat(32));
        let turn=format!("g-{}","4".repeat(32));
        let call="agent01l-real-linux-gh";
        let journal=root.join(format!("lu-reviewed-task-{turn}.sqlite"));
        let store=Arc::new(TaskStore::open(&journal).unwrap());
        store.bind_reviewed_human_turn(&chat,&turn).unwrap();
        let supervisor=BackgroundProcessSupervisor::new_for_reviewed_host(
            Arc::clone(&store),root.clone()
        );
        supervisor.start_host_approved_git_read(TrustedWorkerPlan{
            task_id:call.into(),workspace_id:turn.clone(),
            executable:binary,expected_sha256:sha,
            args:vec!["-c".into(),"core.fsmonitor=false".into(),
                "--no-optional-locks".into(),"-C".into(),repo.as_os_str().into(),
                "rev-parse".into(),"--verify".into(),"HEAD".into()],
            progress_protocol:false,max_runtime_seconds:Some(15),
        },16_384).unwrap();
        let mut completed=false;
        for _ in 0..250{
            supervisor.poll().unwrap();
            if store.get(call).unwrap().unwrap().state==TaskState::Completed{
                completed=true;break
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(completed,"Real Git worker must complete under native supervisor");
        let stdout=supervisor.take_local_version_stdout(call,&turn)
            .unwrap().expect("Reviewed Git stdout unavailable");
        let commit=String::from_utf8_lossy(&stdout.bytes);
        assert_eq!(commit.trim().len(),40);
        assert!(commit.trim().bytes().all(|b|b.is_ascii_hexdigit()));
        drop(supervisor);drop(store);
        let witness=HostOriginalJobCustody::from_host_profile(&root);
        assert!(witness.worker_and_descendants_ended(&chat,&turn,call),
            "NOT_AVAILABLE: independently delegated cgroup-v2 populated-zero receipt required; PGID does not count");
        assert!(!witness.worker_and_descendants_ended(
            &format!("c-{}","5".repeat(32)),&turn,call));
        assert!(!witness.worker_and_descendants_ended(
            &chat,&format!("g-{}","6".repeat(32)),call));
        assert!(!witness.worker_and_descendants_ended(&chat,&turn,"other-gh"));
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(windows)]
    #[test]
    fn agent01l_real_git_job_attests_original_turn_and_reopens_without_replay(){
        use crate::{
            agent_os_custody::HostOriginalJobCustody,
            agent_recovery_decision::IndependentProcessCustody,
        };
        use std::process::Command;
        let where_git=Command::new("where.exe").arg("git.exe").output().unwrap();
        assert!(where_git.status.success(),"Windows Git must be installed");
        let selected=String::from_utf8(where_git.stdout).unwrap()
            .lines().next().expect("Missing Windows git path").trim().to_owned();
        let binary=fs::canonicalize(selected).unwrap();
        let root=std::env::temp_dir().join(format!(
            "lu-agent01l-git-{}-{}",std::process::id(),
            std::time::SystemTime::now().duration_since(
                std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let repo=root.join("approved-git-repo");
        fs::create_dir_all(&repo).unwrap();
        assert!(Command::new(&binary).args(["init","-q"])
            .current_dir(&repo).status().unwrap().success());
        assert!(Command::new(&binary).args([
            "-c","user.name=Lu","-c","user.email=lu@example.test",
            "commit","--allow-empty","-q","-m","AGENT-01L native witness"
        ]).current_dir(&repo).status().unwrap().success());
        let mut hasher=Sha256::new();
        hasher.update(fs::read(&binary).unwrap());
        let sha=format!("{:x}",hasher.finalize());
        let chat=format!("c-{}","1".repeat(32));
        let turn=format!("g-{}","2".repeat(32));
        let call="agent01l-real-git-gh";
        let db=root.join(format!("lu-reviewed-task-{turn}.sqlite"));
        let store=Arc::new(TaskStore::open(&db).unwrap());
        store.bind_reviewed_human_turn(&chat,&turn).unwrap();
        let manager=BackgroundProcessSupervisor::new(Arc::clone(&store));
        manager.start_host_approved_git_read(TrustedWorkerPlan{
            task_id:call.into(),workspace_id:turn.clone(),
            executable:binary,expected_sha256:sha,
            args:vec![
                "-c".into(),"core.fsmonitor=false".into(),
                "--no-optional-locks".into(),"-C".into(),repo.as_os_str().into(),
                "rev-parse".into(),"--verify".into(),"HEAD".into(),
            ],
            progress_protocol:false,max_runtime_seconds:Some(15),
        },16_384).unwrap();
        let mut done=false;
        for _ in 0..250{
            manager.poll().unwrap();
            if store.get(call).unwrap().unwrap().state==TaskState::Completed{
                done=true;break
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(done,"Native Git did not produce a verified completed task");
        let stdout=manager.take_local_version_stdout(call,&turn)
            .unwrap().expect("Native Git stdout missing");
        let observed=String::from_utf8_lossy(&stdout.bytes);
        let observed=observed.trim();
        assert_eq!(observed.len(),40);
        assert!(observed.bytes().all(|b|b.is_ascii_hexdigit()));
        drop(manager);drop(store);
        let verifier=HostOriginalJobCustody::from_host_profile(&root);
        assert!(verifier.worker_and_descendants_ended(&chat,&turn,call),
            "Only a kernel-query-confirmed Windows Job can publish this proof");
        assert!(!verifier.worker_and_descendants_ended(
            &format!("c-{}","3".repeat(32)),&turn,call));
        assert!(!verifier.worker_and_descendants_ended(&chat,
            &format!("g-{}","4".repeat(32)),call));
        assert!(!verifier.worker_and_descendants_ended(&chat,&turn,"other-call-gh"));
        let reopen=rusqlite::Connection::open(&db).unwrap();
        assert!(reopen.execute(
            "DELETE FROM lu_task_windows_job_custody",[]
        ).is_err(),"Kernel witness row must be immutable");
        assert!(reopen.execute(
            "UPDATE lu_task_windows_job_custody SET workspace_id=?1",
            rusqlite::params![format!("g-{}","4".repeat(32))]
        ).is_err(),"OS evidence turn cannot be rebound");
        drop(reopen);
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(windows)]
    #[test]
    fn windows_owned_job_is_contained_and_reaped_on_cancellation(){
        // An ordinary Windows system binary stands in for a reviewed worker.
        // Cmd starts a child ping process, exercising the Job object tree.
        let executable=PathBuf::from(std::env::var_os("ComSpec")
            .unwrap_or_else(||OsString::from(r"C:\Windows\System32\cmd.exe")));
        if !executable.is_file(){return;}
        let mut sha=Sha256::new();
        sha.update(fs::read(&executable).unwrap());
        let digest=format!("{:x}",sha.finalize());
        let store=Arc::new(TaskStore::open(Path::new(":memory:")).unwrap());
        let manager=BackgroundProcessSupervisor::new(Arc::clone(&store));
        manager.start_host_approved(TrustedWorkerPlan{
            task_id:"win-media1".into(),workspace_id:"video".into(),
            executable,expected_sha256:digest,
            args:vec!["/D".into(),"/C".into(),
                "ping -n 20 127.0.0.1 >NUL".into()],progress_protocol:false,max_runtime_seconds:None,
        }).unwrap();
        assert!(manager.can_cancel("win-media1"));
        assert_eq!(manager.cancel_owned("win-media1","wrong"),
            Err(BackgroundProcessError::WrongWorkspace));
        manager.cancel_owned("win-media1","video").unwrap();
        assert!(!manager.can_cancel("win-media1"));
        assert_eq!(store.get("win-media1").unwrap().unwrap().state,
            TaskState::Cancelled);
    }
    #[cfg(unix)]
    #[test]
    fn actual_owned_worker_deadline_is_a_timed_out_terminal_event(){
        let executable=PathBuf::from("/bin/sleep").canonicalize().unwrap();
        let mut sha=Sha256::new();
        sha.update(fs::read(&executable).unwrap());
        let digest=format!("{:x}",sha.finalize());
        let store=Arc::new(TaskStore::open(Path::new(":memory:")).unwrap());
        let manager=BackgroundProcessSupervisor::new(Arc::clone(&store));
        manager.start_host_approved(TrustedWorkerPlan{
            task_id:"deadline1".into(),workspace_id:"c-task-chat".into(),
            executable,expected_sha256:digest,
            args:vec![OsString::from("30")],
            progress_protocol:false,max_runtime_seconds:Some(1),
        }).unwrap();
        std::thread::sleep(Duration::from_millis(1250));
        assert_eq!(manager.poll().unwrap(),1);
        assert!(!manager.can_cancel("deadline1"));
        assert_eq!(store.get("deadline1").unwrap().unwrap().state,TaskState::TimedOut);
        let notices=store.pending_notices_for_workspace("c-task-chat",8).unwrap();
        assert_eq!(notices.len(),1);
        assert_eq!(notices[0].state,TaskState::TimedOut);
    }
    #[cfg(unix)]
    #[test]
    fn real_supervised_worker_can_be_cancelled_without_touching_other_job(){
        let sleep=PathBuf::from("/bin/sleep");
        if !sleep.is_file(){return;}
        // Distributions may expose /bin/sleep through a symlink chain.
        // The admission rule rightly refuses symlink executable paths;
        // use the actual canonical target in this trusted test fixture.
        let sleep=sleep.canonicalize().unwrap();
        let mut sha=Sha256::new();
        sha.update(fs::read(&sleep).unwrap());
        let digest=format!("{:x}",sha.finalize());
        let store=Arc::new(TaskStore::open(Path::new(":memory:")).unwrap());
        let manager=BackgroundProcessSupervisor::new(Arc::clone(&store));
        manager.start_host_approved(TrustedWorkerPlan{
            task_id:"worker-1".into(),workspace_id:"media".into(),
            executable:sleep,expected_sha256:digest,
            args:vec![OsString::from("30")],progress_protocol:false,max_runtime_seconds:None,
        }).unwrap();
        assert_eq!(manager.cancel_owned("worker-1","unrelated"),
            Err(BackgroundProcessError::WrongWorkspace));
        assert!(manager.can_cancel("worker-1"));
        manager.cancel_owned("worker-1","media").unwrap();
        assert!(!manager.can_cancel("worker-1"));
        assert_eq!(store.get("worker-1").unwrap().unwrap().state,TaskState::Cancelled);
        assert_eq!(store.reconcile_interrupted().unwrap(),0);
    }
}
