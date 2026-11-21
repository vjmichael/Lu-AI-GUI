//! IDE-CORE-IPC-01A-R4: a *native-host-only* health probe admission boundary.
//!
//! It is intentionally private to lu-ide-workspace. There is no Tauri/WebView
//! command, model/tool invocation, project source locator or fallback secret.
//! The installed native credential broker, once separately qualified, must
//! supply an OS-owned endpoint and package-scoped credential **for this one
//! call only**. No broker is installed by this source candidate; the safe
//! default is an unavailable Core health check.
//!
//! Lu Core's Core-owned dispatcher / installed enrollment is tracked under
//! https://github.com/vjmichael/Lu-AI-GUI/issues/245, NOT implemented here.
use rand::RngCore;
use std::path::Path;

#[cfg(target_os = "linux")]
use crate::core_ipc_native_transport::{probe_health_with_guard, TrustedSocket};
use crate::core_ipc_readonly::NativeEnrollment;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HostCoreHealthError {
    BrokerUnavailable,
    BrokerInvalid,
    BrokerReused,
    Unauthorized,
    TransportUnavailable,
    InvalidCoreStatus,
    CancelledOrRevoked,
}

/// Advisory, authenticated diagnostic only. A positive observation is not an
/// effect grant, trusted model lease, installed release or completed task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CoreHealthObservation {
    pub connected: bool,
    pub status_code: String,
    pub epoch: u64,
}

/// Implement ONLY in the installed native OS credential custody service.
/// Its callback receives borrowed credentials; the host never caches the key.
/// In particular, a model, WebView, user-selected project or received Hello
/// must not supply these values. No permissive/default implementation exists.
///
/// An explicit callback rather than a returned secret prevents the health
/// layer from holding provider-owned sensitive material beyond the call.
pub(crate) trait ProtectedCoreHealthBroker {
    fn with_enrollment(
        &self,
        operation_ref: &str,
        use_once: &mut dyn FnMut(&Path, u32, &[u8; 32], &[u8; 32]),
    ) -> Result<(), HostCoreHealthError>;
}

#[cfg(target_os = "linux")]
fn probe_native(
    socket: &Path,
    owner_uid: u32,
    package_sha256: &[u8; 32],
    credential: &[u8; 32],
    active: &dyn Fn() -> bool,
) -> Result<CoreHealthObservation, HostCoreHealthError> {
    if package_sha256.iter().all(|byte| *byte == 0) || credential.iter().all(|byte| *byte == 0) {
        return Err(HostCoreHealthError::Unauthorized);
    }
    let endpoint = TrustedSocket {
        socket_path: socket,
        owner_uid,
        access_group_gid: None,
    };
    let enrollment = NativeEnrollment {
        package_sha256,
        credential,
    };
    let report = probe_health_with_guard(&endpoint, &enrollment, active)
        .map_err(|_| HostCoreHealthError::TransportUnavailable)?;
    // The read-only codec verifies the original server MAC, session, exact
    // module identity, canonical protobuf and nonzero Core epoch.
    if report.module_id != "lu.core" || report.epoch == 0 {
        return Err(HostCoreHealthError::InvalidCoreStatus);
    }
    Ok(CoreHealthObservation {
        connected: report.connected,
        status_code: report.status_code,
        epoch: report.epoch,
    })
}

#[cfg(not(target_os = "linux"))]
fn probe_native(
    _socket: &Path,
    _owner_uid: u32,
    _package_sha256: &[u8; 32],
    _credential: &[u8; 32],
    _active: &dyn Fn() -> bool,
) -> Result<CoreHealthObservation, HostCoreHealthError> {
    // Windows is strictly default-DENY until the native signed enrollment,
    // named-pipe service identity and credential broker are actually wired.
    Err(HostCoreHealthError::TransportUnavailable)
}

/// An IDE-native, single-shot health observation. Enforces exactly one broker
/// callback, even if an incorrectly implemented provider returns success
/// without supplying material or attempts duplicate enrollment consumption.
/// No authority is obtained from a health report or the broker's return code.
pub(crate) fn probe_core_health_once(
    broker: &dyn ProtectedCoreHealthBroker,
) -> Result<CoreHealthObservation, HostCoreHealthError> {
    // Legacy standalone diagnostics remain a test-only helper. Production
    // callers must use CoreHealthSession, whose ID is fsynced before issue.
    let mut nonce = [0_u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let id: String = nonce.iter().map(|b| format!("{b:02x}")).collect();
    probe_core_health_guarded(broker, &id, &|| true)
}

pub(crate) fn probe_core_health_guarded(
    broker: &dyn ProtectedCoreHealthBroker,
    operation_ref: &str,
    active: &dyn Fn() -> bool,
) -> Result<CoreHealthObservation, HostCoreHealthError> {
    if !active() {
        return Err(HostCoreHealthError::CancelledOrRevoked);
    }
    let mut count = 0_u8;
    let mut observed = None;
    let broker_result = broker.with_enrollment(
        operation_ref,
        &mut |socket, uid, package_sha256, credential| {
            count = count.saturating_add(1);
            if count != 1 {
                observed = Some(Err(HostCoreHealthError::BrokerReused));
                return;
            }
            observed = Some(if active() {
                probe_native(socket, uid, package_sha256, credential, active)
            } else {
                Err(HostCoreHealthError::CancelledOrRevoked)
            });
        },
    );
    broker_result?;
    if count == 0 {
        return Err(HostCoreHealthError::BrokerUnavailable);
    }
    if count != 1 {
        return Err(HostCoreHealthError::BrokerReused);
    }
    if !active() {
        return Err(HostCoreHealthError::CancelledOrRevoked);
    }
    observed.unwrap_or(Err(HostCoreHealthError::BrokerInvalid))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct OfflineBroker;
    impl ProtectedCoreHealthBroker for OfflineBroker {
        fn with_enrollment(
            &self,
            _operation_ref: &str,
            _use_once: &mut dyn FnMut(&Path, u32, &[u8; 32], &[u8; 32]),
        ) -> Result<(), HostCoreHealthError> {
            Err(HostCoreHealthError::BrokerUnavailable)
        }
    }

    struct SilentBroker;
    impl ProtectedCoreHealthBroker for SilentBroker {
        fn with_enrollment(
            &self,
            _operation_ref: &str,
            _use_once: &mut dyn FnMut(&Path, u32, &[u8; 32], &[u8; 32]),
        ) -> Result<(), HostCoreHealthError> {
            Ok(())
        }
    }

    struct MalformedBroker {
        calls: u8,
    }
    impl ProtectedCoreHealthBroker for MalformedBroker {
        fn with_enrollment(
            &self,
            _operation_ref: &str,
            use_once: &mut dyn FnMut(&Path, u32, &[u8; 32], &[u8; 32]),
        ) -> Result<(), HostCoreHealthError> {
            let zero = [0_u8; 32];
            for _ in 0..self.calls {
                use_once(Path::new("/no/native/core/endpoint"), 0, &zero, &zero);
            }
            Ok(())
        }
    }

    #[test]
    fn no_installed_broker_cannot_claim_core_health() {
        assert_eq!(
            probe_core_health_once(&OfflineBroker),
            Err(HostCoreHealthError::BrokerUnavailable)
        );
    }

    #[test]
    fn broker_returning_success_without_credentials_is_not_accepted() {
        assert_eq!(
            probe_core_health_once(&SilentBroker),
            Err(HostCoreHealthError::BrokerUnavailable)
        );
    }

    #[test]
    fn zero_credential_and_duplicate_broker_invocation_refuse() {
        assert!(matches!(
            probe_core_health_once(&MalformedBroker { calls: 1 }),
            Err(HostCoreHealthError::Unauthorized)
        ));
        assert_eq!(
            probe_core_health_once(&MalformedBroker { calls: 2 }),
            Err(HostCoreHealthError::BrokerReused)
        );
    }
}
