//! Host-controlled CE resource broker. This is a Rust-side broker core,
//! not yet an authenticated worker IPC transport. It never gives a worker a
//! host path, network socket, device handle, or a durable grant token.
//!
//! Every operation rechecks conversation authority, durable digest-bound
//! approval, exact tool effect, scope and the host's own authorizer.

use std::{
    collections::BTreeSet,
    io::{Read, Write},
    path::{Component, Path},
    sync::atomic::{AtomicU64, Ordering},
    sync::{Arc, Mutex},
};

use cap_std::{
    ambient_authority,
    fs::{Dir, OpenOptions},
};

use crate::{
    ce_permissions::{CePermissionError, CePermissionStore},
    ce_schema::CeToolSchema,
    ce_signing::VerifiedCePackage,
    contracts::{ToolAuthorizer, ToolDescriptor, ToolEffect, ToolInvocation},
    conversation_authority::{BoundCallOrigin, ConversationAuthority},
};

pub const MAX_BROKER_FILE_BYTES: usize = 64 * 1024;
const MAX_REQUESTS_PER_SESSION: usize = 4096;
static NEXT_TEMP: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CeBrokerError {
    InvalidPath,
    InvalidRequest,
    Replay,
    PermissionDenied,
    NetworkDenied,
    TooLarge,
    Io(String),
    Unavailable,
}

impl From<std::io::Error> for CeBrokerError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value.to_string())
    }
}

fn relative_path(path: &str) -> Result<&Path, CeBrokerError> {
    if path.is_empty() || path.len() > 512 || path.contains('\0') {
        return Err(CeBrokerError::InvalidPath);
    }
    let parsed = Path::new(path);
    if parsed
        .components()
        .all(|part| matches!(part, Component::Normal(_)))
        && !parsed.is_absolute()
    {
        Ok(parsed)
    } else {
        Err(CeBrokerError::InvalidPath)
    }
}

pub struct CeResourceBroker {
    authority: ConversationAuthority,
    host_policy: Arc<dyn ToolAuthorizer>,
    permissions: CePermissionStore,
    package: VerifiedCePackage,
    profile_id: String,
    scope_id: String,
    root: Dir,
    replay: Arc<Mutex<BTreeSet<(String, u64)>>>,
}

impl CeResourceBroker {
    /// TRUSTED HOST ONLY. root must be a private workspace already selected
    /// by the host, not a path from the CE manifest or worker message.
    pub fn new(
        authority: ConversationAuthority,
        host_policy: Arc<dyn ToolAuthorizer>,
        permissions: CePermissionStore,
        package: VerifiedCePackage,
        profile_id: String,
        scope_id: String,
        root: &Path,
    ) -> Result<Self, CeBrokerError> {
        if profile_id.is_empty()
            || scope_id.is_empty()
            || profile_id.len() > 128
            || scope_id.len() > 128
        {
            return Err(CeBrokerError::InvalidRequest);
        }
        let metadata = std::fs::symlink_metadata(root)?;
        if !metadata.file_type().is_dir() {
            return Err(CeBrokerError::InvalidPath);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o077 != 0 {
                return Err(CeBrokerError::InvalidPath);
            }
        }
        let root = Dir::open_ambient_dir(root, ambient_authority())?;
        let replay = Arc::new(Mutex::new(BTreeSet::new()));
        let weak = Arc::downgrade(&replay);
        authority
            .on_task_revoked(Arc::new(move |task_id| {
                if let Some(replay) = weak.upgrade() {
                    if let Ok(mut replay) = replay.lock() {
                        replay.retain(|(id, _)| id != task_id);
                    }
                }
            }))
            .map_err(|_| CeBrokerError::Unavailable)?;
        Ok(Self {
            authority,
            host_policy,
            permissions,
            package,
            profile_id,
            scope_id,
            root,
            replay,
        })
    }

    pub fn package_id(&self) -> &str {
        &self.package.manifest().id
    }
    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }
    pub fn scope_id(&self) -> &str {
        &self.scope_id
    }
    pub fn artifact_sha256(&self) -> &str {
        self.package.artifact_sha256()
    }

    /// Validate an execution call without consuming a broker request ID.
    pub fn authorize_call(
        &self,
        origin: &BoundCallOrigin,
        invocation: &ToolInvocation,
        descriptor: &ToolDescriptor,
    ) -> Result<(), CeBrokerError> {
        if invocation.workspace_id.as_deref() != Some(self.scope_id.as_str())
            || !self.package.manifest().tools.contains(&descriptor.id)
        {
            return Err(CeBrokerError::InvalidRequest);
        }
        CeToolSchema::for_descriptor(descriptor)
            .and_then(|schema| schema.validate_input(descriptor, &invocation.input.bytes))
            .map_err(|_| CeBrokerError::InvalidRequest)?;
        self.authority
            .authorize_worker_call(origin, invocation, descriptor, self.host_policy.as_ref())
            .map_err(|_| CeBrokerError::PermissionDenied)?;
        self.permissions
            .require(
                &self.package,
                &self.profile_id,
                &self.scope_id,
                &descriptor.id,
                descriptor.effect,
            )
            .map_err(|_| CeBrokerError::PermissionDenied)
    }

    /// Bind the worker supervisor to the SAME durable store used for broker
    /// checks. No caller can substitute a different grant database here.
    pub fn on_task_revoked(
        &self,
        callback: Arc<dyn Fn(&str) + Send + Sync>,
    ) -> Result<(), CeBrokerError> {
        self.authority
            .on_task_revoked(callback)
            .map_err(|_| CeBrokerError::Unavailable)
    }

    pub fn on_permission_revoked(
        &self,
        callback: Arc<dyn Fn(&crate::ce_permissions::CeRevocation) + Send + Sync>,
    ) -> Result<(), CeBrokerError> {
        self.permissions
            .subscribe_revocations(callback)
            .map_err(|_| CeBrokerError::Unavailable)
    }

    fn admit(
        &self,
        origin: &BoundCallOrigin,
        invocation: &ToolInvocation,
        descriptor: &ToolDescriptor,
        effect: ToolEffect,
        request_id: u64,
    ) -> Result<(), CeBrokerError> {
        if request_id == 0
            || descriptor.effect != effect
            || invocation.workspace_id.as_deref() != Some(self.scope_id.as_str())
            || !self.package.manifest().tools.contains(&descriptor.id)
        {
            return Err(CeBrokerError::InvalidRequest);
        }
        CeToolSchema::for_descriptor(descriptor)
            .and_then(|schema| schema.validate_input(descriptor, &invocation.input.bytes))
            .map_err(|_| CeBrokerError::InvalidRequest)?;
        self.authority
            .authorize_worker_call(origin, invocation, descriptor, self.host_policy.as_ref())
            .map_err(|_| CeBrokerError::PermissionDenied)?;
        self.permissions
            .require(
                &self.package,
                &self.profile_id,
                &self.scope_id,
                &descriptor.id,
                descriptor.effect,
            )
            .map_err(|_err: CePermissionError| CeBrokerError::PermissionDenied)?;
        let mut replay = self.replay.lock().map_err(|_| CeBrokerError::Unavailable)?;
        if replay.len() >= MAX_REQUESTS_PER_SESSION {
            return Err(CeBrokerError::TooLarge);
        }
        if !replay.insert((origin.task_id.clone(), request_id)) {
            return Err(CeBrokerError::Replay);
        }
        Ok(())
    }

    /// Recheck and hold both task authority and the durable signing/grant
    /// lease across the actual bounded filesystem effect. Lock order is
    /// task-read -> SQLite IMMEDIATE; neither lock is held for worker execution.
    fn with_effect_fence<R>(
        &self,
        origin: &BoundCallOrigin,
        invocation: &ToolInvocation,
        descriptor: &ToolDescriptor,
        operation: impl FnOnce() -> Result<R, CeBrokerError>,
    ) -> Result<R, CeBrokerError> {
        self.authority
            .with_authorized_worker_effect(
                origin,
                invocation,
                descriptor,
                self.host_policy.as_ref(),
                || {
                    self.permissions.with_authorized_grant(
                        &self.package,
                        &self.profile_id,
                        &self.scope_id,
                        &descriptor.id,
                        descriptor.effect,
                        operation,
                    )
                },
            )
            .map_err(|_| CeBrokerError::PermissionDenied)?
            .map_err(|_| CeBrokerError::PermissionDenied)?
    }

    /// Read at most 64 KiB from the host-selected scope. cap-std resolves
    /// relative to the open directory capability, including symlink handling.
    pub fn read_file(
        &self,
        origin: &BoundCallOrigin,
        invocation: &ToolInvocation,
        descriptor: &ToolDescriptor,
        request_id: u64,
        path: &str,
    ) -> Result<Vec<u8>, CeBrokerError> {
        let path = relative_path(path)?;
        self.admit(
            origin,
            invocation,
            descriptor,
            ToolEffect::ReadWorkspace,
            request_id,
        )?;
        self.with_effect_fence(origin, invocation, descriptor, || {
            let file = self.root.open(path)?;
            let mut data = Vec::new();
            file.take((MAX_BROKER_FILE_BYTES + 1) as u64)
                .read_to_end(&mut data)?;
            if data.len() > MAX_BROKER_FILE_BYTES {
                return Err(CeBrokerError::TooLarge);
            }
            Ok(data)
        })
    }

    /// Write a new/updated scoped file via a create-new temporary file and
    /// atomic rename. No absolute paths, traversal, network, or shell.
    pub fn write_file(
        &self,
        origin: &BoundCallOrigin,
        invocation: &ToolInvocation,
        descriptor: &ToolDescriptor,
        request_id: u64,
        path: &str,
        data: &[u8],
    ) -> Result<(), CeBrokerError> {
        let path = relative_path(path)?;
        if data.len() > MAX_BROKER_FILE_BYTES {
            return Err(CeBrokerError::TooLarge);
        }
        self.admit(
            origin,
            invocation,
            descriptor,
            ToolEffect::WriteWorkspace,
            request_id,
        )?;
        // The final atomic rename is inside the same held authorization
        // lease. Revocation waits for an already-admitted rename to commit
        // or fail; no new write can commit after revocation returns.
        self.with_effect_fence(origin, invocation, descriptor, || {
            let name = format!(
                ".lu-ce-tmp-{}-{}",
                std::process::id(),
                NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
            );
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            let mut file = self.root.open_with(&name, &options)?;
            let result = (|| -> Result<(), CeBrokerError> {
                file.write_all(data)?;
                file.sync_all()?;
                self.root.rename(&name, &self.root, path)?;
                Ok(())
            })();
            if result.is_err() {
                let _ = self.root.remove_file(&name);
            }
            result
        })
    }

    /// No ambient network or device access. A future network broker requires
    /// per-destination allowlists, TLS policy and its own audited grant type.
    pub fn network_request(&self) -> Result<(), CeBrokerError> {
        Err(CeBrokerError::NetworkDenied)
    }
}
