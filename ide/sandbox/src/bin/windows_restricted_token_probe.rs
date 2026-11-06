//! Wave D4-W1: fixed Windows restricted-token creation probe.
//! No child process, project script, desktop integration or authorization grant.
#[cfg(not(windows))]
fn main() {
    eprintln!("Windows restricted token probe is Windows-only; no fallback");
    std::process::exit(2);
}

#[cfg(windows)]
mod probe {
    use std::{
        ffi::OsStr,
        io,
        os::windows::ffi::OsStrExt,
        ptr,
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, LocalFree, HANDLE},
        Security::{
            Authorization::ConvertStringSidToSidW,
            CreateRestrictedToken, IsTokenRestricted, SID_AND_ATTRIBUTES,
            DISABLE_MAX_PRIVILEGE, PSID, TOKEN_ASSIGN_PRIMARY,
            TOKEN_DUPLICATE, TOKEN_QUERY,
        },
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    };

    struct OwnedToken(HANDLE);
    impl Drop for OwnedToken {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: token was returned by a successful Win32 call.
                unsafe { CloseHandle(self.0); }
            }
        }
    }

    struct OwnedSid(PSID);
    impl Drop for OwnedSid {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: SID memory was allocated by ConvertStringSidToSidW.
                unsafe { LocalFree(self.0.cast()); }
            }
        }
    }

    fn fail(action: &'static str) -> io::Error {
        io::Error::new(io::Error::last_os_error().kind(), format!(
            "{action}: {}", io::Error::last_os_error()
        ))
    }

    fn wide(value: &str) -> Vec<u16> {
        OsStr::new(value).encode_wide().chain(std::iter::once(0)).collect()
    }

    pub fn run() -> io::Result<()> {
        // A fixed "Restricted Code" SID: S-1-5-12. The resulting token's
        // DACL access check must satisfy original and restricting SID sets.
        // This demonstrates restriction primitive only; no execution.
        let mut restricted_sid: PSID = ptr::null_mut();
        let code = wide("S-1-5-12");
        // SAFETY: valid null-terminated SID and a live output pointer.
        if unsafe { ConvertStringSidToSidW(code.as_ptr(), &mut restricted_sid) } == 0 {
            return Err(fail("ConvertStringSidToSidW"));
        }
        let sid = OwnedSid(restricted_sid);
        let restrict = SID_AND_ATTRIBUTES {
            Sid: sid.0,
            Attributes: 0,
        };

        let mut raw_source: HANDLE = ptr::null_mut();
        // SAFETY: process pseudo-handle is current trusted probe process,
        // and output is initialized by OpenProcessToken.
        if unsafe { OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_QUERY | TOKEN_DUPLICATE | TOKEN_ASSIGN_PRIMARY,
            &mut raw_source,
        ) } == 0 {
            return Err(fail("OpenProcessToken"));
        }
        let source = OwnedToken(raw_source);

        let mut restricted: HANDLE = ptr::null_mut();
        // SAFETY: source token is live. SID_AND_ATTRIBUTES points to the
        // fixed SID. No pointer comes from external caller or project files.
        let created = unsafe {
            CreateRestrictedToken(
                source.0, DISABLE_MAX_PRIVILEGE,
                0, ptr::null(), 0, ptr::null(),
                1, &restrict, &mut restricted,
            )
        };
        if created == 0 {
            return Err(fail("CreateRestrictedToken"));
        }
        let reduced = OwnedToken(restricted);
        // IsTokenRestricted only asserts a restricting SID is present.
        // It does not prove low integrity, AppContainer or filesystem ACLs.
        if unsafe { IsTokenRestricted(reduced.0) } == 0 {
            return Err(io::Error::other("OS did not mark token as SID-restricted"));
        }
        println!("PASS: native restricted primary token with restrictive SID and privileges removed");
        println!("UNVERIFIED: low integrity, restricted child process startup, filesystem, network, desktop isolation");
        Ok(())
    }
}
#[cfg(windows)]
fn main() {
    if let Err(error) = probe::run() {
        eprintln!("DENIED: restricted-token admission probe: {error}");
        std::process::exit(1);
    }
}
