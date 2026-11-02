//! P2 host-contract regression independent of the Windows llama.cpp smoke.
use lu_core::inference_host::{
    Cancellation, EngineError, EnginePhase, GenerationRequest, InferenceBackend, InferenceHost,
    TokenChunk,
};
use lu_core::model_admission::{AdmittedModel, ModelAdmissionManager};
use std::fs::{self, File};
use std::io::Write;

struct ControlledBackend {
    loaded: bool,
}

impl InferenceBackend for ControlledBackend {
    fn load(&mut self, _model: &AdmittedModel) -> Result<(), String> {
        self.loaded = true;
        Ok(())
    }

    fn unload(&mut self) {
        self.loaded = false;
    }

    fn generate(
        &mut self,
        _request: &GenerationRequest,
        _cancel: &Cancellation,
        emit: &mut dyn FnMut(TokenChunk) -> Result<(), EngineError>,
    ) -> Result<(), String> {
        if !self.loaded {
            return Err("backend not loaded".into());
        }
        emit(TokenChunk {
            text: "hello".into(),
            index: 0,
        })
        .map_err(|err| format!("{err:?}"))?;
        emit(TokenChunk {
            text: " world".into(),
            index: 1,
        })
        .map_err(|err| format!("{err:?}"))?;
        Ok(())
    }
}

#[test]
fn callback_cancellation_fails_closed_and_host_can_unload() {
    let directory = std::env::temp_dir().join(format!(
        "lu-p2-contract-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    fs::create_dir_all(&directory).unwrap();
    let mut file = File::create(directory.join("fixture.gguf")).unwrap();
    file.write_all(b"GGUF").unwrap();
    file.write_all(&3_u32.to_le_bytes()).unwrap();
    file.write_all(&1_u64.to_le_bytes()).unwrap();
    file.write_all(&1_u64.to_le_bytes()).unwrap();
    drop(file);

    let mut manager = ModelAdmissionManager::new();
    manager.admit(&directory, "fixture.gguf").unwrap();
    let mut host = InferenceHost::new(ControlledBackend { loaded: false });
    host.load(manager.current().unwrap()).unwrap();
    assert_eq!(host.phase(), EnginePhase::Ready);

    let cancelled = Cancellation::default();
    let signal = cancelled.clone();
    let mut received = 0;
    let result = host.generate(
        GenerationRequest {
            prompt: "say hello".into(),
            max_new_tokens: 8,
        },
        cancelled,
        |_chunk| {
            received += 1;
            signal.cancel();
            Ok(())
        },
    );
    assert_eq!(received, 1);
    assert_eq!(result, Err(EngineError::Cancelled));
    assert_eq!(host.phase(), EnginePhase::Ready);

    host.unload().unwrap();
    assert_eq!(host.phase(), EnginePhase::Idle);
    fs::remove_dir_all(directory).unwrap();
}
