//! IDE-CORE-IPC-01A-R2: native Linux AF_UNIX endpoint custody and one bounded
//! read-only request. No Tauri command, no HTTP/TCP listener, no tool dispatch.
//!
//! The IDE does not learn its trusted endpoint or key from a workspace,
//! WebView, model, environment variable, Protobuf Hello or arbitrary file.
//! The installed native owner must supply the expected absolute socket path,
//! UID and independently provisioned enrollment from protected custody.
//! This client checks the endpoint and authenticates Core's HMAC handshake.
//! Core independently authenticates the *client* through SO_PEERCRED and its
//! enrollment policy; this code does not replace Core-side OS admission.
//!
//! A pathname and chmod alone do not prove the server's identity. We bind
//! the connection to Core's separately enrolled HMAC, and detect pre/post
//! pathname swaps as defense in depth. Installation and credential custody
//! remain OPEN. There is no Windows positive path without an OS attestor.
use crate::core_ipc_readonly::{health_probe, NativeEnrollment, ProbeError};
use std::{
    io,
    path::{Component, Path},
    time::{Duration, Instant},
};

const IO_DEADLINE: Duration = Duration::from_secs(3);
// Total wall-clock time for the whole handshake + single health request,
// not a renewable timeout for each small read in a trickled frame.
const EXCHANGE_DEADLINE: Duration = Duration::from_secs(12);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LocalTransportError {
    Io(io::ErrorKind),
    InvalidEndpoint,
    EndpointChanged,
    PeerIdentityMismatch,
    Invalidated,
    Probe(ProbeError),
}
impl From<io::Error> for LocalTransportError {
    fn from(value: io::Error) -> Self {
        Self::Io(value.kind())
    }
}
impl From<ProbeError> for LocalTransportError {
    fn from(value: ProbeError) -> Self {
        Self::Probe(value)
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use std::{
        fs::{self, Metadata},
        io::{Read, Write},
        os::unix::{
            fs::{FileTypeExt, MetadataExt, PermissionsExt},
            net::UnixStream,
        },
    };

    /// The *installed native owner*, not IPC/JS/model input, supplies these.
    /// Do not infer `owner_uid` by trusting the current socket's metadata.
    pub(crate) struct TrustedSocket<'a> {
        pub(crate) socket_path: &'a Path,
        pub(crate) owner_uid: u32,
        /// Fixed OS-installed access group; NONE keeps private same-UID 0700/0600.
        /// A group endpoint needs a 0750 directory and 0660 socket, both
        /// owned by the independently pinned service UID and access GID.
        pub(crate) access_group_gid: Option<u32>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct Witness {
        dir_dev: u64,
        dir_ino: u64,
        socket_dev: u64,
        socket_ino: u64,
    }

    fn trusted_parts(
        endpoint: &TrustedSocket<'_>,
    ) -> Result<(Metadata, Metadata), LocalTransportError> {
        let path = endpoint.socket_path;
        if !path.is_absolute()
            || path
                .components()
                .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
        {
            return Err(LocalTransportError::InvalidEndpoint);
        }
        let parent = path.parent().ok_or(LocalTransportError::InvalidEndpoint)?;
        // Reject aliasing through any symlinked ancestor, not just the leaf.
        // The installed owner must supply the canonical OS-native endpoint.
        if fs::canonicalize(parent)? != parent {
            return Err(LocalTransportError::InvalidEndpoint);
        }
        let directory = fs::symlink_metadata(parent)?;
        let socket = fs::symlink_metadata(path)?;
        if !directory.file_type().is_dir()
            || directory.file_type().is_symlink()
            || directory.uid() != endpoint.owner_uid
            || match endpoint.access_group_gid {
                Some(gid) => {
                    directory.gid() != gid || directory.permissions().mode() & 0o777 != 0o750
                }
                None => directory.permissions().mode() & 0o777 != 0o700,
            }
            || !socket.file_type().is_socket()
            || socket.file_type().is_symlink()
            || socket.uid() != endpoint.owner_uid
            || match endpoint.access_group_gid {
                Some(gid) => socket.gid() != gid || socket.permissions().mode() & 0o777 != 0o660,
                None => socket.permissions().mode() & 0o777 != 0o600,
            }
        {
            return Err(LocalTransportError::InvalidEndpoint);
        }
        Ok((directory, socket))
    }

    fn witness(endpoint: &TrustedSocket<'_>) -> Result<Witness, LocalTransportError> {
        let (dir, sock) = trusted_parts(endpoint)?;
        Ok(Witness {
            dir_dev: dir.dev(),
            dir_ino: dir.ino(),
            socket_dev: sock.dev(),
            socket_ino: sock.ino(),
        })
    }

    /// Connects solely to a trusted local Linux AF_UNIX endpoint. No DNS,
    /// inherited sockets, HTTP or unbounded network fetch. Per-operation read
    /// and write deadlines are set before the first Protobuf Hello is sent.
    pub(crate) fn connect(endpoint: &TrustedSocket<'_>) -> Result<UnixStream, LocalTransportError> {
        let before = witness(endpoint)?;
        let stream = UnixStream::connect(endpoint.socket_path)?;
        // Linux SO_PEERCRED binds this accepted fd to the independently pinned
        // Core service UID. Socket pathname metadata alone is not peer proof.
        let peer = rustix::net::sockopt::socket_peercred(&stream)
            .map_err(|_| LocalTransportError::PeerIdentityMismatch)?;
        if peer.uid.as_raw() != endpoint.owner_uid {
            return Err(LocalTransportError::PeerIdentityMismatch);
        }
        stream.set_read_timeout(Some(IO_DEADLINE))?;
        stream.set_write_timeout(Some(IO_DEADLINE))?;
        let after = witness(endpoint)?;
        if before != after {
            return Err(LocalTransportError::EndpointChanged);
        }
        Ok(stream)
    }

    /// A single absolute deadline covers the entire Protobuf/HMAC/health
    /// exchange. The ordinary 3-second socket timeout alone is *renewable*
    /// after each partial read; an adversarial drip-fed frame could otherwise
    /// hold a native host thread for an unbounded time.
    struct DeadlineStream<'a> {
        inner: UnixStream,
        expires: Instant,
        active: &'a dyn Fn() -> bool,
    }
    impl<'a> DeadlineStream<'a> {
        fn new(inner: UnixStream, active: &'a dyn Fn() -> bool) -> Self {
            Self {
                inner,
                expires: Instant::now() + EXCHANGE_DEADLINE,
                active,
            }
        }
        fn remaining(&self) -> io::Result<Duration> {
            if !(self.active)() {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "core ipc invalidated",
                ));
            }
            let remaining = self.expires.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::Error::new(io::ErrorKind::TimedOut, "core ipc deadline"));
            }
            Ok(remaining.min(IO_DEADLINE))
        }
    }
    impl Read for DeadlineStream<'_> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let remaining = self.remaining()?;
            self.inner.set_read_timeout(Some(remaining))?;
            self.inner.read(buf)
        }
    }
    impl Write for DeadlineStream<'_> {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let remaining = self.remaining()?;
            self.inner.set_write_timeout(Some(remaining))?;
            self.inner.write(buf)
        }
        fn flush(&mut self) -> io::Result<()> {
            let _ = self.remaining()?;
            self.inner.flush()
        }
    }

    /// Exactly one Lu Core Health request after both local endpoint checks
    /// and the authenticated Core Hello/Challenge/Proof/Welcome exchange.
    /// A HealthReport is diagnostic data, NOT an authorization grant.
    pub(crate) fn probe_health(
        endpoint: &TrustedSocket<'_>,
        enrolled: &NativeEnrollment<'_>,
    ) -> Result<crate::core_ipc_readonly::proto::HealthReport, LocalTransportError> {
        probe_health_with_guard(endpoint, enrolled, &|| true)
    }

    /// The owner checks cancellation/revocation at every OS read/write, not
    /// just before borrowing the one-use credential. A mid-I/O invalidation
    /// refuses the observation; the durable owner marks the result uncertain.
    pub(crate) fn probe_health_with_guard(
        endpoint: &TrustedSocket<'_>,
        enrolled: &NativeEnrollment<'_>,
        active: &dyn Fn() -> bool,
    ) -> Result<crate::core_ipc_readonly::proto::HealthReport, LocalTransportError> {
        if !active() {
            return Err(LocalTransportError::Invalidated);
        }
        // Refuse zero or missing material before connecting and sending Hello.
        if enrolled.package_sha256.iter().all(|byte| *byte == 0)
            || enrolled.credential.iter().all(|byte| *byte == 0)
        {
            return Err(LocalTransportError::Probe(ProbeError::Unauthorized));
        }
        let stream = connect(endpoint)?;
        let mut bounded = DeadlineStream::new(stream, active);
        health_probe(&mut bounded, enrolled).map_err(Into::into)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use rand::{rngs::OsRng, RngCore};
        use std::os::unix::net::UnixListener;
        fn setup() -> (std::path::PathBuf, std::path::PathBuf, u32) {
            let mut nonce = [0u8; 16];
            OsRng.fill_bytes(&mut nonce);
            let suffix: String = nonce.iter().map(|byte| format!("{byte:02x}")).collect();
            let dir = std::env::temp_dir().join(format!("ide-core-ipc-test-{suffix}"));
            fs::create_dir(&dir).unwrap();
            fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
            let uid = fs::symlink_metadata(&dir).unwrap().uid();
            let path = dir.join("core.sock");
            (dir, path, uid)
        }
        #[test]
        fn genuine_unix_socket_connect_requires_exact_private_metadata() {
            let (dir, path, uid) = setup();
            let listener = UnixListener::bind(&path).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
            let endpoint = TrustedSocket {
                socket_path: &path,
                owner_uid: uid,
                access_group_gid: None,
            };
            let client = connect(&endpoint).unwrap();
            let (server, _) = listener.accept().unwrap();
            assert!(client.local_addr().is_ok());
            drop(server);
            drop(client);
            assert_eq!(
                witness(&endpoint).unwrap().socket_ino,
                fs::symlink_metadata(&path).unwrap().ino()
            );
            fs::set_permissions(&path, fs::Permissions::from_mode(0o666)).unwrap();
            assert!(matches!(
                connect(&endpoint),
                Err(LocalTransportError::InvalidEndpoint)
            ));
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
            assert!(matches!(
                connect(&TrustedSocket {
                    socket_path: &path,
                    owner_uid: uid.wrapping_add(1),
                    access_group_gid: None,
                }),
                Err(LocalTransportError::InvalidEndpoint)
            ));
            drop(listener);
            fs::remove_file(path).unwrap();
            fs::remove_dir(dir).unwrap();
        }
        #[test]
        fn invalid_unix_path_and_symlinked_socket_fail_closed() {
            let (dir, path, uid) = setup();
            let listener = UnixListener::bind(&path).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
            let link = dir.join("alias.sock");
            std::os::unix::fs::symlink(&path, &link).unwrap();
            assert!(matches!(
                connect(&TrustedSocket {
                    socket_path: &link,
                    owner_uid: uid,
                    access_group_gid: None,
                }),
                Err(LocalTransportError::InvalidEndpoint)
            ));
            assert!(matches!(
                connect(&TrustedSocket {
                    socket_path: Path::new("../core.sock"),
                    owner_uid: uid,
                    access_group_gid: None,
                }),
                Err(LocalTransportError::InvalidEndpoint)
            ));
            fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
            assert!(matches!(
                connect(&TrustedSocket {
                    socket_path: &path,
                    owner_uid: uid,
                    access_group_gid: None,
                }),
                Err(LocalTransportError::InvalidEndpoint)
            ));
            drop(listener);
            fs::remove_file(link).unwrap();
            fs::remove_file(path).unwrap();
            fs::remove_dir(dir).unwrap();
        }
        /// Real Linux kernel filesystem + SO_PEERCRED admission in an installed
        /// root-owned service/group shape. `0700/0600` cannot be crossed by an
        /// unprivileged IDE user, so that old policy remains private-only.
        #[test]
        fn installed_group_endpoint_requires_exact_os_owner_group_and_modes() {
            use std::os::unix::fs::MetadataExt;
            let (dir, socket, uid) = setup();
            let gid = fs::metadata(&dir).unwrap().gid();
            let listener = UnixListener::bind(&socket).unwrap();
            fs::set_permissions(&dir, fs::Permissions::from_mode(0o750)).unwrap();
            fs::set_permissions(&socket, fs::Permissions::from_mode(0o660)).unwrap();
            let trusted = TrustedSocket {
                socket_path: &socket,
                owner_uid: uid,
                access_group_gid: Some(gid),
            };
            let client = connect(&trusted).unwrap();
            let (accepted, _) = listener.accept().unwrap();
            assert!(accepted.peer_addr().is_ok());
            drop((client, accepted));
            assert!(matches!(
                connect(&TrustedSocket {
                    access_group_gid: Some(gid.wrapping_add(1)),
                    ..trusted
                }),
                Err(LocalTransportError::InvalidEndpoint)
            ));
            fs::set_permissions(&socket, fs::Permissions::from_mode(0o666)).unwrap();
            assert!(matches!(
                connect(&trusted),
                Err(LocalTransportError::InvalidEndpoint)
            ));
            fs::set_permissions(&socket, fs::Permissions::from_mode(0o660)).unwrap();
            fs::set_permissions(&dir, fs::Permissions::from_mode(0o770)).unwrap();
            assert!(matches!(
                connect(&trusted),
                Err(LocalTransportError::InvalidEndpoint)
            ));
            drop(listener);
            fs::remove_file(socket).unwrap();
            fs::remove_dir(dir).unwrap();
        }

        #[test]
        fn expired_total_deadline_refuses_before_a_socket_read() {
            let (client, _server) = UnixStream::pair().unwrap();
            let mut deadline_stream = DeadlineStream {
                inner: client,
                expires: Instant::now() - Duration::from_millis(1),
                active: &|| true,
            };
            let mut byte = [0_u8; 1];
            assert_eq!(
                deadline_stream.read(&mut byte).unwrap_err().kind(),
                io::ErrorKind::TimedOut
            );
        }

        #[test]
        fn symlinked_parent_directory_cannot_alias_the_core_endpoint() {
            let (dir, path, uid) = setup();
            let listener = UnixListener::bind(&path).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
            let alias = dir.with_extension("alias");
            std::os::unix::fs::symlink(&dir, &alias).unwrap();
            // Keep the joined path alive across the borrowed socket witness.
            let alias_socket = alias.join("core.sock");
            let endpoint = TrustedSocket {
                socket_path: &alias_socket,
                owner_uid: uid,
                access_group_gid: None,
            };
            assert!(matches!(
                connect(&endpoint),
                Err(LocalTransportError::InvalidEndpoint)
            ));
            drop(listener);
            fs::remove_file(alias).unwrap();
            fs::remove_file(path).unwrap();
            fs::remove_dir(dir).unwrap();
        }

        #[test]
        fn absent_enrollment_does_not_connect_to_real_listener() {
            let (dir, path, uid) = setup();
            let listener = UnixListener::bind(&path).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
            listener.set_nonblocking(true).unwrap();
            let zero = [0u8; 32];
            let enrolled = NativeEnrollment {
                package_sha256: &zero,
                credential: &zero,
            };
            let result = probe_health(
                &TrustedSocket {
                    socket_path: &path,
                    owner_uid: uid,
                    access_group_gid: None,
                },
                &enrolled,
            );
            assert!(matches!(
                result,
                Err(LocalTransportError::Probe(ProbeError::Unauthorized))
            ));
            assert!(listener.accept().is_err());
            drop(listener);
            fs::remove_file(path).unwrap();
            fs::remove_dir(dir).unwrap();
        }
    }
}

#[cfg(target_os = "linux")]
pub(crate) use linux::{connect, probe_health, probe_health_with_guard, TrustedSocket};

// Windows pipe admission remains deliberately closed until native broker
// supplies a verified first-party, signed Core pipe/server identity, bounded
// I/O deadlines and protected per-package enrollment. `std::fs::File::open`
// on a caller-chosen \\.\pipe name would NOT meet those conditions; never
// silently substitute it for the host's Windows credential/OS attestor.
