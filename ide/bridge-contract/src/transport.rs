//! Shared, bounded transport primitives. No socket auto-discovery, no grants.
//! Linux uses SO_PEERCRED plus a separately user-paired reciprocal HMAC;
//! Windows requires an independently reviewed named-pipe identity backend.
use crate::{decode, encode, proto, BridgeWireError, MAX_BRIDGE_BYTES};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::io::{Read, Write};

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    Io,
    Framing,
    Protocol(BridgeWireError),
    PairingDenied,
    UnsafeEndpoint,
    PeerIdentityDenied,
}
fn mac(secret: &[u8; 32], tag: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut h = HmacSha256::new_from_slice(secret).expect("32-byte HMAC key");
    h.update(tag);
    for part in parts { h.update(part); }
    let mut output = [0_u8; 32];
    output.copy_from_slice(&h.finalize().into_bytes());
    output
}
pub fn client_proof(secret: &[u8; 32], challenge: &[u8; 32]) -> [u8; 32] {
    mac(secret, b"lu-ide/client/v1", &[challenge])
}
pub fn server_proof(
    secret: &[u8; 32], client_nonce: &[u8; 32],
    server_nonce: &[u8; 32], session_id: &str,
) -> [u8; 32] {
    mac(secret, b"lu-ide/server/v1",
        &[client_nonce, server_nonce, session_id.as_bytes()])
}
pub fn verify_client_proof(
    secret: &[u8; 32], nonce: &[u8; 32], offered: &[u8],
) -> Result<(), TransportError> {
    let expected = client_proof(secret, nonce);
    let mut h = HmacSha256::new_from_slice(secret).expect("32-byte HMAC key");
    h.update(b"lu-ide/client/v1");
    h.update(nonce);
    h.verify_slice(offered).map_err(|_| TransportError::PairingDenied)?;
    if offered.len() != expected.len() { return Err(TransportError::PairingDenied); }
    Ok(())
}
pub fn verify_server_proof(
    secret: &[u8; 32], client_nonce: &[u8; 32],
    ack: &proto::BridgeHelloAck, session_id: &str,
) -> Result<(), TransportError> {
    if ack.echoed_challenge_nonce.as_slice() != client_nonce
        || ack.responder_nonce.len() != 32
        || ack.selected_version != 1 {
        return Err(TransportError::PairingDenied);
    }
    let mut nonce = [0_u8; 32];
    nonce.copy_from_slice(&ack.responder_nonce);
    let mut h = HmacSha256::new_from_slice(secret).expect("32-byte HMAC key");
    h.update(b"lu-ide/server/v1");
    h.update(client_nonce);
    h.update(&nonce);
    h.update(session_id.as_bytes());
    h.verify_slice(&ack.responder_proof)
        .map_err(|_| TransportError::PairingDenied)
}
pub fn pairing_code(secret: &[u8; 32]) -> String {
    secret.iter().map(|b| format!("{b:02x}")).collect()
}
pub fn parse_pairing_code(code: &str) -> Result<[u8; 32], TransportError> {
    if code.len() != 64 || !code.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(TransportError::PairingDenied);
    }
    let mut secret = [0_u8; 32];
    for (index, chunk) in code.as_bytes().chunks_exact(2).enumerate() {
        secret[index] = u8::from_str_radix(std::str::from_utf8(chunk)
            .map_err(|_| TransportError::PairingDenied)?, 16)
            .map_err(|_| TransportError::PairingDenied)?;
    }
    if secret.iter().all(|x| *x == 0) {
        return Err(TransportError::PairingDenied);
    }
    Ok(secret)
}
pub fn write_frame<W: Write>(out: &mut W, frame: &proto::BridgeEnvelope)
    -> Result<(), TransportError> {
    let bytes = encode(frame).map_err(TransportError::Protocol)?;
    let len = u32::try_from(bytes.len()).map_err(|_| TransportError::Framing)?;
    out.write_all(&len.to_be_bytes()).map_err(|_| TransportError::Io)?;
    out.write_all(&bytes).map_err(|_| TransportError::Io)?;
    out.flush().map_err(|_| TransportError::Io)
}
pub fn read_frame<R: Read>(input: &mut R)
    -> Result<proto::BridgeEnvelope, TransportError> {
    let mut header = [0_u8; 4];
    input.read_exact(&mut header).map_err(|_| TransportError::Io)?;
    let size = u32::from_be_bytes(header) as usize;
    if size == 0 || size > MAX_BRIDGE_BYTES {
        return Err(TransportError::Framing);
    }
    let mut payload = vec![0_u8; size];
    input.read_exact(&mut payload).map_err(|_| TransportError::Io)?;
    decode(&payload).map_err(TransportError::Protocol)
}

#[cfg(target_os = "linux")]
pub mod linux {
    use super::TransportError;
    use nix::{
        sys::socket::{getsockopt, sockopt::PeerCredentials},
        unistd::Uid,
    };
    use std::{
        env, fs,
        os::unix::{fs::{FileTypeExt, MetadataExt, PermissionsExt}, net::UnixStream},
        path::PathBuf,
    };
    /// XDG_RUNTIME_DIR must be an OS-managed private directory, not a project,
    /// home-relative, /tmp magic socket or a client-submitted absolute path.
    pub fn endpoint() -> Result<PathBuf, TransportError> {
        let root = PathBuf::from(env::var_os("XDG_RUNTIME_DIR")
            .ok_or(TransportError::UnsafeEndpoint)?);
        let m = fs::symlink_metadata(&root)
            .map_err(|_| TransportError::UnsafeEndpoint)?;
        if !m.file_type().is_dir() || m.file_type().is_symlink()
            || m.uid() != Uid::current().as_raw() || (m.permissions().mode() & 0o077) != 0 {
            return Err(TransportError::UnsafeEndpoint);
        }
        Ok(root.join("lu-ide-bridge-v1.sock"))
    }
    pub fn inspect_socket(path: &PathBuf) -> Result<(), TransportError> {
        let m = fs::symlink_metadata(path).map_err(|_| TransportError::UnsafeEndpoint)?;
        if !m.file_type().is_socket() || m.file_type().is_symlink()
            || m.uid() != Uid::current().as_raw()
            || (m.permissions().mode() & 0o077) != 0 {
            return Err(TransportError::UnsafeEndpoint);
        }
        Ok(())
    }
    pub fn verify_peer(stream: &UnixStream) -> Result<u32, TransportError> {
        let creds = getsockopt(stream, PeerCredentials)
            .map_err(|_| TransportError::PeerIdentityDenied)?;
        if creds.uid() != Uid::current().as_raw() || creds.pid() <= 0 {
            return Err(TransportError::PeerIdentityDenied);
        }
        Ok(creds.pid() as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{bridge_envelope::Body, BridgeHello, BridgeHelloAck, BridgeEnvelope};
    #[test]
    fn pairing_requires_true_hmac_in_both_directions() {
        let secret = [0x43; 32]; let n1 = [9_u8; 32]; let n2 = [10_u8; 32];
        let client = client_proof(&secret, &n1);
        assert!(verify_client_proof(&secret, &n1, &client).is_ok());
        assert!(verify_client_proof(&[0x45; 32], &n1, &client).is_err());
        let ack = BridgeHelloAck {
            selected_version: 1,
            echoed_challenge_nonce: n1.to_vec(),
            responder_nonce: n2.to_vec(),
            advertised_capabilities: vec![1],
            responder_proof: server_proof(&secret,&n1,&n2,&"a".repeat(32)).to_vec(),
        };
        assert!(verify_server_proof(&secret,&n1,&ack,&"a".repeat(32)).is_ok());
        assert!(verify_server_proof(&secret,&n1,&ack,&"b".repeat(32)).is_err());
        let code = pairing_code(&secret);
        assert_eq!(parse_pairing_code(&code).unwrap(), secret);
        assert!(parse_pairing_code(&"0".repeat(64)).is_err());
    }
    #[test]
    fn framed_canonical_data_only_roundtrip() {
        let input = BridgeEnvelope {
            protocol_version:1, request_id:"one".into(),session_id:String::new(), sequence:0,
            body:Some(Body::Hello(BridgeHello {
                client_surface:"lu.ide".into(),minimum_version:1,maximum_version:1,
                challenge_nonce:vec![1;32],client_proof:vec![2;32],
                advertised_capabilities:vec![1],
            })),
        };
        let mut data = Vec::new();
        write_frame(&mut data,&input).unwrap();
        assert_eq!(read_frame(&mut data.as_slice()).unwrap(), input);
        assert!(read_frame(&mut &[0,1,0,0][..]).is_err());
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn unix_kernel_peer_credentials_are_real() {
        let (a,b) = std::os::unix::net::UnixStream::pair().unwrap();
        assert!(linux::verify_peer(&a).unwrap()>0);
        assert!(linux::verify_peer(&b).unwrap()>0);
    }
}
