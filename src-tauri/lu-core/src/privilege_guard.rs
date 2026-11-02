//! Model privilege ceiling ("Mythos Containment Guard").
//!
//! A model, prompt, tool package, or workspace cannot become an administrator
//! by obtaining a native tool descriptor, impersonating an approval, or
//! presenting a permissive authorizer. Lu's general-purpose tool dispatcher
//! has a fixed ceiling: no elevation and no physical device control.
//!
//! This is a capability boundary, not a model-name/persona denylist. A future
//! authenticated, separately isolated device service must have its OWN host
//! policy and physical failsafes; it must not bypass this dispatcher ceiling.
//!
//! IMPORTANT: effects are DECLARED metadata. This guard cannot guarantee that
//! arbitrary shell code or a malicious native implementation does not elevate
//! privileges. The host still needs non-admin OS identity, process sandboxing,
//! audited tool implementations, and a trusted package supply chain.

use crate::contracts::ToolEffect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtectedEffect {
    OperatingSystemElevation,
    PhysicalDeviceControl,
}

/// Called by both the native dispatcher and host policy grant issuance.
/// Do not add a model-facing override or allow-list exception here.
pub fn enforce_model_ceiling(effect: ToolEffect) -> Result<(), ProtectedEffect> {
    match effect {
        ToolEffect::Elevated => Err(ProtectedEffect::OperatingSystemElevation),
        ToolEffect::DeviceControl => Err(ProtectedEffect::PhysicalDeviceControl),
        ToolEffect::ReadWorkspace
        | ToolEffect::WriteWorkspace
        | ToolEffect::ExecuteCode
        | ToolEffect::NetworkRead
        | ToolEffect::NetworkWrite => Ok(()),
    }
}
