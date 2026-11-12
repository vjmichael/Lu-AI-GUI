use lu_ide_workspace::{IdeWorkspaceRegistry,NativeDirectoryPicker,IdeError};
use std::{fs,path::{Path,PathBuf},sync::atomic::AtomicU8};
struct Picker(Option<PathBuf>);
impl NativeDirectoryPicker for Picker{
 fn choose_project_directory(&mut self)->Result<Option<PathBuf>,IdeError>{
  Ok(self.0.take())
 }
}
#[test]
fn reviewed_one_file_stash_is_local_and_preserves_original_bytes(){
 let root=std::env::temp_dir().join(format!("fin03-stash-{}-{:016x}",
  std::process::id(),rand::random::<u64>()));
 let folder=root.join("project");fs::create_dir_all(&folder).unwrap();
 let repo=git2::Repository::init(&folder).unwrap();
 fs::write(folder.join("tracked.txt"),b"head contents\n").unwrap();
 let mut index=repo.index().unwrap();
 index.add_path(Path::new("tracked.txt")).unwrap();index.write().unwrap();
 let tree=repo.find_tree(index.write_tree().unwrap()).unwrap();
 let who=git2::Signature::now("Fixture","fixture@example.invalid").unwrap();
 repo.commit(Some("HEAD"),&who,&who,"initial",&tree,&[]).unwrap();
 drop(tree);drop(index);drop(repo);
 let registry=IdeWorkspaceRegistry::open(root.join("catalog.json")).unwrap();
 let id=registry.register_from_native_picker(&mut Picker(Some(folder.clone())))
  .unwrap().unwrap().project_id;
 fs::write(folder.join("tracked.txt"),b"private unsaved change\n").unwrap();
 let review=registry.review_one_file_stash_save(&id).unwrap();
 assert!(!review.operation_authorized && !review.refs_modified);
 let stale=registry.review_one_file_stash_save(&id).unwrap();
 assert_eq!(review.review_sha256,stale.review_sha256);
 let receipt=registry.execute_reviewed_one_file_stash_save(
  &review,&AtomicU8::new(0)).unwrap();
 assert!(receipt.stash_ref_changed && receipt.original_preserved);
 assert!(!receipt.installed_accepted);
 assert_eq!(fs::read(folder.join("tracked.txt")).unwrap(),b"head contents\n");
 assert_eq!(fs::read(folder.join(".git").join(&receipt.original_backup_name))
  .unwrap(),b"private unsaved change\n");
 assert!(!registry.one_file_stash_recovery_required(&id).unwrap());
 let git=git2::Repository::open(&folder).unwrap();
 assert_eq!(git.find_reference("refs/stash").unwrap().target().unwrap().to_string(),
  receipt.stash_oid);
 drop(git);drop(registry);fs::remove_dir_all(root).unwrap();
}
#[test]
fn stash_review_rejects_untracked_file_and_interrupted_marker(){
 let root=std::env::temp_dir().join(format!("fin03-stash-negative-{}-{:016x}",
  std::process::id(),rand::random::<u64>()));
 let folder=root.join("project");fs::create_dir_all(&folder).unwrap();
 let repo=git2::Repository::init(&folder).unwrap();
 fs::write(folder.join("tracked.txt"),b"head\n").unwrap();
 let mut index=repo.index().unwrap();index.add_path(Path::new("tracked.txt")).unwrap();index.write().unwrap();
 let tree=repo.find_tree(index.write_tree().unwrap()).unwrap();
 let who=git2::Signature::now("Fixture","fixture@example.invalid").unwrap();
 repo.commit(Some("HEAD"),&who,&who,"initial",&tree,&[]).unwrap();
 drop(tree);drop(index);drop(repo);
 let registry=IdeWorkspaceRegistry::open(root.join("catalog.json")).unwrap();
 let id=registry.register_from_native_picker(&mut Picker(Some(folder.clone())))
  .unwrap().unwrap().project_id;
 fs::write(folder.join("tracked.txt"),b"changed\n").unwrap();
 fs::write(folder.join("extra.txt"),b"other").unwrap();
 assert!(registry.review_one_file_stash_save(&id).is_err());
 fs::remove_file(folder.join("extra.txt")).unwrap();
 let review=registry.review_one_file_stash_save(&id).unwrap();
 fs::write(folder.join("tracked.txt"),b"different change\n").unwrap();
 assert!(registry.execute_reviewed_one_file_stash_save(&review,
  &AtomicU8::new(0)).is_err());
 fs::write(folder.join(".git/lu-ide-fin03-stash-active"),b"a".repeat(64)).unwrap();
 assert!(registry.review_one_file_stash_save(&id).is_err());
 assert!(registry.one_file_stash_recovery_required(&id).unwrap());
 drop(registry);fs::remove_dir_all(root).unwrap();
}
