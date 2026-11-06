//! The trusted Lu executable's *internal* extraction-worker entrypoint.
//! No Tauri, model, shell, network or cached user grant is initialized here.
//! A malicious or mismatched worker result can never authorize a host receipt.
use std::{ffi::{OsStr,OsString},path::Path};
use cap_std::{ambient_authority,fs::Dir};
use lu_core::{
    native_archive_process::opened_directory_identity,
    native_archive_tree::{extract_archive_tree_new,preview_archive_tree,observe_tree_completion},
};
fn execute(args:&[OsString])->Result<(),()>{
    if args.len()!=8||args[0]!=OsStr::new("--lu-archive-extract-worker"){
        return Err(())
    }
    let source_parent=Path::new(&args[1]);
    let source_name=args[2].to_str().ok_or(())?;
    let destination_parent=Path::new(&args[3]);
    let target=args[4].to_str().ok_or(())?;
    let sha=args[5].to_str().ok_or(())?;
    let src_id=args[6].to_str().ok_or(())?;
    let dst_id=args[7].to_str().ok_or(())?;
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
pub fn run_archive_process_worker()->i32{
    let args=std::env::args_os().skip(1).collect::<Vec<_>>();
    if execute(&args).is_ok(){0}else{3}
}
#[cfg(test)]
mod tests{
    use super::*;
    use std::{fs,ffi::OsString};
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
        ];
        assert!(execute(&args).is_ok());
        assert_eq!(fs::read(root.join("new-folder/file.txt")).unwrap(),b"approved");
        assert!(execute(&args).is_err(),"no-clobber replay is refused");
        drop(dir);fs::remove_dir_all(root).unwrap();
    }
}
