use std::{fs,io::Write,sync::atomic::{AtomicU64,Ordering}};
use cap_std::{ambient_authority,fs::Dir};
use flate2::{write::GzEncoder,Compression};
use lu_core::{
    native_rpm::{parse_rpm,RpmError,rpm_tree_members},
    native_archive_tree::{preview_archive_tree,extract_archive_tree_new,
        observe_tree_completion},
};
static NEXT:AtomicU64=AtomicU64::new(0);
fn fixture()->(std::path::PathBuf,Dir){
    let root=std::env::temp_dir().join(format!("lu-rpm-{}-{}",
        std::process::id(),NEXT.fetch_add(1,Ordering::SeqCst)));
    fs::create_dir_all(&root).unwrap();
    let dir=Dir::open_ambient_dir(&root,ambient_authority()).unwrap();
    (root,dir)
}
fn be(buf:&mut Vec<u8>,value:u32){buf.extend_from_slice(&value.to_be_bytes())}
fn rpm_header(tags:&[(u32,&str)])->Vec<u8>{
    let mut entries=Vec::new();let mut store=Vec::new();
    for (tag,value) in tags{
        be(&mut entries,*tag);
        be(&mut entries,6); // RPM_STRING_TYPE
        be(&mut entries,store.len() as u32);
        be(&mut entries,1);
        store.extend_from_slice(value.as_bytes());
        store.push(0);
    }
    let mut out=vec![0x8e,0xad,0xe8,0x01,0,0,0,0];
    be(&mut out,tags.len() as u32);
    be(&mut out,store.len() as u32);
    out.extend_from_slice(&entries);
    out.extend_from_slice(&store);
    out
}
fn cpio_record(out:&mut Vec<u8>,name:&str,data:&[u8],mode:u32){
    let mut hdr=b"070701".to_vec();
    for n in [1,mode,0,0,1,0,data.len() as u32,0,0,0,0,
        (name.len()+1) as u32,0]{
        hdr.extend_from_slice(format!("{n:08x}").as_bytes());
    }
    assert_eq!(hdr.len(),110);
    out.extend_from_slice(&hdr);
    out.extend_from_slice(name.as_bytes());
    out.push(0);
    while out.len()%4!=0{out.push(0)}
    out.extend_from_slice(data);
    while out.len()%4!=0{out.push(0)}
}
fn cpio(name:&str,contents:&[u8])->Vec<u8>{
    let mut bytes=Vec::new();
    cpio_record(&mut bytes,name,contents,0o100644);
    cpio_record(&mut bytes,"TRAILER!!!",&[],0);
    bytes
}
fn rpm(filename:&str,content:&[u8])->Vec<u8>{
    let mut gz=GzEncoder::new(Vec::new(),Compression::default());
    gz.write_all(&cpio(filename,content)).unwrap();
    let zipped=gz.finish().unwrap();
    let mut raw=vec![0u8;96];
    raw[..4].copy_from_slice(&[0xed,0xab,0xee,0xdb]);
    raw[4]=3;
    raw.extend_from_slice(&rpm_header(&[])); // signature header
    while raw.len()%8!=0{raw.push(0)}
    raw.extend_from_slice(&rpm_header(&[
        (1000,"example"),
        (1001,"1.0"),
        (1002,"1"),
        (1022,"x86_64"),
        (1124,"cpio"),
        (1125,"gzip"),
    ]));
    raw.extend_from_slice(&zipped);
    raw
}
#[test]
fn rpm_real_header_and_gzipped_newc_extract_as_safe_payload(){
    let (root,dir)=fixture();
    let input=rpm("./usr/share/example/info.txt",b"RPM is inert.");
    let (info,members)=parse_rpm(&input).unwrap();
    assert_eq!(info.name.as_deref(),Some("example"));
    assert_eq!(info.arch.as_deref(),Some("x86_64"));
    assert_eq!(info.compressor,"gzip");
    assert_eq!(members[0].0,"payload/usr/share/example/info.txt");
    fs::write(root.join("example.rpm"),&input).unwrap();
    let manifest=preview_archive_tree(&dir,"example.rpm").unwrap();
    assert_eq!(manifest.file_count,1);
    extract_archive_tree_new(&dir,"example.rpm",&dir,
        "new-rpm-folder",&manifest.sha256).unwrap();
    assert_eq!(fs::read(root.join("new-rpm-folder/payload/usr/share/example/info.txt"))
        .unwrap(),b"RPM is inert.");
    assert!(observe_tree_completion(&dir,"new-rpm-folder",&manifest).unwrap());
    assert!(!root.join("usr/share/example/info.txt").exists());
    drop(dir);fs::remove_dir_all(root).unwrap();
}
#[test]
fn rpm_traversal_and_malformed_or_unsupported_headers_refused(){
    let (root,dir)=fixture();
    let escaped=rpm("../outside.txt",b"unsafe");
    fs::write(root.join("unsafe.rpm"),escaped).unwrap();
    assert_eq!(rpm_tree_members(&dir,"unsafe.rpm").unwrap_err(),
        RpmError::UnsafePayload);
    let mut corrupt=rpm("foo.txt",b"good");
    corrupt[96]=0;
    assert_eq!(parse_rpm(&corrupt).unwrap_err(),RpmError::InvalidHeader);
    let mut bad=rpm("foo.txt",b"good");
    bad[0]=0;
    assert_eq!(parse_rpm(&bad).unwrap_err(),RpmError::InvalidLead);
    assert!(!root.join("outside.txt").exists());
    drop(dir);fs::remove_dir_all(root).unwrap();
}
