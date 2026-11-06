use lu_core::native_libarchive::{
    available,compress_raw,expand_raw,decode_members,NativeFormat,NativeArchiveError,
};
#[test]
fn native_xz_bzip2_zstd_roundtrip_without_shell(){
    if !available(){
        if std::env::var_os("LU_REQUIRE_NATIVE_CODEC").is_some(){
            panic!("Native app-owned libarchive codec library is REQUIRED in CI")
        }
        return
    }
    let payload=b"Lu's offline native compression format test. ".repeat(250);
    for codec in [
        NativeFormat::RawXz,NativeFormat::RawBzip2,NativeFormat::RawZstd,
    ]{
        let encoded=compress_raw(codec,&payload).unwrap();
        assert!(!encoded.is_empty());
        assert_ne!(encoded,payload);
        assert_eq!(expand_raw(codec,&encoded).unwrap(),payload);
        let mut corrupted=encoded.clone();
        corrupted.truncate(2);
        assert!(expand_raw(codec,&corrupted).is_err());
    }
}
#[test]
fn invalid_rar_and_7z_headers_cannot_create_members(){
    if !available(){
        if std::env::var_os("LU_REQUIRE_NATIVE_CODEC").is_some(){
            panic!("Native libarchive not staged");
        }
        return
    }
    for codec in [NativeFormat::SevenZip,NativeFormat::Rar]{
        assert!(decode_members(codec,b"Not a valid archive").is_err());
        assert_eq!(decode_members(codec,&[]).unwrap_err(),
            NativeArchiveError::CorruptArchive);
    }
}
#[test]
fn native_codec_expansion_refuses_large_archive_input(){
    let input=vec![0u8;128*1024*1024+1];
    assert_eq!(expand_raw(NativeFormat::RawXz,&input).unwrap_err(),
        NativeArchiveError::TooLarge);
    assert_eq!(decode_members(NativeFormat::SevenZip,&input).unwrap_err(),
        NativeArchiveError::TooLarge);
}
