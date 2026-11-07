//! Narrow OS-device ACL bootstrap for the *separately restricted* Lu IDE
//! recovery service. This is a privileged INSTALL/BOOT maintenance executable,
//! never a Tauri command, editor action, model tool, or arbitrary SID editor.
//!
//! Windows resets \Device\Null security after boot. An elevated installer /
//! trusted boot repair job may grant only the fixed LuIdeRecoveryBroker service
//! SID READ_CONTROL+WRITE_DAC on NUL. The service must still authenticate each
//! job and manage an exact ephemeral AppContainer SID ACE with durable recovery.
//!
//! This bootstrap alone DOES NOT authorize the production native worker or
//! install any broad ALL_APPLICATION_PACKAGES / All Restricted packages ACE.
#[cfg(not(all(windows, feature = "protected-recovery-service")))]
fn main() {
    eprintln!("DENIED: Windows protected service installation context required");
    std::process::exit(2);
}
#[cfg(all(windows, feature = "protected-recovery-service"))]
mod native {
    use lu_ide_execution_supervisor::windows_service_security::{sid_of_service, ws};
    use std::{ffi::c_void, mem::size_of, ptr};
    use windows_sys::Win32::{
        Foundation::{CloseHandle, LocalFree, HANDLE, INVALID_HANDLE_VALUE},
        Security::{
            AclSizeInformation,
            Authorization::{
                ConvertStringSidToSidW, GetSecurityInfo, SetEntriesInAclW, SetSecurityInfo,
                EXPLICIT_ACCESS_W, GRANT_ACCESS, REVOKE_ACCESS, SE_KERNEL_OBJECT, TRUSTEE_IS_SID,
                TRUSTEE_IS_UNKNOWN, TRUSTEE_W,
            },
            CheckTokenMembership, EqualSid, GetAce, GetAclInformation, IsValidSid,
            ACCESS_ALLOWED_ACE, ACL, ACL_SIZE_INFORMATION, DACL_SECURITY_INFORMATION, PSID,
        },
        Storage::FileSystem::{CreateFileW, OPEN_EXISTING},
    };
    const READ_CONTROL: u32 = 0x0002_0000;
    const WRITE_DAC: u32 = 0x0004_0000;
    const SERVICE_PERMISSIONS: u32 = READ_CONTROL | WRITE_DAC;
    const FILE_SHARE_READ: u32 = 1;
    const FILE_SHARE_WRITE: u32 = 2;
    const DEVICE: &str = r"\\.\NUL";

    fn denied() -> String {
        "DENIED: service-only NUL security provisioning".into()
    }
    struct Sid(PSID);
    impl Drop for Sid {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    LocalFree(self.0);
                }
            }
        }
    }
    struct Handle(HANDLE);
    impl Drop for Handle {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
    struct Local(*mut c_void);
    impl Drop for Local {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    LocalFree(self.0);
                }
            }
        }
    }
    fn parse_sid(value: &str) -> Result<Sid, String> {
        let mut sid = ptr::null_mut();
        if unsafe { ConvertStringSidToSidW(ws(value).as_ptr(), &mut sid) } == 0
            || sid.is_null()
            || unsafe { IsValidSid(sid) } == 0
        {
            return Err(denied());
        }
        Ok(Sid(sid))
    }
    fn must_be_elevated_admin() -> Result<(), String> {
        let admin = parse_sid("S-1-5-32-544")?;
        let mut enabled = 0i32;
        if unsafe { CheckTokenMembership(ptr::null_mut(), admin.0, &mut enabled) } == 0
            || enabled == 0
        {
            return Err(denied());
        }
        Ok(())
    }
    fn open_device(writable: bool) -> Result<Handle, String> {
        let raw = unsafe {
            CreateFileW(
                ws(DEVICE).as_ptr(),
                READ_CONTROL | if writable { WRITE_DAC } else { 0 },
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                ptr::null(),
                OPEN_EXISTING,
                0,
                ptr::null_mut(),
            )
        };
        if raw.is_null() || raw == INVALID_HANDLE_VALUE {
            Err(denied())
        } else {
            Ok(Handle(raw))
        }
    }
    fn with_acl<T>(
        handle: HANDLE,
        callback: impl FnOnce(*mut ACL) -> Result<T, String>,
    ) -> Result<T, String> {
        let mut acl = ptr::null_mut();
        let mut descriptor = ptr::null_mut();
        if unsafe {
            GetSecurityInfo(
                handle,
                SE_KERNEL_OBJECT as i32,
                DACL_SECURITY_INFORMATION,
                ptr::null_mut(),
                ptr::null_mut(),
                &mut acl,
                ptr::null_mut(),
                &mut descriptor,
            )
        } != 0
            || descriptor.is_null()
            || acl.is_null()
        {
            return Err(denied());
        }
        let _owner = Local(descriptor);
        callback(acl)
    }
    fn find_exact_service_ace(acl: *mut ACL, service: PSID) -> Result<Option<u32>, String> {
        let mut info = ACL_SIZE_INFORMATION::default();
        if unsafe {
            GetAclInformation(
                acl,
                (&mut info as *mut ACL_SIZE_INFORMATION).cast(),
                size_of::<ACL_SIZE_INFORMATION>() as u32,
                AclSizeInformation,
            )
        } == 0
        {
            return Err(denied());
        }
        let mut matched = None;
        for index in 0..info.AceCount {
            let mut ptr_ace = ptr::null_mut();
            if unsafe { GetAce(acl, index, &mut ptr_ace) } == 0 || ptr_ace.is_null() {
                return Err(denied());
            }
            let ace = unsafe { &*(ptr_ace as *const ACCESS_ALLOWED_ACE) };
            if ace.Header.AceSize < size_of::<ACCESS_ALLOWED_ACE>() as u16 {
                continue;
            }
            // A deny-ACE collision is treated as a non-adoptable policy; a
            // broad/unknown ACE type is never reinterpreted as grantable.
            if ace.Header.AceType != 0 && ace.Header.AceType != 1 {
                continue;
            }
            let sid = (&ace.SidStart as *const u32).cast_mut().cast::<c_void>();
            if unsafe { EqualSid(sid, service) } != 0 {
                if matched.is_some() || ace.Header.AceType != 0 {
                    return Err(denied());
                }
                matched = Some(ace.Mask);
            }
        }
        Ok(matched)
    }
    fn change(handle: HANDLE, sid: PSID, mode: i32) -> Result<(), String> {
        with_acl(handle, |acl| {
            let mut entry = EXPLICIT_ACCESS_W {
                grfAccessPermissions: SERVICE_PERMISSIONS,
                grfAccessMode: mode,
                grfInheritance: 0,
                Trustee: TRUSTEE_W {
                    pMultipleTrustee: ptr::null_mut(),
                    MultipleTrusteeOperation: 0,
                    TrusteeForm: TRUSTEE_IS_SID,
                    TrusteeType: TRUSTEE_IS_UNKNOWN,
                    ptstrName: sid.cast(),
                },
            };
            let mut next = ptr::null_mut();
            if unsafe { SetEntriesInAclW(1, &mut entry, acl, &mut next) } != 0 || next.is_null() {
                return Err(denied());
            }
            let _new_acl = Local(next.cast());
            if unsafe {
                SetSecurityInfo(
                    handle,
                    SE_KERNEL_OBJECT as i32,
                    DACL_SECURITY_INFORMATION,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    next,
                    ptr::null_mut(),
                )
            } != 0
            {
                return Err(denied());
            }
            Ok(())
        })
    }
    pub fn run() -> Result<(), String> {
        // No project-selected device path, operation, SID, executable or
        // caller-provided policy is ever accepted by this privileged tool.
        let args: Vec<String> = std::env::args().skip(1).collect();
        if args.len() != 1
            || !matches!(
                args[0].as_str(),
                "--grant-fixed-service" | "--verify-fixed-service" | "--revoke-fixed-service"
            )
        {
            return Err(denied());
        }
        let writable = args[0] != "--verify-fixed-service";
        if writable {
            must_be_elevated_admin()?;
        }
        let sid_text = sid_of_service().map_err(|_| denied())?;
        if !sid_text.starts_with("S-1-5-80-") {
            return Err(denied());
        }
        let sid = parse_sid(&sid_text)?;
        let device = open_device(writable)?;
        let was = with_acl(device.0, |acl| find_exact_service_ace(acl, sid.0))?;
        match args[0].as_str() {
            "--grant-fixed-service" => {
                if was.is_none() {
                    change(device.0, sid.0, GRANT_ACCESS)?;
                }
            }
            "--revoke-fixed-service" => {
                if was.is_some() {
                    if was != Some(SERVICE_PERMISSIONS) {
                        return Err(denied());
                    }
                    change(device.0, sid.0, REVOKE_ACCESS)?;
                }
            }
            "--verify-fixed-service" => {}
            _ => return Err(denied()),
        }
        let observed = with_acl(device.0, |acl| find_exact_service_ace(acl, sid.0))?;
        let expected = if args[0] == "--revoke-fixed-service" {
            None
        } else {
            Some(SERVICE_PERMISSIONS)
        };
        if observed != expected {
            return Err(denied());
        }
        println!("PASS: exact Lu IDE service NUL DACL authority verified (no broad package ACE)");
        Ok(())
    }
}
#[cfg(all(windows, feature = "protected-recovery-service"))]
fn main() {
    if let Err(reason) = native::run() {
        eprintln!("{reason}");
        std::process::exit(1);
    }
}
