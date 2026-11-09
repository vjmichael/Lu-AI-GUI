//! IDE-05.4: independent post-termination local package evidence.
//! Never imports packages, invokes a package manager or trusts wheel metadata
//! as proof that an upstream archive was actually downloaded from its URL.
//! All results are evidence, NOT permission to promote the environment.
#![cfg(feature = "package-uv-worker")]
use crate::{
    wo6_output_evidence::{audit_complete_package_output, OutputWitness},
    SupervisorError,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::Read,
    path::Path,
};

const MAX_METADATA: u64 = 128 * 1024;
const MAX_RECORD: u64 = 512 * 1024;
const MAX_LOCK: u64 = 4 * 1024 * 1024;
const MAX_DISTS: usize = 128;
const MAX_RECORD_LINES: usize = 6000;

fn denied() -> SupervisorError { SupervisorError::NoContainment }
fn hex64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b|
        b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
fn valid_component(s: &str) -> bool {
    !s.is_empty() && s.len() <= 200
        && s.bytes().all(|b| b.is_ascii_alphanumeric()
            || matches!(b, b'.' | b'_' | b'-' | b'+'))
        && s != "." && s != ".."
}
fn safe_relative(s: &str) -> bool {
    !s.is_empty() && s.len() <= 1024 && !s.contains('\\')
        && s.split('/').all(valid_component)
}
fn normalized_name(s: &str) -> Option<String> {
    if s.is_empty() || s.len() > 128 ||
        !s.bytes().all(|c| c.is_ascii_alphanumeric()
            || matches!(c, b'.' | b'-' | b'_')) {
        return None;
    }
    let mut result = String::new();
    for c in s.bytes() {
        let c = (c as char).to_ascii_lowercase();
        if matches!(c, '-' | '_' | '.') {
            if !result.ends_with('-') { result.push('-'); }
        } else { result.push(c); }
    }
    if result.starts_with('-') || result.ends_with('-') { None }
    else { Some(result) }
}
fn untrusted_meta(meta: &fs::Metadata) -> bool {
    #[cfg(windows)] {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))] { let _=meta; false }
}
fn read_private(root: &Path, rel: &str, limit: u64) -> Result<Vec<u8>, SupervisorError> {
    if !root.is_absolute() || !safe_relative(rel) { return Err(denied()); }
    let file = root.join(rel);
    let mut parent = root.to_path_buf();
    for component in rel.split('/').take(rel.split('/').count().saturating_sub(1)) {
        parent.push(component);
        let meta = fs::symlink_metadata(&parent).map_err(|_| denied())?;
        if !meta.is_dir() || meta.file_type().is_symlink() || untrusted_meta(&meta) {
            return Err(denied());
        }
    }
    let meta = fs::symlink_metadata(&file).map_err(|_| denied())?;
    if !meta.is_file() || meta.file_type().is_symlink()
        || untrusted_meta(&meta) || meta.len() > limit {
        return Err(denied());
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)] {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(0x20000); // O_NOFOLLOW
    }
    #[cfg(windows)] {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x00200000); // FILE_FLAG_OPEN_REPARSE_POINT
    }
    let mut input: File = options.open(file.clone()).map_err(|_| denied())?;
    let handle_meta = input.metadata().map_err(|_| denied())?;
    if !handle_meta.is_file() || handle_meta.len() != meta.len() {
        return Err(denied());
    }
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    input.take(limit+1).read_to_end(&mut bytes).map_err(|_| denied())?;
    if bytes.len() as u64 != meta.len() { return Err(denied()); }
    let after = fs::symlink_metadata(file).map_err(|_| denied())?;
    if after.len()!=meta.len() || after.modified().ok()!=meta.modified().ok()
        || untrusted_meta(&after) || !after.is_file() { return Err(denied()); }
    Ok(bytes)
}
fn file_hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn verified_bytes(
    root: &Path, table: &BTreeMap<String, (u64, String)>,
    rel: &str, limit: u64,
) -> Result<Vec<u8>, SupervisorError> {
    let expected=table.get(rel).ok_or_else(denied)?;
    if expected.0 > limit { return Err(denied()); }
    let bytes=read_private(root,rel,limit)?;
    if bytes.len() as u64 != expected.0 || file_hash(&bytes)!=expected.1 {
        return Err(denied());
    }
    Ok(bytes)
}
fn inventory_map(witness: &OutputWitness) -> Result<BTreeMap<String,(u64,String)>,SupervisorError> {
    if witness.truncated || witness.file_count!=witness.files.len()
        || !hex64(&witness.tree_sha256) { return Err(denied()); }
    let mut found=BTreeMap::new();
    for entry in &witness.files {
        if !safe_relative(&entry.relative_path) || !hex64(&entry.sha256)
            || found.insert(entry.relative_path.clone(),
                (entry.bytes,entry.sha256.clone())).is_some() {
            return Err(denied());
        }
    }
    Ok(found)
}
/// Returns exactly three CSV fields; quoted comma and escaped quotes are
/// supported. A newline in a pathname is already forbidden by the output
/// inventory, so multiline CSV fields are rejected.
fn record_fields(line: &str) -> Result<[String;3],SupervisorError> {
    let mut fields=Vec::new();
    let mut field=String::new();
    let mut in_quotes=false;
    let mut closed=false;
    let mut chars=line.chars().peekable();
    while let Some(c)=chars.next() {
        if c=='\r' || c=='\n' || c=='\0' { return Err(denied()); }
        if in_quotes {
            if c=='"' {
                if chars.peek()==Some(&'"') {
                    chars.next(); field.push('"');
                } else { in_quotes=false; closed=true; }
            } else { field.push(c); }
        } else if c==',' {
            fields.push(std::mem::take(&mut field)); closed=false;
        } else if c=='"' && field.is_empty() && !closed {
            in_quotes=true;
        } else if closed || c=='"' {
            return Err(denied());
        } else { field.push(c); }
        if field.len() > 1024 { return Err(denied()); }
    }
    if in_quotes { return Err(denied()); }
    fields.push(field);
    fields.try_into().map_err(|_| denied())
}
/// PEP 376 RECORD paths are relative to site-packages and can refer to
/// ../../../bin entrypoints. Resolve lexically and refuse escape from .venv.
fn record_target(site: &str, relative: &str) -> Result<String,SupervisorError> {
    if relative.is_empty() || relative.len()>1024 ||
        relative.contains('\\') || relative.starts_with('/') ||
        relative.contains(':') { return Err(denied()); }
    let mut segments=site.split('/').map(str::to_owned).collect::<Vec<_>>();
    for segment in relative.split('/') {
        match segment {
            ".." => {
                if segments.len() <= 1 { return Err(denied()); }
                segments.pop();
            }
            "." | "" => return Err(denied()),
            value if valid_component(value) => segments.push(value.to_owned()),
            _ => return Err(denied()),
        }
    }
    if segments.first().map(String::as_str)!=Some(".venv") {
        return Err(denied());
    }
    Ok(segments.join("/"))
}
fn parse_headers(bytes: &[u8]) -> Result<BTreeMap<String,Vec<String>>,SupervisorError> {
    let text=std::str::from_utf8(bytes).map_err(|_| denied())?;
    let mut headers=BTreeMap::<String,Vec<String>>::new();
    for line in text.lines() {
        let line=line.trim_end_matches('\r');
        if line.is_empty() { break; }
        if line.starts_with(' ') || line.starts_with('\t') {
            return Err(denied()); // conservative: no folded security fields
        }
        let (key,value)=line.split_once(": ").ok_or_else(denied)?;
        if key.len()>80 || value.len()>2048 || !key.bytes().all(|b|
            b.is_ascii_alphanumeric() || b==b'-') {
            return Err(denied());
        }
        headers.entry(key.to_owned()).or_default().push(value.to_owned());
    }
    Ok(headers)
}
fn single<'a>(headers: &'a BTreeMap<String,Vec<String>>,key:&str)
    -> Result<&'a str,SupervisorError> {
    let fields=headers.get(key).ok_or_else(denied)?;
    if fields.len()!=1 || fields[0].trim()!=fields[0] || fields[0].is_empty(){
        return Err(denied());
    }
    Ok(&fields[0])
}
fn hex_as_base64(hex: &str) -> Result<String,SupervisorError> {
    if !hex64(hex) { return Err(denied()); }
    let mut raw=[0u8;32];
    for (i,byte) in raw.iter_mut().enumerate() {
        *byte=u8::from_str_radix(&hex[i*2..i*2+2],16)
            .map_err(|_| denied())?;
    }
    Ok(URL_SAFE_NO_PAD.encode(raw))
}
#[derive(Debug,Clone)]
struct LockPackage {
    version: String,
    registry_wheel_sha256: Vec<String>,
}
fn verified_lock_packages(lock: &[u8]) -> Result<BTreeMap<String,LockPackage>,SupervisorError> {
    let value=std::str::from_utf8(lock).map_err(|_| denied())?
        .parse::<toml::Value>().map_err(|_| denied())?;
    let packages=value.get("package").and_then(toml::Value::as_array)
        .ok_or_else(denied)?;
    if packages.len()>512 { return Err(denied()); }
    let mut out=BTreeMap::new();
    for pkg in packages {
        let p=pkg.as_table().ok_or_else(denied)?;
        let name=p.get("name").and_then(toml::Value::as_str)
            .and_then(normalized_name).ok_or_else(denied)?;
        let version=p.get("version").and_then(toml::Value::as_str)
            .ok_or_else(denied)?;
        if version.len()>96 || version.is_empty()
            || !version.bytes().all(|c| c.is_ascii_alphanumeric()
                || matches!(c,b'.'|b'-'|b'_'|b'+'|b'!')) { return Err(denied()); }
        let source=p.get("source").and_then(toml::Value::as_table)
            .ok_or_else(denied)?;
        // Never reinterpret mixed source declarations as registry only.
        if source.len()!=1 { return Err(denied()); }
        // Roots / virtual project packages are intentionally not installed
        // by --no-install-workspace. No arbitrary Git/path/URL origins.
        let wheels=if let Some(reg)=source.get("registry") {
            let url=reg.as_str().ok_or_else(denied)?;
            if !url.starts_with("https://") || url.len()>2048 { return Err(denied()); }
            let wheels=p.get("wheels").and_then(toml::Value::as_array)
                .ok_or_else(denied)?;
            if wheels.is_empty() || wheels.len()>128 { return Err(denied()); }
            let mut hashes=Vec::new();
            for wheel in wheels {
                let table=wheel.as_table().ok_or_else(denied)?;
                let hash=table.get("hash").and_then(toml::Value::as_str)
                    .and_then(|h| h.strip_prefix("sha256:"))
                    .ok_or_else(denied)?;
                if !hex64(hash) { return Err(denied()); }
                hashes.push(hash.to_owned());
            }
            hashes
        } else if source.get("virtual").is_some() {
            Vec::new()
        } else {
            return Err(denied());
        };
        if out.insert(name, LockPackage {
            version:version.to_owned(),registry_wheel_sha256:wheels,
        }).is_some() { return Err(denied()); }
    }
    Ok(out)
}
/// Pre-execution policy, NOT merely a postrun observation. Refuse Git,
/// local/path/editable/direct URL packages and missing registry wheel hashes
/// before human approval or OS execution. The frozen lock must equal the
/// originally reviewed lock SHA-256. No network or package-manager process.
pub fn verify_offline_registry_lock(
    frozen_root:&Path, expected_lock_sha256:&str,
)->Result<usize,SupervisorError>{
    if !hex64(expected_lock_sha256){return Err(denied());}
    let bytes=read_private(frozen_root,"uv.lock",MAX_LOCK)?;
    if file_hash(&bytes)!=expected_lock_sha256{return Err(denied());}
    let locked=verified_lock_packages(&bytes)?;
    let registry=locked.values().filter(|package|
        !package.registry_wheel_sha256.is_empty()).count();
    if registry==0{return Err(denied());}
    Ok(registry)
}
#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct InstalledDistributionEvidence {
    pub name: String,
    pub version: String,
    pub metadata_sha256: String,
    pub record_sha256: String,
    pub recorded_file_count: usize,
    pub license_expression: String,
    pub verified_license_file_count: usize,
    pub lock_registry_wheel_hash_count: usize,
    pub actual_wheel_archive_sha256_verified: bool,
}
#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct PackageOutputAudit {
    pub schema_version: u32,
    pub output_tree_sha256: String,
    pub frozen_manifest_sha256: String,
    pub frozen_lock_sha256: String,
    pub installed: Vec<InstalledDistributionEvidence>,
    pub record_hashes_verified: bool,
    pub license_files_verified: bool,
    pub package_versions_match_lock: bool,
    pub source_registry_declarations_observed: bool,
    pub wheel_archives_verified_against_lock: bool,
    pub source_provenance_verified: bool,
    pub approved_for_project_promotion: bool,
    pub network_attestation: &'static str,
    pub review_status: &'static str,
    pub digest_sha256: String,
}
/// The caller MUST additionally require an exact completed native supervisor
/// job, zero exit, verified descendant termination, original source revision
/// unchanged and a private output path supplied by the OS worker (not UI).
/// Even if local integrity passes, origin provenance is explicitly unproven.
pub fn audit_uv_private_output(
    output_root: &Path, frozen_root: &Path,
    expected_manifest_sha256: &str, expected_lock_sha256: &str,
) -> Result<PackageOutputAudit,SupervisorError> {
    if !hex64(expected_manifest_sha256) || !hex64(expected_lock_sha256)
        || !output_root.is_absolute() || !frozen_root.is_absolute() {
        return Err(denied());
    }
    let manifest=read_private(frozen_root,"pyproject.toml",MAX_LOCK)?;
    let lock=read_private(frozen_root,"uv.lock",MAX_LOCK)?;
    if file_hash(&manifest)!=expected_manifest_sha256
        || file_hash(&lock)!=expected_lock_sha256 { return Err(denied()); }
    let locked=verified_lock_packages(&lock)?;
    let inventory=audit_complete_package_output(output_root)?;
    let entries=inventory_map(&inventory)?;
    let mut dist_paths=Vec::<(String,String)>::new(); // (site, dist directory)
    for file in entries.keys() {
        if !file.ends_with(".dist-info/METADATA") { continue; }
        let folder=file.strip_suffix("/METADATA").ok_or_else(denied)?;
        let (site,dist)=folder.rsplit_once('/').ok_or_else(denied)?;
        let parts=site.split('/').collect::<Vec<_>>();
        if parts.len()!=4 || parts[0]!=".venv" || parts[1]!="lib"
            || !parts[2].starts_with("python3.") || parts[3]!="site-packages"
            || !dist.ends_with(".dist-info") {
            return Err(denied());
        }
        dist_paths.push((site.to_owned(),folder.to_owned()));
    }
    if dist_paths.is_empty() || dist_paths.len()>MAX_DISTS {
        // An empty virtualenv is not evidence of installed dependencies.
        return Err(denied());
    }
    let mut installed=Vec::new();
    let mut claimed_files=BTreeSet::new();
    let mut installed_names=BTreeSet::new();
    for (site,dist) in &dist_paths {
        let metadata_path=format!("{dist}/METADATA");
        let metadata=verified_bytes(output_root,&entries,&metadata_path,MAX_METADATA)?;
        let fields=parse_headers(&metadata)?;
        let name=normalized_name(single(&fields,"Name")?).ok_or_else(denied)?;
        let version=single(&fields,"Version")?.to_owned();
        if !installed_names.insert(name.clone()) { return Err(denied()); }
        let locked_pkg=locked.get(&name).ok_or_else(denied)?;
        if locked_pkg.version != version || locked_pkg.registry_wheel_sha256.is_empty() {
            return Err(denied());
        }
        let wheel_path=format!("{dist}/WHEEL");
        let wheel=verified_bytes(output_root,&entries,&wheel_path,MAX_METADATA)?;
        let wheel_fields=parse_headers(&wheel)?;
        if single(&wheel_fields,"Wheel-Version")?.split('.').next()!=Some("1"){
            return Err(denied());
        }
        if entries.contains_key(&format!("{dist}/direct_url.json")) {
            return Err(denied()); // no unreviewed URL/VCS/local/editable origins
        }
        let license_expression=single(&fields,"License-Expression")?;
        if license_expression.len()>256 { return Err(denied()); }
        let license_files=fields.get("License-File").ok_or_else(denied)?;
        if license_files.is_empty() || license_files.len()>64 { return Err(denied()); }
        let record_path=format!("{dist}/RECORD");
        let record=verified_bytes(output_root,&entries,&record_path,MAX_RECORD)?;
        let record_text=std::str::from_utf8(&record).map_err(|_| denied())?;
        let mut record_covered=BTreeSet::new();
        let mut count=0;
        for line in record_text.lines() {
            if line.is_empty() { return Err(denied()); }
            count+=1;
            if count>MAX_RECORD_LINES { return Err(denied()); }
            let [relative,record_hash,size]=record_fields(line.trim_end_matches('\r'))?;
            let target=record_target(site,&relative)?;
            if !record_covered.insert(target.clone()) { return Err(denied()); }
            let observed=entries.get(&target).ok_or_else(denied)?;
            if target == record_path {
                if !record_hash.is_empty() || !size.is_empty() { return Err(denied()); }
                if !claimed_files.insert(target) { return Err(denied()); }
                continue;
            }
            if target.ends_with(".pyc") && record_hash.is_empty() && size.is_empty(){
                continue; // PEP 376 permits un-hashed bytecode.
            }
            let claimed=record_hash.strip_prefix("sha256=").ok_or_else(denied)?;
            if claimed != hex_as_base64(&observed.1)?.as_str() {
                return Err(denied());
            }
            if size.parse::<u64>().ok()!=Some(observed.0) { return Err(denied()); }
            if !claimed_files.insert(target) { return Err(denied()); }
        }
        if !record_covered.contains(&metadata_path) ||
            !record_covered.contains(&wheel_path) ||
            !record_covered.contains(&record_path) {
            return Err(denied());
        }
        for relative in license_files {
            if !safe_relative(relative) { return Err(denied()); }
            let license_path=format!("{dist}/licenses/{relative}");
            if !record_covered.contains(&license_path) ||
                !claimed_files.contains(&license_path) {
                return Err(denied());
            }
        }
        installed.push(InstalledDistributionEvidence {
            name, version,
            metadata_sha256:entries.get(&metadata_path).ok_or_else(denied)?.1.clone(),
            record_sha256:file_hash(&record),
            recorded_file_count:count,
            license_expression:license_expression.to_owned(),
            verified_license_file_count:license_files.len(),
            lock_registry_wheel_hash_count:locked_pkg.registry_wheel_sha256.len(),
            actual_wheel_archive_sha256_verified:false,
        });
    }
    // Every actual site-packages file must be accounted for by an installed
    // distribution RECORD, except optional bytecode. Don't accept undeclared
    // payloads alongside benign metadata.
    for file in entries.keys() {
        if file.contains("/site-packages/")
            && !file.ends_with(".pyc")
            && !claimed_files.contains(file) {
            return Err(denied());
        }
    }
    // Re-hash entire private output after metadata reads; a changed or
    // swapped file denies the inspection instead of silently accepting.
    let final_inventory=audit_complete_package_output(output_root)?;
    if final_inventory.tree_sha256 != inventory.tree_sha256 ||
        final_inventory.file_count != inventory.file_count {
        return Err(denied());
    }
    let mut digest=Sha256::new();
    digest.update(b"lu-ide-package-local-audit-v1\0");
    digest.update(inventory.tree_sha256.as_bytes());
    digest.update(expected_manifest_sha256.as_bytes());
    digest.update(expected_lock_sha256.as_bytes());
    for dist in &installed {
        digest.update(dist.name.as_bytes());digest.update([0]);
        digest.update(dist.version.as_bytes());digest.update([0]);
        digest.update(dist.metadata_sha256.as_bytes());digest.update([0]);
        digest.update(dist.record_sha256.as_bytes());digest.update([0]);
    }
    Ok(PackageOutputAudit {
        schema_version:1,
        output_tree_sha256:inventory.tree_sha256,
        frozen_manifest_sha256:expected_manifest_sha256.to_owned(),
        frozen_lock_sha256:expected_lock_sha256.to_owned(),
        installed,
        record_hashes_verified:true,
        license_files_verified:true,
        package_versions_match_lock:true,
        source_registry_declarations_observed:true,
        wheel_archives_verified_against_lock:false,
        source_provenance_verified:false,
        approved_for_project_promotion:false,
        network_attestation:"os_worker_isolation_contract_not_independent_packet_proof",
        review_status:"local_artifact_integrity_only_origin_and_promotion_unverified",
        digest_sha256:format!("{:x}",digest.finalize()),
    })
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    struct Fixture { root:PathBuf, source:PathBuf, output:PathBuf }
    impl Drop for Fixture {
        fn drop(&mut self) {let _=fs::remove_dir_all(&self.root);}
    }
    fn fixture()->Fixture {
        let root=std::env::temp_dir().join(
            format!("ide054-package-{:032x}",rand::random::<u128>()));
        let source=root.join("frozen");
        let output=root.join("private-output");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&output).unwrap();
        Fixture{root,source,output}
    }
    fn wheel_lock()->String{
        format!(r#"
version = 1
[[package]]
name = "demo"
version = "1.0.0"
source = {{ registry = "https://pypi.org/simple" }}
wheels = [
  {{ url = "https://files.pythonhosted.org/demo.whl", hash = "sha256:{}" }},
]
"#,"a".repeat(64))
    }
    fn record_row(rel:&str,bytes:&[u8])->String{
        format!("{rel},sha256={},{}\n",
            URL_SAFE_NO_PAD.encode(Sha256::digest(bytes)),bytes.len())
    }
    fn wheel_fixture(f:&Fixture)->(String,String) {
        let py=b"[project]\nname = 'root'\nversion = '0.1.0'\n";
        let lock=wheel_lock();
        fs::write(f.source.join("pyproject.toml"),py).unwrap();
        fs::write(f.source.join("uv.lock"),lock.as_bytes()).unwrap();
        let site=f.output.join(".venv/lib/python3.13/site-packages");
        let dist=site.join("demo-1.0.0.dist-info");
        let module=site.join("demo/__init__.py");
        fs::create_dir_all(module.parent().unwrap()).unwrap();
        fs::create_dir_all(dist.join("licenses")).unwrap();
        let metadata=b"Metadata-Version: 2.4\nName: demo\nVersion: 1.0.0\nLicense-Expression: MIT\nLicense-File: LICENSE.txt\n\nExample\n";
        let wheel=b"Wheel-Version: 1.0\nRoot-Is-Purelib: true\nTag: py3-none-any\n";
        let license=b"MIT license source\n";
        let source=b"value=42\n";
        fs::write(&module,source).unwrap();
        fs::write(dist.join("METADATA"),metadata).unwrap();
        fs::write(dist.join("WHEEL"),wheel).unwrap();
        fs::write(dist.join("licenses/LICENSE.txt"),license).unwrap();
        let mut record=String::new();
        record.push_str(&record_row("demo/__init__.py",source));
        record.push_str(&record_row("demo-1.0.0.dist-info/METADATA",metadata));
        record.push_str(&record_row("demo-1.0.0.dist-info/WHEEL",wheel));
        record.push_str(&record_row(
            "demo-1.0.0.dist-info/licenses/LICENSE.txt",license));
        record.push_str("demo-1.0.0.dist-info/RECORD,,\n");
        fs::write(dist.join("RECORD"),record).unwrap();
        (file_hash(py),file_hash(lock.as_bytes()))
    }
    #[test]
    fn independent_private_install_bytes_license_and_lock_are_checked() {
        let f=fixture(); let (py,lock)=wheel_fixture(&f);
        let audit=audit_uv_private_output(&f.output,&f.source,&py,&lock).unwrap();
        assert!(audit.record_hashes_verified && audit.license_files_verified
            && audit.package_versions_match_lock);
        assert_eq!(audit.installed.len(),1);
        assert_eq!(audit.installed[0].name,"demo");
        assert_eq!(audit.installed[0].verified_license_file_count,1);
        assert_eq!(audit.installed[0].recorded_file_count,5);
        assert!(!audit.source_provenance_verified
            && !audit.wheel_archives_verified_against_lock
            && !audit.approved_for_project_promotion);
        assert!(hex64(&audit.digest_sha256));
    }
    #[test]
    fn tampered_installed_bytes_or_record_hash_are_refused() {
        let f=fixture(); let (py,lock)=wheel_fixture(&f);
        fs::write(f.output.join(
            ".venv/lib/python3.13/site-packages/demo/__init__.py"),b"altered").unwrap();
        assert!(audit_uv_private_output(&f.output,&f.source,&py,&lock).is_err());
        let f=fixture(); let (py,lock)=wheel_fixture(&f);
        let record=f.output.join(
            ".venv/lib/python3.13/site-packages/demo-1.0.0.dist-info/RECORD");
        let original=fs::read_to_string(&record).unwrap();
        fs::write(&record,original.replace("sha256=","sha256=x")).unwrap();
        assert!(audit_uv_private_output(&f.output,&f.source,&py,&lock).is_err());
    }
    #[test]
    fn missing_license_file_unlocked_source_and_direct_url_are_denied() {
        let f=fixture(); let (py,lock)=wheel_fixture(&f);
        fs::remove_file(f.output.join(
            ".venv/lib/python3.13/site-packages/demo-1.0.0.dist-info/licenses/LICENSE.txt")
        ).unwrap();
        assert!(audit_uv_private_output(&f.output,&f.source,&py,&lock).is_err());
        let f=fixture(); let (py,lock)=wheel_fixture(&f);
        fs::write(f.output.join(
            ".venv/lib/python3.13/site-packages/demo-1.0.0.dist-info/direct_url.json"),
            br#"{"url":"file:///tmp/foreign","dir_info":{"editable":true}}"#).unwrap();
        assert!(audit_uv_private_output(&f.output,&f.source,&py,&lock).is_err());
        let f=fixture(); let (py,lock)=wheel_fixture(&f);
        fs::write(f.source.join("uv.lock"),
            wheel_lock().replace("registry =","git =")).unwrap();
        let updated=file_hash(&fs::read(f.source.join("uv.lock")).unwrap());
        assert!(audit_uv_private_output(&f.output,&f.source,&py,&updated).is_err());
        assert!(audit_uv_private_output(&f.output,&f.source,&py,&lock).is_err());
    }
    #[test]
    fn source_or_lock_changes_are_never_a_valid_install() {
        let f=fixture(); let (py,lock)=wheel_fixture(&f);
        fs::write(f.source.join("pyproject.toml"),b"changed").unwrap();
        assert!(audit_uv_private_output(&f.output,&f.source,&py,&lock).is_err());
        let f=fixture(); let (py,lock)=wheel_fixture(&f);
        fs::write(f.source.join("uv.lock"),b"changed").unwrap();
        assert!(audit_uv_private_output(&f.output,&f.source,&py,&lock).is_err());
    }
    #[test]
    fn unknown_installed_payload_and_record_traversal_are_denied() {
        let f=fixture(); let (py,lock)=wheel_fixture(&f);
        fs::write(f.output.join(
            ".venv/lib/python3.13/site-packages/backdoor.py"),b"print('x')").unwrap();
        assert!(audit_uv_private_output(&f.output,&f.source,&py,&lock).is_err());
        assert!(record_target(".venv/lib/python3.13/site-packages",
            "../../../../../etc/passwd").is_err());
    }
    #[cfg(unix)]
    #[test]
    fn symlinked_virtualenv_is_not_accepted_as_a_sealed_package() {
        use std::os::unix::fs::symlink;
        let f=fixture();let (py,lock)=wheel_fixture(&f);
        fs::create_dir_all(f.output.join(".venv/bin")).unwrap();
        symlink("/usr/bin/python3",f.output.join(".venv/bin/python")).unwrap();
        assert!(audit_uv_private_output(&f.output,&f.source,&py,&lock).is_err());
    }
}
