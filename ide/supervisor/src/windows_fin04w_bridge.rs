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
use std::{ffi::OsStr, mem::size_of, path::Path, ptr};
use windows_sys::Win32::{
    Foundation::{CloseHandle, GetLastError, HANDLE, INVALID_HANDLE_VALUE},
    Security::{
        CreateRestrictedToken, CreateWellKnownSid, DuplicateTokenEx, EqualSid,
        GetTokenInformation, ImpersonateLoggedOnUser, RevertToSelf, SecurityImpersonation,
        TokenImpersonation,
        IsTokenRestricted, IsValidSid, SetTokenInformation, TokenIntegrityLevel,
        TokenIsAppContainer, TokenRestrictedSids, SID_AND_ATTRIBUTES, TOKEN_GROUPS,
        TOKEN_MANDATORY_LABEL, TOKEN_DUPLICATE, TOKEN_ASSIGN_PRIMARY,
        TOKEN_ADJUST_DEFAULT, TOKEN_IMPERSONATE,
        TOKEN_QUERY, DISABLE_MAX_PRIVILEGE, LUA_TOKEN,
        WinLowLabelSid, WinBuiltinAnyPackageSid,
    },
    Storage::FileSystem::{CreateFileW, OPEN_EXISTING},
    System::Threading::{GetCurrentProcess, OpenProcessToken},
};

// Windows SDK WinNT.h SE_GROUP_INTEGRITY (not exported in windows-sys 0.61).
// A mandatory integrity SID must carry this attribute, not ENABLED.
const SE_GROUP_INTEGRITY_VALUE: u32 = 0x0000_0020;
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

fn verify_token(token: HANDLE, package_sid: windows_sys::Win32::Security::PSID) -> Result<(), SupervisorError> {
    if package_sid.is_null() || unsafe { IsValidSid(package_sid) } == 0
        || unsafe { IsTokenRestricted(token) } == 0
    {
        return Err(denied());
    }
    let mut group_len=0u32;
    unsafe { GetTokenInformation(token, TokenRestrictedSids, ptr::null_mut(), 0, &mut group_len); }
    if group_len < size_of::<TOKEN_GROUPS>() as u32 || group_len > 4096 {
        return Err(denied());
    }
    let mut group_data=vec![0usize; (group_len as usize).div_ceil(size_of::<usize>())];
    let mut observed_groups=0u32;
    if unsafe {
        GetTokenInformation(token, TokenRestrictedSids, group_data.as_mut_ptr().cast(),
            group_len, &mut observed_groups)
    } == 0 || observed_groups != group_len {
        return Err(denied());
    }
    let groups=unsafe { &*(group_data.as_ptr() as *const TOKEN_GROUPS) };
    if groups.GroupCount != 2 {
        return Err(denied());
    }
    let entries=unsafe { std::slice::from_raw_parts(groups.Groups.as_ptr(), 2) };
    let expected_global = any_package_sid()?;
    let global_sid = expected_global.as_ptr().cast_mut().cast();
    let found_package=entries.iter().any(|g| !g.Sid.is_null()
        && unsafe { EqualSid(g.Sid,package_sid) }!=0);
    let found_global=entries.iter().any(|g| !g.Sid.is_null()
        && unsafe { EqualSid(g.Sid,global_sid) }!=0);
    if !found_package || !found_global
        || unsafe { EqualSid(package_sid,global_sid) } != 0 {
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

fn any_package_sid() -> Result<Vec<usize>, SupervisorError> {
    let mut any_package_sid=vec![0usize; 96usize.div_ceil(size_of::<usize>())];
    let mut any_package_size=96u32;
    if unsafe {
        CreateWellKnownSid(WinBuiltinAnyPackageSid, ptr::null_mut(),
            any_package_sid.as_mut_ptr().cast(), &mut any_package_size)
    } == 0 || any_package_size == 0 || any_package_size > 96 {
        return Err(denied());
    }
    Ok(any_package_sid)
}
pub(crate) struct RestrictedPrimaryToken(OwnedToken);
impl RestrictedPrimaryToken {
    pub(crate) fn create(package_sid: windows_sys::Win32::Security::PSID) -> Result<Self, SupervisorError> {
        if package_sid.is_null() || unsafe { IsValidSid(package_sid) } == 0 {
            return Err(denied());
        }
        let mut original = ptr::null_mut();
        if unsafe {
            OpenProcessToken(
                GetCurrentProcess(),
                TOKEN_QUERY | TOKEN_DUPLICATE | TOKEN_ASSIGN_PRIMARY
                    | TOKEN_ADJUST_DEFAULT | TOKEN_IMPERSONATE,
                &mut original,
            )
        } == 0 {
            return Err(denied());
        }
        let original = OwnedToken(original);
        // Actual original per-job AppContainer SID controls access to the
        // exact package LocalState files. Global All Application Packages
        // controls access to Windows system runtime files. In contrast,
        // BUILTIN Users and the parent user's SID would let a restricted
        // process read unrelated same-user projects and must NOT be present
        // in the restricted allowlist.
        let mut any_package_sid=any_package_sid()?;
        let restricted_sids = [
            SID_AND_ATTRIBUTES { Sid: package_sid, Attributes: 0 },
            SID_AND_ATTRIBUTES { Sid: any_package_sid.as_mut_ptr().cast(), Attributes: 0 },
        ];
        let mut restricted = ptr::null_mut();
        if unsafe {
            CreateRestrictedToken(
                original.0, DISABLE_MAX_PRIVILEGE | LUA_TOKEN, 0,
                ptr::null_mut(), 0, ptr::null_mut(),
                restricted_sids.len() as u32,
                restricted_sids.as_ptr(),
                &mut restricted,
            )
        } == 0 {
            return Err(denied());
        }
        let restricted = OwnedToken(restricted);
        let (sid, sid_length) = low_sid()?;
        let mut label = TOKEN_MANDATORY_LABEL::default();
        label.Label.Sid = sid.as_ptr().cast_mut().cast();
        label.Label.Attributes = SE_GROUP_INTEGRITY_VALUE;
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
        verify_token(restricted.0, package_sid)?;
        Ok(Self(restricted))
    }
    pub(crate) fn raw(&self) -> HANDLE {
        self.0.0
    }
}


// FIN-04W OS-only diagnostic: the actual *suspended child* token is
// impersonated briefly to ask Windows about fixed loader/file operations.
// This is NOT evidence that the image loader's internal call succeeded and
// may never lead to a broad ACL grant or permission fallback. Only static
// categories are emitted; no source contents, absolute paths or token IDs.
pub(crate) fn ci_probe_child_loader_access(
    child_process: HANDLE,
    package_sid: windows_sys::Win32::Security::PSID,
    executable: &Path,
    working_dir: &Path,
) -> Result<(), SupervisorError> {
    let mut original = ptr::null_mut();
    if unsafe {
        OpenProcessToken(child_process, TOKEN_QUERY | TOKEN_DUPLICATE, &mut original)
    } == 0 {
        eprintln!("FIN04W_LOADER_PROBE=child_token_unavailable");
        return Err(denied());
    }
    let original = OwnedToken(original);
    verify_token(original.0, package_sid)?;
    let mut raw = ptr::null_mut();
    if unsafe {
        DuplicateTokenEx(
            original.0, TOKEN_QUERY | TOKEN_IMPERSONATE, ptr::null(),
            SecurityImpersonation, TokenImpersonation, &mut raw,
        )
    } == 0 {
        eprintln!("FIN04W_LOADER_PROBE=impersonation_token_unavailable");
        return Err(denied());
    }
    let impersonation = OwnedToken(raw);
    // The disposable CI owner creates one tiny unrelated same-user file.
    // Success reading it under the child's restricted token would expose
    // ambient user documents and MUST block any promotion.
    let outside_root = std::env::var_os("RUNNER_TEMP").ok_or_else(denied)?;
    let outside = Path::new(&outside_root).join(format!(
        "lu-fin04w-external-{:032x}.tmp", rand::random::<u128>()
    ));
    std::fs::write(&outside, b"x").map_err(|_| denied())?;
    if unsafe { ImpersonateLoggedOnUser(impersonation.0) } == 0 {
        let _ = std::fs::remove_file(&outside);
        eprintln!("FIN04W_LOADER_PROBE=impersonation_denied");
        return Err(denied());
    }
    // Do not expose arbitrary user paths. These three locations are
    // selected solely by the already sealed host worker/OS.
    let system_dll = std::env::var_os("SystemRoot")
        .map(|root| Path::new(&root).join("System32/kernel32.dll"));
    let candidates: [(&str, Option<&Path>, u32, u32); 3] = [
        ("sealed_executable", Some(executable), 0xa000_0000, 0),
        // Opening a directory as a HANDLE requires BACKUP_SEMANTICS.
        ("sealed_workdir", Some(working_dir), 0x8000_0000, 0x0200_0000),
        ("system_dll", system_dll.as_deref(), 0xa000_0000, 0),
    ];
    for (label, candidate, desired_access, flags) in candidates {
        let result = if let Some(candidate) = candidate {
            let raw = unsafe {
                CreateFileW(
                    super::wide(candidate.as_os_str()).as_ptr(),
                    desired_access, 0x0000_0007, ptr::null(),
                    OPEN_EXISTING, flags, ptr::null_mut(),
                )
            };
            if raw == INVALID_HANDLE_VALUE || raw.is_null() {
                match unsafe { GetLastError() } {
                    5 => "access_denied",
                    2 | 3 => "missing_path",
                    32 => "sharing_violation",
                    _ => "other_kernel_failure",
                }
            } else {
                unsafe { CloseHandle(raw); }
                "read_execute_allowed"
            }
        } else {
            "source_unavailable"
        };
        eprintln!("FIN04W_LOADER_{label}={result}");
    }
    let outside_read = unsafe {
        CreateFileW(
            super::wide(outside.as_os_str()).as_ptr(),
            0x8000_0000, 0x0000_0007, ptr::null(),
            OPEN_EXISTING, 0, ptr::null_mut(),
        )
    };
    let outside_result = if outside_read == INVALID_HANDLE_VALUE || outside_read.is_null() {
        match unsafe { GetLastError() } {
            5 => "access_denied",
            _ => "other_kernel_failure",
        }
    } else {
        unsafe { CloseHandle(outside_read); }
        "unexpected_read_allowed"
    };
    // Reversion is mandatory before any privileged Job/SCM action.
    if unsafe { RevertToSelf() } == 0 {
        return Err(SupervisorError::Quarantined);
    }
    if std::fs::remove_file(&outside).is_err() {
        return Err(SupervisorError::Quarantined);
    }
    eprintln!("FIN04W_LOADER_OUTSIDE_PRIVATE={outside_result}");
    if outside_result != "access_denied" {
        return Err(SupervisorError::NoContainment);
    }
    eprintln!("FIN04W_LOADER_PROBE=completed_reverted_no_authority");
    Ok(())
}

pub(crate) fn attest_child(process: HANDLE, package_sid: windows_sys::Win32::Security::PSID) -> Result<(), SupervisorError> {
    let mut token = ptr::null_mut();
    if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) } == 0 {
        return Err(denied());
    }
    let token = OwnedToken(token);
    verify_token(token.0, package_sid)
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
