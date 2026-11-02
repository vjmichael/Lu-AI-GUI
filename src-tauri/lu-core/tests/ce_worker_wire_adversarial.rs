//! Adversarial generated-Protobuf fixtures for the signed CE worker IPC.
//! These tests exercise the untrusted wire boundary without launching a worker
//! or granting a package permission. All frames are generated from core.proto.
use lu_core::wire::{
    self, proto, WireError, CE_WORKER_PROTOCOL_VERSION, MAX_CE_WORKER_FRAME_BYTES,
    MAX_CE_WORKER_PAYLOAD_BYTES,
};
use prost::Message;

use proto::ce_worker_frame::Body;

fn frame(body: Body) -> proto::CeWorkerFrame {
    proto::CeWorkerFrame {
        protocol_version: CE_WORKER_PROTOCOL_VERSION,
        body: Some(body),
    }
}

fn all_valid_bodies() -> Vec<Body> {
    vec![
        Body::Invoke(proto::CeWorkerInvoke {
            call_id: "call".into(),
            tool_id: "publisher.worker.execute".into(),
            input_type_url: "type.googleapis.com/lu.v1.CeExecuteRequest".into(),
            input: vec![0x0a, 0x01, b'x'],
        }),
        Body::Read(proto::CeWorkerRead {
            request_id: 7,
            tool_id: "publisher.worker.read".into(),
            relative_path: "notes.txt".into(),
        }),
        Body::Write(proto::CeWorkerWrite {
            request_id: 8,
            tool_id: "publisher.worker.write".into(),
            relative_path: "notes.txt".into(),
            data: b"safe".to_vec(),
        }),
        Body::BrokerResult(proto::CeWorkerBrokerResult {
            request_id: 9,
            ok: true,
            data: b"safe".to_vec(),
        }),
        Body::Result(proto::CeWorkerResult {
            output_type_url: "type.googleapis.com/lu.v1.CeExecuteResult".into(),
            output: vec![0x0a, 0x02, b'o', b'k'],
        }),
    ]
}

#[test]
fn every_worker_message_rejects_nested_unknown_fields_and_duplicate_oneofs() {
    for body in all_valid_bodies() {
        let original = frame(body);
        let canonical = wire::encode_ce_worker_frame(&original).unwrap();
        assert_eq!(wire::decode_ce_worker_frame(&canonical), Ok(original));

        // These small generated messages have a one-byte nested body length.
        assert_eq!(&canonical[..2], &[0x08, 0x01]);
        assert!(canonical[3] < 125);
        let mut nested_unknown = canonical.clone();
        nested_unknown[3] += 2;
        nested_unknown.extend_from_slice(&[0x78, 0x01]);
        assert!(
            wire::decode_ce_worker_frame(&nested_unknown).is_err(),
            "unknown fields inside generated nested messages must not be ignored"
        );

        let mut duplicate_body = canonical.clone();
        duplicate_body.extend_from_slice(&canonical[2..]);
        assert!(
            wire::decode_ce_worker_frame(&duplicate_body).is_err(),
            "a repeated oneof body must not silently replace the first"
        );

        let mut duplicate_version = canonical.clone();
        duplicate_version.extend_from_slice(&[0x08, 0x01]);
        assert!(wire::decode_ce_worker_frame(&duplicate_version).is_err());

        let mut outer_unknown = canonical.clone();
        outer_unknown.extend_from_slice(&[0x78, 0x01]);
        assert!(wire::decode_ce_worker_frame(&outer_unknown).is_err());

        // Overlong varint encoding of version 1 is legal for permissive
        // Protobuf decoders but is not canonical for this security boundary.
        let mut nonminimal_version = vec![0x08, 0x81, 0x00];
        nonminimal_version.extend_from_slice(&canonical[2..]);
        assert!(wire::decode_ce_worker_frame(&nonminimal_version).is_err());

        let truncated = &canonical[..canonical.len() - 1];
        assert!(wire::decode_ce_worker_frame(truncated).is_err());
    }
}

#[test]
fn generated_worker_frames_reject_invalid_semantics_before_dispatch() {
    let invalid = [
        frame(Body::Read(proto::CeWorkerRead {
            request_id: 0,
            tool_id: "publisher.worker.read".into(),
            relative_path: "notes.txt".into(),
        })),
        frame(Body::Write(proto::CeWorkerWrite {
            request_id: 1,
            tool_id: "publisher.worker.write".into(),
            relative_path: String::new(),
            data: b"safe".to_vec(),
        })),
        frame(Body::Invoke(proto::CeWorkerInvoke {
            call_id: String::new(),
            tool_id: "publisher.worker.execute".into(),
            input_type_url: "type.googleapis.com/lu.v1.CeExecuteRequest".into(),
            input: Vec::new(),
        })),
        frame(Body::Result(proto::CeWorkerResult {
            output_type_url: "unregistered/output".into(),
            output: Vec::new(),
        })),
        frame(Body::BrokerResult(proto::CeWorkerBrokerResult {
            request_id: 1,
            ok: false,
            data: b"secret-on-failure".to_vec(),
        })),
    ];
    for bad in invalid {
        assert!(wire::encode_ce_worker_frame(&bad).is_err());
        assert!(wire::decode_ce_worker_frame(&bad.encode_to_vec()).is_err());
    }

    let missing_body = proto::CeWorkerFrame {
        protocol_version: CE_WORKER_PROTOCOL_VERSION,
        body: None,
    };
    assert_eq!(
        wire::decode_ce_worker_frame(&missing_body.encode_to_vec()),
        Err(WireError::MissingField("body"))
    );

    let oversized = frame(Body::Write(proto::CeWorkerWrite {
        request_id: 1,
        tool_id: "publisher.worker.write".into(),
        relative_path: "notes.txt".into(),
        data: vec![0; MAX_CE_WORKER_PAYLOAD_BYTES + 1],
    }));
    assert_eq!(
        wire::encode_ce_worker_frame(&oversized),
        Err(WireError::Oversized)
    );
    assert_eq!(
        wire::decode_ce_worker_frame(&oversized.encode_to_vec()),
        Err(WireError::Oversized)
    );
    assert_eq!(
        wire::decode_ce_worker_frame(&vec![0; MAX_CE_WORKER_FRAME_BYTES + 1]),
        Err(WireError::Oversized)
    );
}

#[test]
fn malformed_generated_worker_wire_never_reaches_a_valid_frame() {
    for malicious in [
        vec![],
        vec![0xff],
        vec![0x08, 0x80],             // unterminated version varint
        vec![0x0a, 0x01, 0x01],       // version with wrong wire type
        vec![0x08, 0x01, 0x1a, 0x7f], // truncated nested read
    ] {
        assert!(wire::decode_ce_worker_frame(&malicious).is_err());
    }
}
