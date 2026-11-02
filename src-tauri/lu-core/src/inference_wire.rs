//! Canonical Protobuf-only P3 inference transport for Lu-owned runtimes.
//! Frame contents are requests/events, not grants of authority. Host admission
//! resolves references and chooses verified engines; no caller may supply paths.
use crate::inference_image_refs::{validate_image_refs, ImageRef};
use crate::wire::proto;
use prost::Message;

pub const INFERENCE_PROTOCOL_VERSION: u32 = 1;
pub const MAX_INFERENCE_FRAME_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_INFERENCE_PROMPT_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_INFERENCE_CHUNK_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InferenceWireError {
    EmptyFrame,
    Oversized,
    Decode(String),
    Noncanonical,
    UnsupportedVersion(u32),
    InvalidField(&'static str),
}

fn valid_ref(reference: &str) -> bool {
    reference.len() <= 128
        && !reference.is_empty()
        && reference.as_bytes()[0].is_ascii_alphanumeric()
        && reference
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
}

fn check_ref(reference: &str, name: &'static str) -> Result<(), InferenceWireError> {
    if !valid_ref(reference) {
        return Err(InferenceWireError::InvalidField(name));
    }
    Ok(())
}

pub fn validate_frame(frame: &proto::InferenceFrame) -> Result<(), InferenceWireError> {
    use proto::inference_event::Kind;
    use proto::inference_frame::Body;

    if frame.protocol_version != INFERENCE_PROTOCOL_VERSION {
        return Err(InferenceWireError::UnsupportedVersion(
            frame.protocol_version,
        ));
    }
    check_ref(&frame.request_id, "request_id")?;
    let body = frame
        .body
        .as_ref()
        .ok_or(InferenceWireError::InvalidField("body"))?;
    match body {
        Body::Load(load) => {
            check_ref(&load.model_ref, "model_ref")?;
            check_ref(&load.backend_id, "backend_id")?;
            if let Some(projector) = &load.projector_ref {
                check_ref(projector, "projector_ref")?;
                if projector == &load.model_ref {
                    return Err(InferenceWireError::InvalidField("projector_ref"));
                }
            }
            if !(256..=1_048_576).contains(&load.context_tokens) {
                return Err(InferenceWireError::InvalidField("context_tokens"));
            }
        }
        Body::Generate(request) => {
            check_ref(&request.session_id, "session_id")?;
            check_ref(&request.generation_id, "generation_id")?;
            if request.prompt.trim().is_empty()
                || request.prompt.len() > MAX_INFERENCE_PROMPT_BYTES
                || !(1..=32_768).contains(&request.max_new_tokens)
            {
                return Err(InferenceWireError::InvalidField("generation limits"));
            }
            let images: Vec<ImageRef<'_>> = request
                .images
                .iter()
                .map(|image| ImageRef {
                    image_ref: &image.image_ref,
                    mime_type: &image.mime_type,
                    sha256: &image.sha256,
                    size_bytes: image.size_bytes as u64,
                })
                .collect();
            validate_image_refs(&images)
                .map_err(|_| InferenceWireError::InvalidField("image_attachments"))?;
        }
        Body::Cancel(request) => {
            check_ref(&request.session_id, "session_id")?;
            check_ref(&request.generation_id, "generation_id")?;
        }
        Body::Unload(request) => check_ref(&request.session_id, "session_id")?,
        Body::Status(request) => check_ref(&request.session_id, "session_id")?,
        Body::Event(event) => {
            check_ref(&event.session_id, "session_id")?;
            if !event.generation_id.is_empty() {
                check_ref(&event.generation_id, "generation_id")?;
            }
            match event
                .kind
                .as_ref()
                .ok_or(InferenceWireError::InvalidField("event.kind"))?
            {
                Kind::State(state) => {
                    match proto::InferencePhase::try_from(state.phase) {
                        Ok(proto::InferencePhase::Unspecified) | Err(_) => {
                            return Err(InferenceWireError::InvalidField("phase"));
                        }
                        Ok(_) => {}
                    }
                    if !state.backend_id.is_empty() {
                        check_ref(&state.backend_id, "backend_id")?;
                    }
                    if !state.model_ref.is_empty() {
                        check_ref(&state.model_ref, "model_ref")?;
                    }
                }
                Kind::TextChunk(chunk) => {
                    check_ref(&event.generation_id, "generation_id")?;
                    if chunk.text.is_empty() || chunk.text.len() > MAX_INFERENCE_CHUNK_BYTES {
                        return Err(InferenceWireError::InvalidField("text_chunk"));
                    }
                }
                Kind::Finished(_) => check_ref(&event.generation_id, "generation_id")?,
                Kind::Failure(failure) => {
                    check_ref(&failure.code, "error_code")?;
                    if failure.message.len() > 1024 {
                        return Err(InferenceWireError::InvalidField("error_message"));
                    }
                }
                Kind::ResourceUsage(_) => {}
            }
        }
    }
    if frame.encoded_len() > MAX_INFERENCE_FRAME_BYTES {
        return Err(InferenceWireError::Oversized);
    }
    Ok(())
}

pub fn encode_frame(frame: &proto::InferenceFrame) -> Result<Vec<u8>, InferenceWireError> {
    validate_frame(frame)?;
    Ok(frame.encode_to_vec())
}

/// Reject unknown fields, duplicated singular fields, and noncanonical
/// encodings rather than silently dropping/normalizing hostile input.
pub fn decode_frame(bytes: &[u8]) -> Result<proto::InferenceFrame, InferenceWireError> {
    if bytes.is_empty() {
        return Err(InferenceWireError::EmptyFrame);
    }
    if bytes.len() > MAX_INFERENCE_FRAME_BYTES {
        return Err(InferenceWireError::Oversized);
    }
    let frame = proto::InferenceFrame::decode(bytes)
        .map_err(|err| InferenceWireError::Decode(err.to_string()))?;
    validate_frame(&frame)?;
    if frame.encode_to_vec() != bytes {
        return Err(InferenceWireError::Noncanonical);
    }
    Ok(frame)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_frame() -> proto::InferenceFrame {
        proto::InferenceFrame {
            protocol_version: INFERENCE_PROTOCOL_VERSION,
            request_id: "req-1".into(),
            body: Some(proto::inference_frame::Body::Load(
                proto::InferenceLoadRequest {
                    model_ref: "catalog.model-1".into(),
                    backend_id: "llama-cpu".into(),
                    projector_ref: Some("catalog.mmproj-1".into()),
                    context_tokens: 4096,
                },
            )),
        }
    }

    #[test]
    fn load_is_typed_and_canonical() {
        let frame = load_frame();
        let encoded = encode_frame(&frame).unwrap();
        assert_eq!(decode_frame(&encoded).unwrap(), frame);
        let mut unknown = encoded.clone();
        unknown.extend_from_slice(&[0x90, 0x06, 0x01]);
        assert_eq!(
            decode_frame(&unknown),
            Err(InferenceWireError::Noncanonical)
        );
        let mut duplicate_version = encoded;
        duplicate_version.extend_from_slice(&[0x08, 0x01]);
        assert_eq!(
            decode_frame(&duplicate_version),
            Err(InferenceWireError::Noncanonical)
        );
    }

    #[test]
    fn reject_paths_as_host_references_and_unbounded_prompts() {
        let mut frame = load_frame();
        if let Some(proto::inference_frame::Body::Load(ref mut load)) = frame.body {
            load.model_ref = "C:\\models\\unsafe.gguf".into();
        }
        assert_eq!(
            encode_frame(&frame),
            Err(InferenceWireError::InvalidField("model_ref"))
        );
        let generated = proto::InferenceFrame {
            protocol_version: INFERENCE_PROTOCOL_VERSION,
            request_id: "req-2".into(),
            body: Some(proto::inference_frame::Body::Generate(
                proto::InferenceGenerateRequest {
                    session_id: "session-2".into(),
                    generation_id: "gen-2".into(),
                    prompt: "x".repeat(MAX_INFERENCE_PROMPT_BYTES + 1),
                    max_new_tokens: 32,
                    images: Vec::new(),
                },
            )),
        };
        assert!(encode_frame(&generated).is_err());
    }

    #[test]
    fn reject_invalid_version_and_missing_event_details() {
        let mut bad = load_frame();
        bad.protocol_version = 3;
        assert_eq!(
            encode_frame(&bad),
            Err(InferenceWireError::UnsupportedVersion(3))
        );
        let frame = proto::InferenceFrame {
            protocol_version: INFERENCE_PROTOCOL_VERSION,
            request_id: "req-3".into(),
            body: Some(proto::inference_frame::Body::Event(proto::InferenceEvent {
                session_id: "s-3".into(),
                generation_id: String::new(),
                sequence: 0,
                kind: Some(proto::inference_event::Kind::TextChunk(
                    proto::InferenceTextChunk {
                        text: "hello".into(),
                        chunk_index: 0,
                    },
                )),
            })),
        };
        assert_eq!(
            encode_frame(&frame),
            Err(InferenceWireError::InvalidField("generation_id"))
        );
        assert!(matches!(
            decode_frame(&[0; 7]),
            Err(InferenceWireError::Decode(_))
        ));
    }

    #[test]
    fn cancellation_and_event_roundtrip_without_json() {
        let frame = proto::InferenceFrame {
            protocol_version: INFERENCE_PROTOCOL_VERSION,
            request_id: "cancel-4".into(),
            body: Some(proto::inference_frame::Body::Cancel(
                proto::InferenceCancelRequest {
                    session_id: "s-4".into(),
                    generation_id: "g-4".into(),
                },
            )),
        };
        assert_eq!(decode_frame(&encode_frame(&frame).unwrap()).unwrap(), frame);
        let event = proto::InferenceFrame {
            protocol_version: INFERENCE_PROTOCOL_VERSION,
            request_id: "ev-4".into(),
            body: Some(proto::inference_frame::Body::Event(proto::InferenceEvent {
                session_id: "s-4".into(),
                generation_id: "g-4".into(),
                sequence: 4,
                kind: Some(proto::inference_event::Kind::Finished(
                    proto::InferenceFinished {
                        cancelled: true,
                        generated_bytes: 10,
                        output_pieces: 1,
                    },
                )),
            })),
        };
        assert_eq!(decode_frame(&encode_frame(&event).unwrap()).unwrap(), event);
    }
    #[test]
    fn multimodal_image_refs_are_typed_and_canonical() {
        let frame = proto::InferenceFrame {
            protocol_version: INFERENCE_PROTOCOL_VERSION,
            request_id: "req-image".into(),
            body: Some(proto::inference_frame::Body::Generate(
                proto::InferenceGenerateRequest {
                    session_id: "s1".into(),
                    generation_id: "g1".into(),
                    prompt: "Describe the image".into(),
                    max_new_tokens: 128,
                    images: vec![proto::InferenceImageAttachment {
                        image_ref: "image-1".into(),
                        mime_type: "image/png".into(),
                        sha256: vec![0x11; 32],
                        size_bytes: 4096,
                    }],
                },
            )),
        };
        assert_eq!(decode_frame(&encode_frame(&frame).unwrap()).unwrap(), frame);
        let mut bad = frame;
        if let Some(proto::inference_frame::Body::Generate(ref mut generate)) = bad.body {
            generate.images[0].image_ref = "C:\\other\\image.png".into();
        }
        assert_eq!(
            encode_frame(&bad),
            Err(InferenceWireError::InvalidField("image_attachments"))
        );
    }
}
