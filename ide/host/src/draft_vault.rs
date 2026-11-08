//! B2: durable *IDE-private* unsaved source snapshots, separate from project files.
//! Only native-host supplied registered project/file IDs are accepted. A
//! checkpoint ID is content-addressed and immutable; the current head uses a
//! compare-and-swap, fsynced pointer. This is NOT a Save to project source.
use crate::{browser::validate_relative,wire};
use serde::{Deserialize,Serialize};
use sha2::{Digest,Sha256};
use std::{
    fs::{self,File,OpenOptions},io::{Read,Write},
    path::{Path,PathBuf},sync::Mutex
};

const MAX_DRAFT_BYTES:usize=512*1024;
const MAX_RECORD_BYTES:u64=600*1024;
const MAX_HISTORY:usize=24;
const MAX_PATH_BYTES:usize=1024;
const SCHEMA:u32=1;
const HEX_LEN:usize=64;
type Result<T>=std::result::Result<T,&'static str>;

fn valid_sha(s:&str)->bool{
    s.len()==HEX_LEN&&s.bytes().all(|c|c.is_ascii_hexdigit())
}
fn sha(data:&[u8])->String{format!("{:x}",Sha256::digest(data))}
fn valid_identity(p:&str,path:&str,base:&str,id:&str,revision:u64)->bool{
    wire::valid_project_id(p)&&path.len()<=MAX_PATH_BYTES&&
    validate_relative(path,false).is_ok()&&
    valid_sha(base)&&id.len()>=8&&id.len()<=128&&
    id.bytes().all(|c|c.is_ascii_alphanumeric()||c==b'-')&&revision<=1_000_000_000
}
fn valid_key(p:&str,path:&str)->bool{
    wire::valid_project_id(p)&&path.len()<=MAX_PATH_BYTES&&validate_relative(path,false).is_ok()
}
fn clean_dir(path:&Path)->Result<()>{
    let m=fs::symlink_metadata(path).map_err(|_|"DRAFT_DIRECTORY_UNAVAILABLE")?;
    if !m.is_dir()||m.file_type().is_symlink(){return Err("DRAFT_UNSAFE_DIRECTORY")}
    #[cfg(windows)]{
        use std::os::windows::fs::MetadataExt;
        if m.file_attributes() & 0x400 != 0 {return Err("DRAFT_UNSAFE_DIRECTORY")}
    }
    Ok(())
}
fn clean_file(path:&Path)->Result<()>{
    let m=fs::symlink_metadata(path).map_err(|_|"DRAFT_RECORD_UNAVAILABLE")?;
    if !m.is_file()||m.file_type().is_symlink()||m.len()>MAX_RECORD_BYTES{
        return Err("DRAFT_RECORD_UNTRUSTED");
    }
    #[cfg(windows)]{
        use std::os::windows::fs::MetadataExt;
        if m.file_attributes() & 0x400 !=0{return Err("DRAFT_RECORD_UNTRUSTED")}
    }
    Ok(())
}
fn private_dir(path:&Path)->Result<()>{
    if path.exists(){return clean_dir(path)}
    fs::create_dir(path).map_err(|_|"DRAFT_DIRECTORY_UNAVAILABLE")?;
    #[cfg(unix)]{
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path,fs::Permissions::from_mode(0o700))
          .map_err(|_|"DRAFT_DIRECTORY_UNAVAILABLE")?;
    }
    clean_dir(path)
}
fn atomic(path:&Path,data:&[u8])->Result<()>{
    if data.len() as u64>MAX_RECORD_BYTES{return Err("DRAFT_RECORD_TOO_LARGE")}
    let parent=path.parent().ok_or("DRAFT_UNSAFE_DIRECTORY")?;
    clean_dir(parent)?;
    if path.exists(){clean_file(path)?}
    let temp=parent.join(format!(".draft-{}-{}.tmp",std::process::id(),
        sha(data).chars().take(16).collect::<String>()));
    // A second writer cannot claim this temp file while the vault is locked.
    let mut opts=OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]{
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f=opts.open(&temp).map_err(|_|"DRAFT_WRITE_FAILED")?;
    let result=(||->Result<()>{
        f.write_all(data).map_err(|_|"DRAFT_WRITE_FAILED")?;
        f.sync_all().map_err(|_|"DRAFT_WRITE_FAILED")?;
        drop(f);
        fs::rename(&temp,path).map_err(|_|"DRAFT_WRITE_FAILED")?;
        #[cfg(unix)] File::open(parent).and_then(|p|p.sync_all())
            .map_err(|_|"DRAFT_WRITE_FAILED")?;
        Ok(())
    })();
    if result.is_err(){let _=fs::remove_file(temp);}
    result
}
fn read_bounded(path:&Path)->Result<Vec<u8>>{
    clean_file(path)?;
    let mut out=Vec::new();
    File::open(path).map_err(|_|"DRAFT_RECORD_UNAVAILABLE")?
       .take(MAX_RECORD_BYTES+1).read_to_end(&mut out)
       .map_err(|_|"DRAFT_RECORD_UNAVAILABLE")?;
    if out.len() as u64>MAX_RECORD_BYTES{return Err("DRAFT_RECORD_UNTRUSTED")}
    Ok(out)
}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct DraftSnapshot{
    pub schema_version:u32,
    pub project_id:String,
    pub relative_path:String,
    pub buffer_id:String,
    pub buffer_revision:u64,
    pub base_disk_revision:String,
    pub draft_sha256:String,
    pub text:String,
    pub checkpoint_id:String,
}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct DraftHead{
    schema_version:u32,
    checkpoint_id:String,
}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct DraftCheckpointInput{
    pub project_id:String,
    pub relative_path:String,
    pub buffer_id:String,
    pub buffer_revision:u64,
    pub base_disk_revision:String,
    pub text:String,
    pub expected_head:Option<String>,
}
pub struct DraftVault{root:PathBuf,gate:Mutex<()>}
impl DraftVault{
    pub fn open(root:PathBuf)->Result<Self>{
        let parent=root.parent().ok_or("DRAFT_UNSAFE_DIRECTORY")?;
        clean_dir(parent)?;
        private_dir(&root)?;
        Ok(Self{root,gate:Mutex::new(())})
    }
    fn folder(&self,p:&str,path:&str,create:bool)->Result<PathBuf>{
        if !valid_key(p,path){return Err("DRAFT_IDENTITY_INVALID")}
        clean_dir(&self.root)?;
        let project=self.root.join(p);
        if create{private_dir(&project)?}
        else{clean_dir(&project)?}
        let name=sha(path.as_bytes());
        let doc=project.join(name);
        if create{private_dir(&doc)?}else{clean_dir(&doc)?}
        Ok(doc)
    }
    fn latest_id_in(&self,dir:&Path)->Result<Option<String>>{
        let path=dir.join("latest.json");
        if !path.exists(){return Ok(None)}
        let record:DraftHead=serde_json::from_slice(&read_bounded(&path)?)
            .map_err(|_|"DRAFT_RECORD_UNTRUSTED")?;
        if record.schema_version!=SCHEMA||!valid_sha(&record.checkpoint_id){
            return Err("DRAFT_RECORD_UNTRUSTED")
        }
        Ok(Some(record.checkpoint_id))
    }
    fn read_in(&self,dir:&Path,id:&str,p:&str,path:&str)->Result<DraftSnapshot>{
        if !valid_sha(id){return Err("DRAFT_REFERENCE_INVALID")}
        let file=dir.join(format!("{id}.json"));
        let data=read_bounded(&file)?;
        let snap:DraftSnapshot=serde_json::from_slice(&data)
            .map_err(|_|"DRAFT_RECORD_UNTRUSTED")?;
        if snap.schema_version!=SCHEMA||snap.checkpoint_id!=id||
           snap.project_id!=p||snap.relative_path!=path||
           !valid_identity(p,path,&snap.base_disk_revision,&snap.buffer_id,snap.buffer_revision)||
           snap.text.len()>MAX_DRAFT_BYTES||sha(snap.text.as_bytes())!=snap.draft_sha256{
            return Err("DRAFT_RECORD_UNTRUSTED")
        }
        let mut verified=snap.clone();
        verified.checkpoint_id.clear();
        let original=serde_json::to_vec(&verified).map_err(|_|"DRAFT_RECORD_UNTRUSTED")?;
        if sha(&original)!=id{return Err("DRAFT_RECORD_UNTRUSTED")}
        Ok(snap)
    }
    pub fn latest(&self,p:&str,path:&str)->Result<Option<DraftSnapshot>>{
        let _g=self.gate.lock().map_err(|_|"DRAFT_VAULT_BUSY")?;
        if !valid_key(p,path){return Err("DRAFT_IDENTITY_INVALID")}
        let dir=match self.folder(p,path,false){
            Ok(d)=>d, Err("DRAFT_DIRECTORY_UNAVAILABLE")=>return Ok(None),
            Err(e)=>return Err(e)
        };
        match self.latest_id_in(&dir)?{
            Some(id)=>self.read_in(&dir,&id,p,path).map(Some),None=>Ok(None)
        }
    }
    pub fn load(&self,p:&str,path:&str,id:&str)->Result<DraftSnapshot>{
        let _g=self.gate.lock().map_err(|_|"DRAFT_VAULT_BUSY")?;
        let dir=self.folder(p,path,false)?;
        self.read_in(&dir,id,p,path)
    }
    pub fn checkpoint(&self,input:DraftCheckpointInput)->Result<DraftSnapshot>{
        let _g=self.gate.lock().map_err(|_|"DRAFT_VAULT_BUSY")?;
        let p=&input.project_id;
        let path=&input.relative_path;
        if !valid_identity(p,path,&input.base_disk_revision,
            &input.buffer_id,input.buffer_revision)||input.text.len()>MAX_DRAFT_BYTES{
            return Err("DRAFT_IDENTITY_INVALID")
        }
        if input.expected_head.as_ref().is_some_and(|x|!valid_sha(x)){
            return Err("DRAFT_REFERENCE_INVALID")
        }
        let dir=self.folder(p,path,true)?;
        let head=self.latest_id_in(&dir)?;
        if input.expected_head!=head{return Err("DRAFT_CHECKPOINT_CONFLICT")}
        if let Some(id)=head.as_ref(){
            let prev=self.read_in(&dir,id,p,path)?;
            if prev.buffer_id==input.buffer_id&&input.buffer_revision<=prev.buffer_revision{
                return Err("DRAFT_REVISION_STALE")
            }
        }
        let count=fs::read_dir(&dir).map_err(|_|"DRAFT_DIRECTORY_UNAVAILABLE")?
            .filter_map(|item|item.ok())
            .filter(|item|item.file_name().to_string_lossy().ends_with(".json"))
            .count();
        if count>=MAX_HISTORY+1{return Err("DRAFT_HISTORY_FULL")}
        let mut snap=DraftSnapshot{
            schema_version:SCHEMA,project_id:p.clone(),relative_path:path.clone(),
            buffer_id:input.buffer_id,buffer_revision:input.buffer_revision,
            base_disk_revision:input.base_disk_revision,
            draft_sha256:sha(input.text.as_bytes()),
            text:input.text,checkpoint_id:String::new(),
        };
        let canonical=serde_json::to_vec(&snap).map_err(|_|"DRAFT_RECORD_UNTRUSTED")?;
        snap.checkpoint_id=sha(&canonical);
        let immutable=dir.join(format!("{}.json",snap.checkpoint_id));
        if immutable.exists(){
            // Identical content-addressed bytes only. Never modify history.
            let existing=self.read_in(&dir,&snap.checkpoint_id,p,path)?;
            if existing.text!=snap.text{return Err("DRAFT_RECORD_UNTRUSTED")}
        }else{
            atomic(&immutable,&serde_json::to_vec(&snap)
                .map_err(|_|"DRAFT_RECORD_UNTRUSTED")?)?;
        }
        let head=DraftHead{schema_version:SCHEMA,
                           checkpoint_id:snap.checkpoint_id.clone()};
        atomic(&dir.join("latest.json"),&serde_json::to_vec(&head)
            .map_err(|_|"DRAFT_RECORD_UNTRUSTED")?)?;
        Ok(snap)
    }
    pub fn discard(&self,p:&str,path:&str,expected_head:&str)->Result<()>{
        let _g=self.gate.lock().map_err(|_|"DRAFT_VAULT_BUSY")?;
        let dir=self.folder(p,path,false)?;
        let actual=self.latest_id_in(&dir)?;
        if actual.as_deref()!=Some(expected_head){return Err("DRAFT_CHECKPOINT_CONFLICT")}
        // This operation deletes IDE-private recoveries only, never project
        // source, and only after an explicit human action.
        for entry in fs::read_dir(&dir).map_err(|_|"DRAFT_DIRECTORY_UNAVAILABLE")?{
            let entry=entry.map_err(|_|"DRAFT_DIRECTORY_UNAVAILABLE")?;
            clean_file(&entry.path())?;
        }
        fs::remove_dir_all(&dir).map_err(|_|"DRAFT_DISCARD_FAILED")?;
        Ok(())
    }
}
