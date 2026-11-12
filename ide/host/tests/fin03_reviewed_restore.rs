use lu_ide_workspace::{IdeWorkspaceRegistry,NativeDirectoryPicker,IdeError};
use std::{fs,path::{Path,PathBuf},sync::atomic::AtomicU8};
struct Picker(Option<PathBuf>);
impl NativeDirectoryPicker for Picker {
 fn choose_project_directory(&mut self)->Result<Option<PathBuf>,IdeError>{
  Ok(self.0.take())
 }
}
fn fixture()->(PathBuf,PathBuf,IdeWorkspaceRegistry,String){
 let root=std::env::temp_dir().join(format!(
  "lu-ide-fin03-restore-{}-{:016x}",std::process::id(),rand::random::<u64>()));
 let folder=root.join("project");fs::create_dir_all(&folder).unwrap();
 let repo=git2::Repository::init(&folder).unwrap();
 fs::write(folder.join("tracked.txt"),b"original HEAD bytes\n").unwrap();
 let mut index=repo.index().unwrap();
 index.add_path(Path::new("tracked.txt")).unwrap();index.write().unwrap();
 let oid=index.write_tree().unwrap();
 let tree=repo.find_tree(oid).unwrap();
 let who=git2::Signature::now("fixture","fixture@example.invalid").unwrap();
 repo.commit(Some("HEAD"),&who,&who,"initial",&tree,&[]).unwrap();
 drop(tree);drop(index);drop(repo);
 let registry=IdeWorkspaceRegistry::open(root.join("catalog.json")).unwrap();
 let id=registry.register_from_native_picker(&mut Picker(Some(folder.clone())))
  .unwrap().unwrap().project_id;
 (root,folder,registry,id)
}
#[test]
fn restore_preserves_modified_original_in_git_private_backup(){
 let (root,folder,registry,id)=fixture();
 fs::write(folder.join("tracked.txt"),b"owner unsaved modification\n").unwrap();
 let review=registry.review_single_file_restore(&id).unwrap();
 assert!(!review.operation_authorized);
 let receipt=registry.execute_approved_single_file_restore(&review,&AtomicU8::new(0)).unwrap();
 assert!(receipt.original_preserved && receipt.source_restored && !receipt.refs_modified);
 assert_eq!(fs::read(folder.join("tracked.txt")).unwrap(),b"original HEAD bytes\n");
 assert_eq!(fs::read(folder.join(".git").join(receipt.backup_relative_git_path)).unwrap(),
  b"owner unsaved modification\n");
 assert!(!registry.single_file_restore_recovery_required(&id).unwrap());
 drop(registry);fs::remove_dir_all(root).unwrap();
}
#[test]
fn restore_rejects_dirty_index_untracked_file_and_source_change(){
 let (root,folder,registry,id)=fixture();
 fs::write(folder.join("tracked.txt"),b"changed\n").unwrap();
 let review=registry.review_single_file_restore(&id).unwrap();
 fs::write(folder.join("tracked.txt"),b"changed again\n").unwrap();
 assert!(registry.execute_approved_single_file_restore(&review,&AtomicU8::new(0)).is_err());
 assert_eq!(fs::read(folder.join("tracked.txt")).unwrap(),b"changed again\n");
 fs::write(folder.join("untracked.txt"),b"extra").unwrap();
 assert!(registry.review_single_file_restore(&id).is_err());
 fs::remove_file(folder.join("untracked.txt")).unwrap();
 let git=git2::Repository::open(&folder).unwrap();
 let mut index=git.index().unwrap();
 index.add_path(Path::new("tracked.txt")).unwrap();index.write().unwrap();
 assert!(registry.review_single_file_restore(&id).is_err());
 drop(index);drop(git);drop(registry);fs::remove_dir_all(root).unwrap();
}
