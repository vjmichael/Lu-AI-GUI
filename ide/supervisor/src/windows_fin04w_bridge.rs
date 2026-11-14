//! FIN-04W debug-CI-only selection and acceptance contract.
//!
//! The earlier experimental low-integrity primary-token launch tried to put
//! AppContainer (S-1-15) package SIDs in CreateRestrictedToken restricting-SID
//! lists. Windows 10 and Windows Server 2022 both returned Win32 error 87
//! before LLDB launched. That path is RETIRED: it was not the documented
//! AppContainer process-creation contract.
//!
//! FIN-04W now diagnoses the existing, independently token-attested Windows
//! AppContainer launch using PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES,
//! zero capabilities, SCM-owned Job, exact inherited handles and a one-use
//! package-specific NUL lease. None of these checks alone proves native LLDB
//! debugger authority. No alternative token, shell, broad ACE, privilege
//! fallback or release-mode promotion is available.
#![cfg(all(windows, debug_assertions, feature = "fin04w-ci-bridge"))]

use crate::JobPlan;

pub(crate) fn eligible(plan: &JobPlan, explicitly_selected: bool) -> bool {
    explicitly_selected
        && matches!(plan.recipe_id.as_str(), "dap_lldb_fixed" | "dap_lldb_project")
        && plan.effect_class == "debug_inspection_isolated"
        && plan.toolchain_sha256.len() == 64
        && plan.toolchain_sha256.bytes().all(|b| b.is_ascii_hexdigit())
        && (plan.recipe_id != "dap_lldb_project" || plan.debug_target_binding_sha256.is_some())
}

// Never use environment variables as authorization. The original single-use
// grant, source digest, SCM lease and kernel Job gates are STILL mandatory.
// This variable selects the disposable CI comparison only.
pub(crate) fn select_ci(plan: &JobPlan) -> bool {
    eligible(
        plan,
        std::env::var_os("LU_IDE_FIN04W_CI_BRIDGE").as_deref()
            == Some(std::ffi::OsStr::new("1"))
            && std::env::var_os("GITHUB_ACTIONS").as_deref()
                == Some(std::ffi::OsStr::new("true")),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_debugger_only_no_project_or_untrusted_recipe() {
        let mut plan = JobPlan {
            project_id: "fixed".into(),
            recipe_id: "dap_lldb_fixed".into(),
            source_head: "a".repeat(40),
            input_snapshot_sha256: "b".repeat(64),
            manifest_sha256: "c".repeat(64),
            toolchain_sha256: "d".repeat(64),
            debug_target_binding_sha256: None,
            runtime_limit_ms: 1000,
            memory_limit_bytes: 16 * 1024 * 1024,
            process_limit: 2,
            log_limit_bytes: 1024,
            effect_class: "debug_inspection_isolated".into(),
        };
        assert!(!eligible(&plan, false));
        assert!(eligible(&plan, true));
        plan.recipe_id = "lsp_clangd".into();
        assert!(!eligible(&plan, true));
        plan.recipe_id = "dap_lldb_project".into();
        assert!(!eligible(&plan, true));
        plan.debug_target_binding_sha256 = Some("e".repeat(64));
        assert!(eligible(&plan, true));
        plan.toolchain_sha256 = "unverified".into();
        assert!(!eligible(&plan, true));
    }
}

