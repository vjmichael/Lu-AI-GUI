//! FIN-04W experimental Windows debugger *execution* boundary.
//!
//! This is a native, debug-only CI prototype. The reviewed LLDB-DAP binary
//! remains supplied by the original sealed toolchain and the original SCM
//! service holds the exact named Job. No Tauri, WebView, AI, arbitrary PID,
//! executable selection, generic command shell or release fallback exists.
//!
//! AppContainer is still used for ordinary LSP/build workers. For the fixed
//! debugger only, this helper constructs a low-integrity restricted primary
//! token so a first-party debugger can exercise the native Windows debugging
//! APIs. Low-integrity / restricted tokens are NOT equivalent to an
//! AppContainer filesystem/network boundary: production remains disabled
//! until native independent read/network/child-custody tests pass.
#![cfg(all(windows, debug_assertions, feature = "fin04w-ci-bridge"))]

use crate::{JobPlan, SupervisorError};
use std::{mem::size_of, ptr};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE},
    Security::{
        CreateRestrictedToken, CreateWellKnownSid, EqualSid, GetTokenInformation,
        IsTokenRestricted, SetTokenInformation, TokenIntegrityLevel,
        TokenIsAppContainer, TOKEN_MANDATORY_LABEL, TOKEN_DUPLICATE,
        TOKEN_QUERY, DISABLE_MAX_PRIVILEGE, LUA_TOKEN, SE_GROUP_INTEGRITY,
        WinLowLabelSid,
    },
    System::Threading::{GetCurrentProcess, OpenProcessToken},
};

fn denied() -> SupervisorError {
    SupervisorError::NoContainment
}

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

struct OwnedToken(HANDLE);
impl Drop for OwnedToken {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CloseHandle(self.0); }
        }
    }
}

fn low_sid() -> Result<(Vec<usize>, u32), SupervisorError> {
    const CAP: usize = 96;
    let mut memory = vec![0usize; CAP.div_ceil(size_of::<usize>())];
    let mut length = CAP as u32;
    if unsafe {
        CreateWellKnownSid(
            WinLowLabelSid,
            ptr::null_mut(),
            memory.as_mut_ptr().cast(),
            &mut length,
        )
    } == 0 || length == 0 || length > CAP as u32 {
        return Err(denied());
    }
    Ok((memory, length))
}

fn verify_token(token: HANDLE) -> Result<(), SupervisorError> {
    if unsafe { IsTokenRestricted(token) } == 0 {
        return Err(denied());
    }
    let mut appcontainer = 1_u32;
    let mut read = 0_u32;
    if unsafe {
        GetTokenInformation(
            token, TokenIsAppContainer, (&mut appcontainer as *mut u32).cast(),
            size_of::<u32>() as u32, &mut read,
        )
    } == 0 || read != size_of::<u32>() as u32 || appcontainer != 0 {
        return Err(denied());
    }
    let (sid, _) = low_sid()?;
    let mut size = 0u32;
    unsafe {
        GetTokenInformation(token, TokenIntegrityLevel, ptr::null_mut(), 0, &mut size);
    }
    if size < size_of::<TOKEN_MANDATORY_LABEL>() as u32 || size > 512 {
        return Err(denied());
    }
    let mut buffer = vec![0usize; (size as usize).div_ceil(size_of::<usize>())];
    let mut observed = 0u32;
    if unsafe {
        GetTokenInformation(
            token, TokenIntegrityLevel, buffer.as_mut_ptr().cast(),
            size, &mut observed,
        )
    } == 0 || observed != size {
        return Err(denied());
    }
    let label = unsafe { &*(buffer.as_ptr() as *const TOKEN_MANDATORY_LABEL) };
    if label.Label.Sid.is_null()
        || unsafe { EqualSid(label.Label.Sid, sid.as_ptr().cast_mut().cast()) } == 0
    {
        return Err(denied());
    }
    Ok(())
}

pub(crate) struct RestrictedPrimaryToken(OwnedToken);
impl RestrictedPrimaryToken {
    pub(crate) fn create() -> Result<Self, SupervisorError> {
        let mut original = ptr::null_mut();
        if unsafe {
            OpenProcessToken(
                GetCurrentProcess(), TOKEN_QUERY | TOKEN_DUPLICATE, &mut original,
            )
        } == 0 {
            return Err(denied());
        }
        let original = OwnedToken(original);
        let mut restricted = ptr::null_mut();
        if unsafe {
            CreateRestrictedToken(
                original.0, DISABLE_MAX_PRIVILEGE | LUA_TOKEN, 0,
                ptr::null_mut(), 0, ptr::null_mut(), 0, ptr::null_mut(),
                &mut restricted,
            )
        } == 0 {
            return Err(denied());
        }
        let restricted = OwnedToken(restricted);
        let (sid, sid_length) = low_sid()?;
        let mut label = TOKEN_MANDATORY_LABEL::default();
        label.Label.Sid = sid.as_ptr().cast_mut().cast();
        label.Label.Attributes = SE_GROUP_INTEGRITY;
        if unsafe {
            SetTokenInformation(
                restricted.0,
                TokenIntegrityLevel,
                (&label as *const TOKEN_MANDATORY_LABEL).cast(),
                (size_of::<TOKEN_MANDATORY_LABEL>() as u32) + sid_length,
            )
        } == 0 {
            return Err(denied());
        }
        verify_token(restricted.0)?;
        Ok(Self(restricted))
    }
    pub(crate) fn raw(&self) -> HANDLE {
        self.0.0
    }
}

pub(crate) fn attest_child(process: HANDLE) -> Result<(), SupervisorError> {
    let mut token = ptr::null_mut();
    if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) } == 0 {
        return Err(denied());
    }
    let token = OwnedToken(token);
    verify_token(token.0)
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
