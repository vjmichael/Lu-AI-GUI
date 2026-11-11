//! G02: generic host-only Workspace inference admission.
//!
//! No NVP film knowledge, external socket, model loader, WebView command or
//! permission token lives here. A trusted Lu caller binds a project, user
//! task and active Workspace; policy must approve the exact operation on
//! EACH entry. The host's existing supervisor remains the only GGUF owner.
//! A successful unit test is NOT a mounted/installed external Workspace.

use crate::{
    inference_bridge::BridgeError,
    inference_host::{EnginePhase, InferenceBackend},
    inference_supervisor::{SupervisorError, SupervisorStatus, TrustedInferenceSupervisor},
    workspace::{WorkspaceHost, WorkspaceState},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceInferenceAction {
    ObserveModel,
    ReserveGeneration,
    RunGeneration,
    CancelGeneration,
    RecordProposal,
}

#[derive(Debug, Clone)]
pub struct WorkspaceInferenceTask {
    /// Must be trusted and checked by Lu host policy, NEVER from an LLM.
    pub module_id: String,
    pub instance_id: String,
    pub project_ref: String,
    pub user_task_ref: String,
}

pub trait WorkspaceInferencePolicy: Send + Sync {
    fn authorize(
        &self,
        task: &WorkspaceInferenceTask,
        action: WorkspaceInferenceAction,
    ) -> bool;
}

#[derive(Debug, PartialEq, Eq)]
pub enum WorkspaceInferenceError {
    InvalidTask,
    MissingOrInactiveWorkspace,
    PermissionDenied,
    ModelNotReady,
    WrongSession,
    WrongGeneration,
    Supervisor(SupervisorError),
}

impl From<SupervisorError> for WorkspaceInferenceError {
    fn from(err: SupervisorError) -> Self { Self::Supervisor(err) }
}

/// The generic Lu WorkspaceHost validates instance ownership; policy
/// independently validates current human task and project permission.
#[derive(Clone)]
pub struct WorkspaceInferenceGate {
    workspace_host: WorkspaceHost,
}

fn reference_ok(s: &str) -> bool {
    !s.is_empty() && s.len() <= 128 &&
        s.bytes().all(|b| b.is_ascii_alphanumeric() ||
            matches!(b, b'.' | b'_' | b'-'))
}

impl WorkspaceInferenceGate {
    pub fn new(workspace_host: WorkspaceHost) -> Self {
        Self { workspace_host }
    }

    pub fn verify(
        &self,
        task: &WorkspaceInferenceTask,
        action: WorkspaceInferenceAction,
        policy: &dyn WorkspaceInferencePolicy,
    ) -> Result<(), WorkspaceInferenceError> {
        if !reference_ok(&task.module_id) ||
            !reference_ok(&task.instance_id) ||
            !reference_ok(&task.project_ref) ||
            !reference_ok(&task.user_task_ref)
        {
            return Err(WorkspaceInferenceError::InvalidTask);
        }
        let instances = self.workspace_host.instances()
            .map_err(|_| WorkspaceInferenceError::MissingOrInactiveWorkspace)?;
        if !instances.iter().any(|i| i.id == task.instance_id &&
            i.module_id == task.module_id && i.state == WorkspaceState::Active)
        {
            return Err(WorkspaceInferenceError::MissingOrInactiveWorkspace);
        }
        if !policy.authorize(task, action) {
            return Err(WorkspaceInferenceError::PermissionDenied);
        }
        Ok(())
    }

    pub fn model_status<B: InferenceBackend + Send + 'static>(
        &self, task: &WorkspaceInferenceTask,
        policy: &dyn WorkspaceInferencePolicy,
        supervisor: &TrustedInferenceSupervisor<B>,
    ) -> Result<(SupervisorStatus, usize), WorkspaceInferenceError> {
        self.verify(task, WorkspaceInferenceAction::ObserveModel, policy)?;
        let status = supervisor.status()?;
        let catalog_size = supervisor.model_ids().len();
        self.verify(task, WorkspaceInferenceAction::ObserveModel, policy)?;
        Ok((status, catalog_size))
    }

    /// Reserve one Lu-owned generation; does not start/clone a model.
    pub fn reserve_generation<B: InferenceBackend + Send + 'static>(
        &self, task: &WorkspaceInferenceTask,
        policy: &dyn WorkspaceInferencePolicy,
        supervisor: &TrustedInferenceSupervisor<B>,
        session_id: &str,
        generation_id: &str,
        request_id: &str,
        prompt: &str,
        max_new_tokens: u32,
    ) -> Result<(), WorkspaceInferenceError> {
        self.verify(task, WorkspaceInferenceAction::ReserveGeneration, policy)?;
        if !reference_ok(session_id) || !reference_ok(generation_id) ||
            !reference_ok(request_id) || prompt.is_empty() ||
            prompt.len() > 64 * 1024 || !(1..=2800).contains(&max_new_tokens)
        {
            return Err(WorkspaceInferenceError::InvalidTask);
        }
        let status = supervisor.status()?;
        if status.session_id.as_deref() != Some(session_id) {
            return Err(WorkspaceInferenceError::WrongSession);
        }
        if status.phase != EnginePhase::Ready {
            return Err(WorkspaceInferenceError::ModelNotReady);
        }
        self.verify(task, WorkspaceInferenceAction::ReserveGeneration, policy)?;
        supervisor.reserve_generation(
            session_id, generation_id, request_id, prompt, max_new_tokens
        )?;
        Ok(())
    }

    /// Run an already-reserved task on a trusted host worker thread. A
    /// mid-stream policy revocation blocks subsequent output delivery.
    /// Only a separate terminal inference event can establish completion.
    pub fn run_reserved<B: InferenceBackend + Send + 'static>(
        &self, task: &WorkspaceInferenceTask,
        policy: &dyn WorkspaceInferencePolicy,
        supervisor: &TrustedInferenceSupervisor<B>,
        session_id: &str,
        generation_id: &str,
        request_id: &str,
        prompt: String,
        max_new_tokens: u32,
        mut emit: impl FnMut(Vec<u8>) -> Result<(), BridgeError>,
    ) -> Result<(), WorkspaceInferenceError> {
        self.verify(task, WorkspaceInferenceAction::RunGeneration, policy)?;
        let status = supervisor.status()?;
        if status.session_id.as_deref() != Some(session_id) {
            return Err(WorkspaceInferenceError::WrongSession);
        }
        if status.generation_id.as_deref() != Some(generation_id) {
            return Err(WorkspaceInferenceError::WrongGeneration);
        }
        self.verify(task, WorkspaceInferenceAction::RunGeneration, policy)?;
        supervisor.run_reserved(
            session_id, generation_id, request_id, prompt, max_new_tokens,
            |frame| {
                self.verify(task, WorkspaceInferenceAction::RunGeneration, policy)
                    .map_err(|_| BridgeError::Consumer)?;
                emit(frame)
            },
        )?;
        self.verify(task, WorkspaceInferenceAction::RunGeneration, policy)?;
        Ok(())
    }

    /// Cancellation acknowledgment is not a terminal worker/engine receipt.
    pub fn cancel_generation<B: InferenceBackend + Send + 'static>(
        &self, task: &WorkspaceInferenceTask,
        policy: &dyn WorkspaceInferencePolicy,
        supervisor: &TrustedInferenceSupervisor<B>,
        session_id: &str,
        generation_id: &str,
        request_id: &str,
    ) -> Result<CancellationRequested, WorkspaceInferenceError> {
        self.verify(task, WorkspaceInferenceAction::CancelGeneration, policy)?;
        let status = supervisor.status()?;
        if status.session_id.as_deref() != Some(session_id) {
            return Err(WorkspaceInferenceError::WrongSession);
        }
        if status.generation_id.as_deref() != Some(generation_id) {
            return Err(WorkspaceInferenceError::WrongGeneration);
        }
        supervisor.cancel(session_id, generation_id, request_id)?;
        Ok(CancellationRequested { terminal_verified: false })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CancellationRequested {
    pub terminal_verified: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        contracts::{ModuleKind, ModuleManifest, CORE_CONTRACT_VERSION},
        registry::NativeToolRegistry,
    };
    struct Policy { allow: bool, required_project: &'static str }
    impl WorkspaceInferencePolicy for Policy {
        fn authorize(&self, task: &WorkspaceInferenceTask,
            _: WorkspaceInferenceAction) -> bool {
            self.allow && task.project_ref == self.required_project &&
                task.user_task_ref == "human-turn-01"
        }
    }
    fn fixture() -> (WorkspaceHost, WorkspaceInferenceTask) {
        let host = WorkspaceHost::new(NativeToolRegistry::new());
        host.register_workspace(
            ModuleManifest {
                id: "nvp.film".into(), display_name: "Film".into(),
                contract_version: CORE_CONTRACT_VERSION,
                kind: ModuleKind::Workspace,
                tools: Vec::new(), panels: Vec::new(), workflows: Vec::new(),
            },
            Vec::new(),
        ).unwrap();
        host.open_instance("nvp.film", "film-01").unwrap();
        (host, WorkspaceInferenceTask {
            module_id:"nvp.film".into(), instance_id:"film-01".into(),
            project_ref:"project-01".into(), user_task_ref:"human-turn-01".into(),
        })
    }
    #[test]
    fn admitted_only_for_active_owned_exact_user_task() {
        let (host,task)=fixture();
        let gate=WorkspaceInferenceGate::new(host.clone());
        let yes=Policy{allow:true,required_project:"project-01"};
        let no=Policy{allow:false,required_project:"project-01"};
        assert_eq!(gate.verify(&task,WorkspaceInferenceAction::ObserveModel,&yes),Ok(()));
        assert_eq!(gate.verify(&task,WorkspaceInferenceAction::ReserveGeneration,&no),
            Err(WorkspaceInferenceError::PermissionDenied));
        let mut alien=task.clone();alien.project_ref="other-project".into();
        assert_eq!(gate.verify(&alien,WorkspaceInferenceAction::ReserveGeneration,&yes),
            Err(WorkspaceInferenceError::PermissionDenied));
        alien=task.clone();alien.module_id="other.module".into();
        assert_eq!(gate.verify(&alien,WorkspaceInferenceAction::ObserveModel,&yes),
            Err(WorkspaceInferenceError::MissingOrInactiveWorkspace));
        host.suspend_instance("film-01").unwrap();
        assert_eq!(gate.verify(&task,WorkspaceInferenceAction::ObserveModel,&yes),
            Err(WorkspaceInferenceError::MissingOrInactiveWorkspace));
        host.resume_instance("film-01").unwrap();
        assert!(gate.verify(&task,WorkspaceInferenceAction::ObserveModel,&yes).is_ok());
        host.close_instance("film-01").unwrap();
        assert_eq!(gate.verify(&task,WorkspaceInferenceAction::ObserveModel,&yes),
            Err(WorkspaceInferenceError::MissingOrInactiveWorkspace));
    }

    struct NoEngine;
    impl InferenceBackend for NoEngine {
        fn load(&mut self, _: &crate::model_admission::AdmittedModel)
            -> Result<(), String> { Ok(()) }
        fn unload(&mut self) {}
        fn generate(
            &mut self,
            _: &crate::inference_host::GenerationRequest,
            _: &crate::inference_host::Cancellation,
            _: &mut dyn FnMut(
                crate::inference_host::TokenChunk
            ) -> Result<(), crate::inference_host::EngineError>,
        ) -> Result<(), String> { Err("no approved model loaded".into()) }
    }

    #[test]
    fn unmounted_model_is_observed_without_authorizing_inference_or_cancel() {
        let (host, task) = fixture();
        let gate = WorkspaceInferenceGate::new(host);
        let policy = Policy { allow: true, required_project: "project-01" };
        let supervisor = TrustedInferenceSupervisor::<NoEngine>::new(
            "bundled-runtime".into(), 1024, Vec::new(), || Ok(NoEngine)
        ).unwrap();
        let (state, count) = gate.model_status(&task, &policy, &supervisor).unwrap();
        assert_eq!(state.phase, EnginePhase::Idle);
        assert_eq!(count, 0);
        assert_eq!(gate.reserve_generation(
            &task, &policy, &supervisor, "absent-session",
            "host-generation", "host-request", "Write a scene", 100,
        ), Err(WorkspaceInferenceError::WrongSession));
        assert_eq!(gate.cancel_generation(
            &task, &policy, &supervisor, "absent-session",
            "host-generation", "host-request",
        ), Err(WorkspaceInferenceError::WrongSession));
        assert_eq!(supervisor.model_ids().len(), 0);
    }
    #[test]
    fn no_browser_created_workspace_or_empty_project_task() {
        let (host,task)=fixture();
        let gate=WorkspaceInferenceGate::new(host);
        let yes=Policy{allow:true,required_project:"project-01"};
        let mut blank=task.clone();blank.user_task_ref.clear();
        assert_eq!(gate.verify(&blank,WorkspaceInferenceAction::RunGeneration,&yes),
            Err(WorkspaceInferenceError::InvalidTask));
        blank=task.clone();blank.instance_id="other".into();
        assert_eq!(gate.verify(&blank,WorkspaceInferenceAction::CancelGeneration,&yes),
            Err(WorkspaceInferenceError::MissingOrInactiveWorkspace));
    }
}
