//! Minimal, explicitly scoped native-tool authorization policy.
//!
//! Only a trusted host may issue or revoke grants; no model-facing command
//! here can mutate policy. This enforces access control, NOT machine ethics.
//! The host must authenticate the person approving a grant and persist an
//! auditable approval record before shipping this in a desktop release.

use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        RwLock,
    },
    time::SystemTime,
};

use crate::privilege_guard::{enforce_model_ceiling, ProtectedEffect};

use crate::contracts::{
    AuthorizationError, ToolAuthorizer, ToolDescriptor, ToolEffect, ToolInvocation,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyError {
    InvalidGrant,
    ProtectedEffect(ProtectedEffect),
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityGrant {
    /// Exact native tool ID. Wildcards are never supported.
    pub tool_id: String,
    /// Effect must match the registered tool descriptor's declared effect.
    pub effect: ToolEffect,
    /// Exact conversation/workspace instance ID. Never a global wildcard.
    pub scope_id: String,
    /// Optional explicit expiration. No hidden duration or idle timeout.
    pub expires_at: Option<SystemTime>,
}

#[derive(Default)]
pub struct PolicyEngine {
    grants: RwLock<BTreeMap<(String, String), CapabilityGrant>>,
    revision: AtomicU64,
}

impl PolicyEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn revision(&self) -> u64 {
        self.revision.load(Ordering::SeqCst)
    }

    /// TRUSTED HOST ONLY: the UI/installer approval path must authenticate
    /// the user and log grant issuance. This API is not exposed to the model.
    pub fn grant(&self, grant: CapabilityGrant) -> Result<u64, PolicyError> {
        if grant.tool_id.trim().is_empty()
            || grant.scope_id.trim().is_empty()
            || grant.tool_id.len() > 128
            || grant.scope_id.len() > 128
        {
            return Err(PolicyError::InvalidGrant);
        }
        enforce_model_ceiling(grant.effect).map_err(PolicyError::ProtectedEffect)?;
        let mut grants = self.grants.write().map_err(|_| PolicyError::Unavailable)?;
        grants.insert((grant.tool_id.clone(), grant.scope_id.clone()), grant);
        Ok(self.revision.fetch_add(1, Ordering::SeqCst) + 1)
    }

    /// Immediate in-memory revocation. Durable policy persistence and an
    /// approval audit journal belong at the host integration boundary.
    pub fn revoke(&self, tool_id: &str, scope_id: &str) -> Result<u64, PolicyError> {
        let mut grants = self.grants.write().map_err(|_| PolicyError::Unavailable)?;
        grants.remove(&(tool_id.to_string(), scope_id.to_string()));
        Ok(self.revision.fetch_add(1, Ordering::SeqCst) + 1)
    }

    pub fn revoke_scope(&self, scope_id: &str) -> Result<u64, PolicyError> {
        let mut grants = self.grants.write().map_err(|_| PolicyError::Unavailable)?;
        grants.retain(|(_, scope), _| scope != scope_id);
        Ok(self.revision.fetch_add(1, Ordering::SeqCst) + 1)
    }
}

impl ToolAuthorizer for PolicyEngine {
    fn authorize(
        &self,
        invocation: &ToolInvocation,
        descriptor: &ToolDescriptor,
    ) -> Result<(), AuthorizationError> {
        let scope = invocation
            .workspace_id
            .as_deref()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                AuthorizationError::new("a bound conversation/workspace scope is required")
            })?;
        if invocation.tool_id != descriptor.id {
            return Err(AuthorizationError::new("tool identity mismatch"));
        }
        enforce_model_ceiling(descriptor.effect)
            .map_err(|_| AuthorizationError::new("protected effect is unavailable to AI agents"))?;
        let grants = self
            .grants
            .read()
            .map_err(|_| AuthorizationError::new("policy unavailable"))?;
        let grant = grants
            .get(&(descriptor.id.clone(), scope.to_string()))
            .ok_or_else(|| AuthorizationError::new("no explicit capability grant"))?;
        if grant.effect != descriptor.effect {
            return Err(AuthorizationError::new(
                "capability effect changed; approval revoked",
            ));
        }
        if grant
            .expires_at
            .is_some_and(|deadline| SystemTime::now() >= deadline)
        {
            return Err(AuthorizationError::new("capability grant expired"));
        }
        Ok(())
    }
}
