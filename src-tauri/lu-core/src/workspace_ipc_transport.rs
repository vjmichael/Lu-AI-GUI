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
    WindowsObserved { pid:u32, session_id:u32, user_sid:String, image_path:String },
    // Only the native post-ReadFile Win32 token observer may supply this
    // production admission witness; package signing is a separate gate.
    WindowsMessageTokenObserved {
        pid:u32, session_id:u32, user_sid:String, image_path:String,
        creation_filetime:u64,
    },
    // Unforgeable outside this module: constructed only after an OS-owned
    // signed image handle and signer certificate pass exact host-owned pins.
    #[cfg(windows)]
    WindowsSignedPackage(WindowsSignedPackageWitness),
}

#[cfg(windows)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsSignedPackageWitness {
    process: lu_win_worker_job::ipc_pipe_peer::WindowsPipePeer,
    image_sha256: [u8; 32],
    signer_certificate_sha256: [u8; 32],
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
    // Raw process metadata must never trigger a challenge or an effect.
    if matches!(peer, PeerEvidence::WindowsUnverified |
        PeerEvidence::WindowsObserved { .. }) {
        return Err(TransportError::PeerNotVerified);
    }
    reject_weak_identity(identity)?;
    let frame=read_frame(stream).await?;
    server_handshake_with_hello(stream,peer,admission,identity,frame).await
}
/// Used by the Windows named-pipe path after its first-frame ReadFile and
/// synchronous OS impersonated-token attestation. Never trust caller data to
/// populate a WindowsMessageTokenObserved witness.
pub async fn server_handshake_with_hello<S: AsyncRead + AsyncWrite + Unpin>(
    stream:&mut S,peer:&PeerEvidence,admission:&dyn PeerAdmission,
    identity:&ServerIdentity,frame:proto::WorkspaceFrame,
) -> Result<TransportSession,TransportError> {
    reject_weak_identity(identity)?;
    if matches!(peer, PeerEvidence::WindowsUnverified |
        PeerEvidence::WindowsObserved { .. }) {
        return Err(TransportError::PeerNotVerified);
    }
    crate::workspace_ipc::validate(&frame)?;
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
/// Native OS-observed process identity is compared against an enrollment
/// pinned by the trusted host/installer. It is NOT derived from Hello, from a
/// path in model output, or from an arbitrary SDK caller. A verified peer is
/// still not authorized to perform a model/tool/workspace effect.
pub struct WindowsPinnedEnrollment {
    pub expected_pid:u32,
    pub expected_creation_filetime:u64,
    pub expected_session_id:u32,
    pub expected_user_sid:String,
    pub expected_executable_path:String,
    pub expected_module_id:String,
    pub expected_role:i32,
    pub expected_package_sha256:[u8;32],
    pub handshake_key:[u8;32],
}
impl PeerAdmission for WindowsPinnedEnrollment {
    fn admit(&self,peer:&PeerEvidence,hello:&proto::Hello)
        ->Result<EnrolledPeer,TransportError>{
        // Legacy synthetic source fixtures only. A release must never
        // authorize a DLC using *only* the message token and a claimed
        // manifest digest; the signed enrollment below is the live path.
        if !cfg!(debug_assertions) {
            return Err(TransportError::PeerNotVerified);
        }
        let PeerEvidence::WindowsMessageTokenObserved{
            pid,session_id,user_sid,image_path,creation_filetime
        }=peer else{return Err(TransportError::PeerNotVerified)};
        // No implicit same-user allowance and no filename-only allowlist.
        // Executable path is an OS-observed hint; caller must independently
        // verify signer/hash and installed package custody before constructing
        // this enrollment. PID is a host-admitted, supervised process.
        if *pid==0||*pid!=self.expected_pid
           ||*creation_filetime==0
           ||*creation_filetime!=self.expected_creation_filetime
           ||*session_id!=self.expected_session_id
           ||user_sid!=&self.expected_user_sid
           ||image_path!=&self.expected_executable_path
           ||hello.claimed_module_id!=self.expected_module_id
           ||hello.claimed_role!=self.expected_role
           ||hello.claimed_package_sha256.as_slice()!=self.expected_package_sha256
           ||self.handshake_key.iter().all(|b|*b==0){
            return Err(TransportError::PeerNotVerified)
        }
        Ok(EnrolledPeer{
            module_id:self.expected_module_id.clone(),
            role:self.expected_role,
            package_sha256:self.expected_package_sha256,
            handshake_key:self.handshake_key,
        })
    }
}

/// Host-owned, installer/supervisor-provisioned *exact* signed DLC pins.
/// Never create from Hello, from the peer's claimed digest or from a model.
/// The host must separately protect this record and securely provision its
/// handshake key. No default, auto-discovery or unsigned fallback is supplied.
#[cfg(windows)]
pub struct WindowsInstalledEnrollment {
    pub expected_pid: u32,
    pub expected_creation_filetime: u64,
    pub expected_session_id: u32,
    pub expected_user_sid: String,
    pub expected_executable_path: String,
    pub expected_image_sha256: [u8; 32],
    pub expected_signer_certificate_sha256: [u8; 32],
    pub expected_module_id: String,
    pub expected_role: i32,
    pub expected_package_sha256: [u8; 32],
    pub handshake_key: [u8; 32],
}
#[cfg(windows)]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ProtectedInstalledIdeRecord {
    schema:String,
    module_id:String,
    role:i32,
    installed_executable_path:String,
    image_sha256:String,
    signer_certificate_sha256:String,
    package_sha256:String,
}
#[cfg(windows)]
impl ProtectedInstalledIdeRecord {
    fn decode_sha256(s:&str)->Result<[u8;32],TransportError>{
        if s.len()!=64 || !s.bytes().all(|c|
            c.is_ascii_digit() || (b'a'..=b'f').contains(&c)){
            return Err(TransportError::PeerNotVerified);
        }
        let mut hash=[0u8;32];
        for (i,b) in hash.iter_mut().enumerate(){
            *b=u8::from_str_radix(&s[i*2..i*2+2],16)
                .map_err(|_|TransportError::PeerNotVerified)?;
        }
        if hash.iter().all(|b|*b==0){return Err(TransportError::PeerNotVerified);}
        Ok(hash)
    }
    fn parse(bytes:&[u8])->Result<Self,TransportError>{
        let record:Self=serde_json::from_slice(bytes)
            .map_err(|_|TransportError::PeerNotVerified)?;
        if record.schema!="lu.win.dlc-enrollment.v1"||
           record.module_id!="lu.ide"||
           record.role!=proto::ModuleRole::Workspace as i32 ||
           record.installed_executable_path.is_empty()||
           record.installed_executable_path.contains('\0')||
           !std::path::Path::new(&record.installed_executable_path).is_absolute(){
            return Err(TransportError::PeerNotVerified);
        }
        Self::decode_sha256(&record.image_sha256)?;
        Self::decode_sha256(&record.signer_certificate_sha256)?;
        Self::decode_sha256(&record.package_sha256)?;
        Ok(record)
    }
}
#[cfg(windows)]
impl WindowsInstalledEnrollment {
    /// Production-only source of signed DLC identity. The native reader
    /// verifies the fixed ProgramData SYSTEM/BA-only protected file and its
    /// parents. Broker key is NEVER persisted in this public pin record.
    /// Only a trusted host/supervisor may supply the observed launched PID
    /// and an independently OS-custodied handshake key.
    fn from_installer_owned_record(
        supervised_pid:u32,
    )->Result<(Self,[u8;32]),TransportError>{
        if supervised_pid==0 {
            return Err(TransportError::PeerNotVerified);
        }
        let bytes=lu_win_worker_job::ipc_installer_enrollment::
            read_installed_lu_ide_pins()
            .map_err(|_|TransportError::PeerNotVerified)?;
        let record=ProtectedInstalledIdeRecord::parse(&bytes)?;
        let record_fingerprint:[u8;32]=Sha256::digest(&bytes).into();
        let approved_package=ProtectedInstalledIdeRecord::decode_sha256(
            &record.package_sha256)?;
        let process=lu_win_worker_job::ipc_pipe_peer::observe_host_process(
            supervised_pid).map_err(|_|TransportError::PeerNotVerified)?;
        if process.image_path!=record.installed_executable_path {
            return Err(TransportError::PeerNotVerified);
        }
        let pin=Self{
            expected_pid:process.pid,
            expected_creation_filetime:process.creation_filetime,
            expected_session_id:process.session_id,
            expected_user_sid:process.user_sid,
            expected_executable_path:record.installed_executable_path,
            expected_image_sha256:ProtectedInstalledIdeRecord::decode_sha256(
                &record.image_sha256)?,
            expected_signer_certificate_sha256:
                ProtectedInstalledIdeRecord::decode_sha256(
                    &record.signer_certificate_sha256)?,
            expected_module_id:record.module_id,
            expected_role:record.role,
            expected_package_sha256:approved_package,
            // Fail closed unless this process is the trusted SYSTEM broker
            // and the package-bound locally persisted OS credential exists.
            handshake_key:lu_win_worker_job::ipc_credential_broker::
                read_installed_ide_key(approved_package)
                .map_err(|_|TransportError::PeerNotVerified)?,
        };
        if !pin.valid_pin(){return Err(TransportError::PeerNotVerified);}
        Ok((pin,record_fingerprint))
    }
}

#[cfg(windows)]
impl WindowsInstalledEnrollment {
    fn valid_pin(&self) -> bool {
        self.expected_pid != 0 && self.expected_creation_filetime != 0 &&
        self.expected_user_sid.starts_with("S-1-") &&
        std::path::Path::new(&self.expected_executable_path).is_absolute() &&
        !self.expected_executable_path.contains('\0') &&
        !self.expected_module_id.is_empty() &&
        self.expected_role > 0 &&
        self.expected_image_sha256.iter().any(|b| *b != 0) &&
        self.expected_signer_certificate_sha256.iter().any(|b| *b != 0) &&
        self.expected_package_sha256.iter().any(|b| *b != 0) &&
        self.handshake_key.iter().any(|b| *b != 0)
    }
}
#[cfg(windows)]
impl PeerAdmission for WindowsInstalledEnrollment {
    fn admit(&self, peer: &PeerEvidence, hello: &proto::Hello)
        -> Result<EnrolledPeer, TransportError>
    {
        if !self.valid_pin() { return Err(TransportError::PeerNotVerified); }
        let PeerEvidence::WindowsSignedPackage(witness) = peer else {
            return Err(TransportError::PeerNotVerified);
        };
        let os = &witness.process;
        if os.pid != self.expected_pid
            || os.creation_filetime != self.expected_creation_filetime
            || os.session_id != self.expected_session_id
            || os.user_sid != self.expected_user_sid
            || os.image_path != self.expected_executable_path
            || !fixed_equal(&self.expected_image_sha256, &witness.image_sha256)
            || !fixed_equal(&self.expected_signer_certificate_sha256,
                &witness.signer_certificate_sha256)
            || hello.claimed_module_id != self.expected_module_id
            || hello.claimed_role != self.expected_role
            || hello.claimed_package_sha256.as_slice() != self.expected_package_sha256
        {
            return Err(TransportError::PeerNotVerified);
        }
        Ok(EnrolledPeer {
            module_id: self.expected_module_id.clone(),
            role: self.expected_role,
            package_sha256: self.expected_package_sha256,
            handshake_key: self.handshake_key,
        })
    }
}

#[cfg(windows)]
pub mod windows {
    use super::*;
    use tokio::net::windows::named_pipe::{
        ClientOptions, NamedPipeClient, NamedPipeServer,
    };
    const PREFIX: &str = r"\\.\pipe\lu-ipc-";
    use std::io::{Read, Seek, SeekFrom};
    use sha2::{Digest as _, Sha256 as ImageSha256};

    const MAX_SIGNED_IMAGE_BYTES: u64 = 512 * 1024 * 1024;

    fn file_sha256(file: &std::fs::File) -> Result<[u8;32],TransportError> {
        let len=file.metadata()?.len();
        if len == 0 || len > MAX_SIGNED_IMAGE_BYTES {
            return Err(TransportError::PeerNotVerified);
        }
        let mut input=file.try_clone()?;
        input.seek(SeekFrom::Start(0))?;
        let mut hasher=ImageSha256::new();
        let mut read_bytes=0u64;
        let mut buffer=[0u8;65536];
        loop {
            let count=input.read(&mut buffer)?;
            if count==0 { break; }
            read_bytes+=count as u64;
            if read_bytes>len { return Err(TransportError::PeerNotVerified); }
            hasher.update(&buffer[..count]);
        }
        if read_bytes!=len || file.metadata()?.len()!=len {
            return Err(TransportError::PeerNotVerified);
        }
        Ok(hasher.finalize().into())
    }

    /// A session owns a stable Win32 process HANDLE and read-only signed
    /// image File; neither may be dropped before disconnect/revocation.
    /// Caller must still apply ConversationAuthority + durable turn grants.
    struct InstalledRevocationWitness {
        package_sha256:[u8;32],
        record_sha256:[u8;32],
        credential_sha256:[u8;32],
    }
    pub struct SignedWindowsSession {
        session:TransportSession,
        lease:lu_win_worker_job::ipc_signed_package::SignedClientLease,
        revocation:Option<InstalledRevocationWitness>,
    }
    impl SignedWindowsSession {
        pub fn session(&self)->&TransportSession {&self.session}
        /// Stable kernel-observed peer identity held by the signed native
        /// lease, never the PID claimed in an IPC Hello.
        pub fn peer_pid(&self)->u32 {self.lease.witness().pid}
        /// Exact OS-observed process creation time held by this signed
        /// native lease. A recycled PID cannot satisfy a prior session's
        /// original worker-custody journal binding.
        pub fn peer_creation_filetime(&self)->u64{
            self.lease.witness().creation_filetime
        }
        /// Production Core dispatch requires the installer-owned record and
        /// package-key revocation witness; debug signed surrogate leases
        /// deliberately never satisfy this predicate.
        pub fn installed_enrollment_active(&self)->bool{
            self.revocation.is_some()
        }
        /// Host-only package identity captured from the protected installer
        /// record, never the peer's claimed package digest. Used exclusively
        /// for exact-key retirement under the SYSTEM broker.
        pub fn installed_package_sha256(&self)->Option<[u8;32]>{
            self.revocation.as_ref().map(|v|v.package_sha256)
        }
        pub fn check_alive(&self)->Result<(),TransportError>{
            self.lease.check_alive().map_err(|_|TransportError::PeerNotVerified)?;
            if let Some(witness)=&self.revocation {
                // Re-read both protected material sources on each received
                // frame. Installer record replacement and Credential Manager
                // deletion/rotation invalidate an already-admitted session.
                let bytes=lu_win_worker_job::ipc_installer_enrollment::
                    read_installed_lu_ide_pins()
                    .map_err(|_|TransportError::PeerNotVerified)?;
                let record_sha:[u8;32]=ImageSha256::digest(bytes).into();
                if !fixed_equal(&witness.record_sha256,&record_sha) {
                    return Err(TransportError::PeerNotVerified);
                }
                let mut key=lu_win_worker_job::ipc_credential_broker::
                    read_installed_ide_key(witness.package_sha256)
                    .map_err(|_|TransportError::PeerNotVerified)?;
                let key_sha:[u8;32]=ImageSha256::digest(key).into();
                // Volatile erasure before any comparison/return; it is not
                // claimed to erase copies made by the OS or compiler.
                lu_win_worker_job::ipc_credential_broker::clear_temporary_key(&mut key);
                if !fixed_equal(&witness.credential_sha256,&key_sha) {
                    return Err(TransportError::PeerNotVerified);
                }
            }
            Ok(())
        }
        /// Preserve the post-ReadFile identity check on EVERY subsequent
        /// frame; session matching is not effect or model authorization.
        pub async fn read_verified_frame(&self,server:&mut NamedPipeServer)
            ->Result<proto::WorkspaceFrame,TransportError>{
            let frame=read_frame(server).await?;
            self.check_alive()?;
            let observed=lu_win_worker_job::ipc_pipe_peer::observe_message_token_peer(server)
                .map_err(|_|TransportError::PeerNotVerified)?;
            if &observed != self.lease.witness() {
                return Err(TransportError::PeerNotVerified);
            }
            self.session.check_request(&frame)?;
            Ok(frame)
        }
    }

    /// The signed client is enrolled only after the first canonical Hello,
    /// *native* post-message client-token witness, held process creation
    /// identity, embedded Authenticode trust, and the two exact SHA-256 pins.
    /// A path, same-user SID, signature or HMAC by itself is insufficient.
    /// This does not grant any model/tool effect.
    async fn accept_signed_peer_impl(
        server:&mut NamedPipeServer,
        pin:&WindowsInstalledEnrollment,
        identity:&ServerIdentity,
    )->Result<SignedWindowsSession,TransportError>{
        if !pin.valid_pin() {return Err(TransportError::PeerNotVerified);}
        server.connect().await?;
        let first=read_frame(server).await?;
        if !matches!(first.body.as_ref(),
            Some(proto::workspace_frame::Body::Hello(_))) {
            return Err(TransportError::Protocol("expected_hello"));
        }
        // This is synchronous: there is no await between the completed
        // first ReadFile and the impersonated-client-token observation.
        let lease=lu_win_worker_job::ipc_signed_package::inspect_signed_client(
            server,&pin.expected_executable_path)
            .map_err(|_|TransportError::PeerNotVerified)?;
        let image_sha256=file_sha256(lease.image_file())?;
        let signer_certificate_sha256: [u8;32]=
            ImageSha256::digest(lease.signer_certificate_der()).into();
        let evidence=PeerEvidence::WindowsSignedPackage(WindowsSignedPackageWitness{
            process:lease.witness().clone(),
            image_sha256,signer_certificate_sha256,
        });
        // Independent trusted pins are checked before any challenge byte.
        let Some(proto::workspace_frame::Body::Hello(hello))=first.body.as_ref() else{
            return Err(TransportError::Protocol("expected_hello"));
        };
        let _=pin.admit(&evidence,hello)?;
        lease.check_alive().map_err(|_|TransportError::PeerNotVerified)?;
        let session=server_handshake_with_hello(
            server,&evidence,pin,identity,first).await?;
        lease.check_alive().map_err(|_|TransportError::PeerNotVerified)?;
        Ok(SignedWindowsSession{session,lease,revocation:None})
    }

    /// Synthetic test admission only: arbitrary caller-constructed pins
    /// are refused in a non-debug release build.
    pub async fn accept_installed_signed_peer(
        server:&mut NamedPipeServer,
        pin:&WindowsInstalledEnrollment,
        identity:&ServerIdentity,
    )->Result<SignedWindowsSession,TransportError>{
        if !cfg!(debug_assertions) {
            return Err(TransportError::PeerNotVerified);
        }
        accept_signed_peer_impl(server,pin,identity).await
    }
    /// Production admission: installer-owned protected record plus broker
    /// credential and supervised PID. No auto-enrollment from Hello.
    pub async fn accept_installer_owned_lu_ide(
        server:&mut NamedPipeServer,
        supervised_pid:u32,
        identity:&ServerIdentity,
    )->Result<SignedWindowsSession,TransportError>{
        let (mut pin,record_sha256)=
            WindowsInstalledEnrollment::from_installer_owned_record(
                supervised_pid)?;
        let witness=InstalledRevocationWitness{
            package_sha256:pin.expected_package_sha256,
            record_sha256,
            credential_sha256:ImageSha256::digest(pin.handshake_key).into(),
        };
        let accepted=accept_signed_peer_impl(server,&pin,identity).await;
        // Do not retain the raw session key in this admission object after
        // handshake. The session only retains a digest to detect key rotation.
        lu_win_worker_job::ipc_credential_broker::clear_temporary_key(
            &mut pin.handshake_key);
        let mut session=accepted?;
        session.revocation=Some(witness);
        session.check_alive()?;
        Ok(session)
    }

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
        // DACL is constructed by the audited Win32 FFI crate AT CREATION.
        // The default Windows named-pipe DACL is not an acceptable endpoint.
        Ok(lu_win_worker_job::ipc_pipe_peer::create_owner_only_pipe(name)?)
    }
    pub async fn accept_peer(
        server: &NamedPipeServer,
    ) -> Result<PeerEvidence, TransportError> {
        server.connect().await?;
        let witness=lu_win_worker_job::ipc_pipe_peer::observe_connected_peer(server)
            .map_err(|_|TransportError::PeerNotVerified)?;
        Ok(PeerEvidence::WindowsObserved {
            pid:witness.pid,
            session_id:witness.session_id,
            user_sid:witness.user_sid,
            image_path:witness.image_path,
        })
    }
    /// The only native Windows path that upgrades a pipe connection from
    /// raw process metadata to message-token observation. It consumes Hello
    /// before any challenge; authentic signer enrollment is still host-owned.
    pub async fn accept_message_peer(server:&mut NamedPipeServer)
        ->Result<(PeerEvidence,proto::WorkspaceFrame),TransportError>{
        server.connect().await?;
        let first=read_frame(server).await?;
        if !matches!(first.body.as_ref(),
            Some(proto::workspace_frame::Body::Hello(_))) {
            return Err(TransportError::Protocol("expected_hello"));
        }
        let witness=lu_win_worker_job::ipc_pipe_peer::observe_message_token_peer(server)
            .map_err(|_|TransportError::PeerNotVerified)?;
        Ok((PeerEvidence::WindowsMessageTokenObserved {
            pid:witness.pid,session_id:witness.session_id,
            user_sid:witness.user_sid,image_path:witness.image_path,
            creation_filetime:witness.creation_filetime,
        },first))
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

    #[cfg(windows)]
    #[test]
    fn installed_record_parser_rejects_untrusted_claims_and_embedded_keys(){
        let correct=serde_json::json!({
            "schema":"lu.win.dlc-enrollment.v1",
            "module_id":"lu.ide",
            "role":1,
            "installed_executable_path":r"C:\Program Files\Lu IDE\LuIDE.exe",
            "image_sha256":"11".repeat(32),
            "signer_certificate_sha256":"22".repeat(32),
            "package_sha256":"77".repeat(32)
        });
        assert!(ProtectedInstalledIdeRecord::parse(
            &serde_json::to_vec(&correct).unwrap()).is_ok());
        for (field,value) in [
            ("schema",serde_json::json!("forged")),
            ("module_id",serde_json::json!("lu.core")),
            ("role",serde_json::json!(3)),
            ("installed_executable_path",serde_json::json!(r".\forged.exe")),
            ("image_sha256",serde_json::json!("11".repeat(31))),
            ("signer_certificate_sha256",serde_json::json!("00".repeat(32))),
            ("package_sha256",serde_json::json!("AA".repeat(32))),
        ]{
            let mut forged=correct.clone();
            forged[field]=value;
            assert!(ProtectedInstalledIdeRecord::parse(
                &serde_json::to_vec(&forged).unwrap()).is_err(),"{field}");
        }
        let mut secret=correct;
        secret["handshake_key"]=serde_json::json!("forbidden");
        assert!(ProtectedInstalledIdeRecord::parse(
            &serde_json::to_vec(&secret).unwrap()).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn signed_dlc_policy_requires_both_trusted_pins_and_exact_process() {
        let process=lu_win_worker_job::ipc_pipe_peer::WindowsPipePeer {
            pid:8123,session_id:7,creation_filetime:4321,
            user_sid:"S-1-5-21-123".into(),
            image_path:r"C:\Program Files\Lu\LuIDE.exe".into(),
        };
        let pin=WindowsInstalledEnrollment {
            expected_pid:process.pid,
            expected_creation_filetime:process.creation_filetime,
            expected_session_id:process.session_id,
            expected_user_sid:process.user_sid.clone(),
            expected_executable_path:process.image_path.clone(),
            expected_image_sha256:[0x11;32],
            expected_signer_certificate_sha256:[0x22;32],
            expected_module_id:"lu.ide".into(),
            expected_role:proto::ModuleRole::Workspace as i32,
            expected_package_sha256:[0x77;32],
            handshake_key:[0x42;32],
        };
        let good_hello=proto::Hello {
            claimed_module_id:"lu.ide".into(),
            claimed_role:proto::ModuleRole::Workspace as i32,
            claimed_package_sha256:vec![0x77;32],
            minimum_major:WIRE_MAJOR,maximum_major:WIRE_MAJOR,
            maximum_minor:WIRE_MINOR,requested_features:vec![],
            client_nonce:vec![0x66;32],
        };
        let valid=PeerEvidence::WindowsSignedPackage(WindowsSignedPackageWitness {
            process:process.clone(),image_sha256:[0x11;32],
            signer_certificate_sha256:[0x22;32],
        });
        assert!(pin.admit(&valid,&good_hello).is_ok());
        let raw=PeerEvidence::WindowsMessageTokenObserved {
            pid:process.pid,creation_filetime:process.creation_filetime,
            session_id:process.session_id,user_sid:process.user_sid.clone(),
            image_path:process.image_path.clone(),
        };
        assert!(matches!(pin.admit(&raw,&good_hello),
            Err(TransportError::PeerNotVerified)));
        for altered in [
            WindowsSignedPackageWitness {
                process:process.clone(),image_sha256:[0x31;32],
                signer_certificate_sha256:[0x22;32],
            },
            WindowsSignedPackageWitness {
                process:process.clone(),image_sha256:[0x11;32],
                signer_certificate_sha256:[0x32;32],
            },
            WindowsSignedPackageWitness {
                process:lu_win_worker_job::ipc_pipe_peer::WindowsPipePeer {
                    creation_filetime:9999,..process.clone()
                },image_sha256:[0x11;32],
                signer_certificate_sha256:[0x22;32],
            },
        ] {
            assert!(pin.admit(&PeerEvidence::WindowsSignedPackage(altered),
                &good_hello).is_err());
        }
        let mut forged=good_hello.clone();
        forged.claimed_package_sha256=vec![0x44;32];
        assert!(pin.admit(&valid,&forged).is_err());
    }

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
