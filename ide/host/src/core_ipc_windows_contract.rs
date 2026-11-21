//! IDE-CORE-IPC-01B-R4: reviewed Lu Core *Windows endpoint syntax* gate.
//!
//! Core source: workspace_ipc_transport.rs, SHA
//! 133198027db1834a0f0904bab2f1decb9d0f9456, at Core PR #244 head
//! bfbffc35320c78e74d52b3918cd587356fffb3a3.
//! This parser grants NO process, signer, issuer, package or named-pipe access.
//! Core's private SCM control pipe is deliberately NOT an IDE endpoint.
//! A real Windows pipe client requires independent SYSTEM installer enrollment,
//! process-generation/signer attestation and Core issue #245's Health service.

const PREFIX: &str = r"\\.\pipe\lu-ipc-";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WindowsCorePipeError {
    NotApprovedCoreEndpoint,
}

/// Validate an issuer-provided *candidate name*.  This is a negative-only
/// syntactic gate, NEVER a positive OS attestation or entitlement.
pub(crate) fn check_windows_core_pipe_name(name: &str) -> Result<(), WindowsCorePipeError> {
    let suffix = name
        .strip_prefix(PREFIX)
        .ok_or(WindowsCorePipeError::NotApprovedCoreEndpoint)?;
    if suffix.len() != 32
        || !suffix
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(WindowsCorePipeError::NotApprovedCoreEndpoint);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_exact_lu_core_public_pipe_syntax_not_scm_control() {
        assert!(
            check_windows_core_pipe_name(r"\\.\pipe\lu-ipc-0123456789abcdef0123456789abcdef")
                .is_ok()
        );
        for name in [
            r"\\.\pipe\LuCoreIpcScmControlV1",
            r"\\.\pipe\lu-ipc-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaA",
            r"\\.\pipe\lu-ipc-short",
            r"\\.\pipe\lu-ipc-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/",
            r"\\.\pipe\lu-ipc-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\..",
            r"\\other\pipe\lu-ipc-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "",
        ] {
            assert_eq!(
                check_windows_core_pipe_name(name),
                Err(WindowsCorePipeError::NotApprovedCoreEndpoint),
                "{name}"
            );
        }
    }
}
