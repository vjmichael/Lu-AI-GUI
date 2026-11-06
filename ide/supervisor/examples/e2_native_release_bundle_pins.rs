//! E2 release-builder-only: measure FULL staged LSP trees with exactly the
//! same Windows native traversal used at runtime, including license notices.
//! A printed hash is NOT approval, signing, or OS containment. Release
//! automation must independently verify the license manifest + Authenticode.
#[cfg(not(all(windows, feature = "lsp-transport", feature = "protected-recovery-service")))]
fn main() {
    eprintln!("DENIED: native Windows E2 resource digest tool unavailable");
    std::process::exit(2);
}
#[cfg(all(windows, feature = "lsp-transport", feature = "protected-recovery-service"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::{env,fs,io::Read,path::PathBuf};
    use sha2::{Digest,Sha256};
    use lu_ide_execution_supervisor::windows_worker::lsp_native::native_lsp_bundle_digest;
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.len() != 1 {
        return Err("DENIED: one stage resource root argument required".into());
    }
    let root=PathBuf::from(&args[0]);
    if !root.is_absolute() ||
        fs::symlink_metadata(&root)?.file_type().is_symlink() ||
        !root.is_dir() {
        return Err("DENIED: staged resource path is not an absolute real directory".into());
    }
    let service=root.join("recovery").join("windows_recovery_service.exe");
    if fs::symlink_metadata(&service)?.file_type().is_symlink() {
        return Err("DENIED: linked protected service".into());
    }
    let mut file=fs::File::open(service)?;
    let mut hash=Sha256::new();
    let mut chunk=[0u8;64*1024];
    let mut bytes=0usize;
    loop {
        let read=file.read(&mut chunk)?;
        if read==0 {break}
        bytes=bytes.saturating_add(read);
        if bytes>128*1024*1024 {
            return Err("DENIED: service beyond review size".into());
        }
        hash.update(&chunk[..read]);
    }
    println!("LU_IDE_E2_D7D_SERVICE_SHA256={:x}",hash.finalize());
    let mut count=0;
    for (id,recipe,key) in [
        ("clangd","lsp_clangd","LU_IDE_E2_CLANGD_BUNDLE_SHA256"),
        ("rust-analyzer","lsp_rust_analyzer","LU_IDE_E2_RUST_ANALYZER_BUNDLE_SHA256"),
        ("ruff","lsp_ruff","LU_IDE_E2_RUFF_BUNDLE_SHA256"),
    ] {
        let path=root.join("language-servers").join(id);
        if !path.is_dir() {continue}
        let digest=native_lsp_bundle_digest(&path,recipe)
            .map_err(|_|"DENIED: staged LSP source invalid")?;
        println!("{key}={digest}");
        count+=1;
    }
    if count==0 {return Err("DENIED: no approved native server assets staged".into());}
    Ok(())
}
