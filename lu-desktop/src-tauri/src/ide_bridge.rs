//! WO10-C/D: Lu-owned, explicitly enabled local IDE inference bridge.
//! No project filesystem, Lu native conversation history, or tool authority.
//! Linux-only first: a private XDG_RUNTIME_DIR AF_UNIX socket, SO_PEERCRED,
//! and a human-transcribed 256-bit HMAC pairing secret. Windows fails closed
//! until the independently audited named-pipe identity backend exists.
use super::{next_id, DesktopState};
use lu_ide_bridge_contract::{
    proto::{self, bridge_envelope::Body, BridgeEnvelope, BridgeHelloAck, BridgeFailure,
        CodingChatReply},
    transport::{self, TransportError},
};
use rand::{rngs::OsRng, RngCore};
use serde::Serialize;
use std::{
    sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex},
    time::Duration,
};
use tauri::AppHandle;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeView {
    pub available: bool,
    pub enabled: bool,
    pub paired: bool,
    pub pairing_code: Option<String>,
    pub connected_to_lu_model: bool,
    pub mode: &'static str,
    pub reason_code: &'static str,
}

pub struct LuIdeBridge {
    enabled: AtomicBool,
    paired: AtomicBool,
    secret: Mutex<Option<[u8; 32]>>,
    active: Mutex<Option<(String, String)>>,
}
impl Default for LuIdeBridge {
    fn default() -> Self {
        Self { enabled: AtomicBool::new(false), paired: AtomicBool::new(false),
            secret: Mutex::new(None), active: Mutex::new(None) }
    }
}
impl LuIdeBridge {
    pub fn status(&self) -> BridgeView {
        #[cfg(target_os = "linux")]
        let available = lu_ide_bridge_contract::transport::linux::endpoint().is_ok();
        #[cfg(not(target_os = "linux"))]
        let available = false;
        let enabled = self.enabled.load(Ordering::Acquire);
        let pairing_code = if enabled {
            self.secret.lock().ok().and_then(|value| value.as_ref()
                .map(transport::pairing_code))
        } else { None };
        BridgeView {
            available, enabled, pairing_code,
            paired: self.paired.load(Ordering::Acquire),
            connected_to_lu_model: false, // model readiness read separately by Lu status
            mode: "opt_in_local_inference_only",
            reason_code: if !available { "LU_IDE_OS_PEER_BACKEND_UNAVAILABLE" }
                else if !enabled { "LU_IDE_BRIDGE_DISABLED" }
                else { "LU_IDE_BRIDGE_USER_PAIRING_REQUIRED" },
        }
    }

    pub fn disable(&self, state: &DesktopState) -> Result<BridgeView, String> {
        self.enabled.store(false, Ordering::Release);
        let active = self.active.lock().map_err(|_| "bridge state unavailable")?.take();
        if let Some((sid,gid)) = active {
            let _ = state.generation_queue.cancel(&sid, &gid);
            let _ = state.supervisor.cancel(&sid, &gid, &next_id("req-"));
        }
        if let Ok(mut secret) = self.secret.lock() {
            if let Some(mut bytes) = secret.take() { bytes.fill(0); }
        }
        Ok(self.status())
    }

    #[cfg(target_os = "linux")]
    pub fn enable(
        self: &Arc<Self>, app: AppHandle, state: Arc<DesktopState>,
    ) -> Result<BridgeView, String> {
        use std::os::unix::{fs::PermissionsExt, net::UnixListener};
        let path = transport::linux::endpoint()
            .map_err(|_| "private XDG runtime directory unavailable")?;
        if self.enabled.swap(true, Ordering::AcqRel) {
            return Err("Lu IDE bridge already enabled".into());
        }
        // Never unlink or replace a pre-existing socket (could belong to a
        // different process). The listener is created only after the user
        // enabled sharing in the Lu Desktop UI.
        let listener = match UnixListener::bind(&path) {
            Ok(listener) => listener,
            Err(_) => {
                self.enabled.store(false, Ordering::Release);
                return Err("private Lu IDE socket occupied or unavailable".into());
            }
        };
        if let Err(err) = std::fs::set_permissions(&path,
            std::fs::Permissions::from_mode(0o600)) {
            let _ = std::fs::remove_file(&path);
            self.enabled.store(false, Ordering::Release);
            return Err(format!("could not restrict Lu IDE socket: {err}"));
        }
        if let Err(err) = listener.set_nonblocking(true) {
            let _ = std::fs::remove_file(&path);
            self.enabled.store(false, Ordering::Release);
            return Err(format!("Lu IDE socket admission unavailable: {err}"));
        }
        let mut secret = [0_u8; 32];
        OsRng.fill_bytes(&mut secret);
        if let Ok(mut guard) = self.secret.lock() {
            *guard = Some(secret);
        } else {
            let _ = std::fs::remove_file(&path);
            self.enabled.store(false, Ordering::Release);
            return Err("Lu IDE pairing state unavailable".into());
        }
        let bridge = Arc::clone(self);
        std::thread::spawn(move || {
            serve(listener, &path, bridge, state, app);
        });
        Ok(self.status())
    }
}

#[cfg(target_os = "linux")]
fn serve(
    listener: std::os::unix::net::UnixListener,
    path: &std::path::Path,
    bridge: Arc<LuIdeBridge>,
    state: Arc<DesktopState>,
    _app: AppHandle,
) {
    use std::{io::ErrorKind, thread};
    while bridge.enabled.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                // OS uid/pid must be verified before parsing any payload.
                if transport::linux::verify_peer(&stream).is_err() { continue; }
                if stream.set_read_timeout(Some(Duration::from_secs(90))).is_err()
                    || stream.set_write_timeout(Some(Duration::from_secs(90))).is_err() {
                    continue;
                }
                if bridge.paired.swap(true, Ordering::AcqRel) { continue; }
                let secret = bridge.secret.lock().ok()
                    .and_then(|x| *x);
                if let Some(secret) = secret {
                    let _ = serve_connection(&mut stream, &bridge, &state, &secret);
                }
                bridge.paired.store(false, Ordering::Release);
            }
            Err(err) if err.kind() == ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(70));
            }
            Err(_) => break,
        }
    }
    // Remove ONLY the original socket inode if it is still the current
    // listener; never delete a different process's replacement.
    use std::os::unix::fs::MetadataExt;
    let old = listener.local_addr().ok().and_then(|addr| addr.as_pathname()
        .and_then(|p| std::fs::symlink_metadata(p).ok()).map(|m| m.ino()));
    if let (Some(original), Ok(current)) = (old, std::fs::symlink_metadata(path)) {
        if current.ino() == original { let _ = std::fs::remove_file(path); }
    }
    bridge.enabled.store(false, Ordering::Release);
}

#[cfg(target_os = "linux")]
fn serve_connection(
    stream: &mut std::os::unix::net::UnixStream,
    bridge: &LuIdeBridge,
    state: &DesktopState,
    secret: &[u8; 32],
) -> Result<(), String> {
    use lu_core::{inference_bridge::BridgeError,
        inference_wire::decode_frame, wire::proto as lu_proto};
    let hello = transport::read_frame(stream).map_err(|_| "invalid peer frame")?;
    let Body::Hello(ref offer) = hello.body.as_ref().ok_or("missing hello")? else {
        return Err("nonhello first message".into());
    };
    if offer.client_surface != "lu.ide" || !offer.advertised_capabilities.contains(&1)
        || offer.challenge_nonce.len() != 32 {
        return Err("unapproved IDE client surface".into());
    }
    let mut nonce = [0_u8; 32];
    nonce.copy_from_slice(&offer.challenge_nonce);
    transport::verify_client_proof(secret, &nonce, &offer.client_proof)
        .map_err(|_| "HMAC pairing denied")?;
    let mut server_nonce = [0_u8; 32];
    OsRng.fill_bytes(&mut server_nonce);
    let session = next_id("");
    let ack = BridgeEnvelope {
        protocol_version:1, request_id: hello.request_id.clone(),
        session_id: session.clone(), sequence:0,
        body: Some(Body::HelloAck(BridgeHelloAck {
            selected_version:1, echoed_challenge_nonce:nonce.to_vec(),
            responder_nonce:server_nonce.to_vec(),
            advertised_capabilities:vec![1],
            responder_proof: transport::server_proof(
                secret, &nonce, &server_nonce, &session).to_vec(),
        })),
    };
    transport::write_frame(stream, &ack).map_err(|_| "HMAC ack send failed")?;
    let mut seq = 0_u64;
    let mut project: Option<String> = None;
    while bridge.enabled.load(Ordering::Acquire) && seq < 128 {
        let request = match transport::read_frame(stream) {
            Ok(v) => v,
            Err(TransportError::Io) => break, // disconnect or timeout
            Err(_) => return Err("invalid post-pairing frame".into()),
        };
        if request.session_id != session || request.sequence != seq + 1 {
            return Err("session replay or sequence mismatch".into());
        }
        seq += 1;
        let Body::ChatRequest(ref chat) = request.body.as_ref()
            .ok_or("missing request body")? else {
            send_failure(stream,&session,seq,&request.request_id,
                "BRIDGE_ONLY_SCOPED_CHAT")?;
            continue;
        };
        if project.as_deref().is_some_and(|p| p != chat.project_id) {
            send_failure(stream,&session,seq,&request.request_id,
                "BRIDGE_PROJECT_CHANGED")?;
            continue;
        }
        project.get_or_insert_with(|| chat.project_id.clone());
        // **No** access to the user's native Lu chat database. History is
        // supplied by the IDE's separately identified coding thread only.
        let outcome = (|| -> Result<CodingChatReply, String> {
            let status = state.supervisor.status()
                .map_err(|_| "Lu model supervisor unavailable")?;
            let sid = status.session_id.ok_or("Load a model in Lu first")?;
            let gid = next_id("g-");
            let rid = next_id("req-");
            let chat_ref = format!("ide-{}-{}", &session[..8], chat.thread_id);
            let (ticket, _) = state.generation_queue
                .enqueue(&sid, &chat_ref, &gid)?;
            ticket.wait_for_slot()?;
            let prior: Vec<lu_proto::InferenceChatTurn> = chat.history.iter()
                .map(|h| lu_proto::InferenceChatTurn {
                    role:h.role.clone(), text:h.text.clone(),
                }).collect();
            state.supervisor.reserve_generation_with_history(
                &sid, &gid, &rid, &chat.prompt, chat.max_new_tokens, &prior
            ).map_err(|_| "Lu model busy or no longer loaded")?;
            if let Ok(mut active) = bridge.active.lock() {
                *active = Some((sid.clone(),gid.clone()));
            }
            let mut response = String::new();
            let result = state.supervisor.run_reserved_with_history(
                &sid, &gid, &rid, chat.prompt.clone(), chat.max_new_tokens,
                prior, |bytes| {
                    let frame = decode_frame(&bytes).map_err(BridgeError::Wire)?;
                    if let Some(lu_proto::inference_frame::Body::Event(event)) = frame.body {
                        if let Some(lu_proto::inference_event::Kind::TextChunk(token)) = event.kind {
                            if response.len().saturating_add(token.text.len()) > 16384 {
                                return Err(BridgeError::Consumer);
                            }
                            response.push_str(&token.text);
                        }
                    }
                    Ok(())
                },
            );
            if let Ok(mut active) = bridge.active.lock() {
                if active.as_ref().is_some_and(|(_, id)| id == &gid) {
                    *active = None;
                }
            }
            result.map_err(|_| "Lu model generation failed or cancelled")?;
            Ok(CodingChatReply {
                project_id: chat.project_id.clone(),
                thread_id: chat.thread_id.clone(),
                generation_id: chat.generation_id.clone(),
                text:response, finish_code:"completed".into(),
                model_session_id:sid,
            })
        })();
        match outcome {
            Ok(reply) => {
                transport::write_frame(stream, &BridgeEnvelope {
                    protocol_version:1, request_id:request.request_id,
                    session_id:session.clone(), sequence:seq,
                    body:Some(Body::ChatReply(reply)),
                }).map_err(|_| "IDE reply disconnected")?;
            }
            Err(_) => {
                send_failure(stream,&session,seq,&request.request_id,
                    "LU_MODEL_UNAVAILABLE_OR_GENERATION_FAILED")?;
            }
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn send_failure(
    stream: &mut std::os::unix::net::UnixStream,
    session: &str, seq: u64, rid: &str, code: &str,
) -> Result<(), String> {
    transport::write_frame(stream,&BridgeEnvelope {
        protocol_version:1,request_id:rid.into(),session_id:session.into(),
        sequence:seq,body:Some(Body::Failure(BridgeFailure { code:code.into() })),
    }).map_err(|_| "bridge failed to send bounded error".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_has_no_listener_secret_or_model_authority() {
        let bridge = LuIdeBridge::default();
        let status = bridge.status();
        assert!(!status.enabled && !status.paired);
        assert!(status.pairing_code.is_none());
        assert!(!status.connected_to_lu_model);
    }
}
