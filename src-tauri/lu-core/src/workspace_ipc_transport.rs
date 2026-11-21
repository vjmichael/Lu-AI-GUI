//! LU-IPC-01D-R1: bounded Lu Protobuf over OS-local byte streams.
//! This is a transport and cryptographic handshake building block, NOT a
//! production peer admission service or a replacement for ConversationAuthority.
//! All model/tool calls must still use the trusted Lu host's original policy,
//! durable journal, per-turn grants and independent domain-owner effect checks.
use crate::workspace_ipc::{
    decode_frame, encode_frame, proto, WorkspaceWireError,
    MAX_FRAME_BYTES, WIRE_MAJOR, WIRE_MINOR,
};
use prost::Message;
use sha2::{Digest, Sha256};
use std::{io, time::Duration};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    time::timeout,
};

pub const FRAME_HEADER_BYTES: usize = 4;
pub const IO_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, PartialEq, Eq)]
pub enum TransportError {
    Io(io::ErrorKind),
    Timeout,
    InvalidFrameLength,
    Wire(WorkspaceWireError),
    Protocol(&'static str),
    PeerNotVerified,
    IdentityMismatch,
    MacMismatch,
}
impl From<io::Error> for TransportError {
    fn from(e: io::Error) -> Self { Self::Io(e.kind()) }
}
impl From<WorkspaceWireError> for TransportError {
    fn from(e: WorkspaceWireError) -> Self { Self::Wire(e) }
}

/// Bounded big-endian-u32 length framing. No frame is allocated until its
/// header passes validation; no zero length, unlimited reads or JSON fallback.
/// Stream processing/recovery backpressure remains the higher Lu layer's job.
pub async fn read_frame<S: AsyncRead + Unpin>(
    stream: &mut S,
) -> Result<proto::WorkspaceFrame, TransportError> {
    let data = timeout(IO_TIMEOUT, async {
        let mut header = [0u8; FRAME_HEADER_BYTES];
        stream.read_exact(&mut header).await?;
        let length = u32::from_be_bytes(header) as usize;
        if length == 0 || length > MAX_FRAME_BYTES {
            return Err(TransportError::InvalidFrameLength);
        }
        let mut body = vec![0u8; length];
        stream.read_exact(&mut body).await?;
        Ok::<Vec<u8>, TransportError>(body)
    }).await.map_err(|_| TransportError::Timeout)??;
    decode_frame(&data).map_err(Into::into)
}
pub async fn write_frame<S: AsyncWrite + Unpin>(
    stream: &mut S, frame: &proto::WorkspaceFrame,
) -> Result<(), TransportError> {
    let data = encode_frame(frame)?;
    if data.is_empty() || data.len() > MAX_FRAME_BYTES {
        return Err(TransportError::InvalidFrameLength);
    }
    timeout(IO_TIMEOUT, async {
        stream.write_all(&(data.len() as u32).to_be_bytes()).await?;
        stream.write_all(&data).await?;
        stream.flush().await?;
        Ok::<(), io::Error>(())
    }).await.map_err(|_| TransportError::Timeout)??;
    Ok(())
}

/// Kernel identity data ONLY. Process/user identity is not a registered,
/// verified package. On Windows, a separate trusted OS token/ACL attestor is
/// required: a raw named pipe alone never produces a verified peer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerEvidence {
    Linux { uid: u32, gid: u32, pid: u32 },
    WindowsUnverified,
}

/// Returned exclusively by a trusted native Core admission policy which has
/// independently verified the kernel peer, enrolled module/package, account,
/// and key provenance. The Hello's package digest never proves any of these.
/// No default allow policy is provided by this transport library.
pub struct EnrolledPeer {
    pub module_id: String,
    pub role: i32,
    pub package_sha256: [u8; 32],
    pub handshake_key: [u8; 32],
}
pub trait PeerAdmission {
    fn admit(&self, peer: &PeerEvidence, hello: &proto::Hello)
        -> Result<EnrolledPeer, TransportError>;
}

/// Host-only values from a CSPRNG and durable session/epoch authority.
/// This library NEVER chooses a predictable nonce or session identifier.
pub struct ServerIdentity {
    pub server_nonce: [u8; 32],
    pub session_ref: String,
    pub core_epoch_ref: String,
}
fn reject_weak_identity(identity: &ServerIdentity) -> Result<(), TransportError> {
    if identity.server_nonce.iter().all(|byte| *byte == 0)
        || identity.session_ref.is_empty()
        || identity.core_epoch_ref.is_empty()
        || identity.session_ref.len() > 128
        || identity.core_epoch_ref.len() > 128
    {
        return Err(TransportError::Protocol("weak_server_identity"));
    }
    Ok(())
}

/// RFC 2104 HMAC-SHA256 over a domain-separated canonical transcript.
/// Uses the existing sha2 dependency; compare only in constant time.
fn hmac(key: &[u8; 32], parts: &[&[u8]]) -> [u8; 32] {
    let mut inner_pad = [0x36u8; 64];
    let mut outer_pad = [0x5cu8; 64];
    for i in 0..32 {
        inner_pad[i] ^= key[i];
        outer_pad[i] ^= key[i];
    }
    let mut inner = Sha256::new();
    inner.update(inner_pad);
    for part in parts { inner.update(part); }
    let inner_hash = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(outer_pad);
    outer.update(inner_hash);
    outer.finalize().into()
}
fn fixed_equal(expected: &[u8; 32], got: &[u8]) -> bool {
    if got.len() != 32 { return false; }
    let mut difference = 0u8;
    for i in 0..32 { difference |= expected[i] ^ got[i]; }
    difference == 0
}
fn client_mac(key: &[u8; 32], hello_wire: &[u8], nonce: &[u8]) -> [u8; 32] {
    hmac(key, &[b"lu.workspace.client.v0\0", hello_wire, nonce])
}
fn server_mac(
    key: &[u8; 32], hello_wire: &[u8], nonce: &[u8],
    session: &str, welcome_without_mac: &proto::Welcome,
) -> [u8; 32] {
    let welcome = welcome_without_mac.encode_to_vec();
    hmac(key, &[
        b"lu.workspace.server.v0\0", hello_wire, nonce,
        session.as_bytes(), &welcome,
    ])
}
fn envelope(
    request_id: String, session_ref: String, body: proto::workspace_frame::Body,
) -> proto::WorkspaceFrame {
    proto::WorkspaceFrame {
        protocol_major: WIRE_MAJOR, protocol_minor: WIRE_MINOR,
        request_id, session_ref, sequence: 0, body: Some(body),
    }
}

/// Admission is intentionally separate from bytes: MAC possession AND
/// independently authenticated OS/package provenance are required.
/// This does not authorize a model session, a tool, a project or an effect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportSession {
    pub session_ref: String,
    pub module_id: String,
    pub core_epoch_ref: String,
}
impl TransportSession {
    /// A post-handshake request must match this host-minted session. This
    /// does NOT check user-turn/task grants (trusted host must do so per call).
    pub fn check_request(&self, frame: &proto::WorkspaceFrame)
        -> Result<(), TransportError> {
        crate::workspace_ipc::validate(frame)?;
        if frame.session_ref != self.session_ref {
            return Err(TransportError::IdentityMismatch);
        }
        if matches!(frame.body.as_ref(),
            Some(proto::workspace_frame::Body::Hello(_))
            | Some(proto::workspace_frame::Body::AuthChallenge(_))
            | Some(proto::workspace_frame::Body::AuthProof(_))
            | Some(proto::workspace_frame::Body::Welcome(_)))
        {
            return Err(TransportError::Protocol("handshake_after_admission"));
        }
        Ok(())
    }
}
pub async fn server_handshake<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S,
    peer: &PeerEvidence,
    admission: &dyn PeerAdmission,
    identity: &ServerIdentity,
) -> Result<TransportSession, TransportError> {
    reject_weak_identity(identity)?;
    // Until a Win32 token/ACL verifier exists, refuse external Windows clients.
    if *peer == PeerEvidence::WindowsUnverified {
        return Err(TransportError::PeerNotVerified);
    }
    let frame = read_frame(stream).await?;
    let hello = match frame.body.as_ref() {
        Some(proto::workspace_frame::Body::Hello(v)) => v,
        _ => return Err(TransportError::Protocol("expected_hello")),
    };
    let enrolled = admission.admit(peer, hello)?;
    if enrolled.handshake_key.iter().all(|b| *b==0) {
        return Err(TransportError::PeerNotVerified);
    }
    if enrolled.module_id != hello.claimed_module_id
        || enrolled.role != hello.claimed_role
        || enrolled.package_sha256.as_slice() != hello.claimed_package_sha256.as_slice()
    {
        return Err(TransportError::IdentityMismatch);
    }
    let hello_wire = encode_frame(&frame)?;
    write_frame(stream, &envelope(
        frame.request_id.clone(), String::new(),
        proto::workspace_frame::Body::AuthChallenge(proto::AuthChallenge {
            server_nonce: identity.server_nonce.to_vec(),
        }),
    )).await?;
    let proof = read_frame(stream).await?;
    if proof.request_id != frame.request_id {
        return Err(TransportError::IdentityMismatch);
    }
    let mac = match proof.body {
        Some(proto::workspace_frame::Body::AuthProof(v)) => v.client_hmac_sha256,
        _ => return Err(TransportError::Protocol("expected_auth_proof")),
    };
    let expected = client_mac(&enrolled.handshake_key, &hello_wire, &identity.server_nonce);
    if !fixed_equal(&expected, &mac) {
        return Err(TransportError::MacMismatch);
    }
    let mut welcome = proto::Welcome {
        selected_major: WIRE_MAJOR, selected_minor: WIRE_MINOR,
        core_epoch_ref: identity.core_epoch_ref.clone(),
        enabled_features: vec![], server_hmac_sha256: vec![],
    };
    welcome.server_hmac_sha256 = server_mac(
        &enrolled.handshake_key, &hello_wire, &identity.server_nonce,
        &identity.session_ref, &welcome,
    ).to_vec();
    write_frame(stream, &envelope(
        frame.request_id, identity.session_ref.clone(),
        proto::workspace_frame::Body::Welcome(welcome),
    )).await?;
    Ok(TransportSession {
        session_ref: identity.session_ref.clone(),
        module_id: enrolled.module_id,
        core_epoch_ref: identity.core_epoch_ref.clone(),
    })
}

/// Client authentication proves the trusted, enrolled Core key rather than
/// trusting the pipe/socket name or an unsolicited Welcome. Key must arrive
/// from the local credential store, never from a model, prompt or IPC frame.
pub async fn client_handshake<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S, hello: proto::WorkspaceFrame, enrolled_key: &[u8; 32],
) -> Result<TransportSession, TransportError> {
    if enrolled_key.iter().all(|b| *b==0) {
        return Err(TransportError::PeerNotVerified);
    }
    if !matches!(hello.body.as_ref(), Some(proto::workspace_frame::Body::Hello(_))) {
        return Err(TransportError::Protocol("client_requires_hello"));
    }
    let hello_wire = encode_frame(&hello)?;
    write_frame(stream, &hello).await?;
    let challenge = read_frame(stream).await?;
    if challenge.request_id != hello.request_id {
        return Err(TransportError::IdentityMismatch);
    }
    let nonce = match challenge.body {
        Some(proto::workspace_frame::Body::AuthChallenge(v)) => v.server_nonce,
        _ => return Err(TransportError::Protocol("expected_challenge")),
    };
    let proof = client_mac(enrolled_key, &hello_wire, &nonce);
    write_frame(stream, &envelope(
        hello.request_id.clone(), String::new(),
        proto::workspace_frame::Body::AuthProof(proto::AuthProof {
            client_hmac_sha256: proof.to_vec(),
        }),
    )).await?;
    let reply = read_frame(stream).await?;
    if reply.request_id != hello.request_id {
        return Err(TransportError::IdentityMismatch);
    }
    let welcome = match reply.body {
        Some(proto::workspace_frame::Body::Welcome(v)) => v,
        _ => return Err(TransportError::Protocol("expected_welcome")),
    };
    let actual = welcome.server_hmac_sha256.clone();
    let mut unsigned = welcome.clone();
    unsigned.server_hmac_sha256.clear();
    let expected = server_mac(enrolled_key, &hello_wire, &nonce, &reply.session_ref, &unsigned);
    if !fixed_equal(&expected, &actual) {
        return Err(TransportError::MacMismatch);
    }
    let hello_claim = match &hello.body {
        Some(proto::workspace_frame::Body::Hello(v)) => v.claimed_module_id.clone(),
        _ => return Err(TransportError::Protocol("client_hello_lost")),
    };
    Ok(TransportSession {
        session_ref: reply.session_ref,
        module_id: hello_claim,
        core_epoch_ref: welcome.core_epoch_ref,
    })
}

/// Linux: socket is bound only under an existing, real, user-owned 0700 dir.
/// Never unlink an existing path to "recover" a listener; stale endpoints
/// need a separate owner+inode-checked recovery transaction.
#[cfg(target_os = "linux")]
pub mod linux {
    use super::*;
    use std::{
        os::unix::fs::{MetadataExt, PermissionsExt},
        path::Path,
    };
    use tokio::net::{UnixListener, UnixStream};

    pub fn bind_socket(path: &Path, expected_owner_uid: u32)
        -> Result<UnixListener, TransportError> {
        let dir = path.parent().ok_or(TransportError::Protocol("missing_private_dir"))?;
        let metadata = std::fs::symlink_metadata(dir)?;
        if !metadata.file_type().is_dir()
            || metadata.file_type().is_symlink()
            || metadata.uid() != expected_owner_uid
            || metadata.permissions().mode() & 0o077 != 0 {
            return Err(TransportError::PeerNotVerified);
        }
        if path.exists() || std::fs::symlink_metadata(path).is_ok() {
            return Err(TransportError::Protocol("endpoint_already_exists"));
        }
        let listener = UnixListener::bind(path)?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        Ok(listener)
    }
    pub async fn accept_peer(listener: &UnixListener)
        -> Result<(UnixStream, PeerEvidence), TransportError> {
        let (stream, _) = listener.accept().await?;
        let cred = stream.peer_cred()?;
        let pid = cred.pid().ok_or(TransportError::PeerNotVerified)?;
        Ok((stream, PeerEvidence::Linux {
            uid: cred.uid(), gid: cred.gid(), pid: pid as u32,
        }))
    }
    pub async fn connect(path: &Path) -> Result<UnixStream, TransportError> {
        Ok(UnixStream::connect(path).await?)
    }
}

/// Windows: a real local named-pipe byte-stream carrier. The *only* exposed
/// peer evidence is WindowsUnverified. No Core dispatch is possible until a
/// separate OS-native impersonated-token, SID, package and endpoint DACL
/// attestor is integrated and installed under an approved service identity.
/// PIPE_REJECT_REMOTE_CLIENTS and FIRST_PIPE_INSTANCE mitigate exposure and
/// squatting, but they do not establish an application's authenticated ID.
#[cfg(windows)]
pub mod windows {
    use super::*;
    use tokio::net::windows::named_pipe::{
        ClientOptions, NamedPipeClient, NamedPipeServer, PipeMode, ServerOptions,
    };
    const PREFIX: &str = r"\\.\pipe\lu-ipc-";
    pub fn check_pipe_name(name: &str) -> Result<(), TransportError> {
        let suffix = name.strip_prefix(PREFIX)
            .ok_or(TransportError::Protocol("pipe_name"))?;
        if suffix.len() != 32 || !suffix.bytes().all(|b|
            b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        {
            return Err(TransportError::Protocol("pipe_name"));
        }
        Ok(())
    }
    pub fn bind_one(name: &str) -> Result<NamedPipeServer, TransportError> {
        check_pipe_name(name)?;
        let pipe = ServerOptions::new()
            .pipe_mode(PipeMode::Byte)
            .first_pipe_instance(true)
            .reject_remote_clients(true)
            .max_instances(1)
            .create(name)?;
        Ok(pipe)
    }
    pub async fn accept_peer(
        server: &NamedPipeServer,
    ) -> Result<PeerEvidence, TransportError> {
        server.connect().await?;
        Ok(PeerEvidence::WindowsUnverified)
    }
    pub fn connect(name: &str) -> Result<NamedPipeClient, TransportError> {
        check_pipe_name(name)?;
        // Tokio's named pipe client defaults to SECURITY_IDENTIFICATION.
        // The mutual HMAC handshake additionally detects forged servers.
        Ok(ClientOptions::new().open(name)?)
    }
}

#[cfg(test)]
mod internal_tests {
    use super::*;

    #[test]
    fn hmac_sha256_matches_independent_reference_vector() {
        // Python hashlib/hmac independent reference:
        // key=00..1f; transcript=domain + "hello-wire-v0" + a5*32.
        let mut key=[0u8;32];
        for (i,b) in key.iter_mut().enumerate(){*b=i as u8;}
        let out=client_mac(&key,b"hello-wire-v0",&[0xa5;32]);
        let expected=[
            0xd9,0x53,0x9f,0xaf,0x84,0xe0,0x89,0xe6,
            0x49,0x7a,0xaf,0x77,0x86,0x42,0x9a,0xca,
            0x6b,0x08,0xf2,0xf5,0xdd,0x41,0x52,0x79,
            0x0c,0xcf,0xef,0x6a,0xe5,0x47,0xe7,0x88,
        ];
        assert_eq!(out,expected);
        assert!(fixed_equal(&expected,&out));
        let mut incorrect=out;
        incorrect[31]^=1;
        assert!(!fixed_equal(&expected,&incorrect));
        assert!(!fixed_equal(&expected,&[]));
    }
}
