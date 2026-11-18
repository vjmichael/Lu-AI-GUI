//! Linux CPU engine package pins are supplied to the compiler, never to the
//! running application. A Windows .exe or an operator-selected executable is
//! not a Linux engine. Keep package files in a root-owned, non-writable tree.
use serde::Deserialize;
use std::{collections::BTreeMap, fs, os::unix::fs::{MetadataExt, PermissionsExt}, path::{Path, PathBuf}};

const NAMES:[&str;8]=["llama-server","libllama-server-impl.so","libggml-base.so.0",
    "libggml-cpu.so.0","libggml.so.0","libllama-common.so.0","libllama.so.0","libmtmd.so.0"];
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Pin { bytes:u64, sha256:String }
type Pins=BTreeMap<String,Pin>;

fn parse_pins(raw:&str)->Result<Pins,String>{
    let pins:Pins=serde_json::from_str(raw).map_err(|_|"Invalid compiled Linux CPU engine pins")?;
    if pins.len()!=NAMES.len() || NAMES.iter().any(|n|!pins.contains_key(*n)) {
        return Err("Linux CPU engine package must pin the complete fixed dependency set".into());
    }
    for p in pins.values(){
        if p.bytes==0 || p.bytes>250_000_000 || !p.sha256.bytes().all(|b|b.is_ascii_hexdigit() && !b.is_ascii_uppercase()) || super::decode_sha(&p.sha256)?.iter().all(|b|*b==0){
            return Err("Invalid Linux CPU engine file identity".into());
        }
    }
    Ok(pins)
}
fn verify_bytes(dir:&Path,pins:&Pins)->Result<(),String>{
    let files=fs::read_dir(dir).map_err(|_|"Linux CPU engine package missing")?
        .collect::<Result<Vec<_>,_>>().map_err(|_|"Linux CPU engine directory unreadable")?;
    if files.len()!=NAMES.len() || files.iter().any(|f|!f.file_name().to_str().is_some_and(|n|pins.contains_key(n))) {
        return Err("Unexpected or missing Linux CPU engine package component".into());
    }
    for (name,pin) in pins { super::check_file(&dir.join(name),&pin.sha256,pin.bytes)?; }
    if fs::metadata(dir.join("llama-server")).map_err(|_|"Linux CPU engine missing")?.permissions().mode()&0o111==0 {
        return Err("Linux CPU engine is not executable".into());
    }
    Ok(())
}
fn protected_path(path:&Path,directory:bool)->Result<(),String>{
    let m=fs::symlink_metadata(path).map_err(|_|"Linux CPU engine path missing")?;
    if (directory && !m.is_dir()) || (!directory && !m.is_file()) || m.uid()!=0 || m.mode()&0o022!=0 {
        return Err("Linux CPU engine package is not an ordinary root-owned protected path".into());
    }
    Ok(())
}
/// Verify the entire package before startup and again before each worker load.
/// The build pipeline must remove build-tree RPATH/RUNPATH and supply exactly
/// these pinned ELF files; the PersistentLlamaBackend uses only this directory
/// as LD_LIBRARY_PATH and removes inherited loader injection variables.
pub(super) fn verify(dir:&Path)->Result<(PathBuf,String),String>{
    let raw=option_env!("LU_LINUX_CPU_ENGINE_PINS").ok_or("Linux CPU runtime was not pinned when Lu was built")?;
    let pins=parse_pins(raw)?;
    let canonical=dir.canonicalize().map_err(|_|"Linux CPU engine package missing")?;
    if canonical!=dir || !canonical.starts_with("/usr/lib") {
        return Err("Linux CPU engine must be in the protected installed library tree".into());
    }
    for p in canonical.ancestors(){protected_path(p,true)?;}
    for name in NAMES{protected_path(&dir.join(name),false)?;}
    verify_bytes(dir,&pins)?;
    Ok((dir.join("llama-server"),pins["llama-server"].sha256.clone()))
}

#[cfg(test)]
mod tests{
    use super::*;
    use sha2::{Digest,Sha256};
    #[test]
    fn exact_bundle_rejects_tampering_symlinks_extra_library_and_missing_pins(){
        let root=std::env::temp_dir().join(super::super::next_id("linux-engine-fixture-"));
        fs::create_dir(&root).unwrap();
        let data=b"synthetic hash-check fixture, not an ELF or production engine";
        let pin=serde_json::json!({"bytes":data.len(),"sha256":format!("{:x}",Sha256::digest(data))});
        let mut raw=serde_json::Map::new();
        for name in NAMES {fs::write(root.join(name),data).unwrap();raw.insert(name.into(),pin.clone());}
        fs::set_permissions(root.join("llama-server"),fs::Permissions::from_mode(0o755)).unwrap();
        let pins=parse_pins(&serde_json::to_string(&raw).unwrap()).unwrap();
        assert!(verify_bytes(&root,&pins).is_ok());
        fs::write(root.join("libllama.so.0"),vec![b'x';data.len()]).unwrap();
        assert!(verify_bytes(&root,&pins).is_err());
        fs::remove_file(root.join("libllama.so.0")).unwrap();
        std::os::unix::fs::symlink(root.join("libmtmd.so.0"),root.join("libllama.so.0")).unwrap();
        assert!(verify_bytes(&root,&pins).is_err());
        fs::remove_file(root.join("libllama.so.0")).unwrap();
        fs::write(root.join("libllama.so.0"),data).unwrap();
        fs::write(root.join("libextra.so"),data).unwrap();
        assert!(verify_bytes(&root,&pins).is_err());
        raw.remove("libllama.so.0");assert!(parse_pins(&serde_json::to_string(&raw).unwrap()).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
