//! P05: operator-approved Hugging Face transfer custody.
//! Durable plan and per-attempt status live in Lu app data. The model, the
//! WebView and HF metadata cannot select a destination or authorize a network
//! transfer. Restart sees interrupted work but NEVER resumes it automatically.
use lu_core::{
    model_provider_staging::{self,ExactFilePlan},
    model_provider_transport,
};
use serde::{Deserialize,Serialize};
use std::{
    collections::HashMap,
    fs::{self,File,OpenOptions},
    io::{Read,Write},
    path::{Path,PathBuf},
    sync::{Arc,Mutex,atomic::{AtomicBool,Ordering}},
    time::{SystemTime,UNIX_EPOCH},
};
use sha2::{Digest,Sha256};

#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct ModelTransferView{
    pub transfer_id:String,
    pub repo_id:String,
    pub revision:String,
    pub filename:String,
    pub state:String,
    pub received_bytes:u64,
    pub total_bytes:u64,
    pub expected_sha256:String,
    pub verified_sha256:Option<String>,
    pub local_verified_file:Option<String>,
    pub needs_new_human_review:bool,
    pub message:String,
}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct StoredTransfer{
    plan:ExactFilePlan,
    reported_license:String,
    state:String,
    verified_sha256:Option<String>,
}
pub struct ModelTransferManager{
    root:PathBuf,
    active:Mutex<HashMap<String,Arc<AtomicBool>>>,
}
fn allowed_id(id:&str)->bool{
    id.starts_with("hf-")&&(16..=96).contains(&id.len())&&
    id.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-'||b==b'_')
}
fn regular_dir(path:&Path)->bool{
    fs::symlink_metadata(path).is_ok_and(|m|m.file_type().is_dir())
}
fn file_len(path:&Path)->Option<u64>{
    fs::symlink_metadata(path).ok().and_then(|m|
        m.file_type().is_file().then_some(m.len()))
}
fn checked_digest(path:&Path,maximum:u64)->Result<(u64,String),String>{
    let bytes=file_len(path).ok_or("Verified downloaded file not found")?;
    if bytes>maximum{return Err("Downloaded file exceeds approved size".into())}
    let mut source=File::open(path).map_err(|_|"Cannot open verified download")?;
    let mut sha=Sha256::new();let mut seen=0u64;let mut buf=[0u8;65536];
    loop{
        let n=source.read(&mut buf).map_err(|_|"Download readback failed")?;
        if n==0{break}
        seen=seen.checked_add(n as u64).ok_or("Download size overflow")?;
        if seen>maximum{return Err("Download exceeds size limit".into())}
        sha.update(&buf[..n]);
    }
    Ok((seen,format!("{:x}",sha.finalize())))
}
impl ModelTransferManager{
    pub fn new(root:PathBuf)->Result<Self,String>{
        fs::create_dir_all(&root).map_err(|_|"Cannot initialize Lu model-transfer folder")?;
        if !regular_dir(&root){return Err("Model-transfer folder cannot be a symlink".into())}
        Ok(Self{root,active:Mutex::new(HashMap::new())})
    }
    fn folder(&self,id:&str)->Result<PathBuf,String>{
        if !allowed_id(id){return Err("Invalid or unowned model transfer ID".into())}
        let dir=self.root.join(id);
        if !regular_dir(&dir){return Err("Transfer folder missing or unsafe".into())}
        Ok(dir)
    }
    fn load(&self,id:&str)->Result<StoredTransfer,String>{
        let dir=self.folder(id)?;
        let path=dir.join("transfer.json");
        let info=fs::symlink_metadata(&path).map_err(|_|"Transfer record missing")?;
        if !info.file_type().is_file()||info.len()>4096{
            return Err("Transfer record is not a bounded regular file".into())
        }
        let bytes=fs::read(path).map_err(|_|"Transfer record unavailable")?;
        let plan:StoredTransfer=serde_json::from_slice(&bytes)
            .map_err(|_|"Transfer record corrupt; no automatic recovery")?;
        plan.plan.validated().map_err(|_|"Recorded model identity invalid")?;
        if plan.reported_license.is_empty()||plan.reported_license.len()>80||
            !matches!(plan.state.as_str(),"REVIEWED"|"RUNNING"|"INTERRUPTED"|"VERIFIED_AT_PUBLICATION") {
            return Err("Transfer record contains an invalid state".into())
        }
        Ok(plan)
    }
    fn store(&self,id:&str,record:&StoredTransfer)->Result<(),String>{
        let dir=self.folder(id)?;
        let bytes=serde_json::to_vec_pretty(record)
            .map_err(|_|"Transfer record serialization unavailable")?;
        if bytes.len()>4096{return Err("Transfer record overflow".into())}
        let ns=SystemTime::now().duration_since(UNIX_EPOCH)
            .map_err(|_|"Transfer clock unavailable")?.as_nanos();
        let tmp=dir.join(format!("transfer-{ns}.pending"));
        let mut file=OpenOptions::new().create_new(true).write(true).open(&tmp)
            .map_err(|_|"Cannot stage durable transfer status")?;
        file.write_all(&bytes).map_err(|_|"Cannot write transfer status")?;
        file.sync_all().map_err(|_|"Cannot sync transfer status")?;
        drop(file);
        fs::rename(&tmp,dir.join("transfer.json"))
            .map_err(|_|"Cannot atomically commit transfer status")?;
        Ok(())
    }
    /// Stage an entirely NEW, profile-owned review. Caller must obtain an
    /// independent native OS license/asset confirmation and fresh HF digest.
    pub fn stage(
        &self,id:&str,plan:ExactFilePlan,license:&str
    )->Result<ModelTransferView,String>{
        plan.validated().map_err(|_|"Invalid immutable HF asset")?;
        if !allowed_id(id)||license.is_empty()||license.len()>80||
            !license.bytes().all(|b|b.is_ascii_alphanumeric()||
                matches!(b,b'-'|b'_'|b'.')){
            return Err("Invalid transfer review identity".into())
        }
        fs::create_dir(self.root.join(id))
            .map_err(|_|"A transfer with this identity already exists")?;
        let record=StoredTransfer{
            plan,reported_license:license.into(),
            state:"REVIEWED".into(),verified_sha256:None,
        };
        self.store(id,&record)?;
        self.status(id)
    }
    /// Enumerate only Lu-generated nonce directories after restart. Invalid
    /// or corrupted records never become executable tasks.
    pub fn recent(&self,limit:usize)->Result<Vec<ModelTransferView>,String>{
        let mut ids=Vec::new();
        for item in fs::read_dir(&self.root).map_err(|_|"Transfer library unavailable")?{
            let item=item.map_err(|_|"Transfer listing interrupted")?;
            let Some(id)=item.file_name().to_str().map(str::to_owned)else{continue};
            if allowed_id(&id)&&regular_dir(&item.path()){ids.push(id)}
            if ids.len()>1024{return Err("Too many transfer directories to inspect".into())}
        }
        ids.sort_unstable_by(|a,b|b.cmp(a));
        Ok(ids.into_iter().take(limit.min(32))
            .filter_map(|id|self.status(&id).ok()).collect())
    }
    pub fn status(&self,id:&str)->Result<ModelTransferView,String>{
        let record=self.load(id)?;
        let folder=self.folder(id)?;
        let active=self.active.lock().map_err(|_|"Transfer state unavailable")?
            .contains_key(id);
        let part=folder.join("received.part");
        let received=file_len(&part).unwrap_or(0);
        let received=if record.state=="VERIFIED_AT_PUBLICATION"{record.plan.expected_bytes}else{received};
        let (state,needs_review)=if record.state=="RUNNING"&&!active{
            ("INTERRUPTED_REVIEW_REQUIRED".to_owned(),true)
        }else if record.state=="INTERRUPTED"{
            ("INTERRUPTED_REVIEW_REQUIRED".to_owned(),true)
        }else if record.state=="VERIFIED_AT_PUBLICATION"{
            ("VERIFIED_AT_PUBLICATION".to_owned(),false)
        }else{(record.state.clone(),false)};
        let completed=folder.join(&record.plan.filename);
        let local=if record.state=="VERIFIED_AT_PUBLICATION" &&
            file_len(&completed)==Some(record.plan.expected_bytes){
            Some(completed.to_string_lossy().into_owned())
        }else{None};
        Ok(ModelTransferView{
            transfer_id:id.into(),repo_id:record.plan.repo_id,
            revision:record.plan.revision,filename:record.plan.filename,
            state,received_bytes:received,total_bytes:record.plan.expected_bytes,
            expected_sha256:record.plan.sha256,
            verified_sha256:record.verified_sha256,
            local_verified_file:local,needs_new_human_review:needs_review,
            message:"Only hash-checked bytes are published. Interrupted transfers require fresh review. No file is auto-imported or loaded.".into(),
        })
    }
    pub fn reviewed_plan(&self,id:&str)->Result<(ExactFilePlan,String),String>{
        let entry=self.load(id)?;
        if !matches!(entry.state.as_str(),"INTERRUPTED"|"RUNNING"){
            return Err("Only an interrupted, non-published transfer may resume".into())
        }
        let is_active=self.active.lock().map_err(|_|"Transfer active state unavailable")?
            .contains_key(id);
        if is_active{return Err("Transfer is still running".into())}
        let folder=self.folder(id)?;
        let size=file_len(&folder.join("received.part"))
            .ok_or("No resumable partial model exists")?;
        if size>entry.plan.expected_bytes{
            return Err("Partial bytes exceed immutable model size".into())
        }
        Ok((entry.plan,entry.reported_license))
    }
    /// Cancellation directly signals the owned curl child. A persisted
    /// cancellation notice is NOT proof of process termination or file state.
    pub fn cancel(&self,id:&str)->Result<bool,String>{
        self.folder(id)?;
        let active=self.active.lock().map_err(|_|"Transfer lock unavailable")?;
        if let Some(flag)=active.get(id){
            flag.store(true,Ordering::Release);
            Ok(true)
        }else{Ok(false)}
    }
    /// Executed inside a host-supervised blocking worker, never a Tauri
    /// renderer callback. A new OS confirmation is required for EACH attempt.
    pub fn execute_reviewed(
        &self,id:&str,pinned_curl:&Path
    )->Result<ModelTransferView,String>{
        let cancel=Arc::new(AtomicBool::new(false));
        {
            let mut active=self.active.lock().map_err(|_|"Transfer lock unavailable")?;
            if active.contains_key(id){return Err("Transfer already active".into())}
            active.insert(id.into(),Arc::clone(&cancel));
        }
        struct Owner<'a>{owner:&'a ModelTransferManager,id:String}
        impl Drop for Owner<'_>{
            fn drop(&mut self){
                if let Ok(mut active)=self.owner.active.lock(){active.remove(&self.id);}
            }
        }
        let _guard=Owner{owner:self,id:id.into()};
        let mut item=self.load(id)?;
        if !matches!(item.state.as_str(),"REVIEWED"|"RUNNING"|"INTERRUPTED"){
            return Err("Transfer already published or not reviewed".into())
        }
        let dir=self.folder(id)?;
        let part=dir.join("received.part");
        let offset=file_len(&part).unwrap_or(0);
        if offset>item.plan.expected_bytes{return Err("Part exceeds approved size".into())}
        item.state="RUNNING".into();
        self.store(id,&item)?;
        let result=(||{
            if offset<item.plan.expected_bytes{
                let _network=model_provider_transport::download_approved_part(
                    pinned_curl,&item.plan,&part,offset,&cancel,|_|{}
                ).map_err(|e|format!("Verified network transfer interrupted: {e:?}"))?;
            }
            let receipt=model_provider_staging::publish_verified_part(
                &self.root,id,&item.plan,&cancel
            ).map_err(|e|format!("Downloaded bytes not independently verified: {e:?}"))?;
            Ok::<_,String>(receipt)
        })();
        match result{
            Ok(receipt)=>{
                item.state="VERIFIED_AT_PUBLICATION".into();
                item.verified_sha256=Some(receipt.sha256);
                self.store(id,&item)?;
                self.status(id)
            },
            Err(reason)=>{
                item.state="INTERRUPTED".into();
                self.store(id,&item)?;
                Err(reason)
            }
        }
    }
    /// Independently re-read the published file before presenting the path
    /// as currently usable for a separate native GGUF import.
    pub fn verify_published_now(&self,id:&str)->Result<ModelTransferView,String>{
        let record=self.load(id)?;
        if record.state!="VERIFIED_AT_PUBLICATION"{
            return Err("No completed transfer receipt".into())
        }
        let file=self.folder(id)?.join(&record.plan.filename);
        let (bytes,digest)=checked_digest(&file,record.plan.expected_bytes)?;
        if bytes!=record.plan.expected_bytes||digest!=record.plan.sha256 ||
            record.verified_sha256.as_deref()!=Some(digest.as_str()){
            return Err("Published file changed since transfer; no valid import proof".into())
        }
        self.status(id)
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    fn sandbox()->PathBuf{
        let dir=std::env::temp_dir().join(format!("lu-hf-transfer-{}-{}",
            std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir(&dir).unwrap();dir
    }
    fn plan()->ExactFilePlan{
        ExactFilePlan{
            repo_id:"org/model".into(),
            revision:"0123456789abcdef0123456789abcdef01234567".into(),
            filename:"small.gguf".into(),expected_bytes:1024,
            sha256:"0".repeat(64)
        }
    }
    #[test]
    fn no_automatic_resume_after_restart_and_replay_is_refused(){
        let root=sandbox();
        let manager=ModelTransferManager::new(root.join("transfers")).unwrap();
        let view=manager.stage("hf-originalnonce000001",plan(),"apache-2.0").unwrap();
        assert_eq!(view.state,"REVIEWED");
        assert!(manager.stage("hf-originalnonce000001",plan(),"apache-2.0").is_err());
        let dir=manager.folder("hf-originalnonce000001").unwrap();
        fs::write(dir.join("received.part"),&[0u8;24]).unwrap();
        let mut saved=manager.load("hf-originalnonce000001").unwrap();
        saved.state="RUNNING".into();manager.store("hf-originalnonce000001",&saved).unwrap();
        drop(manager);
        let restarted=ModelTransferManager::new(root.join("transfers")).unwrap();
        let observed=restarted.status("hf-originalnonce000001").unwrap();
        assert_eq!(observed.state,"INTERRUPTED_REVIEW_REQUIRED");
        assert!(observed.needs_new_human_review);
        let (identity,_)=restarted.reviewed_plan("hf-originalnonce000001").unwrap();
        assert_eq!(identity.expected_bytes,1024);
        assert!(!restarted.cancel("hf-originalnonce000001").unwrap());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn hostile_or_cross_directory_ids_are_refused(){
        let root=sandbox();
        let m=ModelTransferManager::new(root.join("transfers")).unwrap();
        for id in ["../secret","hf-short","hf-../escape","unknown"]{
            assert!(m.status(id).is_err());
        }
        fs::remove_dir_all(root).unwrap();
    }
}