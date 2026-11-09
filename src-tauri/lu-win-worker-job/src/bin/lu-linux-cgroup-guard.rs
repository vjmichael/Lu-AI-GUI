//! First-party, crash-surviving Linux cgroup-v2 custodian.
//!
//! This binary is NOT a shell, tool, or model-selectable process runner.
//! The Lu host launches it outside the per-operation worker cgroup, with a
//! private stdin pipe. Host death closes that pipe. Only this first-party
//! custodian issues cgroup.kill, then reads cgroup.events populated 0 and
//! durably writes a narrowly scoped historical OS receipt.
//! No sudo, PID from SQLite, global cgroup write or guessed PGID is used.
#[cfg(target_os="linux")]
mod linux{
    use std::{env,fs::{self,File,OpenOptions},io::{self,Read,Write},
        os::unix::{fs::{MetadataExt,OpenOptionsExt},io::AsRawFd},
        path::{Component,Path,PathBuf},time::{Duration,Instant}};
    const ROOT:&str="/sys/fs/cgroup";
    const MAX_RECEIPT:usize=4096;
    fn exact(s:&str,p:&str)->bool{
        s.len()==p.len()+32&&s.starts_with(p)&&s[p.len()..].bytes()
            .all(|b|b.is_ascii_hexdigit()&&!b.is_ascii_uppercase())
    }
    fn call(s:&str)->bool{
        s.len()>3&&s.len()<=128&&s.bytes().all(|b|
            b.is_ascii_alphanumeric()||b==b'-'||b==b'_')
            &&["-gh","-gs","-gb","-gc","-fm"]
                .iter().any(|suffix|s.ends_with(suffix))
    }
    fn nonce()->io::Result<String>{
        let mut bytes=[0u8;16];
        File::open("/dev/urandom")?.read_exact(&mut bytes)?;
        Ok(bytes.iter().map(|v|format!("{v:02x}")).collect::<String>())
    }
    fn current_cgroup()->io::Result<PathBuf>{
        if !Path::new(ROOT).join("cgroup.controllers").is_file(){
            return Err(io::Error::other("not a unified cgroup-v2 mount"))
        }
        let own=fs::read_to_string("/proc/self/cgroup")?;
        let lines:Vec<&str>=own.lines().filter_map(|l|l.strip_prefix("0::")).collect();
        if lines.len()!=1||!lines[0].starts_with('/'){
            return Err(io::Error::other("ambiguous unified cgroup identity"))
        }
        let rel=Path::new(lines[0].trim_start_matches('/'));
        if rel.components().any(|c|!matches!(c,Component::Normal(_))){
            // The root (empty relative path) is allowed when it is genuinely
            // delegated. No arbitrary ancestor can be selected via args.
            if !rel.as_os_str().is_empty(){
                return Err(io::Error::other("untrusted cgroup path"))
            }
        }
        let root=Path::new(ROOT).join(rel);
        if !root.is_dir(){
            return Err(io::Error::other("current cgroup directory missing"))
        }
        Ok(root)
    }
    fn populated_zero(path:&Path)->io::Result<bool>{
        let s=fs::read_to_string(path.join("cgroup.events"))?;
        let mut observed=None;
        for line in s.lines(){
            let mut parts=line.split_ascii_whitespace();
            if parts.next()==Some("populated"){
                let value=parts.next();
                if observed.is_some()||parts.next().is_some(){
                    return Err(io::Error::other("ambiguous cgroup.events"))
                }
                observed=match value{
                    Some("0")=>Some(true),Some("1")=>Some(false),
                    _=>return Err(io::Error::other("invalid populated state"))
                };
            }
        }
        observed.ok_or_else(||io::Error::other("missing populated kernel observation"))
    }
    fn save_receipt(
        profile:&Path,chat:&str,turn:&str,call_id:&str,nonce:&str,
    )->io::Result<()>{
        let path=profile.join(format!("lu-cgroup-os-{turn}-{call_id}.receipt"));
        // create_new refuses a stale receipt or symlink. The file lives in
        // the native host's fixed profile, not in any model-selected folder.
        let mut file=OpenOptions::new().write(true).create_new(true)
            .mode(0o600).open(&path)?;
        let record=format!(
            "LU_CGROUP_V2_EMPTY_V1\nCHAT={chat}\nTURN={turn}\nCALL={call_id}\nNONCE={nonce}\nPOPULATED=0\n"
        );
        if record.len()>MAX_RECEIPT{return Err(io::Error::other("oversized OS proof"))}
        file.write_all(record.as_bytes())?;
        file.sync_all()?;
        File::open(profile)?.sync_all()?;
        Ok(())
    }
    fn end_owned_subtree(path:&Path)->io::Result<()>{
        // cgroup.kill is recursive and includes descendants that call setsid,
        // migrate within this subtree or start a nested cgroup. The helper
        // itself is outside this path, so it survives the kill.
        fs::write(path.join("cgroup.kill"),b"1")?;
        let deadline=Instant::now()+Duration::from_secs(8);
        loop{
            if populated_zero(path)?{return Ok(())}
            if Instant::now()>=deadline{
                return Err(io::Error::other("kernel did not observe populated 0"))
            }
            std::thread::sleep(Duration::from_millis(15));
        }
    }
    fn try_run()->io::Result<()>{
        let args:Vec<String>=env::args().collect();
        if args.len()!=5{return Err(io::Error::other("expected host profile/chat/turn/call"))}
        let profile=PathBuf::from(&args[1]);
        let (chat,turn,call_id)=(&args[2],&args[3],&args[4]);
        if !exact(chat,"c-")||!exact(turn,"g-")||!call(call_id)
            ||!profile.is_absolute(){
            return Err(io::Error::other("invalid host-controlled OS scope"))
        }
        let meta=fs::symlink_metadata(&profile)?;
        if !meta.is_dir()||meta.file_type().is_symlink()
            ||meta.uid()!=unsafe{geteuid()}{
            return Err(io::Error::other("untrusted OS receipt profile"))
        }
        let group_root=match current_cgroup(){
            Ok(root)=>root,
            Err(_)=>{
                io::stdout().write_all(b"UNAVAILABLE\n")?;
                io::stdout().flush()?;
                return Ok(())
            }
        };
        let nonce=nonce()?;
        let group=group_root.join(format!("lu-review-{nonce}"));
        // No privilege escalation: if the OS has not delegated this parent
        // then create_dir fails. This is reported as NOT AVAILABLE.
        match fs::create_dir(&group){
            Ok(())=>{},
            Err(e) if e.kind()==io::ErrorKind::PermissionDenied
                ||matches!(e.raw_os_error(),Some(1|13|30))=>{
                io::stdout().write_all(b"UNAVAILABLE\n")?;
                io::stdout().flush()?;
                return Ok(())
            },
            Err(e)=>return Err(e),
        }
        // Require genuine cgroup-v2 features. A foreign mount may not
        // contain these kernel files. Never fall back to killpg as proof.
        if !group.join("cgroup.kill").exists()
            ||!group.join("cgroup.procs").exists()
            ||OpenOptions::new().write(true)
                .open(group.join("cgroup.kill")).is_err()
            ||OpenOptions::new().write(true)
                .open(group.join("cgroup.procs")).is_err()
            ||!matches!(populated_zero(&group),Ok(true)){
            let _=fs::remove_dir(&group);
            io::stdout().write_all(b"UNAVAILABLE\n")?;
            io::stdout().flush()?;
            return Ok(())
        }
        // This profile receipt name must be unoccupied prior to worker
        // admission; old DB/task names cannot be reused for new effects.
        let receipt=profile.join(format!("lu-cgroup-os-{turn}-{call_id}.receipt"));
        if fs::symlink_metadata(&receipt).is_ok(){
            let _=fs::remove_dir(&group);
            return Err(io::Error::other("existing OS witness cannot be replayed"))
        }
        // First-party helper proves to the host which kernel cgroup it
        // actually created. This is a private stdout IPC, not model output.
        writeln!(io::stdout(),"READY\t{}",group.display())?;
        io::stdout().flush()?;
        // Exactly this custodian owns the lifetime pipe. It stays outside
        // the worker subtree and has NO parent-death signal.
        let mut buf=[0u8;32];
        let mut stdin=io::stdin().lock();
        loop{
            match stdin.read(&mut buf){
                Ok(0)=>break,
                Ok(_)=>{}, // never parse model text as commands
                Err(ref e) if e.kind()==io::ErrorKind::Interrupted=>{},
                Err(_)=>break,
            }
        }
        end_owned_subtree(&group)?;
        save_receipt(&profile,chat,turn,call_id,&nonce)?;
        // Empty cgroups can usually be removed. Nested cgroups may prevent
        // rmdir; the kernel emptiness witness still remains valid.
        let _=fs::remove_dir(&group);
        Ok(())
    }
    unsafe extern "C"{fn geteuid()->u32;}
    pub fn main(){
        if let Err(e)=try_run(){
            eprintln!("Lu native cgroup custodian denied OS witness: {e}");
            std::process::exit(91)
        }
    }
}
#[cfg(target_os="linux")]
fn main(){linux::main()}
#[cfg(not(target_os="linux"))]
fn main(){std::process::exit(98)}
