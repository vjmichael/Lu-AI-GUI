// INT02-F: independent *build-time* remeasurement of reviewed offline
// language-server resources. Runtime still checks compile-time pins against
// the installed bundle before asking for one-use native OS authorization.
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet, env, fs::{self, File}, io::Read,
    path::{Path, PathBuf},
};

fn digest_file(path: &Path) -> Result<String, String> {
    let metadata = fs::symlink_metadata(path).map_err(|_| "unavailable resource")?;
    if !metadata.file_type().is_file() || metadata.len() > 1024*1024*1024 {
        return Err("unsafe resource type/size".into());
    }
    let mut file = File::open(path).map_err(|_| "unreadable resource")?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 64*1024];
    loop {
        let read = file.read(&mut buffer).map_err(|_| "unreadable resource")?;
        if read == 0 { break; }
        hash.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
fn collect_files(folder: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let metadata = fs::symlink_metadata(folder).map_err(|_| "unavailable bundle root")?;
    if !metadata.file_type().is_dir() { return Err("linked bundle root".into()); }
    for entry in fs::read_dir(folder).map_err(|_| "unreadable bundle root")? {
        let path = entry.map_err(|_| "unreadable entry")?.path();
        let info = fs::symlink_metadata(&path).map_err(|_| "unreadable resource")?;
        if info.file_type().is_dir() {
            collect_files(&path, out)?;
        } else if info.file_type().is_file() {
            out.push(path);
        } else {
            return Err("untrusted resource link or file".into());
        }
        if out.len() > 1024 { return Err("language bundle file limit".into()); }
    }
    Ok(())
}
fn windows_digest_folder(folder: &Path, root: &Path,
    whole: &mut Sha256) -> Result<(), String> {
    let mut children = fs::read_dir(folder)
        .map_err(|_| "unreadable Windows release tree")?
        .map(|entry| entry.map(|value| value.path())
            .map_err(|_| "unreadable Windows release entry"))
        .collect::<Result<Vec<_>, _>>()?;
    children.sort_by_key(|path| path.file_name().unwrap().to_os_string());
    for child in children {
        let meta = fs::symlink_metadata(&child).map_err(|_| "untrusted Windows resource")?;
        if meta.file_type().is_dir() {
            windows_digest_folder(&child, root, whole)?;
        } else if meta.file_type().is_file() {
            let relative = child.strip_prefix(root).map_err(|_| "foreign native file")?
                .to_str().ok_or("non-UTF8 native path")?.replace('\\', "/");
            whole.update(relative.as_bytes());
            whole.update([0]);
            whole.update(meta.len().to_le_bytes());
            whole.update(digest_file(&child)?.as_bytes());
            whole.update([0]);
        } else {
            return Err("linked or special Windows native resource".into());
        }
    }
    Ok(())
}
fn bundle_digest(root: &Path, windows: bool) -> Result<String, String> {
    let mut files = Vec::new();
    collect_files(root, &mut files)?;
    if files.is_empty() { return Err("empty reviewed language bundle".into()); }
    let mut whole = Sha256::new();
    if windows {
        // Windows signed release staging uses depth-first, sibling-name order.
        // A flat sort can disagree when a directory prefixes a filename.
        windows_digest_folder(root, root, &mut whole)?;
    } else {
        files.sort_by_key(|path| path.strip_prefix(root).unwrap().to_path_buf());
        for file in files {
            let relative = file.strip_prefix(root).map_err(|_| "unsafe bundle entry")?
                .to_str().ok_or("non-UTF8 bundle path")?.replace('\\', "/");
            whole.update(relative.as_bytes());
            whole.update([0]);
            whole.update(digest_file(&file)?.as_bytes());
            whole.update([0]);
        }
    }
    Ok(format!("{:x}", whole.finalize()))
}
fn checked_pin(value: &str) -> Result<&str, String> {
    if value.len() != 64 || !value.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("invalid build-time release digest".into());
    }
    Ok(value)
}
fn json(path: &Path) -> Result<Value, String> {
    let metadata = fs::symlink_metadata(path).map_err(|_| "missing signed-off evidence")?;
    if !metadata.file_type().is_file() || metadata.len() > 128*1024 {
        return Err("untrusted release evidence".into());
    }
    serde_json::from_slice(&fs::read(path).map_err(|_| "unreadable evidence")?)
        .map_err(|_| "invalid release evidence".into())
}
fn offline_lsp_pins() -> Result<(), String> {
    if env::var_os("CARGO_FEATURE_NATIVE_LSP").is_none() {
        return Ok(());
    }
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR")
        .map_err(|_| "missing cargo manifest root")?);
    let pins_path = root.join("lsp-compile-pins.json");
    println!("cargo:rerun-if-changed={}", pins_path.display());
    if !pins_path.exists() {
        // A developer feature build without reviewed resources stays
        // unrunnable. Ignore caller-supplied environment hashes entirely:
        // the only admission source is the remeasured staged package.
        for key in [
            "LU_IDE_E2_CLANGD_BUNDLE_SHA256",
            "LU_IDE_E2_RUST_ANALYZER_BUNDLE_SHA256",
            "LU_IDE_E2_RUFF_BUNDLE_SHA256",
            "LU_IDE_E2_PYRIGHT_BUNDLE_SHA256",
            "LU_IDE_E2_D7D_SERVICE_SHA256",
        ] {
            println!("cargo:rustc-env={key}=");
        }
        return Ok(());
    }
    let windows = match env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("windows") => true,
        Ok("linux") => false,
        _ => return Err("unsupported native LSP target OS".into()),
    };
    let platform = if windows { "windows" } else { "linux" };
    let evidence_path = root.join("resources").join(if windows {
        "release-evidence.json"
    } else {
        "linux-lsp-staging-evidence.json"
    });
    println!("cargo:rerun-if-changed={}", evidence_path.display());
    let pins = json(&pins_path)?;
    let evidence = json(&evidence_path)?;
    if pins["schema"] != "lu.ide.int02f.compile-pins.v1" ||
        pins["platform"] != platform ||
        pins["releaseInstalled"] != false ||
        pins["nativeHumanAcceptance"] != false ||
        evidence["schema"] != if windows {
            "lu.ide.e2.staged-release.v1"
        } else {
            "lu.ide.linux-lsp-staging.v1"
        } {
        return Err("compiled bundle evidence mismatch".into());
    }
    let records = evidence["servers"].as_object()
        .ok_or("missing reviewed provider receipts")?;
    let expected = pins["pins"].as_object()
        .ok_or("missing reviewed release pin inputs")?;
    if records.is_empty() { return Err("empty reviewed installed resources".into()); }
    let mut observed = BTreeSet::new();
    for (id, receipt) in records {
        let (name, env_pin) = match id.as_str() {
            "clangd" => ("clangd", "LU_IDE_E2_CLANGD_BUNDLE_SHA256"),
            "rust-analyzer" => ("rust-analyzer", "LU_IDE_E2_RUST_ANALYZER_BUNDLE_SHA256"),
            "ruff" => ("ruff", "LU_IDE_E2_RUFF_BUNDLE_SHA256"),
            "pyright" => ("pyright-langserver", "LU_IDE_E2_PYRIGHT_BUNDLE_SHA256"),
            _ => return Err("unreviewed executable bundle".into()),
        };
        let bundle = root.join("resources").join("language-servers").join(id);
        let executable = bundle.join("bin").join(format!("{}{}",
            name, if windows { ".exe" } else { "" }));
        let digest = digest_file(&executable)?;
        if digest != checked_pin(receipt["binarySha256"].as_str()
            .ok_or("missing binary hash")?)? {
            return Err("native binary changed after installer staging".into());
        }
        let full = bundle_digest(&bundle, windows)?;
        if full != checked_pin(receipt["bundleSha256"].as_str()
            .ok_or("missing bundle hash")?)? ||
            expected.get(env_pin).and_then(Value::as_str) != Some(full.as_str()) ||
            receipt["rustEnvPin"] != env_pin {
            return Err("compiled pin differs from reviewed complete native bundle".into());
        }
        println!("cargo:rerun-if-changed={}", bundle.display());
        observed.insert(env_pin.to_owned());
        println!("cargo:rustc-env={env_pin}={full}");
    }
    if windows {
        let service = root.join("resources").join("recovery")
            .join("windows_recovery_service.exe");
        let digest = digest_file(&service)?;
        if digest != checked_pin(evidence["serviceSha256"].as_str()
            .ok_or("missing protected-service hash")?)? ||
            expected.get("LU_IDE_E2_D7D_SERVICE_SHA256").and_then(Value::as_str) != Some(digest.as_str()) {
            return Err("protected service differs from staged release".into());
        }
        observed.insert("LU_IDE_E2_D7D_SERVICE_SHA256".into());
        println!("cargo:rustc-env=LU_IDE_E2_D7D_SERVICE_SHA256={digest}");
    }
    if expected.keys().cloned().collect::<BTreeSet<_>>() != observed {
        return Err("unreviewed or omitted compile-time provider pin".into());
    }
    // Never inherit an ambient build environment value for a provider absent
    // from the approved manifest. An absent provider remains native-unavailable.
    for key in [
        "LU_IDE_E2_CLANGD_BUNDLE_SHA256",
        "LU_IDE_E2_RUST_ANALYZER_BUNDLE_SHA256",
        "LU_IDE_E2_RUFF_BUNDLE_SHA256",
        "LU_IDE_E2_PYRIGHT_BUNDLE_SHA256",
        "LU_IDE_E2_D7D_SERVICE_SHA256",
    ] {
        if !observed.contains(key) {
            println!("cargo:rustc-env={key}=");
        }
    }
    Ok(())
}
// FIN-02: never allow an "integrated alpha" build to fall back to the
// permissive development path in which absent staged LSP pins are accepted
// only so that the IDE's default (inert) UI can compile.

// FIN-02B: strict native runtime bundle measurements at compile time.
// These pins are observations, not vendor signatures or OS containment proof.
fn fin02_scan(root:&Path,dir:&Path,depth:usize,
    files:&mut Vec<(String,u64,String)>,total:&mut u64)->Result<(),String>{
    if depth>32 {return Err("FIN02_RUNTIME_DEPTH_DENIED".into());}
    for child in fs::read_dir(dir).map_err(|_|"FIN02_RUNTIME_DIRECTORY_UNREADABLE")?{
        let path=child.map_err(|_|"FIN02_RUNTIME_ENTRY_UNAVAILABLE")?.path();
        let meta=fs::symlink_metadata(&path).map_err(|_|"FIN02_RUNTIME_METADATA_DENIED")?;
        if meta.file_type().is_symlink(){return Err("FIN02_RUNTIME_LINK_DENIED".into());}
        if meta.is_dir(){
            fin02_scan(root,&path,depth+1,files,total)?;
        }else if meta.is_file(){
            if meta.len()>1024*1024*1024 || files.len()>=40000{
                return Err("FIN02_RUNTIME_QUOTA_DENIED".into());
            }
            *total=total.checked_add(meta.len())
                .ok_or("FIN02_RUNTIME_QUOTA_OVERFLOW")?;
            if *total>4*1024*1024*1024{return Err("FIN02_RUNTIME_SIZE_DENIED".into());}
            let rel=path.strip_prefix(root).map_err(|_|"FIN02_RUNTIME_PATH_ESCAPED")?
                .to_str().ok_or("FIN02_RUNTIME_NON_UTF8")?.replace('\\',"/");
            if rel.len()>1024 || rel.split('/').any(|s|
                s.is_empty() || s=="." || s==".."){
                return Err("FIN02_RUNTIME_PATH_DENIED".into());
            }
            let sha=digest_file(&path)?;
            let post=fs::symlink_metadata(&path)
                .map_err(|_|"FIN02_RUNTIME_CHANGED")?;
            if post.len()!=meta.len() || !post.is_file(){
                return Err("FIN02_RUNTIME_CHANGED".into());
            }
            files.push((rel,meta.len(),sha));
        }else{
            return Err("FIN02_RUNTIME_SPECIAL_FILE_DENIED".into());
        }
    }
    Ok(())
}
fn fin02_tree_hash(root:&Path,kind:&str)->Result<String,String>{
    let meta=fs::symlink_metadata(root).map_err(|_|"FIN02_RUNTIME_ROOT_MISSING")?;
    if !meta.is_dir() || meta.file_type().is_symlink(){
        return Err("FIN02_RUNTIME_ROOT_UNTRUSTED".into());
    }
    let mut files=Vec::new();let mut total=0u64;
    fin02_scan(root,root,0,&mut files,&mut total)?;
    if files.is_empty(){return Err("FIN02_RUNTIME_EMPTY".into());}
    let mut h=Sha256::new();
    match kind {
        "runtime"=>{
            // Identical ordered tree transcript to the native runtime_bundles
            // rehash: path, NUL, little-endian size, sha, NUL.
            h.update(b"lu-ide-runtime-tree-v1\0");
            files.sort_by(|a,b|a.0.cmp(&b.0));
            for (name,size,sha) in files {
                h.update(name.as_bytes());h.update([0]);
                h.update(size.to_le_bytes());h.update(sha.as_bytes());h.update([0]);
            }
        },
        "windows_cargo"=>{
            // The Windows native D7 custody hash uses DFS sibling-name
            // order, not lexicographically flat-path order.
            fn dfs(root:&Path,dir:&Path,h:&mut Sha256)->Result<(),String>{
                let mut children=fs::read_dir(dir)
                    .map_err(|_|"FIN02_CARGO_DIRECTORY_UNAVAILABLE")?
                    .map(|e|e.map(|x|x.path()).map_err(|_|"FIN02_CARGO_ENTRY_DENIED".into()))
                    .collect::<Result<Vec<PathBuf>,String>>()?;
                children.sort_by_key(|p|p.file_name().unwrap().to_os_string());
                for p in children {
                    let meta=fs::symlink_metadata(&p).map_err(|_|"FIN02_CARGO_METADATA")?;
                    if meta.file_type().is_symlink(){
                        return Err("FIN02_CARGO_LINK_DENIED".into());
                    }
                    if meta.is_dir(){dfs(root,&p,h)?;}
                    else if meta.is_file(){
                        let rel=p.strip_prefix(root).map_err(|_|"FIN02_CARGO_PATH")?
                            .to_str().ok_or("FIN02_CARGO_NON_UTF8")?.replace('\\',"/");
                        h.update(rel.as_bytes());h.update([0]);
                        h.update(meta.len().to_le_bytes());
                        h.update(digest_file(&p)?.as_bytes());h.update([0]);
                    }else{return Err("FIN02_CARGO_SPECIAL".into());}
                }
                Ok(())
            }
            dfs(root,root,&mut h)?;
        },
        "native_bundle"=>return bundle_digest(root,
            env::var("CARGO_CFG_TARGET_OS").as_deref()==Ok("windows")),
        _=>return Err("FIN02_UNKNOWN_RUNTIME_KIND".into()),
    }
    Ok(format!("{:x}",h.finalize()))
}
fn fin02_linux_cargo_hash()->Result<String,String>{
    let linker=if env::var("CARGO_CFG_TARGET_ARCH").as_deref()
        ==Ok("x86_64"){
        vec!["/usr/bin/x86_64-linux-gnu-gcc-14",
             "/usr/bin/x86_64-linux-gnu-gcc-13",
             "/usr/bin/x86_64-linux-gnu-gcc-12"]
    }else{
        vec!["/usr/bin/aarch64-linux-gnu-gcc-14",
             "/usr/bin/aarch64-linux-gnu-gcc-13",
             "/usr/bin/aarch64-linux-gnu-gcc-12"]
    };
    let gcc=linker.into_iter()
        .find(|p|Path::new(p).is_file() &&
            !fs::symlink_metadata(p).is_ok_and(|m|m.file_type().is_symlink()))
        .ok_or("FIN02_ROOT_LINKER_MISSING")?;
    let mut h=Sha256::new();
    for p in ["/usr/bin/bwrap","/usr/bin/cargo","/usr/bin/rustc",gcc]{
        let meta=fs::symlink_metadata(p).map_err(|_|"FIN02_SYSTEM_TOOL_MISSING")?;
        if !meta.is_file() || meta.file_type().is_symlink() ||
            meta.len()>32*1024*1024 {return Err("FIN02_SYSTEM_TOOL_UNTRUSTED".into());}
        #[cfg(unix)]{
            use std::os::unix::fs::MetadataExt;
            if meta.uid()!=0 || meta.mode()&0o6022!=0{
                return Err("FIN02_SYSTEM_TOOL_NOT_ROOT_OWNED".into());
            }
        }
        let digest=digest_file(Path::new(p))?;
        h.update(p.as_bytes());h.update([0]);
        h.update(digest.as_bytes());h.update([0]);
    }
    Ok(format!("{:x}",h.finalize()))
}
fn fin02_checked_sha(v:&Value)->Result<&str,String>{
    let s=v.as_str().ok_or("FIN02_RUNTIME_PIN_MISSING")?;
    if s.len()!=64 || !s.bytes().all(|c|c.is_ascii_hexdigit() &&
        !c.is_ascii_uppercase()) || s.bytes().all(|c|c==b'0'){
        return Err("FIN02_RUNTIME_PIN_INVALID".into());
    }
    Ok(s)
}
fn fin02_require_runtime_stage()->Result<(),String>{
    if env::var_os("CARGO_FEATURE_FIN02_PROTECTED_ALPHA").is_none(){
        return Ok(());
    }
    let target=env::var("CARGO_CFG_TARGET_OS")
        .map_err(|_|"FIN02_TARGET_UNKNOWN")?;
    if target!="windows" && target!="linux"{
        return Err("FIN02_UNSUPPORTED_TARGET".into());
    }
    let root=PathBuf::from(env::var("CARGO_MANIFEST_DIR")
        .map_err(|_|"FIN02_DESKTOP_ROOT_UNKNOWN")?);
    let manifest=root.join("fin02-runtime-pins.json");
    println!("cargo:rerun-if-changed={}",manifest.display());
    let data=json(&manifest)?;
    if data["schema"]!="lu.ide.fin02.runtime-pins.v1" ||
        data["platform"]!=target || data["releaseInstalled"]!=false ||
        data["nativeHumanAcceptance"]!=false ||
        data["executionAuthorized"]!=false {
        return Err("FIN02_RUNTIME_STAGE_SCHEMA_DENIED".into());
    }
    let records=data["pins"].as_object().ok_or("FIN02_RUNTIME_PINS_MISSING")?;
    let windows=target=="windows";
    let linux_uv=env::var_os("CARGO_FEATURE_FIN02_LINUX_OFFLINE_UV").is_some();
    if linux_uv && windows {
        return Err("FIN05_WINDOWS_UV_SOURCE_ACL_UNVERIFIED".into());
    }
    // Do not accept unrelated package-uv flags without the exact integrated
    // Linux release profile and measured uv+Python runtime.
    if env::var_os("CARGO_FEATURE_NATIVE_PACKAGE_UV").is_some() &&
       !linux_uv {
        return Err("FIN05_PACKAGE_UV_REQUIRES_LINUX_RELEASE_PROFILE".into());
    }
    let service="LU_IDE_WO6_WINDOWS_D7D_SERVICE_SHA256";
    let mut expected=BTreeSet::new();
    let runtimes=[
        ("cargo",if windows{"LU_IDE_WO6_WINDOWS_CARGO_BUNDLE_SHA256"}
            else{"LU_IDE_WO6_LINUX_TOOLCHAIN_SHA256"}),
        ("python_pytest",if windows{"LU_IDE_DEV03_WINDOWS_PYTHON_SHA256"}
            else{"LU_IDE_DEV03_LINUX_PYTHON_SHA256"}),
        ("js_tests",if windows{"LU_IDE_DEV03_WINDOWS_NODE_SHA256"}
            else{"LU_IDE_DEV03_LINUX_NODE_SHA256"}),
        ("cpp_cmake_build",if windows{"LU_IDE_DEV03_WINDOWS_CMAKE_SHA256"}
            else{"LU_IDE_DEV03_LINUX_CMAKE_SHA256"}),
        ("lldb_dap",if windows{"LU_IDE_WO6_WINDOWS_LLDB_DAP_SHA256"}
            else{"LU_IDE_WO6_LINUX_LLDB_DAP_SHA256"}),
    ];
    let mut all=Vec::from(runtimes);
    if linux_uv {
        all.push(("uv_sync_offline","LU_IDE05_LINUX_UV_SHA256"));
    }
    for (name,key) in all {
        expected.insert(key.to_string());
        let pin=fin02_checked_sha(records.get(key)
            .ok_or("FIN02_REQUIRED_PIN_ABSENT")?)?;
        let actual=match name {
            "cargo" if windows=>{
                let at=root.join("resources/build-tools/windows-rust-offline");
                println!("cargo:rerun-if-changed={}",at.display());
                for required in ["bin/cargo.exe","bin/rustc.exe"]{
                    if !at.join(required).is_file(){
                        return Err("FIN02_CARGO_REQUIRED_BINARY_MISSING".into());
                    }
                }
                for required in ["lib","cargo-home"]{
                    if !at.join(required).is_dir(){
                        return Err("FIN02_CARGO_REQUIRED_DIRECTORY_MISSING".into());
                    }
                }
                fin02_tree_hash(&at,"windows_cargo")?
            },
            "cargo"=>fin02_linux_cargo_hash()?,
            "lldb_dap"=>{
                let stage=root.join("resources/debug-adapters/lldb-dap");
                println!("cargo:rerun-if-changed={}",stage.display());
                let staged=fin02_tree_hash(&stage,"native_bundle")?;
                if windows {staged} else {
                    let installed=PathBuf::from("/opt/lu-ide/debug-adapters/lldb-dap");
                    let actual=fin02_tree_hash(&installed,"native_bundle")?;
                    if staged!=actual{
                        return Err("FIN02_LINUX_PACKAGE_PAYLOAD_MISMATCH".into());
                    }
                    actual
                }
            },
            profile=>{
                let stage=if windows {
                    root.join("resources/build-tools/windows-runtimes").join(profile)
                }else{
                    root.join("resources/build-tools/linux-runtimes").join(profile)
                };
                println!("cargo:rerun-if-changed={}",stage.display());
                let staged=fin02_tree_hash(&stage,"runtime")?;
                if windows {staged} else {
                    let installed=PathBuf::from("/opt/lu-ide/runtimes").join(profile);
                    let actual=fin02_tree_hash(&installed,"runtime")?;
                    if actual!=staged{
                        return Err("FIN02_LINUX_PACKAGE_PAYLOAD_MISMATCH".into());
                    }
                    actual
                }
            }
        };
        if actual!=pin{return Err("FIN02_INSTALLED_RUNTIME_HASH_DIFFERS".into());}
        println!("cargo:rustc-env={key}={actual}");
    }
    if windows {
        let path=root.join("resources/recovery/windows_recovery_service.exe");
        let actual=digest_file(&path)?;
        let pin=fin02_checked_sha(records.get(service)
            .ok_or("FIN02_RECOVERY_SERVICE_PIN_MISSING")?)?;
        if pin!=actual{return Err("FIN02_RECOVERY_SERVICE_HASH_DIFFERS".into());}
        expected.insert(service.to_string());
        // The same bytes are used by the reviewed LSP and Build/Test paths.
        println!("cargo:rustc-env={service}={actual}");
    }
    if records.keys().cloned().collect::<BTreeSet<_>>()!=expected{
        return Err("FIN02_EXTRA_OR_MISSING_RUNTIME_PIN".into());
    }
    Ok(())
}
fn fin02_require_reviewed_lsp_stage() -> Result<(), String> {
    if env::var_os("CARGO_FEATURE_FIN02_PROTECTED_ALPHA").is_none() {
        return Ok(());
    }
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR")
        .map_err(|_| "FIN02_DESKTOP_BUILD_ROOT_MISSING")?);
    let pins = root.join("lsp-compile-pins.json");
    if !pins.is_file() {
        return Err("FIN02_REVIEWED_LSP_STAGE_REQUIRED".into());
    }
    // offline_lsp_pins below must independently validate the staged
    // provider and protected service against the exact release source.
    Ok(())
}
/// FIN-04R source provenance for a protected Windows *observer*, never
/// permission to step, set a breakpoint, inspect values or replay actions.
/// A standalone lldb-dap.exe copied into resources is NOT evidence that
/// the first-party DEBUG_EVENT hook was compiled into it. Require a
/// reviewed, byte-pinned LLVM 18.1.8 patch inventory in addition to the
/// FIN-02 whole-runtime hash and the original signed service checks.
/// Absence of this independent build-stage evidence denies packaging.
fn fin04_require_reviewed_owner_stage() -> Result<(), String> {
    if env::var_os("CARGO_FEATURE_FIN04_RELEASE_EVENT_OBSERVER").is_none() {
        return Ok(());
    }
    if env::var("CARGO_CFG_TARGET_OS").ok().as_deref() != Some("windows")
        || env::var_os("CARGO_FEATURE_FIN02_PROTECTED_ALPHA").is_none()
        || env::var_os("CARGO_FEATURE_NATIVE_PROJECT_DEBUG").is_none()
        || env::var_os("CARGO_FEATURE_DEV046D_NATIVE_DIALOG").is_some()
    {
        return Err("FIN04R_PROTECTED_RELEASE_OBSERVER_PROFILE_DENIED".into());
    }
    let root=PathBuf::from(env::var("CARGO_MANIFEST_DIR")
        .map_err(|_|"FIN04R_BUILD_ROOT_UNAVAILABLE")?);
    let stage=root.join("resources/debug-adapters/lldb-dap");
    let provenance=stage.join("fin04-owner-provenance.json");
    println!("cargo:rerun-if-changed={}",provenance.display());
    let data=json(&provenance)?;
    if data["schema"]!="lu.ide.fin04.first-party-lldb-owner.v1"
        || data["upstreamCommit"]!="3b5b5c1ec4a3095ab096dd780e84d7ab81f3d7ff"
        || data["observerOnly"]!=true
        || data["productionDebuggerActions"]!=false
        || data["replayAuthorized"]!=false
    {
        return Err("FIN04R_OWNER_SOURCE_PROVENANCE_DENIED".into());
    }
    let header=root.join("../../llvm18_owner/LuIdeOwnerEvent.h");
    let patcher=root.join("../../llvm18_owner/apply_llvm18_owner_hook.py");
    let cpp=stage.join("provenance/DebuggerThread.cpp");
    // Native MSVC C2039/C3861 proof: the reviewed LLVM 18.1.8 DAP
    // ProgressEvent.h must be the exact, independently rehashed version
    // containing the pinned <chrono> include. Never accept an arbitrary
    // system LLDB or transitive third-party header as proof.
    let progress=stage.join("provenance/ProgressEvent.h");
    let binary=stage.join("bin/lldb-dap.exe");
    for file in [&header,&patcher,&cpp,&progress,&binary] {
        println!("cargo:rerun-if-changed={}",file.display());
    }
    let checks=[
        ("headerSha256",&header),
        ("patcherSha256",&patcher),
        ("patchedDebuggerThreadSha256",&cpp),
        ("patchedProgressEventHeaderSha256",&progress),
        ("lldbDapExeSha256",&binary),
    ];
    for (key,path) in checks {
        let expected=fin02_checked_sha(&data[key])?;
        if digest_file(path)?!=expected {
            return Err(format!("FIN04R_REVIEWED_OWNER_SOURCE_OR_BINARY_MISMATCH:{key}"));
        }
    }
    // The reviewed source must actually contain the authenticated event
    // callbacks. A checksum alone cannot establish the first-party patch's
    // intended trust boundary. Real OS execution is an additional gate.
    let meta=fs::symlink_metadata(&cpp)
        .map_err(|_|"FIN04R_PATCHED_SOURCE_UNAVAILABLE")?;
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len()>1024*1024 {
        return Err("FIN04R_PATCHED_SOURCE_UNTRUSTED".into());
    }
    let patched=fs::read_to_string(&cpp)
        .map_err(|_|"FIN04R_PATCHED_SOURCE_UNREADABLE")?;
    for anchor in [
        "#include \"LuIdeOwnerEvent.h\"",
        "lu_ide_owner::OnReceived(dbe);",
        "lu_ide_owner::OnContinued(dbe, continued);",
    ] {
        if patched.matches(anchor).count()!=1 {
            return Err("FIN04R_PATCHED_EVENT_OWNER_ANCHOR_MISSING".into());
        }
    }
    let progress_meta=fs::symlink_metadata(&progress)
        .map_err(|_|"FIN04R_DAP_CHRONO_SOURCE_UNAVAILABLE")?;
    if !progress_meta.is_file() || progress_meta.file_type().is_symlink()
        || progress_meta.len()>128*1024 {
        return Err("FIN04R_DAP_CHRONO_SOURCE_UNTRUSTED".into());
    }
    let progress_bytes=fs::read_to_string(&progress)
        .map_err(|_|"FIN04R_DAP_CHRONO_SOURCE_UNREADABLE")?;
    for anchor in ["#include <atomic>", "#include <chrono>",
                   "std::chrono::system_clock::now()"] {
        if progress_bytes.matches(anchor).count()!=1 {
            return Err("FIN04R_PINNED_LLVM_DAP_CHRONO_PATCH_MISSING".into());
        }
    }
    Ok(())
}

fn main() {
    if let Err(error) = fin02_require_reviewed_lsp_stage() {
        panic!("protected alpha admission refused: {error}");
    }
    if let Err(error) = offline_lsp_pins() {
        panic!("offline LSP admission refused: {error}");
    }
    if let Err(error) = fin02_require_runtime_stage() {
        panic!("FIN-02 reviewed native runtime stage denied: {error}");
    }
    if let Err(error) = fin04_require_reviewed_owner_stage() {
        panic!("FIN-04R first-party protected event owner denied: {error}");
    }
    tauri_build::build();
}
