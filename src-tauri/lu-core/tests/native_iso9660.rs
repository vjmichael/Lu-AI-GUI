use cap_std::{ambient_authority,fs::Dir};
use lu_core::native_iso9660::{inspect_iso,extract_iso_file_new,IsoError};
use std::{fs,path::PathBuf,sync::atomic::{AtomicU64,Ordering}};
static NEXT:AtomicU64=AtomicU64::new(0);
const S:usize=2048;
fn lebe32(out:&mut[u8],v:u32){
    out[..4].copy_from_slice(&v.to_le_bytes());
    out[4..8].copy_from_slice(&v.to_be_bytes());
}
fn lebe16(out:&mut[u8],v:u16){
    out[..2].copy_from_slice(&v.to_le_bytes());
    out[2..4].copy_from_slice(&v.to_be_bytes());
}
fn record(extent:u32,size:u32,name:&[u8],dir:bool)->Vec<u8>{
    let len=33+name.len()+(name.len()%2==0) as usize;
    let mut r=vec![0u8;len];
    r[0]=len as u8;
    lebe32(&mut r[2..10],extent);
    lebe32(&mut r[10..18],size);
    r[25]=if dir{2}else{0};
    lebe16(&mut r[28..32],1);
    r[32]=name.len() as u8;
    r[33..33+name.len()].copy_from_slice(name);
    r
}
fn image_with_name(name:&[u8])->Vec<u8>{
    let mut b=vec![0u8;22*S];
    let p=16*S;
    b[p]=1;
    b[p+1..p+6].copy_from_slice(b"CD001");
    b[p+6]=1;
    lebe32(&mut b[p+80..p+88],22);
    lebe16(&mut b[p+128..p+132],2048);
    let root=record(20,2048,&[0],true);
    b[p+156..p+156+root.len()].copy_from_slice(&root);
    let records=[
        record(20,2048,&[0],true),
        record(20,2048,&[1],true),
        record(21,11,name,false),
    ];
    let mut pos=20*S;
    for r in records{b[pos..pos+r.len()].copy_from_slice(&r);pos+=r.len();}
    b[21*S..21*S+11].copy_from_slice(b"hello world");
    b
}
fn fixture()->(PathBuf,Dir){
    let path=std::env::temp_dir().join(format!("lu-iso-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::SeqCst)));
    fs::create_dir_all(&path).unwrap();
    let dir=Dir::open_ambient_dir(&path,ambient_authority()).unwrap();
    (path,dir)
}
#[test]
fn list_iso9660_and_extract_exact_regular_file_without_mounting(){
    let (root,dir)=fixture();
    fs::write(root.join("disc.iso"),image_with_name(b"README.TXT;1")).unwrap();
    let m=inspect_iso(&dir,"disc.iso",16).unwrap();
    assert_eq!(m.total_entries,1);
    assert_eq!(m.entries[0].path,"README.TXT");
    assert_eq!(m.entries[0].bytes,11);
    assert!(!m.truncated);
    let receipt=extract_iso_file_new(&dir,"disc.iso","README.TXT",&dir,
        "copy.txt","transaction-123456789").unwrap();
    assert_eq!(receipt.bytes_written,11);
    assert_eq!(fs::read(root.join("copy.txt")).unwrap(),b"hello world");
    assert!(extract_iso_file_new(&dir,"disc.iso","README.TXT",&dir,
        "copy.txt","transaction-987654321").is_err());
    assert_eq!(fs::read(root.join("copy.txt")).unwrap(),b"hello world");
    drop(dir);fs::remove_dir_all(root).unwrap();
}
#[test]
fn rejects_traversal_and_malformed_primary_volume(){
    let (root,dir)=fixture();
    fs::write(root.join("disc.iso"),image_with_name(b"../BAD;1")).unwrap();
    assert_eq!(inspect_iso(&dir,"disc.iso",16).unwrap_err(),IsoError::UnsafeEntry);
    let mut invalid=image_with_name(b"README.TXT;1");
    invalid[16*S+1]=b'X';
    fs::write(root.join("disc.iso"),invalid).unwrap();
    assert_eq!(inspect_iso(&dir,"disc.iso",16).unwrap_err(),IsoError::InvalidImage);
    assert_eq!(inspect_iso(&dir,"../disc.iso",16).unwrap_err(),IsoError::BadName);
    drop(dir);fs::remove_dir_all(root).unwrap();
}
#[test]
fn iso_list_limit_does_not_grant_more_read_authority(){
    let (root,dir)=fixture();
    fs::write(root.join("disc.iso"),image_with_name(b"README.TXT;1")).unwrap();
    assert_eq!(inspect_iso(&dir,"disc.iso",0).unwrap_err(),IsoError::TooLarge);
    assert_eq!(inspect_iso(&dir,"disc.iso",257).unwrap_err(),IsoError::TooLarge);
    assert_eq!(extract_iso_file_new(&dir,"disc.iso","UNKNOWN",&dir,
        "new.txt","transaction-123456789").unwrap_err(),IsoError::NotFound);
    assert!(!root.join("new.txt").exists());
    drop(dir);fs::remove_dir_all(root).unwrap();
}
#[cfg(unix)]
#[test]
fn iso_image_source_must_not_be_symlink(){
    use std::os::unix::fs::symlink;
    let (root,dir)=fixture();
    fs::write(root.join("real.iso"),image_with_name(b"README.TXT;1")).unwrap();
    symlink("real.iso",root.join("alias.iso")).unwrap();
    assert_eq!(inspect_iso(&dir,"alias.iso",16).unwrap_err(),IsoError::NotFile);
    drop(dir);fs::remove_dir_all(root).unwrap();
}
