//! TOOLS-09 disposable first-party search scope and negative fixtures.
//! Native file handles only. No shell, IDE indexer, or model-granted path.
use super::*;
use prost::Message;
use std::{fs, path::PathBuf, sync::atomic::{AtomicU64,Ordering},
    time::{Duration,Instant}};
use sha2::{Digest,Sha256};
static NEXT:AtomicU64=AtomicU64::new(0);

fn sandbox()->(PathBuf,Dir){
    let path=std::env::temp_dir().join(format!(
        "lu-tools09-{}-{}",std::process::id(),
        NEXT.fetch_add(1,Ordering::Relaxed)));
    fs::create_dir_all(&path).unwrap();
    let root=Dir::open_ambient_dir(&path,ambient_authority()).unwrap();
    (path,root)
}
fn request(query:&str, recursive:bool)->proto::SearchTextRequest{
    proto::SearchTextRequest{
        path:String::new(),query:query.into(),max_files:Some(64),
        max_matches:Some(128),recursive:Some(recursive),
        file_suffix:None,
    }
}
fn decode(out:ToolOutput)->proto::SearchTextResult{
    proto::SearchTextResult::decode(out.payload.bytes.as_slice()).unwrap()
}
#[test]
fn tools09_nested_utf8_filter_and_original_digest_are_stable(){
    let (path,root)=sandbox();
    fs::create_dir_all(path.join("a/b/c")).unwrap();
    fs::write(path.join("root.rs"),"needle root\n").unwrap();
    fs::write(path.join("a/b/c/deep.rs"),"needle nested\n").unwrap();
    fs::write(path.join("a/b/c/other.txt"),"needle ignored\n").unwrap();
    let mut req=request("needle",true);
    req.file_suffix=Some(".rs".into());
    let out=decode(search(&root,req).unwrap());
    assert_eq!(out.hits.len(),2);
    assert!(!out.truncated, "{:?}",out.limitations);
    assert_eq!(out.scanned_directories,4);
    let deep=out.hits.iter().find(|h|h.path=="a/b/c/deep.rs").unwrap();
    assert_eq!(deep.size_bytes,"needle nested\n".len() as u64);
    assert_eq!(deep.sha256_hex,format!("{:x}",Sha256::digest(b"needle nested\n")));
    assert_eq!(deep.line_number,1);
    let plain=decode(search(&root,request("needle",false)).unwrap());
    assert_eq!(plain.hits.len(),1);
    assert_eq!(plain.hits[0].path,"root.rs");
    let no_match=decode(search(&root,request("absent phrase",true)).unwrap());
    assert!(no_match.hits.is_empty());
    assert!(!no_match.truncated);
    let v=SearchCodec.decode(&ProtoPayload{
        type_url:SEARCH_OUT.into(),bytes:out.encode_to_vec()
    }).unwrap();
    assert_eq!(v["status"],"COMPLETE");
    assert_eq!(v["hits"][0]["sha256_hex"].as_str().unwrap().len(),64);
    drop(root);fs::remove_dir_all(path).unwrap();
}
#[test]
fn tools09_binary_large_and_changed_data_never_produce_complete_no_match(){
    let (path,root)=sandbox();
    fs::write(path.join("corrupt.txt"),[b'x',0xff,b'y']).unwrap();
    fs::write(path.join("huge.txt"),vec![b'z';65537]).unwrap();
    fs::write(path.join("small.txt"),"needle\n").unwrap();
    fs::write(path.join(".env"),"needle credential\n").unwrap();
    fs::write(path.join("credentials"),"needle secret\n").unwrap();
    let reply=decode(search(&root,request("needle",true)).unwrap());
    assert_eq!(reply.hits.len(),1);
    assert!(reply.truncated);
    assert!(reply.limitations.contains(&"INVALID_UTF8".into()));
    assert!(reply.limitations.contains(&"FILE_SIZE_LIMIT".into()));
    assert!(reply.hits.iter().all(|h|h.path!=".env"&&h.path!="credentials"));
    assert!(reply.skipped_entries>=2);
    let before=root.symlink_metadata("small.txt").unwrap();
    let changed=root.symlink_metadata("huge.txt").unwrap();
    assert!(!same_file_identity(&before,&changed));
    drop(root);fs::remove_dir_all(path).unwrap();
}
#[test]
fn tools09_scope_filter_and_path_attack_are_denied_without_ambient_open(){
    let (path,root)=sandbox();
    fs::create_dir(path.join("safe")).unwrap();
    fs::write(path.join("safe/content.rs"),"needle safe").unwrap();
    for bad in ["../outside","safe/../outside","/absolute","safe\\escape",
        "safe//file","safe/.",".git","safe/.hidden","C:/windows",
        "safe/\0secret"]{
        let mut req=request("needle",true);
        req.path=bad.into();
        assert!(search(&root,req).is_err(),"invalid path admitted: {bad:?}");
    }
    for bad in ["../hidden","*.rs","a/b",".env","\0"]{
        let mut req=request("needle",true);
        req.file_suffix=Some(bad.into());
        assert!(search(&root,req).is_err());
    }
    let mut scoped=request("needle",true);
    scoped.path="safe".into();
    let reply=decode(search(&root,scoped).unwrap());
    assert_eq!(reply.hits.len(),1);
    assert_eq!(reply.hits[0].path,"safe/content.rs");
    drop(root);fs::remove_dir_all(path).unwrap();
}
#[test]
fn tools09_limit_reasons_and_cooperative_cancel_are_explicit(){
    let (path,root)=sandbox();
    for i in 0..70{fs::write(path.join(format!("f{i:03}.rs")),"needle").unwrap();}
    let mut req=request("needle",false);
    req.max_files=Some(3);
    let limited=decode(search(&root,req).unwrap());
    assert_eq!(limited.scanned_files,3);
    assert!(limited.truncated);
    assert!(limited.limitations.contains(&"FILE_LIMIT".into()));
    let polls=std::sync::atomic::AtomicU32::new(0);
    let aborted=search_with_cancel(&root,request("needle",true),||{
        // Synchronous Fn closure with interior test-only atomic state.
        polls.fetch_add(1,Ordering::Relaxed)>=2
    },Instant::now());
    // This test exercises the branch without an OS child or model.
    assert_eq!(aborted.unwrap_err().code,"CANCELLED");
    let timeout=decode(search_with_cancel(
        &root,request("needle",true),||false,
        Instant::now()-Duration::from_secs(4)).unwrap());
    assert!(timeout.truncated);
    assert!(timeout.limitations.contains(&"TIME_BUDGET".into()));
    drop(root);fs::remove_dir_all(path).unwrap();
}
#[test]
fn tools09_entry_and_match_ceiling_honest_partial_markers(){
    let (path,root)=sandbox();
    let mut many=String::new();
    for _ in 0..140{many.push_str("needle\n");}
    fs::write(path.join("many.txt"),many).unwrap();
    let mut req=request("needle",false);
    req.max_matches=Some(2);
    let out=decode(search(&root,req).unwrap());
    assert_eq!(out.hits.len(),2);
    assert!(out.truncated);
    assert!(out.limitations.contains(&"MATCH_LIMIT".into()));
    drop(root);fs::remove_dir_all(path).unwrap();
}
#[cfg(unix)]
#[test]
fn tools09_symlinked_external_directory_and_file_are_never_read(){
    use std::os::unix::fs::symlink;
    let (path,root)=sandbox();
    let external=path.with_extension("private");
    fs::create_dir(&external).unwrap();
    fs::write(external.join("secret.txt"),"needle PRIVILEGED").unwrap();
    symlink(&external,path.join("out-of-scope")).unwrap();
    symlink(external.join("secret.txt"),path.join("alias.txt")).unwrap();
    fs::write(path.join("public.txt"),"needle PUBLIC").unwrap();
    let reply=decode(search(&root,request("needle",true)).unwrap());
    assert_eq!(reply.hits.len(),1);
    assert_eq!(reply.hits[0].path,"public.txt");
    assert!(reply.truncated);
    assert!(reply.limitations.contains(&"LINK_EXCLUDED".into()));
    let mut escaped=request("needle",true);
    escaped.path="out-of-scope".into();
    assert!(search(&root,escaped).is_err());
    drop(root);fs::remove_dir_all(path).unwrap();
    fs::remove_dir_all(external).unwrap();
}
#[cfg(windows)]
#[test]
fn tools09_windows_reparse_directory_denied_when_native_fixture_can_create_it(){
    use std::os::windows::fs::symlink_dir;
    let (path,root)=sandbox();
    fs::create_dir(path.join("physical")).unwrap();
    fs::write(path.join("physical/a.txt"),"needle").unwrap();
    if symlink_dir(path.join("physical"),path.join("alias")).is_ok(){
        let mut req=request("needle",true);
        req.path="alias".into();
        assert!(search(&root,req).is_err());
    }
    drop(root);fs::remove_dir_all(path).unwrap();
}
