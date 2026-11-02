//! Lu-owned inference host/Protobuf adapter (P3). No Tauri or Jan dependency.
//! The trusted desktop host resolves model_refs to AdmittedModel handles;
//! this layer never opens a path provided by an untrusted request.
use crate::inference_host::{
    Cancellation, EngineError, EnginePhase, GenerationRequest, InferenceBackend, InferenceHost,
    TokenChunk,
};
use crate::inference_wire::{
    encode_frame, validate_frame, InferenceWireError, INFERENCE_PROTOCOL_VERSION,
};
use crate::mmproj_admission::AdmittedProjector;
use crate::model_admission::AdmittedModel;
use crate::multimodal_saved_turn::PinnedImage;
use crate::wire::proto;

#[derive(Debug, PartialEq, Eq)]
pub enum BridgeError {
    Wire(InferenceWireError),
    UnauthorizedSession,
    UnauthorizedModel,
    UnauthorizedGeneration,
    ContextMismatch,
    UnsupportedBackend,
    UnsupportedModality,
    WrongOperation,
    Engine(EngineError),
    Consumer,
}
impl From<InferenceWireError> for BridgeError {
    fn from(e: InferenceWireError) -> Self {
        Self::Wire(e)
    }
}
impl From<EngineError> for BridgeError {
    fn from(e: EngineError) -> Self {
        Self::Engine(e)
    }
}

pub struct InferenceBridge<B: InferenceBackend> {
    host: InferenceHost<B>,
    session_id: String,
    backend_id: String,
    context_tokens: u32,
    model_ref: Option<String>,
    sequence: u64,
}
impl<B: InferenceBackend> InferenceBridge<B> {
    /// Both IDs MUST be allocated by the trusted host, never model output.
    pub fn new(
        backend: B,
        session_id: String,
        backend_id: String,
        context_tokens: u32,
    ) -> Result<Self, BridgeError> {
        let probe = proto::InferenceFrame {
            protocol_version: INFERENCE_PROTOCOL_VERSION,
            request_id: "probe".into(),
            body: Some(proto::inference_frame::Body::Status(
                proto::InferenceStatusRequest {
                    session_id: session_id.clone(),
                },
            )),
        };
        validate_frame(&probe)?;
        let probe = proto::InferenceFrame {
            protocol_version: INFERENCE_PROTOCOL_VERSION,
            request_id: "probe".into(),
            body: Some(proto::inference_frame::Body::Load(
                proto::InferenceLoadRequest {
                    model_ref: "probe".into(),
                    backend_id: backend_id.clone(),
                    projector_ref: None,
                    context_tokens,
                },
            )),
        };
        validate_frame(&probe)?;
        Ok(Self {
            host: InferenceHost::new(backend),
            session_id,
            backend_id,
            context_tokens,
            model_ref: None,
            sequence: 0,
        })
    }
    pub fn phase(&self) -> EnginePhase {
        self.host.phase()
    }
    pub fn process_id(&self) -> Option<u32> {
        self.host.process_id()
    }

    fn next_event(
        &mut self,
        request_id: &str,
        generation_id: &str,
        kind: proto::inference_event::Kind,
    ) -> Result<Vec<u8>, BridgeError> {
        self.sequence = self.sequence.saturating_add(1);
        encode_frame(&proto::InferenceFrame {
            protocol_version: INFERENCE_PROTOCOL_VERSION,
            request_id: request_id.to_owned(),
            body: Some(proto::inference_frame::Body::Event(proto::InferenceEvent {
                session_id: self.session_id.clone(),
                generation_id: generation_id.to_owned(),
                sequence: self.sequence,
                kind: Some(kind),
            })),
        })
        .map_err(Into::into)
    }
    fn state(
        &mut self,
        request_id: &str,
        phase: proto::InferencePhase,
    ) -> Result<Vec<u8>, BridgeError> {
        self.next_event(
            request_id,
            "",
            proto::inference_event::Kind::State(proto::InferenceStateChanged {
                phase: phase as i32,
                backend_id: self.backend_id.clone(),
                model_ref: self.model_ref.clone().unwrap_or_default(),
            }),
        )
    }
    pub fn load(
        &mut self,
        frame: &proto::InferenceFrame,
        bound_model_ref: &str,
        admitted: &AdmittedModel,
    ) -> Result<Vec<u8>, BridgeError> {
        self.load_with_projector(frame, bound_model_ref, admitted, None)
    }
    /// The projector is a separately admitted file, tied to this exact model
    /// catalog identity. The opaque ref is not a permission or path.
    pub fn load_with_projector(
        &mut self,
        frame: &proto::InferenceFrame,
        bound_model_ref: &str,
        admitted: &AdmittedModel,
        projector: Option<(&str, &AdmittedProjector)>,
    ) -> Result<Vec<u8>, BridgeError> {
        validate_frame(frame)?;
        let Some(proto::inference_frame::Body::Load(request)) = frame.body.as_ref() else {
            return Err(BridgeError::WrongOperation);
        };
        if request.backend_id != self.backend_id {
            return Err(BridgeError::UnsupportedBackend);
        }
        // The host must bind the opaque reference to the exact admitted file
        // and provision a matching context limit before any backend load.
        if request.model_ref != bound_model_ref {
            return Err(BridgeError::UnauthorizedModel);
        }
        if request.context_tokens != self.context_tokens {
            return Err(BridgeError::ContextMismatch);
        }
        match (&request.projector_ref, projector) {
            (None, None) => {}
            (Some(received), Some((approved, _))) if received == approved => {}
            _ => return Err(BridgeError::UnauthorizedModel),
        }
        self.host
            .load_with_projector(admitted, projector.map(|(_, p)| p))?;
        self.model_ref = Some(request.model_ref.clone());
        self.state(&frame.request_id, proto::InferencePhase::Ready)
    }
    pub fn unload(&mut self, frame: &proto::InferenceFrame) -> Result<Vec<u8>, BridgeError> {
        validate_frame(frame)?;
        let Some(proto::inference_frame::Body::Unload(request)) = frame.body.as_ref() else {
            return Err(BridgeError::WrongOperation);
        };
        if request.session_id != self.session_id {
            return Err(BridgeError::UnauthorizedSession);
        }
        self.host.unload()?;
        self.model_ref = None;
        self.state(&frame.request_id, proto::InferencePhase::Idle)
    }
    /// Signal an active generation without acquiring the mutable inference
    /// host lock. The trusted supervisor supplies the registered generation
    /// identity and its existing cancellation token, not model-controlled IDs.
    /// This only requests cancellation; the backend must confirm termination.
    pub fn signal_cancel(
        frame: &proto::InferenceFrame,
        active_session_id: &str,
        active_generation_id: &str,
        cancellation: &Cancellation,
    ) -> Result<(), BridgeError> {
        validate_frame(frame)?;
        let Some(proto::inference_frame::Body::Cancel(request)) = frame.body.as_ref() else {
            return Err(BridgeError::WrongOperation);
        };
        if request.session_id != active_session_id {
            return Err(BridgeError::UnauthorizedSession);
        }
        if request.generation_id != active_generation_id {
            return Err(BridgeError::UnauthorizedGeneration);
        }
        cancellation.cancel();
        Ok(())
    }

    pub fn status(&mut self, frame: &proto::InferenceFrame) -> Result<Vec<u8>, BridgeError> {
        validate_frame(frame)?;
        let Some(proto::inference_frame::Body::Status(request)) = frame.body.as_ref() else {
            return Err(BridgeError::WrongOperation);
        };
        if request.session_id != self.session_id {
            return Err(BridgeError::UnauthorizedSession);
        }
        let phase = match self.host.phase() {
            EnginePhase::Idle => proto::InferencePhase::Idle,
            EnginePhase::Ready => proto::InferencePhase::Ready,
            EnginePhase::Generating => proto::InferencePhase::Generating,
        };
        self.state(&frame.request_id, phase)
    }
    /// This is called from a host-owned worker thread. Cancellation must use a
    /// separately accessible cloned token, NOT a mutex held by this generator.
    pub fn generate(
        &mut self,
        frame: &proto::InferenceFrame,
        cancel: Cancellation,
        emit: impl FnMut(Vec<u8>) -> Result<(), BridgeError>,
    ) -> Result<(), BridgeError> {
        self.generate_with_verified_images(frame, &[], cancel, emit)
    }
    /// The caller must supply the host-pinned images from the EXACT saved turn.
    /// Protobuf image refs alone are never accepted as authority to load data.
    pub fn generate_with_verified_images(
        &mut self,
        frame: &proto::InferenceFrame,
        images: &[PinnedImage],
        cancel: Cancellation,
        mut emit: impl FnMut(Vec<u8>) -> Result<(), BridgeError>,
    ) -> Result<(), BridgeError> {
        validate_frame(frame)?;
        let Some(proto::inference_frame::Body::Generate(request)) = frame.body.as_ref() else {
            return Err(BridgeError::WrongOperation);
        };
        if request.session_id != self.session_id {
            return Err(BridgeError::UnauthorizedSession);
        }
        // No image is authorized by its reference string. Require one-to-one
        // matching host-pinned bytes, MIME, size and digest for this request.
        if request.images.len() != images.len() {
            return Err(BridgeError::UnsupportedModality);
        }
        for (reference, pinned) in request.images.iter().zip(images.iter()) {
            if reference.image_ref != pinned.image_ref
                || reference.mime_type != pinned.mime_type
                || reference.sha256.as_slice() != pinned.sha256
                || usize::try_from(reference.size_bytes).ok() != Some(pinned.bytes.len())
            {
                return Err(BridgeError::UnsupportedModality);
            }
        }
        let request_id = frame.request_id.clone();
        let generation_id = request.generation_id.clone();
        let started = self.next_event(
            &request_id,
            &generation_id,
            proto::inference_event::Kind::State(proto::InferenceStateChanged {
                phase: proto::InferencePhase::Generating as i32,
                backend_id: self.backend_id.clone(),
                model_ref: self.model_ref.clone().unwrap_or_default(),
            }),
        )?;
        // No generating event is emitted unless engine is already ready.
        if self.host.phase() != EnginePhase::Ready {
            return Err(BridgeError::Engine(EngineError::NotLoaded));
        }
        emit(started)?;
        let session_id = self.session_id.clone();
        let mut sequence = self.sequence;
        let mut pieces = 0u32;
        let mut bytes = 0u64;
        let generation = GenerationRequest {
            prompt: request.prompt.clone(),
            max_new_tokens: request.max_new_tokens,
        };
        let mut forward_chunk = |chunk: TokenChunk| {
            sequence = sequence.saturating_add(1);
            pieces = pieces.saturating_add(1);
            bytes = bytes.saturating_add(chunk.text.len() as u64);
            let event = proto::InferenceFrame {
                protocol_version: INFERENCE_PROTOCOL_VERSION,
                request_id: request_id.clone(),
                body: Some(proto::inference_frame::Body::Event(proto::InferenceEvent {
                    session_id: session_id.clone(),
                    generation_id: generation_id.clone(),
                    sequence,
                    kind: Some(proto::inference_event::Kind::TextChunk(
                        proto::InferenceTextChunk {
                            text: chunk.text,
                            chunk_index: chunk.index,
                        },
                    )),
                })),
            };
            let wire = encode_frame(&event)
                .map_err(|e| EngineError::Backend(format!("invalid streamed event: {e:?}")))?;
            emit(wire).map_err(|_| EngineError::Backend("stream receiver unavailable".into()))
        };
        let result = if images.is_empty() {
            self.host
                .generate(generation, cancel.clone(), &mut forward_chunk)
        } else {
            self.host
                .generate_images(generation, images, cancel.clone(), &mut forward_chunk)
        };
        self.sequence = sequence;
        // A final status belongs to the same correlation ID. Error details from
        // untrusted engines are never surfaced as raw path/prompt diagnostics.
        match result {
            Ok(()) => {
                let completed = self.next_event(
                    &request_id,
                    &generation_id,
                    proto::inference_event::Kind::Finished(proto::InferenceFinished {
                        cancelled: false,
                        generated_bytes: bytes,
                        output_pieces: pieces,
                    }),
                )?;
                emit(completed)?;
                Ok(())
            }
            Err(error) => {
                let category = if cancel.is_cancelled() {
                    "CANCELLED"
                } else {
                    "GENERATION_FAILED"
                };
                let failed = self.next_event(
                    &request_id,
                    &generation_id,
                    proto::inference_event::Kind::Failure(proto::InferenceFailure {
                        code: category.into(),
                        retryable: false,
                        message: "Native inference did not complete".into(),
                    }),
                )?;
                emit(failed)?;
                Err(BridgeError::Engine(error))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference_wire::decode_frame;
    use crate::model_admission::ModelAdmissionManager;
    use std::{fs, io::Write};

    struct FakeBackend;
    impl InferenceBackend for FakeBackend {
        fn load(&mut self, _: &AdmittedModel) -> Result<(), String> {
            Ok(())
        }
        fn unload(&mut self) {}
        fn generate(
            &mut self,
            _: &GenerationRequest,
            _: &Cancellation,
            emit: &mut dyn FnMut(TokenChunk) -> Result<(), EngineError>,
        ) -> Result<(), String> {
            emit(TokenChunk {
                text: "hello".into(),
                index: 0,
            })
            .map_err(|e| format!("{e:?}"))?;
            Ok(())
        }
    }
    fn admitted() -> (std::path::PathBuf, ModelAdmissionManager) {
        let dir = std::env::temp_dir().join(format!(
            "lu-infer-bridge-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        fs::create_dir_all(&dir).unwrap();
        let mut f = fs::File::create(dir.join("tiny.gguf")).unwrap();
        f.write_all(b"GGUF").unwrap();
        f.write_all(&3u32.to_le_bytes()).unwrap();
        f.write_all(&1u64.to_le_bytes()).unwrap();
        f.write_all(&1u64.to_le_bytes()).unwrap();
        drop(f);
        let mut models = ModelAdmissionManager::new();
        models.admit(&dir, "tiny.gguf").unwrap();
        (dir, models)
    }
    fn frame(request_id: &str, body: proto::inference_frame::Body) -> proto::InferenceFrame {
        proto::InferenceFrame {
            protocol_version: INFERENCE_PROTOCOL_VERSION,
            request_id: request_id.into(),
            body: Some(body),
        }
    }
    #[test]
    fn host_only_model_load_and_protobuf_stream() {
        let (dir, models) = admitted();
        let mut bridge =
            InferenceBridge::new(FakeBackend, "host-session".into(), "llama-cpu".into(), 512)
                .unwrap();
        let load = frame(
            "load-1",
            proto::inference_frame::Body::Load(proto::InferenceLoadRequest {
                model_ref: "model-1".into(),
                backend_id: "llama-cpu".into(),
                projector_ref: None,
                context_tokens: 512,
            }),
        );
        let event = bridge
            .load(&load, "model-1", models.current().unwrap())
            .unwrap();
        assert!(decode_frame(&event).is_ok());
        let generate = frame(
            "generate-1",
            proto::inference_frame::Body::Generate(proto::InferenceGenerateRequest {
                session_id: "host-session".into(),
                generation_id: "gen-1".into(),
                prompt: "Hello".into(),
                max_new_tokens: 8,
                images: Vec::new(),
            }),
        );
        let mut events = Vec::new();
        bridge
            .generate(&generate, Cancellation::default(), |bytes| {
                events.push(decode_frame(&bytes).unwrap());
                Ok(())
            })
            .unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].request_id, "generate-1");
        assert!(matches!(
            events[1].body.as_ref(),
            Some(proto::inference_frame::Body::Event(proto::InferenceEvent {
                kind: Some(proto::inference_event::Kind::TextChunk(_)),
                ..
            }))
        ));
        assert!(matches!(
            events[2].body.as_ref(),
            Some(proto::inference_frame::Body::Event(proto::InferenceEvent {
                kind: Some(proto::inference_event::Kind::Finished(_)),
                ..
            }))
        ));
        let unload = frame(
            "unload-1",
            proto::inference_frame::Body::Unload(proto::InferenceUnloadRequest {
                session_id: "host-session".into(),
            }),
        );
        bridge.unload(&unload).unwrap();
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn cancellation_signal_checks_host_active_generation() {
        let token = Cancellation::default();
        let request = frame(
            "cancel-1",
            proto::inference_frame::Body::Cancel(proto::InferenceCancelRequest {
                session_id: "host-session".into(),
                generation_id: "gen-1".into(),
            }),
        );
        assert_eq!(
            InferenceBridge::<FakeBackend>::signal_cancel(
                &request,
                "host-session",
                "gen-2",
                &token,
            ),
            Err(BridgeError::UnauthorizedGeneration)
        );
        assert!(!token.is_cancelled());
        assert!(InferenceBridge::<FakeBackend>::signal_cancel(
            &request,
            "host-session",
            "gen-1",
            &token,
        )
        .is_ok());
        assert!(token.is_cancelled());
    }

    #[test]
    fn rejects_wrong_session_and_unauthorized_projector() {
        let (dir, models) = admitted();
        let mut bridge =
            InferenceBridge::new(FakeBackend, "host-session".into(), "llama-cpu".into(), 512)
                .unwrap();
        let with_projector = frame(
            "load-2",
            proto::inference_frame::Body::Load(proto::InferenceLoadRequest {
                model_ref: "model-1".into(),
                backend_id: "llama-cpu".into(),
                projector_ref: Some("mmproj-1".into()),
                context_tokens: 512,
            }),
        );
        assert_eq!(
            bridge.load(&with_projector, "model-1", models.current().unwrap()),
            Err(BridgeError::UnauthorizedModel)
        );
        let without_projector = frame(
            "load-3",
            proto::inference_frame::Body::Load(proto::InferenceLoadRequest {
                model_ref: "model-1".into(),
                backend_id: "llama-cpu".into(),
                projector_ref: None,
                context_tokens: 512,
            }),
        );
        assert_eq!(
            bridge.load(&without_projector, "model-2", models.current().unwrap()),
            Err(BridgeError::UnauthorizedModel)
        );
        let wrong_context = frame(
            "load-4",
            proto::inference_frame::Body::Load(proto::InferenceLoadRequest {
                model_ref: "model-1".into(),
                backend_id: "llama-cpu".into(),
                projector_ref: None,
                context_tokens: 1024,
            }),
        );
        assert_eq!(
            bridge.load(&wrong_context, "model-1", models.current().unwrap()),
            Err(BridgeError::ContextMismatch)
        );
        let unload = frame(
            "unload-2",
            proto::inference_frame::Body::Unload(proto::InferenceUnloadRequest {
                session_id: "other-session".into(),
            }),
        );
        assert_eq!(
            bridge.unload(&unload),
            Err(BridgeError::UnauthorizedSession)
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
