//! R10 regression: the Windows Lu model UI imports after startup, so a
//! supervisor constructed with no model must accept host-authorized models.
//! Projector registration is bound to the exact model reference, and the
//! text-only backend must reject rather than drop an attached projector.
use lu_core::{
    inference_host::{Cancellation, EngineError, GenerationRequest, InferenceBackend, TokenChunk},
    inference_supervisor::{SupervisorError, TrustedInferenceSupervisor},
    mmproj_admission::admit_projector,
    model_admission::{AdmittedModel, ModelAdmissionManager},
    projector_identity::projector_filename_for_model,
};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

struct TextOnly;
impl InferenceBackend for TextOnly {
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
            text: "yes".into(),
            index: 0,
        })
        .map_err(|e| format!("{e:?}"))
    }
}
fn fixture() -> (PathBuf, AdmittedModel) {
    let tick = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("lu-r10-catalog-{}-{tick}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let mut file = fs::File::create(root.join("model.gguf")).unwrap();
    file.write_all(b"GGUF").unwrap();
    file.write_all(&3u32.to_le_bytes()).unwrap();
    file.write_all(&1u64.to_le_bytes()).unwrap();
    file.write_all(&1u64.to_le_bytes()).unwrap();
    drop(file);
    let mut manager = ModelAdmissionManager::new();
    let model = manager.admit(&root, "model.gguf").unwrap().clone();
    (root, model)
}
#[test]
fn runtime_import_and_restart_retain_host_scoping() {
    let (root, model) = fixture();
    let supervisor =
        TrustedInferenceSupervisor::new("llama-cpu".into(), 512, vec![], || Ok(TextOnly)).unwrap();
    assert!(supervisor.model_ids().is_empty());
    assert_eq!(
        supervisor.load("model-1", "session-1".into(), "req-0"),
        Err(SupervisorError::UnknownModel)
    );
    supervisor
        .register_model("model-1".into(), model.clone())
        .unwrap();
    assert_eq!(supervisor.model_ids(), vec!["model-1".to_string()]);
    assert_eq!(
        supervisor.register_model("model-1".into(), model.clone()),
        Err(SupervisorError::InvalidRequest)
    );
    supervisor
        .load("model-1", "session-1".into(), "req-1")
        .unwrap();
    assert_eq!(
        supervisor.register_model("model-2".into(), model.clone()),
        Err(SupervisorError::Busy)
    );
    supervisor.unload("session-1", "req-2").unwrap();
    supervisor
        .load("model-1", "session-2".into(), "req-3")
        .unwrap();
    supervisor.unload("session-2", "req-4").unwrap();
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn paired_projector_bound_to_one_model_rejects_text_only_backend() {
    let (root, model) = fixture();
    let id = "m-00112233445566778899aabbccddeeff";
    let name = projector_filename_for_model(id).unwrap();
    fs::copy(root.join("model.gguf"), root.join(&name)).unwrap();
    let projector = admit_projector(&root, &name).unwrap();
    let supervisor =
        TrustedInferenceSupervisor::new("llama-cpu".into(), 512, vec![], || Ok(TextOnly)).unwrap();
    supervisor.register_model(id.into(), model).unwrap();
    assert_eq!(
        supervisor.register_projector_for_model("m-aabbcc", projector.clone()),
        Err(SupervisorError::UnknownModel)
    );
    supervisor
        .register_projector_for_model(id, projector.clone())
        .unwrap();
    assert!(supervisor
        .register_projector_for_model(id, projector)
        .is_err());
    let error = supervisor.load(id, "s-1".into(), "req-1").unwrap_err();
    assert!(
        matches!(error, SupervisorError::Bridge(_)),
        "must fail closed: {error:?}"
    );
    assert!(supervisor.status().is_err() || supervisor.status().unwrap().pid.is_none());
    fs::remove_dir_all(root).unwrap();
}
