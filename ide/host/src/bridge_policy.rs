//! WO10-B: optional bridge permission model and replay guards.
//!
//! No production pairing constructor exists yet. Until Windows named-pipe
//! identity and Linux peer credentials are independently implemented and
//! verified, any live bridge request MUST fail closed. This module is a
//! transport-independent protocol test target, not proof of a live Lu link.
use crate::bridge_wire::{self, proto, BRIDGE_VERSION};
use rand::{rngs::OsRng, RngCore};
use serde::Serialize;
use std::collections::BTreeSet;

const MAX_REQUESTS_PER_SESSION: u64 = 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeStatus {
    pub enabled: bool,
    pub paired: bool,
    pub endpoint_verified: bool,
    pub peer_authenticated: bool,
    pub active_project: Option<String>,
    pub connection_state: &'static str,
    pub reason_code: &'static str,
    pub mode: &'static str,
    pub autonomous_execution_allowed: bool,
    pub default_project_disclosure: bool,
}

pub fn disconnected_status() -> BridgeStatus {
    BridgeStatus {
        enabled: false,
        paired: false,
        endpoint_verified: false,
        peer_authenticated: false,
        active_project: None,
        connection_state: "not_connected",
        reason_code: "LU_BRIDGE_NATIVE_PEER_AUTH_NOT_YET_ACCEPTED",
        mode: "optional_data_only_contract",
        autonomous_execution_allowed: false,
        default_project_disclosure: false,
    }
}

// Only a native OS transport can create this evidence, after checking the
// actual kernel peer credentials AND installer-pinned application identity.
// It has no public constructor. A caller-provided string or a message flag is
// never considered evidence.
struct VerifiedOsPeer {
    surface: &'static str,
}
// Only a trusted native human confirmation pathway may construct a grant.
// A model proposal or Protobuf payload has no way to mint one.
struct HumanTurnConsent {
    project_id: String,
    request_kind: &'static str,
}

#[derive(Debug)]
struct Session {
    id: String,
    peer_surface: &'static str,
    project_id: String,
    allowed_capabilities: BTreeSet<i32>,
    last_sequence: u64,
    revoked: bool,
}

impl Session {
    fn after_verified_pairing(
        peer: VerifiedOsPeer,
        consent: HumanTurnConsent,
        hello: &proto::BridgeEnvelope,
    ) -> Result<(Self, proto::BridgeEnvelope), &'static str> {
        use proto::bridge_envelope::Body;
        bridge_wire::validate(hello).map_err(|_| "BRIDGE_HELLO_INVALID")?;
        if peer.surface != "lu.core" || consent.request_kind != "project_context"
            || !crate::wire::valid_project_id(&consent.project_id) {
            return Err("BRIDGE_OS_PEER_OR_HUMAN_CONSENT_INVALID");
        }
        let Body::Hello(info) = hello.body.as_ref().ok_or("BRIDGE_HELLO_ABSENT")? else {
            return Err("BRIDGE_HELLO_EXPECTED");
        };
        if info.client_surface != peer.surface {
            return Err("BRIDGE_PEER_SURFACE_MISMATCH");
        }
        let supported: BTreeSet<i32> = [2, 3].into_iter().collect();
        let advertised: BTreeSet<i32> = info.advertised_capabilities.iter().copied().collect();
        let allowed_capabilities = supported.intersection(&advertised).copied()
            .collect::<BTreeSet<_>>();
        if allowed_capabilities.is_empty() {
            return Err("BRIDGE_NO_NEGOTIATED_DATA_CAPABILITY");
        }
        let mut random = [0u8; 16];
        OsRng.fill_bytes(&mut random);
        let id = random.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
        let mut challenge = [0u8; 32];
        OsRng.fill_bytes(&mut challenge);
        let ack = proto::BridgeEnvelope {
            protocol_version: BRIDGE_VERSION,
            request_id: hello.request_id.clone(),
            session_id: id.clone(),
            sequence: 0,
            body: Some(Body::HelloAck(proto::BridgeHelloAck {
                selected_version: BRIDGE_VERSION,
                echoed_challenge_nonce: info.challenge_nonce.clone(),
                responder_nonce: challenge.to_vec(),
                advertised_capabilities: allowed_capabilities.iter().copied().collect(),
            })),
        };
        bridge_wire::validate(&ack).map_err(|_| "BRIDGE_ACK_INVALID")?;
        Ok((Self {
            id, peer_surface: peer.surface,
            project_id: consent.project_id,
            allowed_capabilities, last_sequence: 0, revoked: false,
        }, ack))
    }

    /// Validated data-offer admission is NOT authorization to stage, save, Git
    /// commit, build, read arbitrary source, or disclose an IDE conversation.
    fn admit_offer(&mut self, frame: &[u8]) -> Result<&'static str, &'static str> {
        use proto::bridge_envelope::Body;
        if self.revoked { return Err("BRIDGE_SESSION_REVOKED"); }
        let request = bridge_wire::decode(frame).map_err(|_| "BRIDGE_INVALID_FRAME")?;
        if request.session_id != self.id || request.sequence != self.last_sequence + 1
            || request.sequence > MAX_REQUESTS_PER_SESSION {
            return Err("BRIDGE_SESSION_REPLAY_OR_WRONG_PEER");
        }
        let result = match request.body.as_ref().ok_or("BRIDGE_BODY_ABSENT")? {
            Body::ContextOffer(offer) => {
                if !self.allowed_capabilities.contains(&2)
                    || offer.project_id != self.project_id {
                    return Err("BRIDGE_CONTEXT_OUT_OF_SCOPE");
                }
                "context_for_review_only"
            }
            Body::ProposalOffer(offer) => {
                if !self.allowed_capabilities.contains(&3)
                    || offer.project_id != self.project_id {
                    return Err("BRIDGE_PROPOSAL_OUT_OF_SCOPE");
                }
                // A proposal offer is UNTRUSTED. Its bytes must go through
                // IdeProposalInbox::import + independently approved staging;
                // never to source.save, Git, worker, or a shell.
                "proposal_for_inbox_only"
            }
            Body::Goodbye(_) => {
                self.revoked = true;
                "session_closed"
            }
            _ => return Err("BRIDGE_MESSAGE_NOT_DATA_OFFER"),
        };
        self.last_sequence += 1;
        Ok(result)
    }

    fn revoke(&mut self) {
        self.revoked = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proto::{bridge_envelope::Body, BridgeEnvelope, BridgeHello, ProjectContextOffer, ProposedChangeOffer};
    fn project() -> String { format!("ide-{}", "a".repeat(32)) }
    fn hello() -> BridgeEnvelope {
        BridgeEnvelope {
            protocol_version: 1, request_id: "request-1".into(),
            session_id: String::new(), sequence: 0,
            body: Some(Body::Hello(BridgeHello {
                client_surface: "lu.core".into(), minimum_version: 1,
                maximum_version: 1, challenge_nonce: vec![17; 32],
                advertised_capabilities: vec![2, 3],
            })),
        }
    }
    fn paired() -> Session {
        let (session, ack) = Session::after_verified_pairing(
            VerifiedOsPeer { surface: "lu.core" },
            HumanTurnConsent { project_id: project(), request_kind: "project_context" },
            &hello(),
        ).unwrap();
        assert_eq!(ack.session_id, session.id);
        assert!(!session.revoked);
        session
    }
    fn context(session: &Session, seq: u64, project_id: String) -> Vec<u8> {
        bridge_wire::encode(&BridgeEnvelope {
            protocol_version: 1, request_id: format!("request-{seq}"),
            session_id: session.id.clone(), sequence: seq,
            body: Some(Body::ContextOffer(ProjectContextOffer {
                project_id, thread_id: "ide-thread".into(),
                source_revision: "c".repeat(64),
                summary: "selected reviewable context".into(),
            })),
        }).unwrap()
    }
    #[test]
    fn default_is_offline_without_peer_or_user_authority() {
        let status = disconnected_status();
        assert!(!status.enabled && !status.paired && !status.peer_authenticated);
        assert!(!status.autonomous_execution_allowed);
        assert!(!status.default_project_disclosure);
    }
    #[test]
    fn cross_project_and_replay_are_denied_even_after_test_pairing() {
        let mut session = paired();
        assert_eq!(session.peer_surface, "lu.core");
        assert_eq!(session.admit_offer(&context(&session,1,project())).unwrap(),
            "context_for_review_only");
        assert_eq!(session.admit_offer(&context(&session,1,project())),
            Err("BRIDGE_SESSION_REPLAY_OR_WRONG_PEER"));
        assert_eq!(session.admit_offer(&context(&session,2,format!("ide-{}", "b".repeat(32)))),
            Err("BRIDGE_CONTEXT_OUT_OF_SCOPE"));
        assert_eq!(session.admit_offer(&context(&session,2,project())).unwrap(),
            "context_for_review_only");
        session.revoke();
        assert_eq!(session.admit_offer(&context(&session,3,project())),
            Err("BRIDGE_SESSION_REVOKED"));
    }
    #[test]
    fn false_peer_claim_and_missing_human_consent_denied() {
        assert!(Session::after_verified_pairing(
            VerifiedOsPeer { surface: "lu.ide" },
            HumanTurnConsent { project_id: project(), request_kind: "project_context" },
            &hello(),
        ).is_err());
        assert!(Session::after_verified_pairing(
            VerifiedOsPeer { surface: "lu.core" },
            HumanTurnConsent { project_id: project(), request_kind: "execution" },
            &hello(),
        ).is_err());
        let mut no_caps = hello();
        if let Some(Body::Hello(info)) = &mut no_caps.body {
            info.advertised_capabilities = vec![1, 4];
        }
        assert!(Session::after_verified_pairing(
            VerifiedOsPeer { surface: "lu.core" },
            HumanTurnConsent { project_id: project(), request_kind: "project_context" },
            &no_caps,
        ).is_err());
    }
    #[test]
    fn proposal_bytes_cannot_gain_execution_rights() {
        let mut session = paired();
        let msg = BridgeEnvelope {
            protocol_version: 1, request_id: "request-1".into(),
            session_id: session.id.clone(), sequence: 1,
            body: Some(Body::ProposalOffer(ProposedChangeOffer {
                project_id: project(), thread_id: "ide-thread".into(),
                task_id: "task-1".into(),
                proposal_json: br#"{\"claimedProducer\":\"Lu\",\"edits\":[]}"#.to_vec(),
            })),
        };
        let bytes = bridge_wire::encode(&msg).unwrap();
        assert_eq!(session.admit_offer(&bytes).unwrap(), "proposal_for_inbox_only");
    }
}
