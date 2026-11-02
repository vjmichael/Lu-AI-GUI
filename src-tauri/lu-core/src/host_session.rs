//! Host-owned window/profile binding. No model or webview can mint a session.
//! This does not authenticate a person sharing an unlocked OS account.
use crate::host_resource_authority::{AuthorizedResource, HostPrincipal, ResourceAction};
use crate::ownership_store::{OwnershipStore, OwnershipStoreError};
use std::{collections::BTreeMap, sync::RwLock};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    InvalidWindow,
    UnknownWindow,
    Poisoned,
    Ownership(OwnershipStoreError),
}

/// The host creates these bindings from trusted window lifecycle events.
/// IDs supplied in prompts or IPC payloads must never enroll sessions.
#[derive(Default)]
pub struct HostSessionRegistry {
    windows: RwLock<BTreeMap<String, HostPrincipal>>,
}

impl HostSessionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Trusted Rust-side startup/window callback only. Refuses replacement of
    /// a live window's principal; close/reopen is an explicit lifecycle action.
    pub fn bind_host_window(
        &self,
        window_label: &str,
        principal: HostPrincipal,
    ) -> Result<(), SessionError> {
        if window_label.is_empty() || window_label.len() > 128 {
            return Err(SessionError::InvalidWindow);
        }
        let mut windows = self.windows.write().map_err(|_| SessionError::Poisoned)?;
        if windows.contains_key(window_label) {
            return Err(SessionError::InvalidWindow);
        }
        windows.insert(window_label.to_owned(), principal);
        Ok(())
    }

    pub fn unbind_host_window(&self, window_label: &str) -> Result<(), SessionError> {
        let mut windows = self.windows.write().map_err(|_| SessionError::Poisoned)?;
        windows.remove(window_label);
        Ok(())
    }

    pub fn require_window_resource(
        &self,
        window_label: &str,
        conversation_id: &str,
        action: ResourceAction,
        store: &OwnershipStore,
    ) -> Result<AuthorizedResource, SessionError> {
        let windows = self.windows.read().map_err(|_| SessionError::Poisoned)?;
        let principal = windows
            .get(window_label)
            .ok_or(SessionError::UnknownWindow)?;
        store
            .require(principal, conversation_id, action)
            .map_err(SessionError::Ownership)
    }
}

/// Explicit opt-in permissions remain separate from session identity.
pub fn read_only_principal(profile_id: String, project_id: String) -> HostPrincipal {
    HostPrincipal {
        profile_id,
        active_project_id: project_id,
        export_enabled: false,
        training_enabled: false,
        maximum_sensitivity: crate::host_resource_authority::Sensitivity::Internal,
    }
}
