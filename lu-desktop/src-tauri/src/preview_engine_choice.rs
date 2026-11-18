//! Host-owned backend selection for the dual-engine Windows preview.
//! Neither model messages nor model metadata can choose a program path.
//! Preferences are bounded native profile data, not executable authority.
use std::{fs::{self,OpenOptions},io::Write,path::Path};

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub(crate) enum BackendChoice { Cpu, Vulkan }
impl BackendChoice {
    pub(crate) fn id(self)->&'static str{
        match self{Self::Cpu=>"cpu",Self::Vulkan=>"vulkan"}
    }
    pub(crate) fn from_id(value:&str)->Result<Self,String>{
        match value{ "cpu"=>Ok(Self::Cpu),"vulkan"=>Ok(Self::Vulkan),
            _=>Err("Unknown engine mode. Choose only CPU or Vulkan".into()) }
    }
}
const SETTINGS_NAME:&str="lu-preview-engine-backend.txt";
pub(crate) fn read_choice(profile:&Path)->Result<BackendChoice,String>{
    let file=profile.join(SETTINGS_NAME);
    let meta=match fs::symlink_metadata(&file){
        Ok(m)=>m,
        Err(e) if e.kind()==std::io::ErrorKind::NotFound=>
            return Ok(BackendChoice::Vulkan), // Radeon preview default
        Err(_)=>return Err("Unable to inspect Lu backend preference".into())
    };
    if !meta.file_type().is_file() || meta.len()==0 || meta.len()>16{
        return Err("Untrusted backend preference entry".into())
    }
    #[cfg(windows)] {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes()&0x400!=0{
            return Err("Backend selection must not be a reparse point".into())
        }
    }
    let bytes=fs::read(&file)
        .map_err(|_|"Cannot read Lu native backend selection")?;
    let raw=std::str::from_utf8(&bytes)
        .map_err(|_|"Backend mode must be UTF-8")?;
    BackendChoice::from_id(raw)
}
pub(crate) fn write_choice(profile:&Path,target:BackendChoice)->Result<(),String>{
    if !profile.is_dir() {return Err("Lu profile directory unavailable".into())}
    let file=profile.join(SETTINGS_NAME);
    if fs::symlink_metadata(&file).is_ok(){
        // Do not follow an unexpected link or overwrite untrusted content.
        read_choice(profile)?;
    }
    let tmp=profile.join(format!("{SETTINGS_NAME}.{}.tmp",std::process::id()));
    let mut output=OpenOptions::new().write(true).create_new(true).open(&tmp)
        .map_err(|_|"Temporary backend setting already exists")?;
    let outcome=(||{
        output.write_all(target.id().as_bytes())
            .map_err(|_|"Unable to persist backend selection")?;
        output.sync_all().map_err(|_|"Backend selection not durable")?;
        drop(output);
        if file.exists() {
            fs::remove_file(&file)
                .map_err(|_|"Could not replace existing backend selection")?;
        }
        fs::rename(&tmp,&file)
            .map_err(|_|"Backend selection rename failed")?;
        Ok(())
    })();
    if outcome.is_err(){let _=fs::remove_file(tmp);}
    outcome
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn backend_is_explicit_and_never_a_model_supplied_executable(){
        assert_eq!(BackendChoice::from_id("cpu").unwrap().id(),"cpu");
        assert_eq!(BackendChoice::from_id("vulkan").unwrap().id(),"vulkan");
        for bad in ["", "CPU", "vulkan; cmd", "../llama.exe", "cuda",
            "vulkan\n", "\\0"]{
            assert!(BackendChoice::from_id(bad).is_err());
        }
    }
    #[test]
    fn checked_setting_survives_restart_and_rejects_tampering(){
        let root=std::env::temp_dir().join(format!(
            "lu-preview-choice-{}-{}",std::process::id(),
            std::time::SystemTime::now().duration_since(
                std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        assert_eq!(read_choice(&root).unwrap(),BackendChoice::Vulkan);
        write_choice(&root,BackendChoice::Cpu).unwrap();
        assert_eq!(read_choice(&root).unwrap(),BackendChoice::Cpu);
        write_choice(&root,BackendChoice::Vulkan).unwrap();
        assert_eq!(read_choice(&root).unwrap(),BackendChoice::Vulkan);
        fs::write(root.join(SETTINGS_NAME),b"../../evil.exe").unwrap();
        assert!(read_choice(&root).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
