//! IDE-CORE-IPC-01A-R1: audited, bounded *read-only* Lu Core health probe.
//!
//! Core owns the Protobuf protocol, per-turn authorization, single model,
//! session journal and OS peer admission. This module owns NONE of those.
//! It has no Tauri/WebView command and cannot register or invoke a capability.
//! A trusted installed IDE transport owner must provide a connected local
//! named pipe or Unix-domain socket, configure OS I/O deadlines, authenticate
//! the Core endpoint and obtain a package-bound key from protected custody.
//! Bytes are bounded here; a generic Read+Write cannot enforce wall timeouts.
use prost::Message;
use rand::{rngs::OsRng, RngCore};
use sha2::{Digest, Sha256};
use std::io::{self, Read, Write};

pub(crate) mod proto {
    include!(concat!(env!("OUT_DIR"), "/lu.workspace.v0.rs"));
}
use proto::workspace_frame::Body;

const MAX_FRAME_BYTES: usize = 256 * 1024;
const MAJOR: u32 = 1;
const MINOR: u32 = 0;
const IDE_MODULE: &str = "lu.ide";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProbeError {
    Io(io::ErrorKind),
    InvalidFrame,
    Noncanonical,
    Version,
    Identity,
    Unauthorized,
    UnexpectedMessage,
    BadMac,
    InvalidReceipt,
}
impl From<io::Error> for ProbeError {
    fn from(error: io::Error) -> Self {
        Self::Io(error.kind())
    }
}

/// Caller must originate in the privileged native enrollment/credential
/// broker, never JS, a project file, a model prompt or an IPC response.
/// No default constructor or hard-coded production secret exists.
pub(crate) struct NativeEnrollment<'a> {
    pub package_sha256: &'a [u8; 32],
    pub credential: &'a [u8; 32],
}

fn name_ok(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, b'-' | b'_' | b'.' | b':' | b'/'))
}
fn frame(request_id: String, session_ref: String, body: Body) -> proto::WorkspaceFrame {
    proto::WorkspaceFrame {
        protocol_major: MAJOR,
        protocol_minor: MINOR,
        request_id,
        session_ref,
        sequence: 0,
        body: Some(body),
    }
}
fn write_frame<W: Write>(io: &mut W, message: &proto::WorkspaceFrame) -> Result<(), ProbeError> {
    let data = message.encode_to_vec();
    if data.is_empty() || data.len() > MAX_FRAME_BYTES {
        return Err(ProbeError::InvalidFrame);
    }
    io.write_all(&(data.len() as u32).to_be_bytes())?;
    io.write_all(&data)?;
    io.flush()?;
    Ok(())
}
fn read_frame<R: Read>(io: &mut R) -> Result<proto::WorkspaceFrame, ProbeError> {
    let mut header = [0u8; 4];
    io.read_exact(&mut header)?;
    let length = u32::from_be_bytes(header) as usize;
    if length == 0 || length > MAX_FRAME_BYTES {
        return Err(ProbeError::InvalidFrame);
    }
    let mut data = vec![0u8; length];
    io.read_exact(&mut data)?;
    let decoded =
        proto::WorkspaceFrame::decode(data.as_slice()).map_err(|_| ProbeError::InvalidFrame)?;
    // Core uses a full-schema canonical check. Our audited projection rejects
    // unknown tags, duplicate fields and malleable wire forms rather than
    // silently dropping fields outside this one read-only exchange.
    if decoded.encode_to_vec() != data {
        return Err(ProbeError::Noncanonical);
    }
    if decoded.protocol_major != MAJOR || decoded.protocol_minor != MINOR {
        return Err(ProbeError::Version);
    }
    if !name_ok(&decoded.request_id) || decoded.sequence != 0 {
        return Err(ProbeError::InvalidFrame);
    }
    Ok(decoded)
}
fn mac(key: &[u8; 32], pieces: &[&[u8]]) -> [u8; 32] {
    let mut inside = [0x36u8; 64];
    let mut outside = [0x5cu8; 64];
    for n in 0..32 {
        inside[n] ^= key[n];
        outside[n] ^= key[n];
    }
    let mut hash = Sha256::new();
    hash.update(inside);
    for piece in pieces {
        hash.update(piece);
    }
    let inner = hash.finalize();
    let mut outer_hash = Sha256::new();
    outer_hash.update(outside);
    outer_hash.update(inner);
    outer_hash.finalize().into()
}
fn same_mac(expected: &[u8; 32], observed: &[u8]) -> bool {
    if observed.len() != 32 {
        return false;
    }
    let mut diff = 0;
    for (a, b) in expected.iter().zip(observed) {
        diff |= a ^ b;
    }
    diff == 0
}
fn ticket() -> String {
    let mut bytes = [0u8; 16];
    OsRng.fill_bytes(&mut bytes);
    let mut value = String::from("ide-ipc-");
    for byte in bytes {
        value.push_str(&format!("{byte:02x}"));
    }
    value
}

/// Exactly one authenticated, read-only HealthRequest, with no tool invocation.
/// Uses the same canonical handshake transcript as Core's client_handshake.
/// `io` must already be independently approved and deadline-configured by
/// the native transport owner. A synthetic fixture can exercise the codec but
/// is never evidence that a signed installed IDE process was admitted.
pub(crate) fn health_probe<T: Read + Write>(
    io: &mut T,
    enrolled: &NativeEnrollment<'_>,
) -> Result<proto::HealthReport, ProbeError> {
    if enrolled.package_sha256.iter().all(|x| *x == 0)
        || enrolled.credential.iter().all(|x| *x == 0)
    {
        return Err(ProbeError::Unauthorized);
    }
    let mut nonce = [0u8; 32];
    OsRng.fill_bytes(&mut nonce);
    let hello_id = ticket();
    let hello = frame(
        hello_id.clone(),
        String::new(),
        Body::Hello(proto::Hello {
            claimed_module_id: IDE_MODULE.into(),
            claimed_role: proto::ModuleRole::Workspace as i32,
            claimed_package_sha256: enrolled.package_sha256.to_vec(),
            minimum_major: MAJOR,
            maximum_major: MAJOR,
            maximum_minor: MINOR,
            requested_features: vec![],
            client_nonce: nonce.to_vec(),
        }),
    );
    let original_hello_wire = hello.encode_to_vec();
    write_frame(io, &hello)?;
    let challenge = read_frame(io)?;
    if challenge.request_id != hello_id || !challenge.session_ref.is_empty() {
        return Err(ProbeError::Identity);
    }
    let server_nonce = match challenge.body {
        Some(Body::AuthChallenge(value))
            if value.server_nonce.len() == 32
                && value.server_nonce.iter().any(|byte| *byte != 0) =>
        {
            value.server_nonce
        }
        _ => return Err(ProbeError::UnexpectedMessage),
    };
    let proof = mac(
        enrolled.credential,
        &[
            b"lu.workspace.client.v0\0",
            &original_hello_wire,
            &server_nonce,
        ],
    );
    write_frame(
        io,
        &frame(
            hello_id.clone(),
            String::new(),
            Body::AuthProof(proto::AuthProof {
                client_hmac_sha256: proof.to_vec(),
            }),
        ),
    )?;
    let response = read_frame(io)?;
    if response.request_id != hello_id || !name_ok(&response.session_ref) {
        return Err(ProbeError::Identity);
    }
    let session = response.session_ref;
    let welcome = match response.body {
        Some(Body::Welcome(value)) => value,
        _ => return Err(ProbeError::UnexpectedMessage),
    };
    if welcome.selected_major != MAJOR
        || welcome.selected_minor != MINOR
        || !name_ok(&welcome.core_epoch_ref)
    {
        return Err(ProbeError::Version);
    }
    let mut unsigned = welcome.clone();
    unsigned.server_hmac_sha256.clear();
    let welcome_wire = unsigned.encode_to_vec();
    let expected = mac(
        enrolled.credential,
        &[
            b"lu.workspace.server.v0\0",
            &original_hello_wire,
            &server_nonce,
            session.as_bytes(),
            &welcome_wire,
        ],
    );
    if !same_mac(&expected, &welcome.server_hmac_sha256) {
        return Err(ProbeError::BadMac);
    }
    // Reviewed against Core workspace_ipc_transport.rs at
    // bfbffc35320c78e74d52b3918cd587356fffb3a3.  Its server never
    // negotiates any feature for the lu.ide module.  A signed Welcome with
    // unexpected capabilities is not consent to expand this Health-only path.
    if !welcome.enabled_features.is_empty() {
        return Err(ProbeError::UnexpectedMessage);
    }
    let health_id = ticket();
    write_frame(
        io,
        &frame(
            health_id.clone(),
            session.clone(),
            Body::Health(proto::HealthRequest {}),
        ),
    )?;
    let receipt = read_frame(io)?;
    if receipt.request_id != health_id || receipt.session_ref != session {
        return Err(ProbeError::Identity);
    }
    let health = match receipt.body {
        Some(Body::HealthReport(value)) => value,
        Some(Body::Failure(_)) => return Err(ProbeError::Unauthorized),
        _ => return Err(ProbeError::UnexpectedMessage),
    };
    if health.module_id != "lu.core"
        || !name_ok(&health.status_code)
        || health.status_code.len() > 96
        || health.epoch == 0
    {
        return Err(ProbeError::InvalidReceipt);
    }
    // A valid server MAC and exact lu.core module identity still grant no effects.
    // `connected == false` must be returned honestly to the caller; no
    // user-visible success or execution grant is inferred from HealthReport.
    Ok(health)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn framing_refuses_zero_and_overlarge_before_allocating() {
        for size in [0u32, (MAX_FRAME_BYTES + 1) as u32] {
            let mut data = Cursor::new(size.to_be_bytes().to_vec());
            assert_eq!(read_frame(&mut data), Err(ProbeError::InvalidFrame));
        }
    }

    #[test]
    fn noncanonical_unknown_wire_tags_are_rejected() {
        let msg = frame(
            "health-1".into(),
            "session-1".into(),
            Body::Health(proto::HealthRequest {}),
        );
        let mut raw = msg.encode_to_vec();
        // Unknown field #100, varint 1; full Core validation rejects it.
        raw.extend_from_slice(&[0xa0, 0x06, 0x01]);
        let mut framed = (raw.len() as u32).to_be_bytes().to_vec();
        framed.extend_from_slice(&raw);
        assert_eq!(
            read_frame(&mut Cursor::new(framed)),
            Err(ProbeError::Noncanonical)
        );
        assert_eq!(
            read_frame(&mut Cursor::new(vec![0, 0, 0, 10, 1, 2])),
            Err(ProbeError::Io(io::ErrorKind::UnexpectedEof))
        );
    }

    #[test]
    fn readonly_health_frame_encodes_exact_core_v0_tags() {
        let msg = frame(
            "health-1".into(),
            "session-1".into(),
            Body::Health(proto::HealthRequest {}),
        );
        let mut bytes = Vec::new();
        write_frame(&mut bytes, &msg).unwrap();
        let decoded = read_frame(&mut Cursor::new(bytes)).unwrap();
        assert!(matches!(decoded.body, Some(Body::Health(_))));
    }

    #[test]
    fn missing_enrollment_fails_before_any_bytes_are_emitted() {
        let mut output = Vec::new();
        let none = [0u8; 32];
        let material = NativeEnrollment {
            package_sha256: &none,
            credential: &none,
        };
        assert_eq!(
            health_probe(&mut Cursor::new(&mut output), &material),
            Err(ProbeError::Unauthorized)
        );
        assert!(output.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn synthetic_unix_stream_performs_one_authenticated_health_exchange() {
        use std::{os::unix::net::UnixStream, thread, time::Duration};
        let (mut ide, mut core) = UnixStream::pair().unwrap();
        for side in [&ide, &core] {
            side.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
            side.set_write_timeout(Some(Duration::from_secs(3)))
                .unwrap();
        }
        let key = [0x4cu8; 32];
        let package = [0x5du8; 32];
        let worker = thread::spawn(move || {
            let hello = read_frame(&mut core).unwrap();
            let original = hello.encode_to_vec();
            assert!(matches!(hello.body, Some(Body::Hello(_))));
            let server_nonce = [0x33u8; 32];
            write_frame(
                &mut core,
                &frame(
                    hello.request_id.clone(),
                    String::new(),
                    Body::AuthChallenge(proto::AuthChallenge {
                        server_nonce: server_nonce.to_vec(),
                    }),
                ),
            )
            .unwrap();
            let proof = read_frame(&mut core).unwrap();
            let Some(Body::AuthProof(got)) = proof.body else {
                panic!("expected proof")
            };
            let expected = mac(
                &key,
                &[b"lu.workspace.client.v0\0", &original, &server_nonce],
            );
            assert!(same_mac(&expected, &got.client_hmac_sha256));
            let session = "session-protected".to_string();
            let mut welcome = proto::Welcome {
                selected_major: MAJOR,
                selected_minor: MINOR,
                core_epoch_ref: "core-epoch-one".into(),
                enabled_features: vec![],
                server_hmac_sha256: vec![],
            };
            let unsigned = welcome.encode_to_vec();
            welcome.server_hmac_sha256 = mac(
                &key,
                &[
                    b"lu.workspace.server.v0\0",
                    &original,
                    &server_nonce,
                    session.as_bytes(),
                    &unsigned,
                ],
            )
            .to_vec();
            write_frame(
                &mut core,
                &frame(hello.request_id, session.clone(), Body::Welcome(welcome)),
            )
            .unwrap();
            let request = read_frame(&mut core).unwrap();
            assert_eq!(request.session_ref, session);
            assert!(matches!(request.body, Some(Body::Health(_))));
            write_frame(
                &mut core,
                &frame(
                    request.request_id,
                    session,
                    Body::HealthReport(proto::HealthReport {
                        module_id: "lu.core".into(),
                        connected: true,
                        status_code: "ready".into(),
                        epoch: 1,
                    }),
                ),
            )
            .unwrap();
        });
        let material = NativeEnrollment {
            package_sha256: &package,
            credential: &key,
        };
        let report = health_probe(&mut ide, &material).unwrap();
        assert!(report.connected);
        assert_eq!(report.module_id, "lu.core");
        worker.join().unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn authenticated_unrequested_feature_is_refused_before_health() {
        use std::{io::Read, os::unix::net::UnixStream, thread, time::Duration};
        let (mut ide, mut core) = UnixStream::pair().unwrap();
        ide.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        core.set_read_timeout(Some(Duration::from_millis(250)))
            .unwrap();
        let key = [0x4cu8; 32];
        let package = [0x5du8; 32];
        let worker = thread::spawn(move || {
            let hello = read_frame(&mut core).unwrap();
            let original = hello.encode_to_vec();
            let server_nonce = [0x33u8; 32];
            write_frame(
                &mut core,
                &frame(
                    hello.request_id.clone(),
                    String::new(),
                    Body::AuthChallenge(proto::AuthChallenge {
                        server_nonce: server_nonce.to_vec(),
                    }),
                ),
            )
            .unwrap();
            let proof = read_frame(&mut core).unwrap();
            let Some(Body::AuthProof(got)) = proof.body else {
                panic!("expected AuthProof")
            };
            assert!(same_mac(
                &mac(
                    &key,
                    &[b"lu.workspace.client.v0\0", &original, &server_nonce]
                ),
                &got.client_hmac_sha256
            ));
            let session = "session-protected";
            let mut welcome = proto::Welcome {
                selected_major: MAJOR,
                selected_minor: MINOR,
                core_epoch_ref: "core-epoch-one".into(),
                enabled_features: vec!["gpu-lease-v1".into()],
                server_hmac_sha256: vec![],
            };
            let unsigned = welcome.encode_to_vec();
            welcome.server_hmac_sha256 = mac(
                &key,
                &[
                    b"lu.workspace.server.v0\0",
                    &original,
                    &server_nonce,
                    session.as_bytes(),
                    &unsigned,
                ],
            )
            .to_vec();
            write_frame(
                &mut core,
                &frame(hello.request_id, session.into(), Body::Welcome(welcome)),
            )
            .unwrap();
            // A validly signed *but unrequested* feature must not result in a
            // HealthRequest, capability request or any other IPC message.
            let mut one = [0u8; 1];
            match core.read(&mut one) {
                Ok(0) => (),
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    ()
                }
                other => panic!("unexpected post-Welcome client bytes: {other:?}"),
            }
        });
        let enrollment = NativeEnrollment {
            package_sha256: &package,
            credential: &key,
        };
        assert_eq!(
            health_probe(&mut ide, &enrollment),
            Err(ProbeError::UnexpectedMessage)
        );
        worker.join().unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn forged_server_welcome_fails_before_health_request() {
        use std::{os::unix::net::UnixStream, thread, time::Duration};
        let (mut ide, mut core) = UnixStream::pair().unwrap();
        core.set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();
        ide.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        let key = [0x4cu8; 32];
        let package = [0x5du8; 32];
        let worker = thread::spawn(move || {
            let hello = read_frame(&mut core).unwrap();
            write_frame(
                &mut core,
                &frame(
                    hello.request_id.clone(),
                    String::new(),
                    Body::AuthChallenge(proto::AuthChallenge {
                        server_nonce: vec![0x33u8; 32],
                    }),
                ),
            )
            .unwrap();
            let _proof = read_frame(&mut core).unwrap();
            write_frame(
                &mut core,
                &frame(
                    hello.request_id,
                    "session-protected".into(),
                    Body::Welcome(proto::Welcome {
                        selected_major: 1,
                        selected_minor: 0,
                        core_epoch_ref: "core-epoch-one".into(),
                        enabled_features: vec![],
                        server_hmac_sha256: vec![0x55u8; 32],
                    }),
                ),
            )
            .unwrap();
            // A forged server cannot induce an IDE health request.
            assert!(read_frame(&mut core).is_err());
        });
        let material = NativeEnrollment {
            package_sha256: &package,
            credential: &key,
        };
        assert_eq!(health_probe(&mut ide, &material), Err(ProbeError::BadMac));
        worker.join().unwrap();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn installed_boundary_fixture_runs_issuer_to_core_health_then_closes_one_use_journal() {
        use crate::core_ipc_issuer::{
            proto::{NativeHealthLeaseReply, NativeHealthLeaseRequest},
            InstalledIssuerAnchor, UnixCredentialIssuer,
        };
        use crate::core_ipc_lifecycle::{
            CoreHealthSession, HealthJournal, JournalState, LifecycleError, Terminal,
        };
        use std::{
            fs,
            os::unix::{
                fs::{MetadataExt, PermissionsExt},
                net::UnixListener,
            },
            thread,
            time::{Duration, SystemTime, UNIX_EPOCH},
        };
        let mut entropy = [0u8; 16];
        OsRng.fill_bytes(&mut entropy);
        let name: String = entropy.iter().map(|x| format!("{x:02x}")).collect();
        let root = std::env::temp_dir().join(format!("ide-core-end-to-end-01b-{name}"));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let uid = fs::metadata(&root).unwrap().uid();
        let issuer_dir = root.join("issuer");
        let core_dir = root.join("core");
        let journal_dir = root.join("journal");
        for dir in [&issuer_dir, &core_dir, &journal_dir] {
            fs::create_dir(dir).unwrap();
            fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let issuer_socket = issuer_dir.join("issue.sock");
        let core_socket = core_dir.join("core.sock");
        let issuer = UnixListener::bind(&issuer_socket).unwrap();
        let core = UnixListener::bind(&core_socket).unwrap();
        for socket in [&issuer_socket, &core_socket] {
            fs::set_permissions(socket, fs::Permissions::from_mode(0o600)).unwrap();
        }
        let key = [0x54u8; 32];
        let package = [0x75u8; 32];
        let core_side = thread::spawn(move || {
            let (mut stream, _) = core.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let hello = read_frame(&mut stream).unwrap();
            let hello_wire = hello.encode_to_vec();
            let Some(Body::Hello(client)) = hello.body else {
                panic!("unexpected Core Hello");
            };
            assert_eq!(client.claimed_package_sha256, package);
            let server_nonce = [0x29u8; 32];
            write_frame(
                &mut stream,
                &frame(
                    hello.request_id.clone(),
                    String::new(),
                    Body::AuthChallenge(proto::AuthChallenge {
                        server_nonce: server_nonce.to_vec(),
                    }),
                ),
            )
            .unwrap();
            let proof = read_frame(&mut stream).unwrap();
            let Some(Body::AuthProof(got)) = proof.body else {
                panic!("missing proof");
            };
            assert!(same_mac(
                &mac(
                    &key,
                    &[b"lu.workspace.client.v0\0", &hello_wire, &server_nonce]
                ),
                &got.client_hmac_sha256
            ));
            let session = "approved-native-fixture".to_owned();
            let mut welcome = proto::Welcome {
                selected_major: MAJOR,
                selected_minor: MINOR,
                core_epoch_ref: "native-fixture-epoch".into(),
                enabled_features: vec![],
                server_hmac_sha256: vec![],
            };
            let unsigned_wire = welcome.encode_to_vec();
            welcome.server_hmac_sha256 = mac(
                &key,
                &[
                    b"lu.workspace.server.v0\0",
                    &hello_wire,
                    &server_nonce,
                    session.as_bytes(),
                    &unsigned_wire,
                ],
            )
            .to_vec();
            write_frame(
                &mut stream,
                &frame(hello.request_id, session.clone(), Body::Welcome(welcome)),
            )
            .unwrap();
            let health = read_frame(&mut stream).unwrap();
            assert!(matches!(health.body, Some(Body::Health(_))));
            assert_eq!(health.session_ref, session);
            write_frame(
                &mut stream,
                &frame(
                    health.request_id,
                    session,
                    Body::HealthReport(proto::HealthReport {
                        module_id: "lu.core".into(),
                        connected: true,
                        status_code: "ready".into(),
                        epoch: 42,
                    }),
                ),
            )
            .unwrap();
        });
        let issuer_side = thread::spawn({
            let socket = core_socket.clone();
            move || {
                let (mut stream, _) = issuer.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut header = [0u8; 4];
                stream.read_exact(&mut header).unwrap();
                let len = u32::from_be_bytes(header) as usize;
                assert!(len <= 4096);
                let mut wire = vec![0u8; len];
                stream.read_exact(&mut wire).unwrap();
                let request = NativeHealthLeaseRequest::decode(wire.as_slice()).unwrap();
                assert_eq!(request.schema_version, 1);
                assert_eq!(request.scope, "core.health.readonly");
                assert_eq!(request.journal_operation_ref.len(), 32);
                assert_eq!(request.installed_package_sha256, package);
                let expires = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    + 15;
                let reply = NativeHealthLeaseReply {
                    schema_version: 1,
                    operation_nonce: request.operation_nonce,
                    installed_package_sha256: request.installed_package_sha256,
                    scope: request.scope,
                    core_socket_path: socket.to_str().unwrap().into(),
                    core_owner_uid: uid,
                    credential: key.to_vec(),
                    expires_unix_seconds: expires,
                    journal_operation_ref: request.journal_operation_ref,
                }
                .encode_to_vec();
                stream
                    .write_all(&(reply.len() as u32).to_be_bytes())
                    .unwrap();
                stream.write_all(&reply).unwrap();
            }
        });
        let broker = UnixCredentialIssuer::new(InstalledIssuerAnchor {
            issuer_socket: issuer_socket.clone(),
            issuer_uid: uid,
            issuer_group_gid: None,
            core_uid: uid,
            package_sha256: package,
        })
        .unwrap();
        let journal = HealthJournal::open(&journal_dir, uid).unwrap();
        let session = CoreHealthSession::new(journal);
        let observed = session.run_once(&broker).unwrap();
        assert_eq!(observed.state, Terminal::Observed);
        let report = observed.observation.unwrap();
        assert!(report.connected);
        assert_eq!(report.status_code, "ready");
        assert_eq!(report.epoch, 42);
        assert_eq!(
            session.run_once(&broker),
            Err(LifecycleError::AlreadyConsumed)
        );
        core_side.join().unwrap();
        issuer_side.join().unwrap();
        let journal = HealthJournal::open(&journal_dir, uid).unwrap();
        assert_eq!(
            journal.inspect(&observed.operation_id),
            Ok(JournalState::Final(Terminal::Observed))
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn hmac_transcript_matches_independent_known_vector() {
        let key = [7u8; 32];
        let observed = mac(&key, &[b"test", b"-", b"vector"]);
        assert_eq!(
            observed,
            [
                0x55, 0xb0, 0x6b, 0x90, 0xb6, 0x0d, 0x5b, 0x5c, 0x76, 0x43, 0x12, 0xad, 0x87, 0xda,
                0xf9, 0x83, 0xcc, 0xa1, 0x0d, 0x70, 0x9a, 0x00, 0x77, 0x0a, 0xa7, 0x82, 0x0f, 0xea,
                0xa9, 0x9f, 0xd7, 0xb8,
            ]
        );
    }
}
