//! IDE-CORE-IPC-01B: OS-local, one-use native credential issuer CLIENT.
//!
//! An actual installed issuer is separately required. The caller must create
//! `InstalledIssuerAnchor` from verified native installation custody, not from
//! a WebView, project, model, shell environment or received IPC payload.
//! The issuer MUST independently attest the calling package and OS identity
//! before releasing any secret. This client pins Linux SO_PEERCRED, the
//! protected issuer endpoint, original package SHA and Core owner UID; it
//! cannot itself attest a publisher signature or install the service.
//!
//! No TCP, fallback file, arbitrary endpoint discovery, credential cache,
//! Tauri command, execution authority or model-facing API is provided.
use crate::{
    core_ipc_host::{HostCoreHealthError, ProtectedCoreHealthBroker},
    core_ipc_native_transport::{connect, TrustedSocket},
};
use prost::Message;
use rand::{rngs::OsRng, RngCore};
use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use zeroize::{Zeroize, Zeroizing};

pub(crate) mod proto {
    include!(concat!(env!("OUT_DIR"), "/lu.ide.v1.rs"));
}
const SCOPE: &str = "core.health.readonly";
const SCHEMA: u32 = 1;
const MAX_WIRE: usize = 4096;
const IO_LIMIT: Duration = Duration::from_secs(2);
const TOTAL_LIMIT: Duration = Duration::from_secs(6);
const MAX_TTL_SECONDS: u64 = 60;

/// Set only at an authenticated installed native startup boundary. Data
/// cannot come from frontend input and is never obtained from the server.
pub(crate) struct InstalledIssuerAnchor {
    pub(crate) issuer_socket: PathBuf,
    pub(crate) issuer_uid: u32,
    pub(crate) issuer_group_gid: Option<u32>,
    pub(crate) core_uid: u32,
    pub(crate) package_sha256: [u8; 32],
}

/// Consumes one installed broker lease, including failures; constructing
/// another adapter is an explicit new native operation, never an auto retry.
pub(crate) struct UnixCredentialIssuer {
    anchor: InstalledIssuerAnchor,
    used: AtomicBool,
}
impl UnixCredentialIssuer {
    pub(crate) fn new(anchor: InstalledIssuerAnchor) -> Result<Self, HostCoreHealthError> {
        if !anchor.issuer_socket.is_absolute() || anchor.package_sha256.iter().all(|&b| b == 0) {
            return Err(HostCoreHealthError::BrokerInvalid);
        }
        Ok(Self {
            anchor,
            used: AtomicBool::new(false),
        })
    }

    fn exchange(
        &self,
        operation_ref: &str,
        callback: &mut dyn FnMut(&Path, u32, &[u8; 32], &[u8; 32]),
    ) -> Result<(), HostCoreHealthError> {
        if operation_ref.len() != 32
            || !operation_ref
                .bytes()
                .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
        {
            return Err(HostCoreHealthError::BrokerInvalid);
        }
        let endpoint = TrustedSocket {
            socket_path: &self.anchor.issuer_socket,
            owner_uid: self.anchor.issuer_uid,
            access_group_gid: self.anchor.issuer_group_gid,
        };
        // connect() checks canonical 0700 parent, 0600 socket, inode witness,
        // exact filesystem UID AND kernel SO_PEERCRED on the accepted fd.
        let mut io = connect(&endpoint).map_err(|_| HostCoreHealthError::BrokerUnavailable)?;
        let until = Instant::now() + TOTAL_LIMIT;
        let mut nonce = [0_u8; 32];
        OsRng.fill_bytes(&mut nonce);
        let request = proto::NativeHealthLeaseRequest {
            schema_version: SCHEMA,
            operation_nonce: nonce.to_vec(),
            installed_package_sha256: self.anchor.package_sha256.to_vec(),
            scope: SCOPE.into(),
            journal_operation_ref: operation_ref.into(),
        };
        let mut wire = request.encode_to_vec();
        send_bounded(&mut io, &wire, until).map_err(|_| HostCoreHealthError::BrokerUnavailable)?;
        wire.zeroize();
        let mut response_wire =
            receive_bounded(&mut io, until).map_err(|_| HostCoreHealthError::BrokerUnavailable)?;
        let decoded = proto::NativeHealthLeaseReply::decode(response_wire.as_slice())
            .map_err(|_| HostCoreHealthError::BrokerInvalid);
        let canonical = decoded
            .as_ref()
            .map(|v| v.encode_to_vec() == response_wire)
            .unwrap_or(false);
        response_wire.zeroize();
        let mut response = decoded?;
        // Keep the secret in a Drop-zeroized buffer across all early returns.
        let mut credential = Zeroizing::new([0_u8; 32]);
        if response.credential.len() == 32 {
            credential.copy_from_slice(&response.credential);
        }
        response.credential.zeroize();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| HostCoreHealthError::BrokerInvalid)?
            .as_secs();
        if !canonical
            || response.schema_version != SCHEMA
            || response.operation_nonce.as_slice() != nonce
            || response.installed_package_sha256.as_slice() != self.anchor.package_sha256
            || response.scope != SCOPE
            || response.journal_operation_ref != operation_ref
            || response.core_owner_uid != self.anchor.core_uid
            || response.core_socket_path.is_empty()
            || response.core_socket_path.len() > 107
            || response.expires_unix_seconds < now
            || response.expires_unix_seconds > now.saturating_add(MAX_TTL_SECONDS)
            || credential.iter().all(|&b| b == 0)
        {
            return Err(HostCoreHealthError::BrokerInvalid);
        }
        // One callback only. The caller checks the exact trusted Core socket
        // and authenticates the Core Welcome MAC before sending Health.
        callback(
            Path::new(&response.core_socket_path),
            self.anchor.core_uid,
            &self.anchor.package_sha256,
            &credential,
        );
        Ok(())
    }
}
impl ProtectedCoreHealthBroker for UnixCredentialIssuer {
    fn with_enrollment(
        &self,
        operation_ref: &str,
        callback: &mut dyn FnMut(&Path, u32, &[u8; 32], &[u8; 32]),
    ) -> Result<(), HostCoreHealthError> {
        if self.used.swap(true, Ordering::AcqRel) {
            return Err(HostCoreHealthError::BrokerReused);
        }
        self.exchange(operation_ref, callback)
    }
}

fn io_deadline(io: &UnixStream, remaining: Instant) -> std::io::Result<()> {
    let time = remaining.saturating_duration_since(Instant::now());
    if time.is_zero() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "issuer deadline",
        ));
    }
    io.set_read_timeout(Some(time.min(IO_LIMIT)))?;
    io.set_write_timeout(Some(time.min(IO_LIMIT)))
}
fn write_all_deadline(
    io: &mut UnixStream,
    mut src: &[u8],
    expires: Instant,
) -> std::io::Result<()> {
    while !src.is_empty() {
        io_deadline(io, expires)?;
        let n = io.write(src)?;
        if n == 0 {
            return Err(std::io::ErrorKind::WriteZero.into());
        }
        src = &src[n..];
    }
    Ok(())
}
fn read_exact_deadline(
    io: &mut UnixStream,
    mut buf: &mut [u8],
    expires: Instant,
) -> std::io::Result<()> {
    while !buf.is_empty() {
        io_deadline(io, expires)?;
        let n = io.read(buf)?;
        if n == 0 {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }
        buf = &mut buf[n..];
    }
    Ok(())
}
fn send_bounded(io: &mut UnixStream, payload: &[u8], expires: Instant) -> std::io::Result<()> {
    if payload.is_empty() || payload.len() > MAX_WIRE {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    write_all_deadline(io, &(payload.len() as u32).to_be_bytes(), expires)?;
    write_all_deadline(io, payload, expires)
}
fn receive_bounded(io: &mut UnixStream, expires: Instant) -> std::io::Result<Vec<u8>> {
    let mut header = [0u8; 4];
    read_exact_deadline(io, &mut header, expires)?;
    let len = u32::from_be_bytes(header) as usize;
    if len == 0 || len > MAX_WIRE {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    let mut wire = vec![0u8; len];
    read_exact_deadline(io, &mut wire, expires)?;
    Ok(wire)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        os::unix::{
            fs::{MetadataExt, PermissionsExt},
            net::UnixListener,
        },
        sync::{atomic::AtomicUsize, Arc},
        thread,
    };

    fn fixture() -> (PathBuf, PathBuf, u32, UnixListener) {
        let mut random = [0u8; 16];
        OsRng.fill_bytes(&mut random);
        let name: String = random.iter().map(|b| format!("{b:02x}")).collect();
        let dir = std::env::temp_dir().join(format!("ide-issuer-01b-{name}"));
        fs::create_dir(&dir).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
        let socket = dir.join("issuer.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).unwrap();
        let uid = fs::metadata(&dir).unwrap().uid();
        (dir, socket, uid, listener)
    }
    fn serve(
        listener: UnixListener,
        corrupt: bool,
        stale: bool,
        foreign_operation: bool,
    ) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut req = [0u8; 4];
            stream.read_exact(&mut req).unwrap();
            let mut body = vec![0u8; u32::from_be_bytes(req) as usize];
            stream.read_exact(&mut body).unwrap();
            let r = proto::NativeHealthLeaseRequest::decode(body.as_slice()).unwrap();
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let response = proto::NativeHealthLeaseReply {
                schema_version: 1,
                operation_nonce: r.operation_nonce,
                installed_package_sha256: if corrupt {
                    vec![9u8; 32]
                } else {
                    r.installed_package_sha256
                },
                scope: SCOPE.into(),
                core_socket_path: "/native/protected/core.sock".into(),
                core_owner_uid: 1000,
                credential: vec![0x93; 32],
                expires_unix_seconds: if stale { now - 1 } else { now + 15 },
                journal_operation_ref: if foreign_operation {
                    "00000000000000000000000000000000".into()
                } else {
                    r.journal_operation_ref
                },
            };
            let bytes = response.encode_to_vec();
            stream
                .write_all(&(bytes.len() as u32).to_be_bytes())
                .unwrap();
            stream.write_all(&bytes).unwrap();
        })
    }
    #[test]
    fn real_linux_peer_custody_delivers_one_ephemeral_health_lease() {
        let (dir, socket, uid, listener) = fixture();
        let server = serve(listener, false, false, false);
        let broker = UnixCredentialIssuer::new(InstalledIssuerAnchor {
            issuer_socket: socket.clone(),
            issuer_uid: uid,
            issuer_group_gid: None,
            core_uid: 1000,
            package_sha256: [0x7c; 32],
        })
        .unwrap();
        let count = AtomicUsize::new(0);
        broker
            .with_enrollment(
                "abcdabcdabcdabcdabcdabcdabcdabcd",
                &mut |path, core_uid, pkg, key| {
                    count.fetch_add(1, Ordering::AcqRel);
                    assert_eq!(path, Path::new("/native/protected/core.sock"));
                    assert_eq!(core_uid, 1000);
                    assert_eq!(pkg, &[0x7c; 32]);
                    assert_eq!(key, &[0x93; 32]);
                },
            )
            .unwrap();
        assert_eq!(
            broker.with_enrollment("abcdabcdabcdabcdabcdabcdabcdabcd", &mut |_, _, _, _| {
                panic!("replay callback");
            }),
            Err(HostCoreHealthError::BrokerReused)
        );
        assert_eq!(count.load(Ordering::Acquire), 1);
        server.join().unwrap();
        fs::remove_file(socket).unwrap();
        fs::remove_dir(dir).unwrap();
    }
    #[test]
    fn issuer_reply_package_mismatch_or_expiry_refuses_borrow() {
        for (corrupt, stale) in [(true, false), (false, true)] {
            let (dir, socket, uid, listener) = fixture();
            let server = serve(listener, corrupt, stale, false);
            let broker = UnixCredentialIssuer::new(InstalledIssuerAnchor {
                issuer_socket: socket.clone(),
                issuer_uid: uid,
                issuer_group_gid: None,
                core_uid: 1000,
                package_sha256: [0x7c; 32],
            })
            .unwrap();
            let result = broker
                .with_enrollment("abcdabcdabcdabcdabcdabcdabcdabcd", &mut |_, _, _, _| {
                    panic!("invalid lease callback")
                });
            assert_eq!(result, Err(HostCoreHealthError::BrokerInvalid));
            server.join().unwrap();
            fs::remove_file(socket).unwrap();
            fs::remove_dir(dir).unwrap();
        }
    }
    #[test]
    fn issuer_must_echo_durable_operation_id_before_credential_consumption() {
        let (dir, socket, uid, listener) = fixture();
        let server = serve(listener, false, false, true);
        let broker = UnixCredentialIssuer::new(InstalledIssuerAnchor {
            issuer_socket: socket.clone(),
            issuer_uid: uid,
            issuer_group_gid: None,
            core_uid: 1000,
            package_sha256: [0x7c; 32],
        })
        .unwrap();
        assert_eq!(
            broker.with_enrollment(
                "abcdabcdabcdabcdabcdabcdabcdabcd",
                &mut |_, _, _, _| panic!("unbound lease")
            ),
            Err(HostCoreHealthError::BrokerInvalid)
        );
        server.join().unwrap();
        fs::remove_file(socket).unwrap();
        fs::remove_dir(dir).unwrap();
    }
    #[test]
    fn wrong_installed_peer_uid_refuses_before_borrow_or_exchange() {
        let (dir, socket, uid, listener) = fixture();
        let broker = UnixCredentialIssuer::new(InstalledIssuerAnchor {
            issuer_socket: socket.clone(),
            issuer_uid: uid.wrapping_add(1),
            issuer_group_gid: None,
            core_uid: 1000,
            package_sha256: [0x7c; 32],
        })
        .unwrap();
        assert_eq!(
            broker.with_enrollment(
                "abcdabcdabcdabcdabcdabcdabcdabcd",
                &mut |_, _, _, _| panic!("wrong uid")
            ),
            Err(HostCoreHealthError::BrokerUnavailable)
        );
        drop(listener);
        fs::remove_file(socket).unwrap();
        fs::remove_dir(dir).unwrap();
    }
}
