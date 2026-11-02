//! Optional Lu-native Core Extension (CE) package metadata and lifecycle.
//!
//! This is a trusted-host, in-memory control plane. It does NOT load an
//! archive, execute external code, authenticate publishers, persist approvals,
//! or provide OS worker isolation. External artifacts stay quarantined.
//! Only already-compiled trusted native implementations can be registered,
//! and only read-only effects are accepted until cancellation and isolation
//! have been independently implemented and tested.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, RwLock},
};

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::{
    ce_signing::{CePublisherTrust, SignatureError, VerifiedCePackage},
    contracts::{ModuleKind, ModuleManifest, NativeTool, ToolEffect, CORE_CONTRACT_VERSION},
    core_extension::{CoreExtensionError, CoreExtensionHost},
};

pub const MAX_CE_MANIFEST_BYTES: usize = 32 * 1024;
pub const MAX_CE_ARCHIVE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_CE_PACKAGES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CeVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl CeVersion {
    /// Strict x.y.z versions only for the initial manifest contract.
    /// Prereleases, build metadata and arbitrary semver ranges are rejected.
    pub fn parse(raw: &str) -> Result<Self, CePackageError> {
        let parts: Vec<_> = raw.split('.').collect();
        if parts.len() != 3 {
            return Err(CePackageError::InvalidVersion(raw.into()));
        }
        let mut nums = [0_u32; 3];
        for (index, part) in parts.iter().enumerate() {
            if part.is_empty()
                || (part.len() > 1 && part.starts_with('0'))
                || !part.bytes().all(|b| b.is_ascii_digit())
            {
                return Err(CePackageError::InvalidVersion(raw.into()));
            }
            nums[index] = part
                .parse()
                .map_err(|_| CePackageError::InvalidVersion(raw.into()))?;
        }
        Ok(Self {
            major: nums[0],
            minor: nums[1],
            patch: nums[2],
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeDependency {
    pub id: String,
    pub min_version: CeVersion,
    pub max_exclusive: Option<CeVersion>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CePackageManifest {
    pub id: String,
    pub version: CeVersion,
    pub contract_version: u16,
    /// A claim from the package, NOT a verified publisher identity.
    pub publisher: String,
    /// An integrity digest, NOT a code signature or safety attestation.
    pub artifact_sha256: String,
    pub dependencies: Vec<CeDependency>,
    pub tools: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CePackageStatus {
    Quarantined,
    /// Publisher signature and exact bytes verified, but not approved or enabled.
    Verified,
    Disabled,
    Enabled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CePackageRecord {
    pub manifest: CePackageManifest,
    pub status: CePackageStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CePackageError {
    InvalidManifest(&'static str),
    InvalidVersion(String),
    UnsupportedContract,
    DigestMismatch,
    ArchiveTooLarge,
    DuplicatePackage(String),
    TooManyPackages,
    UnknownPackage(String),
    MissingDependency(String),
    DependencyVersion(String),
    DependencyCycle(String),
    EnabledDependent(String),
    PackageEnabled(String),
    AlreadyEnabled(String),
    QuarantinedCannotExecute,
    SignedWorkerRequired,
    Signature(SignatureError),
    IsolationRequired,
    ModuleMismatch,
    Unavailable,
    Registry(CoreExtensionError),
}

fn valid_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.contains('.')
        && s.split('.').all(|part| {
            !part.is_empty()
                && part.len() <= 64
                && part.bytes().enumerate().all(|(i, b)| {
                    b.is_ascii_lowercase()
                        || (i > 0 && (b.is_ascii_digit() || b == b'_' || b == b'-'))
                })
        })
}

fn only_keys(object: &Map<String, Value>, allowed: &[&str]) -> bool {
    object.keys().all(|key| allowed.contains(&key.as_str()))
}

fn field_str<'a>(
    object: &'a Map<String, Value>,
    name: &'static str,
) -> Result<&'a str, CePackageError> {
    object
        .get(name)
        .and_then(Value::as_str)
        .ok_or(CePackageError::InvalidManifest(name))
}

impl CePackageManifest {
    /// Rejects unknown fields and ambiguous ranges; no arbitrary executable
    /// paths, network endpoints, or permissions can enter the manifest.
    pub fn parse(json: &str) -> Result<Self, CePackageError> {
        if json.len() > MAX_CE_MANIFEST_BYTES {
            return Err(CePackageError::InvalidManifest(
                "manifest exceeds byte limit",
            ));
        }
        let value: Value = serde_json::from_str(json)
            .map_err(|_| CePackageError::InvalidManifest("invalid JSON"))?;
        let object = value
            .as_object()
            .ok_or(CePackageError::InvalidManifest("expected object"))?;
        if !only_keys(
            object,
            &[
                "format_version",
                "id",
                "version",
                "contract_version",
                "publisher",
                "artifact_sha256",
                "dependencies",
                "tools",
            ],
        ) {
            return Err(CePackageError::InvalidManifest("unknown manifest field"));
        }
        if object.get("format_version").and_then(Value::as_u64) != Some(1) {
            return Err(CePackageError::InvalidManifest("format_version"));
        }
        let contract_version = object
            .get("contract_version")
            .and_then(Value::as_u64)
            .ok_or(CePackageError::InvalidManifest("contract_version"))?;
        if contract_version != u64::from(CORE_CONTRACT_VERSION) {
            return Err(CePackageError::UnsupportedContract);
        }
        let id = field_str(object, "id")?.to_owned();
        let version = CeVersion::parse(field_str(object, "version")?)?;
        let publisher = field_str(object, "publisher")?.to_owned();
        let artifact_sha256 = field_str(object, "artifact_sha256")?.to_owned();
        let deps = object
            .get("dependencies")
            .and_then(Value::as_array)
            .ok_or(CePackageError::InvalidManifest("dependencies"))?;
        let tools = object
            .get("tools")
            .and_then(Value::as_array)
            .ok_or(CePackageError::InvalidManifest("tools"))?;
        if deps.len() > 32 || tools.len() > 16 {
            return Err(CePackageError::InvalidManifest(
                "too many dependencies or tools",
            ));
        }
        let mut dependencies = Vec::new();
        for dep in deps {
            let d = dep
                .as_object()
                .ok_or(CePackageError::InvalidManifest("dependency object"))?;
            if !only_keys(d, &["id", "min_version", "max_exclusive"]) {
                return Err(CePackageError::InvalidManifest("unknown dependency field"));
            }
            let max_exclusive = d
                .get("max_exclusive")
                .map(|v| {
                    v.as_str()
                        .ok_or(CePackageError::InvalidManifest("max_exclusive"))
                        .and_then(CeVersion::parse)
                })
                .transpose()?;
            dependencies.push(CeDependency {
                id: field_str(d, "id")?.into(),
                min_version: CeVersion::parse(field_str(d, "min_version")?)?,
                max_exclusive,
            });
        }
        let mut tool_ids = Vec::new();
        for tool in tools {
            tool_ids.push(
                tool.as_str()
                    .ok_or(CePackageError::InvalidManifest("tool ID"))?
                    .to_owned(),
            );
        }
        let manifest = Self {
            id,
            version,
            contract_version: CORE_CONTRACT_VERSION,
            publisher,
            artifact_sha256,
            dependencies,
            tools: tool_ids,
        };
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn validate(&self) -> Result<(), CePackageError> {
        if !valid_id(&self.id) {
            return Err(CePackageError::InvalidManifest("id"));
        }
        if self.contract_version != CORE_CONTRACT_VERSION {
            return Err(CePackageError::UnsupportedContract);
        }
        if self.publisher.trim().is_empty()
            || self.publisher.len() > 128
            || self.publisher.chars().any(char::is_control)
        {
            return Err(CePackageError::InvalidManifest("publisher"));
        }
        if self.artifact_sha256.len() != 64
            || !self
                .artifact_sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(CePackageError::InvalidManifest("artifact_sha256"));
        }
        if self.dependencies.len() > 32 || self.tools.len() > 16 {
            return Err(CePackageError::InvalidManifest("too many entries"));
        }
        let mut deps = BTreeSet::new();
        for d in &self.dependencies {
            if !valid_id(&d.id) || d.id == self.id || !deps.insert(d.id.clone()) {
                return Err(CePackageError::InvalidManifest("dependency ID"));
            }
            if d.max_exclusive.is_some_and(|max| max <= d.min_version) {
                return Err(CePackageError::InvalidManifest("dependency range"));
            }
        }
        let mut tools = BTreeSet::new();
        for id in &self.tools {
            if !valid_id(id)
                || !id.starts_with(&(self.id.clone() + "."))
                || !tools.insert(id.clone())
            {
                return Err(CePackageError::InvalidManifest("tool ID"));
            }
        }
        Ok(())
    }
}

fn visit(
    id: &str,
    records: &BTreeMap<String, CePackageRecord>,
    visiting: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
    order: &mut Vec<String>,
) -> Result<(), CePackageError> {
    if visited.contains(id) {
        return Ok(());
    }
    if !visiting.insert(id.to_owned()) {
        return Err(CePackageError::DependencyCycle(id.into()));
    }
    let record = records
        .get(id)
        .ok_or_else(|| CePackageError::MissingDependency(id.into()))?;
    for dep in &record.manifest.dependencies {
        let installed = records
            .get(&dep.id)
            .ok_or_else(|| CePackageError::MissingDependency(dep.id.clone()))?;
        if installed.manifest.version < dep.min_version
            || dep
                .max_exclusive
                .is_some_and(|max| installed.manifest.version >= max)
        {
            return Err(CePackageError::DependencyVersion(dep.id.clone()));
        }
        visit(&dep.id, records, visiting, visited, order)?;
    }
    visiting.remove(id);
    visited.insert(id.into());
    order.push(id.into());
    Ok(())
}

/// Host-only CE package registry. In-memory lifecycle states are intentionally
/// not a persisted installation or an executable package manager.
#[derive(Clone)]
pub struct CePackageManager {
    host: CoreExtensionHost,
    records: Arc<RwLock<BTreeMap<String, CePackageRecord>>>,
}

impl CePackageManager {
    pub fn new(host: CoreExtensionHost) -> Self {
        Self {
            host,
            records: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }

    fn stage(
        &self,
        manifest: CePackageManifest,
        status: CePackageStatus,
    ) -> Result<(), CePackageError> {
        manifest.validate()?;
        let mut records = self
            .records
            .write()
            .map_err(|_| CePackageError::Unavailable)?;
        if records.contains_key(&manifest.id) {
            return Err(CePackageError::DuplicatePackage(manifest.id));
        }
        if records.len() >= MAX_CE_PACKAGES {
            return Err(CePackageError::TooManyPackages);
        }
        records.insert(manifest.id.clone(), CePackageRecord { manifest, status });
        Ok(())
    }

    /// External archives are checked for integrity but never extracted,
    /// executed or registered. The bytes are NOT retained by this prototype.
    pub fn stage_external(
        &self,
        manifest_json: &str,
        archive: &[u8],
    ) -> Result<(), CePackageError> {
        if archive.is_empty() || archive.len() > MAX_CE_ARCHIVE_BYTES {
            return Err(CePackageError::ArchiveTooLarge);
        }
        let manifest = CePackageManifest::parse(manifest_json)?;
        if format!("{:x}", Sha256::digest(archive)) != manifest.artifact_sha256 {
            return Err(CePackageError::DigestMismatch);
        }
        self.stage(manifest, CePackageStatus::Quarantined)
    }

    /// Host-only: a pinned publisher signature authenticates exact bytes.
    /// This DOES NOT approve, install, register or launch the executable.
    /// The caller must separately stage bytes and obtain a durable grant.
    pub fn stage_signed_external(
        &self,
        trust: &CePublisherTrust,
        envelope_json: &str,
        artifact: &[u8],
    ) -> Result<VerifiedCePackage, CePackageError> {
        let verified = trust
            .verify(envelope_json, artifact)
            .map_err(CePackageError::Signature)?;
        self.stage(verified.manifest().clone(), CePackageStatus::Verified)?;
        Ok(verified)
    }

    /// TRUSTED HOST ONLY: the implementation must already be compiled into
    /// Lu. Never expose this method as a model or webview IPC command.
    pub fn stage_bundled_trusted(&self, manifest: CePackageManifest) -> Result<(), CePackageError> {
        self.stage(manifest, CePackageStatus::Disabled)
    }

    pub fn get(&self, id: &str) -> Result<Option<CePackageRecord>, CePackageError> {
        let records = self
            .records
            .read()
            .map_err(|_| CePackageError::Unavailable)?;
        Ok(records.get(id).cloned())
    }

    pub fn dependency_order(&self, id: &str) -> Result<Vec<String>, CePackageError> {
        let records = self
            .records
            .read()
            .map_err(|_| CePackageError::Unavailable)?;
        let mut order = Vec::new();
        visit(
            id,
            &records,
            &mut BTreeSet::new(),
            &mut BTreeSet::new(),
            &mut order,
        )?;
        Ok(order)
    }

    /// Atomic metadata + native registry transition. No model grants are made.
    /// Only read-only bundled implementations may be enabled at this stage.
    pub fn enable_bundled_trusted(
        &self,
        id: &str,
        module: ModuleManifest,
        implementations: Vec<Arc<dyn NativeTool>>,
    ) -> Result<(), CePackageError> {
        let mut records = self
            .records
            .write()
            .map_err(|_| CePackageError::Unavailable)?;
        let record = records
            .get(id)
            .ok_or_else(|| CePackageError::UnknownPackage(id.into()))?;
        if record.status == CePackageStatus::Quarantined {
            return Err(CePackageError::QuarantinedCannotExecute);
        }
        if record.status == CePackageStatus::Verified {
            return Err(CePackageError::SignedWorkerRequired);
        }
        if record.status == CePackageStatus::Enabled {
            return Err(CePackageError::AlreadyEnabled(id.into()));
        }
        if module.kind != ModuleKind::CoreExtension
            || module.id != id
            || module.contract_version != record.manifest.contract_version
            || module
                .tools
                .iter()
                .map(|d| d.id.as_str())
                .collect::<BTreeSet<_>>()
                != record
                    .manifest
                    .tools
                    .iter()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>()
        {
            return Err(CePackageError::ModuleMismatch);
        }
        if module
            .tools
            .iter()
            .any(|tool| tool.effect != ToolEffect::ReadWorkspace)
        {
            return Err(CePackageError::IsolationRequired);
        }
        let mut order = Vec::new();
        visit(
            id,
            &records,
            &mut BTreeSet::new(),
            &mut BTreeSet::new(),
            &mut order,
        )?;
        for dependency in order.iter().filter(|dependency| dependency.as_str() != id) {
            if records
                .get(dependency)
                .is_none_or(|r| r.status != CePackageStatus::Enabled)
            {
                return Err(CePackageError::MissingDependency(dependency.clone()));
            }
        }
        self.host
            .register_trusted(module, implementations)
            .map_err(CePackageError::Registry)?;
        records.get_mut(id).expect("record verified above").status = CePackageStatus::Enabled;
        Ok(())
    }

    /// Refuses removal while an enabled dependent still needs the extension.
    /// Registry unload closes admission and returns busy for in-flight calls;
    /// a trusted host must wait/cancel and retry. This is not OS isolation.
    pub fn disable_bundled_trusted(&self, id: &str) -> Result<(), CePackageError> {
        let mut records = self
            .records
            .write()
            .map_err(|_| CePackageError::Unavailable)?;
        let record = records
            .get(id)
            .ok_or_else(|| CePackageError::UnknownPackage(id.into()))?;
        if record.status != CePackageStatus::Enabled {
            return Err(CePackageError::PackageEnabled(
                "extension is not enabled".into(),
            ));
        }
        if let Some(dependent) = records.values().find(|r| {
            r.status == CePackageStatus::Enabled
                && r.manifest.dependencies.iter().any(|d| d.id == id)
        }) {
            return Err(CePackageError::EnabledDependent(
                dependent.manifest.id.clone(),
            ));
        }
        self.host
            .unregister_trusted(id)
            .map_err(CePackageError::Registry)?;
        records.get_mut(id).expect("record verified above").status = CePackageStatus::Disabled;
        Ok(())
    }

    pub fn uninstall_disabled(&self, id: &str) -> Result<(), CePackageError> {
        let mut records = self
            .records
            .write()
            .map_err(|_| CePackageError::Unavailable)?;
        let record = records
            .get(id)
            .ok_or_else(|| CePackageError::UnknownPackage(id.into()))?;
        if record.status == CePackageStatus::Enabled {
            return Err(CePackageError::PackageEnabled(id.into()));
        }
        if let Some(dependent) = records
            .values()
            .find(|r| r.manifest.dependencies.iter().any(|d| d.id == id))
        {
            return Err(CePackageError::EnabledDependent(
                dependent.manifest.id.clone(),
            ));
        }
        records.remove(id);
        Ok(())
    }

    /// Disabled-only metadata replacement with rollback on incompatible
    /// dependents. This does not replace executable code or files on disk.
    pub fn replace_disabled_trusted(
        &self,
        id: &str,
        replacement: CePackageManifest,
    ) -> Result<(), CePackageError> {
        replacement.validate()?;
        if replacement.id != id {
            return Err(CePackageError::ModuleMismatch);
        }
        let mut records = self
            .records
            .write()
            .map_err(|_| CePackageError::Unavailable)?;
        let old = records
            .get(id)
            .ok_or_else(|| CePackageError::UnknownPackage(id.into()))?;
        if old.status != CePackageStatus::Disabled {
            return Err(CePackageError::PackageEnabled(id.into()));
        }
        if replacement.version <= old.manifest.version {
            return Err(CePackageError::InvalidVersion(
                "upgrade must increase version".into(),
            ));
        }
        for dependent in records.values() {
            for dep in &dependent.manifest.dependencies {
                if dep.id == id
                    && (replacement.version < dep.min_version
                        || dep
                            .max_exclusive
                            .is_some_and(|max| replacement.version >= max))
                {
                    return Err(CePackageError::DependencyVersion(
                        dependent.manifest.id.clone(),
                    ));
                }
            }
        }
        // No mutation before every validation above passes.
        records.get_mut(id).expect("record verified above").manifest = replacement;
        Ok(())
    }
}
