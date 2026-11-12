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
fn main() {
    if let Err(error) = fin02_require_reviewed_lsp_stage() {
        panic!("protected alpha admission refused: {error}");
    }
    if let Err(error) = offline_lsp_pins() {
        panic!("offline LSP admission refused: {error}");
    }
    tauri_build::build();
}
