//! The trusted Lu executable's *internal* extraction-worker entrypoint.
//! No Tauri, model, shell, network or cached user grant is initialized here.
//! A malicious or mismatched worker result can never authorize a host receipt.
use std::{ffi::{OsStr,OsString},path::Path};
use cap_std::{ambient_authority,fs::Dir};
use lu_core::{
    native_archive_process::opened_directory_identity,
    native_archive_tree::{extract_archive_tree_new,preview_archive_tree,observe_tree_completion},
};
fn check_parent(raw:&OsString)->Result<(),()>{
    let pid=raw.to_str().ok_or(())?.parse::<u32>().map_err(|_|())?;
    if pid<=1{return Err(())}
    #[cfg(all(target_os="linux",not(test)))]
    lu_core::native_archive_process::bind_archive_worker_parent(pid).map_err(|_|())?;
    // In Windows, Job Object kill-on-close provides the parent-death fence.
    // Test harnesses must not install Linux PDEATHSIG or RLIMIT_AS on themselves.
    Ok(())
}
fn execute(args:&[OsString])->Result<(),()>{
    if args.len()!=9||args[0]!=OsStr::new("--lu-archive-extract-worker"){
        return Err(())
    }
    let source_parent=Path::new(&args[1]);
    let source_name=args[2].to_str().ok_or(())?;
    let destination_parent=Path::new(&args[3]);
    let target=args[4].to_str().ok_or(())?;
    let sha=args[5].to_str().ok_or(())?;
    let src_id=args[6].to_str().ok_or(())?;
    let dst_id=args[7].to_str().ok_or(())?;
    check_parent(&args[8])?;
    if !source_parent.is_absolute()||!destination_parent.is_absolute()||
        src_id.is_empty()||dst_id.is_empty(){
        return Err(())
    }
    let source=Dir::open_ambient_dir(source_parent,ambient_authority()).map_err(|_|())?;
    let destination=Dir::open_ambient_dir(destination_parent,ambient_authority())
        .map_err(|_|())?;
    if opened_directory_identity(&source).map_err(|_|())?!=src_id||
        opened_directory_identity(&destination).map_err(|_|())?!=dst_id {
        return Err(())
    }
    let manifest=preview_archive_tree(&source,source_name).map_err(|_|())?;
    if manifest.sha256!=sha{return Err(())}
    let actual=extract_archive_tree_new(
        &source,source_name,&destination,target,sha
    ).map_err(|_|())?;
    if actual.manifest_sha256!=manifest.sha256 ||
        !observe_tree_completion(&destination,target,&manifest).map_err(|_|())?{
        return Err(())
    }
    Ok(())
}
fn execute_preview(args:&[OsString])->Result<(),()>{
    use std::{fs::{self,OpenOptions},io::Write};
    if args.len()!=6||args[0]!=OsStr::new("--lu-archive-preview-worker"){
        return Err(())
    }
    let source_path=Path::new(&args[1]);
    let source_name=args[2].to_str().ok_or(())?;
    let output_path=Path::new(&args[3]);
    let identity=args[4].to_str().ok_or(())?;
    check_parent(&args[5])?;
    if !source_path.is_absolute()||!output_path.is_absolute()||identity.is_empty(){
        return Err(())
    }
    let source=Dir::open_ambient_dir(source_path,ambient_authority()).map_err(|_|())?;
    if opened_directory_identity(&source).map_err(|_|())?!=identity{return Err(())}
    let manifest=preview_archive_tree(&source,source_name).map_err(|_|())?;
    if !lu_core::native_archive_tree::verify_tree_manifest(&manifest){
        return Err(())
    }
    let bytes=serde_json::to_vec(&manifest).map_err(|_|())?;
    if bytes.len()>1024*1024{return Err(())}
    let mut file=OpenOptions::new().create_new(true).write(true)
        .open(output_path).map_err(|_|())?;
    file.write_all(&bytes).map_err(|_|())?;
    file.sync_all().map_err(|_|())?;
    drop(file);
    // A file is a transport witness, not filesystem write permission.
    let meta=fs::symlink_metadata(output_path).map_err(|_|())?;
    if !meta.is_file()||meta.file_type().is_symlink(){return Err(())}
    Ok(())
}
pub fn run_archive_process_worker()->i32{
    let args=std::env::args_os().skip(1).collect::<Vec<_>>();
    if args.first().is_some_and(|s|s==OsStr::new("--lu-archive-preview-worker")){
        if execute_preview(&args).is_ok(){0}else{3}
    }else if execute(&args).is_ok(){0}else{3}
}
#[cfg(test)]
mod tests{
    use super::*;
    use std::{fs,ffi::OsString};
    #[test]
    fn preview_worker_returns_exact_manifest_without_extraction(){
        let root=std::env::temp_dir().join(format!(
            "lu-archive-preview-test-{}",std::process::id()
        ));
        let _=fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let dir=Dir::open_ambient_dir(&root,ambient_authority()).unwrap();
        fs::write(root.join("file.txt"),b"preview").unwrap();
        lu_core::native_linux_archive::create_linux_archive_new(
            &dir,"preview.tar",&["file.txt".into()],
            "archive-preview-stage-123456789"
        ).unwrap();
        let expected=preview_archive_tree(&dir,"preview.tar").unwrap();
        let output=root.join("preview.json");
        let args:Vec<OsString>=vec![
            "--lu-archive-preview-worker".into(),root.as_os_str().into(),
            "preview.tar".into(),output.as_os_str().into(),
            opened_directory_identity(&dir).unwrap().into(),
            std::process::id().to_string().into(),
        ];
        assert!(execute_preview(&args).is_ok());
        let restored:lu_core::native_archive_tree::TreeManifest=
            serde_json::from_slice(&fs::read(&output).unwrap()).unwrap();
        assert_eq!(restored,expected);
        assert!(execute_preview(&args).is_err(),"no-clobber preview");
        assert!(!root.join("preview-extracted").exists());
        drop(dir);fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn mismatched_parent_identity_refuses_any_publication(){
        let root=std::env::temp_dir().join(format!(
            "lu-archive-worker-{}-{}",std::process::id(),"identity"
        ));
        let _=fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let source=Dir::open_ambient_dir(&root,ambient_authority()).unwrap();
        fs::write(root.join("a.txt"),b"data").unwrap();
        lu_core::native_linux_archive::create_linux_archive_new(
            &source,"payload.tar",&["a.txt".into()],
            "archive-worker-stage-123456789"
        ).unwrap();
        let manifest=preview_archive_tree(&source,"payload.tar").unwrap();
        let identity=opened_directory_identity(&source).unwrap();
        let args:Vec<OsString>=vec![
            "--lu-archive-extract-worker".into(),
            root.as_os_str().into(),"payload.tar".into(),
            root.as_os_str().into(),"out".into(),
            manifest.sha256.into(),"wrong-source-identity".into(),identity,
            std::process::id().to_string().into(),
        ];
        assert!(execute(&args).is_err());
        assert!(!root.join("out").exists());
        drop(source);fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn valid_host_bound_worker_extraction_reconciles_full_tree(){
        let root=std::env::temp_dir().join(format!(
            "lu-archive-worker-{}-{}",std::process::id(),"valid"
        ));
        let _=fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let dir=Dir::open_ambient_dir(&root,ambient_authority()).unwrap();
        fs::write(root.join("file.txt"),b"approved").unwrap();
        lu_core::native_linux_archive::create_linux_archive_new(
            &dir,"approved.tar",&["file.txt".into()],
            "archive-worker-stage-987654321"
        ).unwrap();
        let manifest=preview_archive_tree(&dir,"approved.tar").unwrap();
        let identity=opened_directory_identity(&dir).unwrap();
        let args:Vec<OsString>=vec![
            "--lu-archive-extract-worker".into(),root.as_os_str().into(),
            "approved.tar".into(),root.as_os_str().into(),"new-folder".into(),
            manifest.sha256.into(),identity.clone().into(),identity.into(),
            std::process::id().to_string().into(),
        ];
        assert!(execute(&args).is_ok());
        assert_eq!(fs::read(root.join("new-folder/file.txt")).unwrap(),b"approved");
        assert!(execute(&args).is_err(),"no-clobber replay is refused");
        drop(dir);fs::remove_dir_all(root).unwrap();
    }
}
