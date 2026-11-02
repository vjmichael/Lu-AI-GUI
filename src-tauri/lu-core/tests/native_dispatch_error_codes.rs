//! Stable, non-sensitive Lu error-family identifiers for typed native tools.
//! These are contract tests, not JavaScript/MCP wire serialization.
use lu_core::{privilege_guard::ProtectedEffect, registry::DispatchError, ToolFailure};

#[test]
fn every_dispatch_error_variant_has_a_stable_uppercase_error_family() {
    let variants = [
        (
            DispatchError::UnknownTool("secret-tool".into()),
            "UNKNOWN_TOOL",
        ),
        (
            DispatchError::ModuleDraining("ext-1".into()),
            "MODULE_DRAINING",
        ),
        (DispatchError::InvalidCallId, "INVALID_CALL_ID"),
        (
            DispatchError::WorkspaceRequired("workspace-1".into()),
            "WORKSPACE_REQUIRED",
        ),
        (
            DispatchError::InputTypeMismatch {
                expected: "lu.protos.Read".into(),
                actual: "lu.protos.Write".into(),
            },
            "INPUT_TYPE_MISMATCH",
        ),
        (
            DispatchError::Denied("private authorization detail".into()),
            "TOOL_DENIED",
        ),
        (
            DispatchError::ProtectedEffect(ProtectedEffect::OperatingSystemElevation),
            "PROTECTED_EFFECT",
        ),
        (
            DispatchError::ProtectedEffect(ProtectedEffect::PhysicalDeviceControl),
            "PROTECTED_EFFECT",
        ),
        (
            DispatchError::Execution(ToolFailure::new(
                "PRIVATE_BACKEND_CODE",
                "private worker error",
                true,
            )),
            "TOOL_EXECUTION_FAILED",
        ),
        (DispatchError::Poisoned, "REGISTRY_UNAVAILABLE"),
    ];
    for (variant, expected) in variants {
        assert_eq!(variant.stable_code(), expected);
        assert!(variant
            .stable_code()
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte == b'_'));
        assert!(!variant.stable_code().contains("private"));
        assert!(!variant.stable_code().contains("secret"));
    }
}

#[test]
fn dynamic_diagnostic_text_never_changes_the_public_error_family() {
    let private_paths = [
        DispatchError::UnknownTool("C:\\Users\\Private\\secret.txt".into()),
        DispatchError::UnknownTool("renamed-tool".into()),
    ];
    assert_eq!(
        private_paths[0].stable_code(),
        private_paths[1].stable_code()
    );
    assert_eq!(
        DispatchError::Denied("user says no".into()).stable_code(),
        DispatchError::Denied("plugin revoked".into()).stable_code(),
    );
    assert_eq!(
        DispatchError::Execution(ToolFailure::new("E123", "first", true)).stable_code(),
        DispatchError::Execution(ToolFailure::new("E456", "second", false)).stable_code(),
    );
}
