//! CP-12B: approved single-use local diagnostic snapshot, no live archive or tool access.
use prost::Message;
use serde::Serialize;
const VERSION: u32 = 1;
const DX_VERSION: u32 = 2;
#[derive(Clone, PartialEq, Message, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Summary {
    #[prost(uint32, tag = "1")]
    pub complete: u32,
    #[prost(uint32, tag = "2")]
    pub observed_failure: u32,
    #[prost(uint32, tag = "3")]
    pub bounded: u32,
    #[prost(uint32, tag = "4")]
    pub incomplete: u32,
    #[prost(uint32, tag = "5")]
    pub unavailable: u32,
}
impl Summary {
    fn valid(&self) -> bool {
        [
            self.complete,
            self.observed_failure,
            self.bounded,
            self.incomplete,
            self.unavailable,
        ]
        .into_iter()
        .map(u64::from)
        .sum::<u64>()
            <= 64
    }
}
#[derive(Clone, PartialEq, Message)]
struct Request {
    #[prost(uint32, tag = "1")]
    version: u32,
    #[prost(uint32, tag = "2")]
    dx_version: u32,
    #[prost(bytes = "vec", tag = "3")]
    nonce: Vec<u8>,
    #[prost(bytes = "vec", tag = "4")]
    mac: Vec<u8>,
}
#[derive(Clone, PartialEq, Message)]
struct Response {
    #[prost(uint32, tag = "1")]
    version: u32,
    #[prost(uint32, tag = "2")]
    dx_version: u32,
    #[prost(bytes = "vec", tag = "3")]
    nonce: Vec<u8>,
    #[prost(message, optional, tag = "4")]
    summary: Option<Summary>,
    #[prost(bytes = "vec", tag = "5")]
    mac: Vec<u8>,
}
#[cfg(any(
    target_os = "linux",
    all(target_os = "windows", feature = "windows_dx_eval")
))]
mod protocol {
    use super::*;
    use rand::{rngs::OsRng, RngCore};
    use sha2::{Digest, Sha256};
    use std::{io, time::Duration};
    use subtle::ConstantTimeEq;
    pub(super) const TTL: Duration = Duration::from_secs(120);
    pub(super) const FRAME: Duration = Duration::from_secs(3);
    pub(super) const TICK: Duration = Duration::from_millis(20);
    pub(super) const MAX: usize = 4096;
    pub(super) fn random<const N: usize>() -> io::Result<[u8; N]> {
        let mut b = [0; N];
        OsRng
            .try_fill_bytes(&mut b)
            .map_err(|_| io::Error::other("random unavailable"))?;
        Ok(b)
    }
    pub(super) fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }
    pub(super) fn unhex<const N: usize>(s: &str) -> Option<[u8; N]> {
        if s.len() != N * 2
            || !s
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        {
            return None;
        }
        let mut b = [0; N];
        for (i, v) in b.iter_mut().enumerate() {
            *v = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
        }
        Some(b)
    }
    // RFC 2104 HMAC over existing SHA-256 with a fixed 256-bit key.
    pub(super) fn mac(key: &[u8; 32], domain: &[u8], body: &[u8]) -> [u8; 32] {
        let mut i = [0x36; 64];
        let mut o = [0x5c; 64];
        for (n, k) in key.iter().enumerate() {
            i[n] ^= k;
            o[n] ^= k;
        }
        let d = Sha256::new()
            .chain_update(i)
            .chain_update(domain)
            .chain_update(body)
            .finalize();
        Sha256::new()
            .chain_update(o)
            .chain_update(d)
            .finalize()
            .into()
    }
    pub(super) fn request(key: &[u8; 32], nonce: Vec<u8>) -> Request {
        let mut r = Request {
            version: VERSION,
            dx_version: DX_VERSION,
            nonce,
            mac: vec![],
        };
        r.mac = mac(key, b"lu.dx.request.v1\0", &r.encode_to_vec()).to_vec();
        r
    }
    pub(super) fn response(key: &[u8; 32], nonce: Vec<u8>, summary: Summary) -> Response {
        let mut r = Response {
            version: VERSION,
            dx_version: DX_VERSION,
            nonce,
            summary: Some(summary),
            mac: vec![],
        };
        r.mac = mac(key, b"lu.dx.response.v1\0", &r.encode_to_vec()).to_vec();
        r
    }
    pub(super) fn canonical<M: Message + Default>(b: &[u8]) -> Option<M> {
        let m = M::decode(b).ok()?;
        (m.encode_to_vec() == b).then_some(m)
    }
    pub(super) fn auth_request(key: &[u8; 32], b: &[u8]) -> Option<Request> {
        let r: Request = canonical(b)?;
        if r.version != VERSION
            || r.dx_version != DX_VERSION
            || r.nonce.len() != 32
            || r.mac.len() != 32
        {
            return None;
        }
        let e = request(key, r.nonce.clone());
        bool::from(r.mac.ct_eq(&e.mac)).then_some(r)
    }
    pub(super) fn auth_response(key: &[u8; 32], nonce: &[u8], b: &[u8]) -> Option<Summary> {
        let r: Response = canonical(b)?;
        if r.version != VERSION
            || r.dx_version != DX_VERSION
            || r.nonce != nonce
            || r.mac.len() != 32
        {
            return None;
        }
        let s = r.summary?;
        if !s.valid() {
            return None;
        }
        let e = response(key, nonce.to_vec(), s.clone());
        bool::from(r.mac.ct_eq(&e.mac)).then_some(s)
    }
}
#[cfg(target_os = "linux")]
mod linux {
    use super::protocol::*;
    use super::*;
    use std::{
        fs,
        io::{self, Read, Write},
        os::{
            fd::AsRawFd,
            unix::{
                fs::{DirBuilderExt, FileTypeExt, MetadataExt, PermissionsExt},
                net::{UnixListener, UnixStream},
            },
        },
        path::{Path, PathBuf},
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc, Mutex,
        },
        thread::{self, JoinHandle},
        time::{Duration, Instant},
    };
    fn uid() -> u32 {
        unsafe { libc::geteuid() }
    }
    fn peer(s: &UnixStream) -> bool {
        let mut c: libc::ucred = unsafe { std::mem::zeroed() };
        let mut n = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        let rc = unsafe {
            libc::getsockopt(
                s.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                (&mut c as *mut libc::ucred).cast(),
                &mut n,
            )
        };
        rc == 0 && n as usize == std::mem::size_of::<libc::ucred>() && c.uid == uid()
    }
    fn tmp_ok() -> bool {
        fs::symlink_metadata("/tmp")
            .is_ok_and(|m| m.is_dir() && m.uid() == 0 && m.mode() & 0o1000 != 0)
    }
    fn private_dir(p: &Path) -> bool {
        fs::symlink_metadata(p)
            .is_ok_and(|m| m.is_dir() && m.uid() == uid() && m.mode() & 0o7777 == 0o700)
    }
    fn private_socket(p: &Path) -> bool {
        fs::symlink_metadata(p).is_ok_and(|m| {
            m.file_type().is_socket() && m.uid() == uid() && m.mode() & 0o7777 == 0o600
        })
    }
    fn identity(p: &Path) -> io::Result<(u64, u64)> {
        let m = fs::symlink_metadata(p)?;
        Ok((m.dev(), m.ino()))
    }
    struct Endpoint {
        dir: PathBuf,
        dir_id: (u64, u64),
        socket_id: Option<(u64, u64)>,
    }
    impl Drop for Endpoint {
        fn drop(&mut self) {
            if identity(&self.dir).ok() != Some(self.dir_id) {
                return;
            }
            let p = self.dir.join("s");
            if self.socket_id.is_some() && identity(&p).ok() == self.socket_id {
                let _ = fs::remove_file(p);
            }
            let _ = fs::remove_dir(&self.dir);
        }
    }
    fn path(id: &[u8; 16]) -> PathBuf {
        PathBuf::from(format!("/tmp/lu-dx-{}", hex(id)))
    }
    fn expired(stop: &AtomicBool, end: Instant) -> io::Result<()> {
        if stop.load(Ordering::Acquire) || Instant::now() >= end {
            Err(io::Error::new(io::ErrorKind::TimedOut, "exchange closed"))
        } else {
            Ok(())
        }
    }
    fn transfer(
        s: &mut UnixStream,
        b: &mut [u8],
        write: bool,
        stop: &AtomicBool,
        end: Instant,
    ) -> io::Result<()> {
        let mut pos = 0;
        while pos < b.len() {
            expired(stop, end)?;
            let r = if write {
                s.write(&b[pos..])
            } else {
                s.read(&mut b[pos..])
            };
            match r {
                Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
                Ok(n) => pos += n,
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock
                            | io::ErrorKind::TimedOut
                            | io::ErrorKind::Interrupted
                    ) =>
                {
                    thread::sleep(TICK)
                }
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
    fn read(s: &mut UnixStream, stop: &AtomicBool, end: Instant) -> io::Result<Vec<u8>> {
        let mut h = [0; 4];
        transfer(s, &mut h, false, stop, end)?;
        let n = u32::from_be_bytes(h) as usize;
        if n == 0 || n > MAX {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let mut b = vec![0; n];
        transfer(s, &mut b, false, stop, end)?;
        Ok(b)
    }
    fn write(
        s: &mut UnixStream,
        mut b: Vec<u8>,
        stop: &AtomicBool,
        end: Instant,
    ) -> io::Result<()> {
        if b.is_empty() || b.len() > MAX {
            return Err(io::ErrorKind::InvalidData.into());
        }
        transfer(s, &mut (b.len() as u32).to_be_bytes(), true, stop, end)?;
        transfer(s, &mut b, true, stop, end)
    }
    struct Session {
        stop: Arc<AtomicBool>,
        worker: JoinHandle<()>,
    }
    impl Session {
        fn stop(self) {
            self.stop.store(true, Ordering::Release);
            let _ = self.worker.join();
        }
    }
    #[derive(Default)]
    pub(crate) struct Controller {
        session: Mutex<Option<Session>>,
        closed: AtomicBool,
    }
    impl Controller {
        pub fn active(&self) -> bool {
            self.session
                .lock()
                .is_ok_and(|s| s.as_ref().is_some_and(|s| !s.worker.is_finished()))
        }
        pub fn stop(&self) -> bool {
            if let Ok(mut lock) = self.session.lock() {
                if let Some(s) = lock.take() {
                    s.stop();
                    return true;
                }
            }
            false
        }
        pub fn shutdown(&self) {
            self.closed.store(true, Ordering::Release);
            self.stop();
        }
        pub fn start(&self, summary: Summary) -> Result<String, String> {
            self.start_inner(summary, TTL)
                .map_err(|_| "Local sharing unavailable".into())
        }
        fn start_inner(&self, summary: Summary, ttl: Duration) -> io::Result<String> {
            let mut lock = self
                .session
                .lock()
                .map_err(|_| io::Error::other("sharing unavailable"))?;
            if self.closed.load(Ordering::Acquire) || !summary.valid() || !tmp_ok() {
                return Err(io::ErrorKind::PermissionDenied.into());
            }
            if lock.as_ref().is_some_and(|s| !s.worker.is_finished()) {
                return Err(io::ErrorKind::AlreadyExists.into());
            }
            if let Some(s) = lock.take() {
                s.stop()
            }
            let id = random::<16>()?;
            let key = random::<32>()?;
            let dir = path(&id);
            fs::DirBuilder::new().mode(0o700).create(&dir)?;
            let mut ep = Endpoint {
                dir_id: identity(&dir)?,
                dir,
                socket_id: None,
            };
            if !private_dir(&ep.dir) {
                return Err(io::ErrorKind::PermissionDenied.into());
            }
            let p = ep.dir.join("s");
            let l = UnixListener::bind(&p)?;
            ep.socket_id = Some(identity(&p)?);
            fs::set_permissions(&p, fs::Permissions::from_mode(0o600))?;
            if !private_socket(&p) {
                return Err(io::ErrorKind::PermissionDenied.into());
            }
            l.set_nonblocking(true)?;
            let stop = Arc::new(AtomicBool::new(false));
            let w = stop.clone();
            let expires = Instant::now() + ttl;
            let worker = thread::Builder::new()
                .name("lu-diagnostic-share".into())
                .spawn(move || {
                    let _ep = ep;
                    while expired(&w, expires).is_ok() {
                        match l.accept() {
                            Ok((mut s, _)) => {
                                if !peer(&s) || s.set_nonblocking(true).is_err() {
                                    continue;
                                }
                                let end = (Instant::now() + FRAME).min(expires);
                                let Ok(b) = read(&mut s, &w, end) else {
                                    continue;
                                };
                                let Some(r) = auth_request(&key, &b) else {
                                    continue;
                                };
                                let _ = write(
                                    &mut s,
                                    response(&key, r.nonce, summary.clone()).encode_to_vec(),
                                    &w,
                                    end,
                                );
                                break;
                            }
                            Err(e) if e.kind() == io::ErrorKind::WouldBlock => thread::sleep(TICK),
                            Err(_) => break,
                        }
                    }
                })?;
            *lock = Some(Session { stop, worker });
            Ok(format!("lu1-{}-{}", hex(&id), hex(&key)))
        }
    }
    impl Drop for Controller {
        fn drop(&mut self) {
            self.shutdown();
        }
    }
    pub(crate) fn receive(code: &str) -> Result<Summary, String> {
        receive_inner(code)
            .map_err(|_| "The invitation is invalid, expired, incompatible or unavailable.".into())
    }
    fn receive_inner(code: &str) -> io::Result<Summary> {
        let bad = || io::Error::from(io::ErrorKind::InvalidData);
        if code.len() != 101 || !tmp_ok() {
            return Err(bad());
        }
        let mut parts = code.split('-');
        if parts.next() != Some("lu1") {
            return Err(bad());
        }
        let id = unhex::<16>(parts.next().ok_or_else(bad)?).ok_or_else(bad)?;
        let key = unhex::<32>(parts.next().ok_or_else(bad)?).ok_or_else(bad)?;
        if parts.next().is_some() {
            return Err(bad());
        }
        let dir = path(&id);
        let p = dir.join("s");
        if !private_dir(&dir) || !private_socket(&p) {
            return Err(bad());
        }
        let socket = socket2::Socket::new(socket2::Domain::UNIX, socket2::Type::STREAM, None)?;
        socket.connect_timeout(&socket2::SockAddr::unix(&p)?, FRAME)?;
        let fd: std::os::fd::OwnedFd = socket.into();
        let mut s = UnixStream::from(fd);
        if !peer(&s) {
            return Err(bad());
        }
        s.set_nonblocking(true)?;
        let nonce = random::<32>()?.to_vec();
        let stop = AtomicBool::new(false);
        let end = Instant::now() + FRAME;
        write(
            &mut s,
            request(&key, nonce.clone()).encode_to_vec(),
            &stop,
            end,
        )?;
        auth_response(&key, &nonce, &read(&mut s, &stop, end)?).ok_or_else(bad)
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        fn summary() -> Summary {
            Summary {
                complete: 1,
                observed_failure: 2,
                bounded: 3,
                incomplete: 4,
                unavailable: 5,
            }
        }
        fn endpoint(code: &str) -> PathBuf {
            path(&unhex::<16>(code.split('-').nth(1).unwrap()).unwrap())
        }
        #[test]
        fn native_roundtrip_single_use() {
            let c = Controller::default();
            assert!(!c.active());
            let code = c.start(summary()).unwrap();
            assert!(c.active());
            assert_eq!(receive(&code).unwrap(), summary());
            c.stop();
            assert!(!endpoint(&code).exists());
            assert!(receive(&code).is_err());
        }
        #[test]
        fn invalid_key_version_and_unknown_fields() {
            let c = Controller::default();
            let code = c.start(summary()).unwrap();
            let bad = format!("{}{}", &code[..37], "0".repeat(64));
            assert!(receive(&bad).is_err());
            let mut s = UnixStream::connect(endpoint(&code).join("s")).unwrap();
            s.set_nonblocking(true).unwrap();
            let k = unhex::<32>(code.split('-').nth(2).unwrap()).unwrap();
            let mut r = request(&k, vec![1; 32]);
            r.version = 99;
            r.mac.clear();
            r.mac = mac(&k, b"lu.dx.request.v1\0", &r.encode_to_vec()).to_vec();
            let stop = AtomicBool::new(false);
            let end = Instant::now() + FRAME;
            write(&mut s, r.encode_to_vec(), &stop, end).unwrap();
            assert!(read(&mut s, &stop, end).is_err());
            assert_eq!(receive(&code).unwrap(), summary());
            c.stop();
            let mut b = request(&k, vec![1; 32]).encode_to_vec();
            b.extend([0x78, 1]);
            assert!(auth_request(&k, &b).is_none());
        }
        #[test]
        fn revocation_rotation_expiry_and_slow_frame() {
            let c = Controller::default();
            let a = c.start(summary()).unwrap();
            let mut s = UnixStream::connect(endpoint(&a).join("s")).unwrap();
            s.write_all(&[0]).unwrap();
            let t = Instant::now();
            assert!(c.stop());
            assert!(t.elapsed() < Duration::from_secs(1));
            assert!(!endpoint(&a).exists());
            let b = c
                .start_inner(summary(), Duration::from_millis(120))
                .unwrap();
            assert_ne!(a, b);
            assert!(receive(&a).is_err());
            thread::sleep(Duration::from_millis(200));
            assert!(receive(&b).is_err());
            c.shutdown();
            assert!(c.start(summary()).is_err());
        }
        #[test]
        fn cp12br1_incompatible_diagnostic_version_and_oversized_frame_leave_invitation_unconsumed()
        {
            let c = Controller::default();
            let code = c.start(summary()).unwrap();
            let key = unhex::<32>(code.split('-').nth(2).unwrap()).unwrap();
            let stop = AtomicBool::new(false);
            for (version, dx_version) in [(0, DX_VERSION), (VERSION, 1), (VERSION, DX_VERSION + 1)]
            {
                let mut req = request(&key, vec![9; 32]);
                req.version = version;
                req.dx_version = dx_version;
                req.mac.clear();
                req.mac = mac(&key, b"lu.dx.request.v1\0", &req.encode_to_vec()).to_vec();
                let mut s = UnixStream::connect(endpoint(&code).join("s")).unwrap();
                s.set_nonblocking(true).unwrap();
                let end = Instant::now() + FRAME;
                write(&mut s, req.encode_to_vec(), &stop, end).unwrap();
                assert!(read(&mut s, &stop, end).is_err());
                let mut rsp = response(&key, vec![9; 32], summary());
                rsp.version = version;
                rsp.dx_version = dx_version;
                rsp.mac.clear();
                rsp.mac = mac(&key, b"lu.dx.response.v1\0", &rsp.encode_to_vec()).to_vec();
                assert!(auth_response(&key, &[9; 32], &rsp.encode_to_vec()).is_none());
            }
            let mut stream = UnixStream::connect(endpoint(&code).join("s")).unwrap();
            stream.write_all(&((MAX + 1) as u32).to_be_bytes()).unwrap();
            stream.set_nonblocking(true).unwrap();
            assert!(read(&mut stream, &stop, Instant::now() + FRAME).is_err());
            assert_eq!(receive(&code).unwrap(), summary());
            c.stop();
        }
        #[test]
        fn response_tampering_and_nonce() {
            let k = [5; 32];
            let n = vec![2; 32];
            let mut r = response(&k, n.clone(), summary());
            assert_eq!(auth_response(&k, &n, &r.encode_to_vec()), Some(summary()));
            assert!(auth_response(&k, &[3; 32], &r.encode_to_vec()).is_none());
            r.summary.as_mut().unwrap().complete += 1;
            assert!(auth_response(&k, &n, &r.encode_to_vec()).is_none());
        }
        // The ignored entry points are invoked only by their parent tests.
        fn child(name: &str) -> std::process::Command {
            let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
            cmd.args(["--exact", name, "--ignored", "--nocapture"])
                .env("LU_DX_TEST_CHILD", "1");
            cmd
        }
        #[test]
        fn independent_process_receives() {
            let c = Controller::default();
            let code = c.start(summary()).unwrap();
            let result = child("dx_ipc::linux::tests::child_receive")
                .env("LU_DX_TEST_CODE", &code)
                .output()
                .unwrap();
            assert!(result.status.success(), "child exchange failed");
            assert!(receive(&code).is_err());
            c.stop();
        }
        #[test]
        #[ignore = "parent subprocess entry"]
        fn child_receive() {
            assert_eq!(std::env::var("LU_DX_TEST_CHILD").as_deref(), Ok("1"));
            let code = std::env::var("LU_DX_TEST_CODE").unwrap();
            assert_eq!(receive(&code).unwrap(), summary());
        }
        #[test]
        fn different_account_refused_if_root() {
            use std::os::unix::process::CommandExt;
            if uid() != 0 {
                eprintln!("cross-account witness requires privileged test runner");
                return;
            }
            let c = Controller::default();
            let code = c.start(summary()).unwrap();
            let result = child("dx_ipc::linux::tests::child_wrong_uid")
                .env("LU_DX_TEST_CODE", &code)
                .uid(65534)
                .gid(65534)
                .output()
                .unwrap();
            assert!(result.status.success(), "different-account witness failed");
            assert_eq!(receive(&code).unwrap(), summary());
            c.stop();
        }
        #[test]
        #[ignore = "parent subprocess entry"]
        fn child_wrong_uid() {
            assert_eq!(uid(), 65534);
            let code = std::env::var("LU_DX_TEST_CODE").unwrap();
            assert_eq!(
                UnixStream::connect(endpoint(&code).join("s"))
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::PermissionDenied
            );
            assert!(receive(&code).is_err());
        }
        #[test]
        fn abrupt_sender_exit_invalidates_invitation() {
            use std::io::BufRead;
            use std::process::Stdio;
            let mut child = child("dx_ipc::linux::tests::child_sender")
                .stdout(Stdio::piped())
                .spawn()
                .unwrap();
            let stdout = child.stdout.take().unwrap();
            let (tx, rx) = std::sync::mpsc::channel();
            let reader = thread::spawn(move || {
                for l in io::BufReader::new(stdout).lines().map_while(Result::ok) {
                    if let Some(code) = l.strip_prefix("INVITE:") {
                        let _ = tx.send(code.to_owned());
                        break;
                    }
                }
            });
            let code = rx.recv_timeout(Duration::from_secs(5));
            let _ = child.kill();
            let status = child.wait().unwrap();
            let _ = reader.join();
            assert!(!status.success());
            let code = code.expect("sender not ready");
            assert!(receive(&code).is_err());
            let old = endpoint(&code);
            assert!(old.is_dir());
            // Only the disposable test owns this abandoned path; production
            // startup never trusts or reuses abandoned endpoint names.
            fs::remove_file(old.join("s")).unwrap();
            fs::remove_dir(&old).unwrap();
            let c = Controller::default();
            let fresh = c.start(summary()).unwrap();
            assert_ne!(fresh, code);
            assert!(receive(&code).is_err());
            assert_eq!(receive(&fresh).unwrap(), summary());
            c.stop();
        }
        #[test]
        #[ignore = "parent subprocess entry"]
        fn child_sender() {
            let c = Controller::default();
            let code = c.start(summary()).unwrap();
            println!("INVITE:{code}");
            io::stdout().flush().unwrap();
            thread::sleep(Duration::from_secs(30));
        }
        #[test]
        fn rfc4231_hmac_vector() {
            let mut k = [0; 32];
            k[..20].fill(0x0b);
            assert_eq!(
                hex(&mac(&k, b"", b"Hi There")),
                "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
            );
        }
    }
}
#[cfg(target_os = "linux")]
pub(crate) use linux::{receive, Controller};
#[cfg(all(target_os = "windows", feature = "windows_dx_eval"))]
#[path = "dx_windows_pipe.rs"]
mod windows_pipe;
#[cfg(all(target_os = "windows", feature = "windows_dx_eval"))]
pub(crate) use windows_pipe::{receive, Controller};

/// Windows evaluation is never in the default release build.
pub(crate) const fn supported() -> bool {
    cfg!(any(
        target_os = "linux",
        all(target_os = "windows", feature = "windows_dx_eval")
    ))
}
#[cfg(not(any(
    target_os = "linux",
    all(target_os = "windows", feature = "windows_dx_eval")
)))]
#[derive(Default)]
pub(crate) struct Controller;
#[cfg(not(any(
    target_os = "linux",
    all(target_os = "windows", feature = "windows_dx_eval")
)))]
impl Controller {
    pub fn active(&self) -> bool {
        false
    }
    pub fn stop(&self) -> bool {
        false
    }
    pub fn shutdown(&self) {}
    pub fn start(&self, _: Summary) -> Result<String, String> {
        Err("Local sharing is not available on this platform.".into())
    }
}
#[cfg(not(any(
    target_os = "linux",
    all(target_os = "windows", feature = "windows_dx_eval")
)))]
pub(crate) fn receive(_: &str) -> Result<Summary, String> {
    Err("Local sharing is not available on this platform.".into())
}

#[cfg(all(test, target_os = "windows", not(feature = "windows_dx_eval")))]
mod windows_disabled_tests {
    use super::*;
    #[test]
    fn cp12br1_windows_transport_is_unavailable_without_explicit_qualification() {
        let controller = Controller::default();
        let summary = Summary {
            complete: 0,
            observed_failure: 0,
            bounded: 0,
            incomplete: 0,
            unavailable: 0,
        };
        assert!(!controller.active());
        assert!(controller.start(summary).is_err());
        assert!(receive("untrusted invitation").is_err());
        assert!(!controller.stop());
        controller.shutdown();
    }
}
