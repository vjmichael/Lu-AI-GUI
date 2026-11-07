use cap_std::{ambient_authority,fs::Dir};
use lu_core::native_archive_split::{
    plan_split,split,inspect,reassemble,read_plan,folder,SplitError,MANIFEST,
    reconcile_publication,PublicationReview,
};
use std::{fs,io::Write,path::PathBuf,sync::atomic::{AtomicU64,Ordering}};
static SEQUENCE:AtomicU64=AtomicU64::new(1);
fn temp()->(PathBuf,Dir){
    let folder=std::env::temp_dir().join(format!("lu-p08-{}-{}",
        std::process::id(),SEQUENCE.fetch_add(1,Ordering::SeqCst)));
    fs::create_dir(&folder).unwrap();
    let root=Dir::open_ambient_dir(&folder,ambient_authority()).unwrap();
    (folder,root)
}
fn archive(path:&PathBuf)->Vec<u8>{
    let mut bytes=vec![0u8;196_637];
    for (i,v) in bytes.iter_mut().enumerate(){*v=((i*31+7)%251) as u8;}
    fs::write(path.join("sample.zip"),&bytes).unwrap();
    bytes
}
#[test]
fn p08_exact_source_parts_and_roundtrip_to_new_output_parent(){
    let (path,root)=temp();
    let bytes=archive(&path);
    let plan=plan_split(&root,"sample.zip",65_536).unwrap();
    assert_eq!(plan.chunks.len(),4);
    let result=split(&root,&plan,"p08-source-12345678").unwrap();
    assert!(result.complete);
    assert_eq!(result.sha256,plan.sha256);
    assert_eq!(result.destination,"sample.zip.lu-parts");
    assert_eq!(inspect(&root,&result.destination).unwrap(),plan);
    let out=path.join("reassembled");fs::create_dir(&out).unwrap();
    let output=Dir::open_ambient_dir(&out,ambient_authority()).unwrap();
    let joined=reassemble(&root,&output,&result.destination,"p08-join-12345678").unwrap();
    assert_eq!(joined.destination,"sample.zip");
    assert_eq!(fs::read(out.join("sample.zip")).unwrap(),bytes);
    assert_eq!(fs::read(path.join("sample.zip")).unwrap(),bytes);
    assert!(matches!(reassemble(&root,&output,&result.destination,"p08-join-87654321"),
        Err(SplitError::Exists)));
    drop(output);drop(root);fs::remove_dir_all(path).unwrap();
}
#[test]
fn p08_staged_resume_rehashes_every_part_and_refuses_corrupt_intermediates(){
    let (path,root)=temp();archive(&path);
    let plan=plan_split(&root,"sample.zip",65_536).unwrap();
    let stage=path.join(".lu-p08-split-p08-resume-12345678");
    fs::create_dir(&stage).unwrap();
    let mut bytes=serde_json::to_vec_pretty(&plan).unwrap();bytes.push(b'\n');
    fs::write(stage.join(MANIFEST),bytes).unwrap();
    let mut first=fs::File::create(stage.join("part-0001.lup")).unwrap();
    first.write_all(&fs::read(path.join("sample.zip")).unwrap()[..65_536]).unwrap();
    first.sync_all().unwrap();drop(first);
    let receipt=split(&root,&plan,"p08-resume-12345678").unwrap();
    assert!(receipt.complete);
    assert!(stage.join("part-0004.lup").exists());
    assert_eq!(inspect(&root,"sample.zip.lu-parts").unwrap().chunks.len(),4);
    drop(root);fs::remove_dir_all(path).unwrap();

    let (path,root)=temp();archive(&path);
    let plan=plan_split(&root,"sample.zip",65_536).unwrap();
    let stage=path.join(".lu-p08-split-p08-badpart-12345678");
    fs::create_dir(&stage).unwrap();
    let mut bytes=serde_json::to_vec_pretty(&plan).unwrap();bytes.push(b'\n');
    fs::write(stage.join(MANIFEST),bytes).unwrap();
    fs::write(stage.join("part-0001.lup"),vec![1u8;65_536]).unwrap();
    assert!(matches!(split(&root,&plan,"p08-badpart-12345678"),
        Err(SplitError::PartCorrupt)));
    assert!(!path.join("sample.zip.lu-parts").exists());
    drop(root);fs::remove_dir_all(path).unwrap();
}
#[test]
fn p08_corrupt_chunk_or_manifest_never_reconstructs_output(){
    let (path,root)=temp();archive(&path);
    let plan=plan_split(&root,"sample.zip",65_536).unwrap();
    split(&root,&plan,"p08-guard-12345678").unwrap();
    let chunks=path.join("sample.zip.lu-parts");
    let out=path.join("output");fs::create_dir(&out).unwrap();
    let output=Dir::open_ambient_dir(&out,ambient_authority()).unwrap();
    fs::write(chunks.join("part-0002.lup"),vec![0u8;65_536]).unwrap();
    assert!(matches!(inspect(&root,"sample.zip.lu-parts"),Err(SplitError::PartCorrupt)));
    assert!(reassemble(&root,&output,"sample.zip.lu-parts","p08-join-12345678").is_err());
    assert!(!out.join("sample.zip").exists());
    let mut altered=plan.clone();
    altered.chunks[0].name="../evil".into();
    let mut serialized=serde_json::to_vec_pretty(&altered).unwrap();
    serialized.push(b'\n');
    fs::write(chunks.join(MANIFEST),serialized).unwrap();
    let inner=root.open_dir("sample.zip.lu-parts").unwrap();
    assert!(matches!(read_plan(&inner),Err(SplitError::Invalid)));
    drop(inner);drop(output);drop(root);fs::remove_dir_all(path).unwrap();
}
#[test]
fn p08_refuses_unsafe_names_stale_sources_and_conflicting_destinations(){
    let (path,root)=temp();archive(&path);
    let plan=plan_split(&root,"sample.zip",65_536).unwrap();
    assert!(plan_split(&root,"../sample.zip",65_536).is_err());
    assert!(matches!(plan_split(&root,"sample.zip",2),Err(SplitError::Invalid)));
    assert!(matches!(plan_split(&root,"sample.txt",65_536),
        Err(SplitError::Unsupported)|Err(SplitError::Invalid)));
    assert!(folder("../../bad.zip").is_err());
    let modified=vec![9u8;196_637];fs::write(path.join("sample.zip"),modified).unwrap();
    assert!(matches!(split(&root,&plan,"p08-stale-12345678"),Err(SplitError::Changed)));
    assert!(!path.join("sample.zip.lu-parts").exists());
    drop(root);fs::remove_dir_all(path).unwrap();
}
#[cfg(unix)]
#[test]
fn p08_refuses_source_and_part_symlink_aliases(){
    use std::os::unix::fs::symlink;
    let (path,root)=temp();archive(&path);
    symlink("sample.zip",path.join("alias.zip")).unwrap();
    assert!(matches!(plan_split(&root,"alias.zip",65_536),Err(SplitError::Invalid)));
    let plan=plan_split(&root,"sample.zip",65_536).unwrap();
    split(&root,&plan,"p08-links-12345678").unwrap();
    let p=path.join("sample.zip.lu-parts/part-0001.lup");
    fs::remove_file(&p).unwrap();
    symlink("../sample.zip",&p).unwrap();
    assert!(inspect(&root,"sample.zip.lu-parts").is_err());
    drop(root);fs::remove_dir_all(path).unwrap();
}

#[test]
fn p08_recovery_inspects_partial_publication_without_replaying_or_deleting(){
    let (path,root)=temp();archive(&path);
    let plan=plan_split(&root,"sample.zip",65_536).unwrap();
    assert_eq!(reconcile_publication(&root,&plan).unwrap(),PublicationReview::Absent);
    let name=folder("sample.zip").unwrap();
    fs::create_dir(path.join(&name)).unwrap();
    fs::write(path.join(&name).join("part-0001.lup"),
        &fs::read(path.join("sample.zip")).unwrap()[..65_536]).unwrap();
    assert_eq!(reconcile_publication(&root,&plan).unwrap(),
        PublicationReview::Incomplete{verified_parts:1,expected_parts:4});
    assert!(path.join(&name).join("part-0001.lup").exists());
    // The manifest is a completion marker. A partial group that falsely
    // contains it must NOT be accepted or silently reconstructed.
    let mut manifest=serde_json::to_vec_pretty(&plan).unwrap();manifest.push(b'\n');
    fs::write(path.join(&name).join(MANIFEST),&manifest).unwrap();
    assert!(matches!(reconcile_publication(&root,&plan),
        Err(SplitError::IncompletePublication)));
    fs::remove_file(path.join(&name).join(MANIFEST)).unwrap();
    fs::write(path.join(&name).join("unexpected.txt"),b"untrusted").unwrap();
    assert!(matches!(reconcile_publication(&root,&plan),
        Err(SplitError::StageConflict)));
    assert!(path.join(&name).join("unexpected.txt").exists());
    drop(root);fs::remove_dir_all(path).unwrap();
}
