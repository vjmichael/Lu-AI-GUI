use lu_ide_workspace::{IdeError, IdeWorkspaceRegistry, NativeDirectoryPicker};
use std::{fs, path::PathBuf};
struct Picker(Option<PathBuf>);
impl NativeDirectoryPicker for Picker {
 fn choose_project_directory(&mut self)->Result<Option<PathBuf>,IdeError>{Ok(self.0.take())}
}
fn fixture()->(PathBuf,IdeWorkspaceRegistry,String) {
 let root=std::env::temp_dir().join(format!("lu-ide-index-{}-{:016x}",
  std::process::id(),rand::random::<u64>()));
 let project=root.join("project");
 fs::create_dir_all(&project).unwrap();
 let registry=IdeWorkspaceRegistry::open(root.join("catalog.json")).unwrap();
 let id=registry.register_from_native_picker(&mut Picker(Some(project)))
  .unwrap().unwrap().project_id;
 (root,registry,id)
}
#[test]
fn native_index_handles_more_than_legacy_five_hundred_files_without_webview_loops(){
 let (root,r,id)=fixture();
 let project=root.join("project");
 for dir in 0..24 {
  let folder=project.join(format!("src-{dir:02}"));
  fs::create_dir(&folder).unwrap();
  for file in 0..36 {
   fs::write(folder.join(format!("file-{file:03}.rs")),b"source").unwrap();
  }
 }
 let index=r.index_project_files(&id).unwrap();
 println!("DEV01_INDEX_REAL_NATIVE_METRICS=files:{};dirs:{};examined:{};elapsed_ms:{};partial:{}",
  index.paths.len(),index.directories_visited,index.entries_examined,index.elapsed_ms,index.truncated);
 assert_eq!(index.project_id,id);
 assert_eq!(index.paths.len(),864);
 assert_eq!(index.directories_visited,25);
 assert!(!index.truncated);
 assert!(index.paths.contains(&"src-00/file-000.rs".to_owned()));
 fs::remove_dir_all(root).unwrap();
}
#[test]
fn oversized_native_directory_yields_visible_partial_budget_not_full_success(){
 let (root,r,id)=fixture();
 for i in 0..1005 {
  fs::write(root.join("project").join(format!("file-{i:04}.rs")),b"x").unwrap();
 }
 let indexed=r.index_project_files(&id).unwrap();
 assert!(indexed.truncated);
 assert!(indexed.entries_examined<=1000);
 assert!(indexed.paths.len()<=1000);
 fs::remove_dir_all(root).unwrap();
}
#[test]
fn project_ignore_paths_are_not_scanned_or_disclosed(){
 let (root,r,id)=fixture();
 let project=root.join("project");
 for ignored in [".git","node_modules","target",".lu-ide-trash"] {
  fs::create_dir(project.join(ignored)).unwrap();
  fs::write(project.join(ignored).join("private.js"),b"private").unwrap();
 }
 fs::write(project.join("real.rs"),b"real").unwrap();
 let result=r.index_project_files(&id).unwrap();
 assert_eq!(result.paths,vec!["real.rs".to_owned()]);
 assert_eq!(result.ignored_entries,4);
 fs::remove_dir_all(root).unwrap();
}
#[cfg(unix)]
#[test]
fn symlink_files_and_directories_never_cross_held_project_root(){
 use std::os::unix::fs::symlink;
 let (root,r,id)=fixture();
 fs::write(root.join("outside"),b"private").unwrap();
 symlink(root.join("outside"),root.join("project/external")).unwrap();
 let entries=r.index_project_files(&id).unwrap();
 assert!(entries.paths.is_empty());
 assert!(entries.ignored_entries>=1);
 fs::remove_dir_all(root).unwrap();
}
#[test]
fn a_fresh_native_index_sees_current_files_and_fails_if_project_closed(){
 let (root,r,id)=fixture();
 assert!(r.index_project_files(&id).unwrap().paths.is_empty());
 fs::write(root.join("project/updated.rs"),b"new").unwrap();
 assert_eq!(r.index_project_files(&id).unwrap().paths,vec!["updated.rs".to_owned()]);
 r.close(&id).unwrap();
 assert_eq!(r.index_project_files(&id).unwrap_err().code(),"PROJECT_NOT_OPEN");
 fs::remove_dir_all(root).unwrap();
}
