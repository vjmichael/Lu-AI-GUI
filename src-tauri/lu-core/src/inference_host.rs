//! Lu-owned inference engine lifecycle and streaming control surface.
//! Backend-agnostic host; actual GGUF execution must be implemented by a
//! separately verified, in-process backend (not Jan's inference manager).
use crate::mmproj_admission::AdmittedProjector;
use crate::model_admission::AdmittedModel;
use crate::multimodal_saved_turn::PinnedImage;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnginePhase {
    Idle,
    Ready,
    Generating,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    Busy,
    NotLoaded,
    InvalidRequest,
    Cancelled,
    Backend(String),
    AdmissionChanged,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationRequest {
    pub prompt: String,
    pub max_new_tokens: u32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenChunk {
    pub text: String,
    pub index: u32,
}
/// A thread-safe signal held by the host; UI cancellation does not require
/// model cooperation to reach this flag. Backends must check it frequently.
#[derive(Clone, Default)]
pub struct Cancellation {
    flag: Arc<AtomicBool>,
}
impl Cancellation {
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::Acquire)
    }
}
/// The backend must pin or import immutable bytes after admission. Never
/// trust only metadata recheck as a race-free model identity guarantee.
pub trait InferenceBackend {
    fn process_id(&self) -> Option<u32> {
        None
    }
    fn load(&mut self, admitted: &AdmittedModel) -> Result<(), String>;
    fn load_with_projector(
        &mut self,
        model: &AdmittedModel,
        projector: Option<&AdmittedProjector>,
    ) -> Result<(), String> {
        if projector.is_some() {
            return Err("inference backend does not support MMProj".into());
        }
        self.load(model)
    }
    fn unload(&mut self);
    /// Image bytes can only originate from the host's verified saved-turn
    /// resolver. A backend without vision support fails closed.
    fn generate_with_images(
        &mut self,
        _request: &GenerationRequest,
        _images: &[PinnedImage],
        _cancel: &Cancellation,
        _emit: &mut dyn FnMut(TokenChunk) -> Result<(), EngineError>,
    ) -> Result<(), String> {
        Err("inference backend does not support image generation".into())
    }
    fn generate(
        &mut self,
        request: &GenerationRequest,
        cancel: &Cancellation,
        emit: &mut dyn FnMut(TokenChunk) -> Result<(), EngineError>,
    ) -> Result<(), String>;
}
pub struct InferenceHost<B: InferenceBackend> {
    backend: B,
    phase: EnginePhase,
    cancellation: Option<Cancellation>,
}
impl<B: InferenceBackend> InferenceHost<B> {
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            phase: EnginePhase::Idle,
            cancellation: None,
        }
    }
    pub fn phase(&self) -> EnginePhase {
        self.phase
    }
    pub fn process_id(&self) -> Option<u32> {
        self.backend.process_id()
    }
    pub fn load(&mut self, model: &AdmittedModel) -> Result<(), EngineError> {
        if self.phase != EnginePhase::Idle {
            return Err(EngineError::Busy);
        }
        model.recheck().map_err(|_| EngineError::AdmissionChanged)?;
        self.backend.load(model).map_err(EngineError::Backend)?;
        self.phase = EnginePhase::Ready;
        Ok(())
    }
    pub fn load_with_projector(
        &mut self,
        model: &AdmittedModel,
        projector: Option<&AdmittedProjector>,
    ) -> Result<(), EngineError> {
        if self.phase != EnginePhase::Idle {
            return Err(EngineError::Busy);
        }
        model.recheck().map_err(|_| EngineError::AdmissionChanged)?;
        if let Some(projector) = projector {
            projector
                .recheck()
                .map_err(|_| EngineError::AdmissionChanged)?;
        }
        self.backend
            .load_with_projector(model, projector)
            .map_err(EngineError::Backend)?;
        self.phase = EnginePhase::Ready;
        Ok(())
    }
    pub fn unload(&mut self) -> Result<(), EngineError> {
        if self.phase == EnginePhase::Generating {
            return Err(EngineError::Busy);
        }
        if self.phase == EnginePhase::Ready {
            self.backend.unload();
        }
        self.phase = EnginePhase::Idle;
        Ok(())
    }
    pub fn generate(
        &mut self,
        request: GenerationRequest,
        cancel: Cancellation,
        mut emit: impl FnMut(TokenChunk) -> Result<(), EngineError>,
    ) -> Result<(), EngineError> {
        if self.phase != EnginePhase::Ready {
            return Err(EngineError::NotLoaded);
        }
        if request.prompt.trim().is_empty()
            || request.max_new_tokens == 0
            || request.max_new_tokens > 32768
            || request.prompt.len() > 4 * 1024 * 1024
        {
            return Err(EngineError::InvalidRequest);
        }
        if cancel.is_cancelled() {
            return Err(EngineError::Cancelled);
        }
        self.phase = EnginePhase::Generating;
        self.cancellation = Some(cancel.clone());
        let result = self.backend.generate(&request, &cancel, &mut |chunk| {
            if cancel.is_cancelled() {
                return Err(EngineError::Cancelled);
            }
            emit(chunk)
        });
        self.cancellation = None;
        self.phase = EnginePhase::Ready;
        if cancel.is_cancelled() {
            return Err(EngineError::Cancelled);
        }
        result.map_err(EngineError::Backend)
    }
    pub fn generate_images(
        &mut self,
        request: GenerationRequest,
        images: &[PinnedImage],
        cancel: Cancellation,
        mut emit: impl FnMut(TokenChunk) -> Result<(), EngineError>,
    ) -> Result<(), EngineError> {
        if self.phase != EnginePhase::Ready {
            return Err(EngineError::NotLoaded);
        }
        if images.is_empty()
            || images.len() > 4
            || request.prompt.trim().is_empty()
            || request.prompt.len() > 16 * 1024
            || !(1..=32768).contains(&request.max_new_tokens)
        {
            return Err(EngineError::InvalidRequest);
        }
        if cancel.is_cancelled() {
            return Err(EngineError::Cancelled);
        }
        self.phase = EnginePhase::Generating;
        self.cancellation = Some(cancel.clone());
        let result = self
            .backend
            .generate_with_images(&request, images, &cancel, &mut |chunk| {
                if cancel.is_cancelled() {
                    return Err(EngineError::Cancelled);
                }
                emit(chunk)
            });
        self.cancellation = None;
        self.phase = EnginePhase::Ready;
        if cancel.is_cancelled() {
            return Err(EngineError::Cancelled);
        }
        result.map_err(EngineError::Backend)
    }
    pub fn cancel(&self) {
        if let Some(cancel) = &self.cancellation {
            cancel.cancel();
        }
    }
}
impl<B: InferenceBackend> Drop for InferenceHost<B> {
    fn drop(&mut self) {
        if self.phase != EnginePhase::Idle {
            self.backend.unload();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_admission::ModelAdmissionManager;
    use std::{fs, io::Write};
    struct FakeBackend {
        loaded: bool,
    }
    impl InferenceBackend for FakeBackend {
        fn load(&mut self, _: &AdmittedModel) -> Result<(), String> {
            self.loaded = true;
            Ok(())
        }
        fn unload(&mut self) {
            self.loaded = false;
        }
        fn generate(
            &mut self,
            _: &GenerationRequest,
            cancel: &Cancellation,
            emit: &mut dyn FnMut(TokenChunk) -> Result<(), EngineError>,
        ) -> Result<(), String> {
            if !self.loaded {
                return Err("not loaded".into());
            }
            if !cancel.is_cancelled() {
                emit(TokenChunk {
                    text: "hello".into(),
                    index: 0,
                })
                .map_err(|e| format!("{e:?}"))?;
            }
            Ok(())
        }
    }
    #[test]
    fn a_text_backend_never_silently_converts_an_image_turn() {
        let root = std::env::temp_dir().join(format!("lu-engine-image-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let mut f = fs::File::create(root.join("small.gguf")).unwrap();
        f.write_all(b"GGUF").unwrap();
        f.write_all(&3u32.to_le_bytes()).unwrap();
        f.write_all(&1u64.to_le_bytes()).unwrap();
        f.write_all(&1u64.to_le_bytes()).unwrap();
        drop(f);
        let mut catalog = ModelAdmissionManager::new();
        catalog.admit(&root, "small.gguf").unwrap();
        let mut host = InferenceHost::new(FakeBackend { loaded: false });
        host.load(catalog.current().unwrap()).unwrap();
        let image = crate::multimodal_saved_turn::PinnedImage {
            image_ref: "img-test".into(),
            mime_type: "image/png",
            sha256: [0x31; 32],
            bytes: vec![0x89, 0x50, 0x4e, 0x47],
        };
        let result = host.generate_images(
            GenerationRequest {
                prompt: "Describe it".into(),
                max_new_tokens: 50,
            },
            &[image],
            Cancellation::default(),
            |_| Ok(()),
        );
        assert!(matches!(result, Err(EngineError::Backend(_))));
        assert_eq!(host.phase(), EnginePhase::Ready);
        host.unload().unwrap();
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn lifecycle_and_streaming_contract() {
        let root = std::env::temp_dir().join(format!("lu-engine-host-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let mut file = fs::File::create(root.join("test.gguf")).unwrap();
        file.write_all(b"GGUF").unwrap();
        file.write_all(&3u32.to_le_bytes()).unwrap();
        file.write_all(&1u64.to_le_bytes()).unwrap();
        file.write_all(&1u64.to_le_bytes()).unwrap();
        drop(file);
        let mut admission = ModelAdmissionManager::new();
        admission.admit(&root, "test.gguf").unwrap();
        let mut host = InferenceHost::new(FakeBackend { loaded: false });
        assert_eq!(host.phase(), EnginePhase::Idle);
        assert_eq!(
            host.generate(
                GenerationRequest {
                    prompt: "x".into(),
                    max_new_tokens: 1
                },
                Cancellation::default(),
                |_| Ok(())
            ),
            Err(EngineError::NotLoaded)
        );
        host.load(admission.current().unwrap()).unwrap();
        let mut chunks = vec![];
        host.generate(
            GenerationRequest {
                prompt: "x".into(),
                max_new_tokens: 8,
            },
            Cancellation::default(),
            |c| {
                chunks.push(c.text);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(chunks, vec!["hello"]);
        assert_eq!(host.phase(), EnginePhase::Ready);
        host.unload().unwrap();
        assert_eq!(host.phase(), EnginePhase::Idle);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn pre_cancelled_generation_does_not_enter_backend() {
        let cancel = Cancellation::default();
        cancel.cancel();
        assert!(cancel.is_cancelled());
    }
}
