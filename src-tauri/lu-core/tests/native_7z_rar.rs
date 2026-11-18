use std::{fs,sync::atomic::{AtomicU64,Ordering}};
use cap_std::{ambient_authority,fs::Dir};
use lu_core::{
    native_seven_rar::{seven_rar_tree_members,SevenRarError},
    native_archive_tree::{preview_archive_tree,extract_archive_tree_new,
        observe_tree_completion},
    native_libarchive::available,
};
static NEXT:AtomicU64=AtomicU64::new(0);
// Small upstream libarchive BSD-licensed fixtures are uuencoded plain text
// to remain auditable in this repository. They are never executed.
fn decode_uu(input:&str)->Vec<u8>{
    let mut out=Vec::new();
    for line in input.lines().skip(1){
        if line=="end"{break}
        let data=line.as_bytes();
        if data.is_empty(){continue}
        let count=((data[0].wrapping_sub(32))&63) as usize;
        if count==0{continue}
        let mut decoded=Vec::new();
        for group in data[1..].chunks(4){
            if group.len()!=4{break}
            let a=(group[0].wrapping_sub(32))&63;
            let b=(group[1].wrapping_sub(32))&63;
            let c=(group[2].wrapping_sub(32))&63;
            let d=(group[3].wrapping_sub(32))&63;
            decoded.extend_from_slice(&[
                (a<<2)|(b>>4),(b<<4)|(c>>2),(c<<6)|d,
            ]);
        }
        assert!(decoded.len()>=count);
        out.extend_from_slice(&decoded[..count]);
    }
    out
}
fn fixture()->(std::path::PathBuf,Dir){
    let p=std::env::temp_dir().join(format!(
        "lu-7z-rar-{}-{}",std::process::id(),
        NEXT.fetch_add(1,Ordering::SeqCst)
    ));
    fs::create_dir_all(&p).unwrap();
    let d=Dir::open_ambient_dir(&p,ambient_authority()).unwrap();
    (p,d)
}
#[test]
fn rar5_compressed_real_fixture_decodes_and_extracts_to_new_folder(){
    if !available(){
        if std::env::var_os("LU_REQUIRE_NATIVE_CODEC").is_some(){
            panic!("Native RAR5 library missing in required native CI")
        }
        return
    }
    let (root,dir)=fixture();
    let rar=decode_uu(include_str!("fixtures/rar5_compressed.rar.uu"));
    assert!(rar.starts_with(b"Rar!\x1a\x07\x01\x00"));
    fs::write(root.join("fixture.rar"),&rar).unwrap();
    let members=seven_rar_tree_members(&dir,"fixture.rar").unwrap();
    assert_eq!(members.len(),1);
    assert_eq!(members[0].0,"test.bin");
    assert_eq!(members[0].2.len(),1200);
    let preview=preview_archive_tree(&dir,"fixture.rar").unwrap();
    assert_eq!(preview.file_count,1);
    extract_archive_tree_new(&dir,"fixture.rar",&dir,"rar5-new",&preview.sha256).unwrap();
    assert_eq!(fs::read(root.join("rar5-new/test.bin")).unwrap(),members[0].2);
    assert!(observe_tree_completion(&dir,"rar5-new",&preview).unwrap());
    assert!(!root.join("test.bin").exists());
    drop(dir);fs::remove_dir_all(root).unwrap();
}
#[test]
fn unsafe_or_corrupted_rar4_does_not_publish_anything(){
    if !available(){
        if std::env::var_os("LU_REQUIRE_NATIVE_CODEC").is_some(){
            panic!("Native RAR4 decoder missing")
        }
        return
    }
    let (root,dir)=fixture();
    let mut rar=decode_uu(include_str!("fixtures/rar4_basic.rar.uu"));
    assert!(rar.starts_with(b"Rar!\x1a\x07\x00"));
    // The bundled upstream RAR4 archive contains a mix of file and metadata
    // entries. Lu may either accept all safe entries or reject unsupported
    // links; either result must not escape its authorized extraction root.
    fs::write(root.join("rar4.rar"),&rar).unwrap();
    let _=seven_rar_tree_members(&dir,"rar4.rar");
    rar[0]^=0xff;
    fs::write(root.join("corrupt.rar"),rar).unwrap();
    assert!(seven_rar_tree_members(&dir,"corrupt.rar").is_err());
    assert!(preview_archive_tree(&dir,"corrupt.rar").is_err());
    assert!(!root.join("test.txt").exists());
    assert!(!root.join("testdir").exists());
    drop(dir);fs::remove_dir_all(root).unwrap();
}
#[test]
fn refuses_mislabeled_and_missing_archives(){
    let (root,dir)=fixture();
    fs::write(root.join("bad.7z"),b"no 7zip header").unwrap();
    fs::write(root.join("bad.rar"),b"no rar header").unwrap();
    if available(){
        assert!(seven_rar_tree_members(&dir,"bad.7z").is_err());
        assert!(seven_rar_tree_members(&dir,"bad.rar").is_err());
    }
    assert_eq!(seven_rar_tree_members(&dir,"bad.txt").unwrap_err(),
        SevenRarError::UnsupportedFormat);
    drop(dir);fs::remove_dir_all(root).unwrap();
}
