use lu_ide_workspace::{IdeError, IdeWorkspaceRegistry, NativeDirectoryPicker};
use std::{fs, path::PathBuf};
struct Picker(Option<PathBuf>);
impl NativeDirectoryPicker for Picker {
 fn choose_project_directory(&mut self)->Result<Option<PathBuf>,IdeError>{Ok(self.0.take())}
}
fn fixture()->(PathBuf,IdeWorkspaceRegistry,String){
 let root=std::env::temp_dir().join(format!("lu-ide-dev13-{}-{:016x}",std::process::id(),rand::random::<u64>()));
 let project=root.join("project");
 fs::create_dir_all(project.join("src")).unwrap();
 fs::create_dir_all(project.join("docs")).unwrap();
 fs::write(project.join("src/main.rs"),b"fn main() {}").unwrap();
 fs::write(project.join("docs/keep.rs"),b"not overwritten").unwrap();
 let registry=IdeWorkspaceRegistry::open(root.join("catalog.json")).unwrap();
 let id=registry.register_from_native_picker(&mut Picker(Some(project))).unwrap().unwrap().project_id;
 (root,registry,id)
}
#[test]
fn move_no_clobber_and_source_revision(){
 let (root,r,id)=fixture();
 let sha=r.preview_source_file(&id,"src/main.rs").unwrap().revision;
 assert_eq!(r.move_source_file(&id,"src/main.rs","docs/keep.rs",&sha).unwrap_err().code(),"SOURCE_TARGET_ALREADY_EXISTS");
 assert_eq!(fs::read(root.join("project/docs/keep.rs")).unwrap(),b"not overwritten".to_vec());
 assert_eq!(r.move_source_file(&id,"src/main.rs","docs/new.rs",&"a".repeat(64)).unwrap_err().code(),"FILE_CHANGED_ON_DISK");
 let receipt=r.move_source_file(&id,"src/main.rs","docs/new.rs",&sha).unwrap();
 assert_eq!(receipt.operation,"move");
 assert_eq!(receipt.current_relative_path.as_deref(),Some("docs/new.rs"));
 assert!(!root.join("project/src/main.rs").exists());
 assert_eq!(fs::read(root.join("project/docs/new.rs")).unwrap(),b"fn main() {}".to_vec());
 fs::remove_dir_all(root).unwrap();
}
#[test]
fn refuse_traversal_case_alias_and_directory_moves(){
 let (root,r,id)=fixture();
 let sha=r.preview_source_file(&id,"src/main.rs").unwrap().revision;
 for path in ["../escape","/tmp/escape",".git/config",".lu-ide-trash/payload","docs/../../outside","docs\\outside"]{
  assert_eq!(r.move_source_file(&id,"src/main.rs",path,&sha).unwrap_err().code(),"INVALID_RELATIVE_PATH","{path}");
 }
 assert_eq!(r.move_source_file(&id,"src/main.rs","src/MAIN.rs",&sha).unwrap_err().code(),"SOURCE_TARGET_ALREADY_EXISTS");
 assert_eq!(r.move_source_file(&id,"src","docs/src",&sha).unwrap_err().code(),"NOT_A_FILE");
 assert!(root.join("project/src/main.rs").exists());
 fs::remove_dir_all(root).unwrap();
}
#[test]
fn trashed_file_survives_registry_restart_and_restores_without_overwrite(){
 let (root,r,id)=fixture();
 let sha=r.preview_source_file(&id,"src/main.rs").unwrap().revision;
 let receipt=r.trash_source_file(&id,"src/main.rs",&sha).unwrap();
 let token=receipt.trash_token.unwrap();
 assert!(!root.join("project/src/main.rs").exists());
 let entries=r.list_trashed_source_files(&id).unwrap();
 assert_eq!(entries.len(),1);
 assert!(entries[0].payload_present && !entries[0].original_present);
 assert!(r.list_source_directory(&id,"").unwrap().entries.iter().all(|x|x.name!=".lu-ide-trash"));
 assert_eq!(r.preview_source_file(&id,&format!(".lu-ide-trash/{token}.payload")).unwrap_err().code(),"INVALID_RELATIVE_PATH");
 drop(r);
 let r=IdeWorkspaceRegistry::open(root.join("catalog.json")).unwrap();
 r.reopen(&id).unwrap();
 let restored=r.restore_source_file(&id,&token).unwrap();
 assert_eq!(restored.operation,"restore");
 assert_eq!(restored.revision,sha);
 assert_eq!(fs::read(root.join("project/src/main.rs")).unwrap(),b"fn main() {}".to_vec());
 assert!(r.list_trashed_source_files(&id).unwrap().is_empty());
 fs::remove_dir_all(root).unwrap();
}
#[test]
fn restore_external_collision_keeps_recovery_payload_and_external_content(){
 let (root,r,id)=fixture();
 let sha=r.preview_source_file(&id,"src/main.rs").unwrap().revision;
 let token=r.trash_source_file(&id,"src/main.rs",&sha).unwrap().trash_token.unwrap();
 fs::write(root.join("project/src/main.rs"),b"external").unwrap();
 assert_eq!(r.restore_source_file(&id,&token).unwrap_err().code(),"SOURCE_TARGET_ALREADY_EXISTS");
 assert_eq!(fs::read(root.join("project/src/main.rs")).unwrap(),b"external".to_vec());
 assert!(r.list_trashed_source_files(&id).unwrap()[0].payload_present);
 fs::remove_dir_all(root).unwrap();
}
#[test]
fn invalid_token_and_closed_project_are_denied(){
 let (root,r,id)=fixture();
 for token in ["../escape","ide-short","ide-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"]{
  assert_eq!(r.restore_source_file(&id,token).unwrap_err().code(),"INVALID_RELATIVE_PATH");
 }
 r.close(&id).unwrap();
 assert_eq!(r.trash_source_file(&id,"src/main.rs",&"a".repeat(64)).unwrap_err().code(),"PROJECT_NOT_OPEN");
 assert!(root.join("project/src/main.rs").exists());
 fs::remove_dir_all(root).unwrap();
}
#[cfg(unix)]
#[test]
fn symlink_trash_directory_is_never_followed(){
 use std::os::unix::fs::symlink;
 let (root,r,id)=fixture();
 fs::create_dir(root.join("outside")).unwrap();
 symlink(root.join("outside"),root.join("project/.lu-ide-trash")).unwrap();
 let sha=r.preview_source_file(&id,"src/main.rs").unwrap().revision;
 assert_eq!(r.trash_source_file(&id,"src/main.rs",&sha).unwrap_err().code(),"NOT_A_DIRECTORY");
 assert!(root.join("project/src/main.rs").exists());
 assert_eq!(fs::read_dir(root.join("outside")).unwrap().count(),0);
 fs::remove_dir_all(root).unwrap();
}
