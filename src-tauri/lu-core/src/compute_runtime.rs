//! Model-neutral inference backend selection contract.
//! The OS GPU driver is always outside Lu's managed package boundary.
//! Driver probes and binary signatures must be implemented by trusted adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ComputeBackend {
    Cpu,
    Vulkan,
    Rocm,
    Cuda,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendProbe {
    pub backend: ComputeBackend,
    pub package_present: bool,
    pub driver_compatible: bool,
    pub model_compatible: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendPreference {
    Automatic,
    Require(ComputeBackend),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectionError {
    NoCompatibleBackend,
    RequestedBackendUnavailable(ComputeBackend),
    DuplicateProbe(ComputeBackend),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendSelection {
    pub backend: ComputeBackend,
    pub reason: &'static str,
}

impl BackendProbe {
    pub fn ready(&self) -> bool {
        self.package_present && self.driver_compatible && self.model_compatible
    }
}

/// This orders *compatible* backends only. It is NOT a performance ranking;
/// a future measured preference can override it after controlled benchmarks.
/// It never executes or downloads a package, installs a driver, or silently
/// degrades a strictly required backend to another backend.
pub fn select_backend(
    probes: &[BackendProbe],
    preference: BackendPreference,
    auto_order: &[ComputeBackend],
) -> Result<BackendSelection, SelectionError> {
    let mut observed = std::collections::BTreeMap::new();
    for probe in probes {
        if observed.insert(probe.backend, probe).is_some() {
            return Err(SelectionError::DuplicateProbe(probe.backend));
        }
    }
    match preference {
        BackendPreference::Require(backend) => {
            if observed.get(&backend).is_some_and(|probe| probe.ready()) {
                Ok(BackendSelection {
                    backend,
                    reason: "explicitly selected compatible backend",
                })
            } else {
                Err(SelectionError::RequestedBackendUnavailable(backend))
            }
        }
        BackendPreference::Automatic => {
            for backend in auto_order {
                if observed.get(backend).is_some_and(|probe| probe.ready()) {
                    return Ok(BackendSelection {
                        backend: *backend,
                        reason: "first compatible backend in configured order",
                    });
                }
            }
            Err(SelectionError::NoCompatibleBackend)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedEngineCandidate {
    pub backend: ComputeBackend,
    /// Evidence from the actual engine startup and small model inference.
    pub initialized: bool,
    pub inference_passed: bool,
    pub median_tokens_per_second_milli: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutoTuneError {
    NoVerifiedEngine,
    DuplicateEngine(ComputeBackend),
}

/// Benchmark scores are trusted only after an actual inference smoke test.
/// Never interpret PCI ID, binary presence or '--list-devices' as readiness.
pub fn choose_verified_engine(
    candidates: &[VerifiedEngineCandidate],
) -> Result<ComputeBackend, AutoTuneError> {
    let mut seen = std::collections::BTreeSet::new();
    let mut winner: Option<(ComputeBackend, u64)> = None;
    for candidate in candidates {
        if !seen.insert(candidate.backend) {
            return Err(AutoTuneError::DuplicateEngine(candidate.backend));
        }
        if !candidate.initialized || !candidate.inference_passed {
            continue;
        }
        let Some(score) = candidate.median_tokens_per_second_milli else {
            continue;
        };
        if score == 0 {
            continue;
        }
        if winner.is_none_or(|(_, best)| score > best) {
            winner = Some((candidate.backend, score));
        }
    }
    winner
        .map(|(backend, _)| backend)
        .ok_or(AutoTuneError::NoVerifiedEngine)
}

/// A P03 route is not eligible merely because a model header claims vision or
/// a GPU driver/device is detected. The trusted host supplies live, testable
/// runtime evidence. No implicit CPU/ROCm/CUDA fallback from a strict Vulkan
/// request is allowed.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum ModelTaskKind{Text,Vision}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct ModelTaskEvidence{
    pub model_file_admitted:bool,
    pub projector_file_admitted:bool,
    pub live_vision_probe_passed:bool,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum ModelRoutingError{
    ModelNotAdmitted,ProjectorNotAdmitted,VisionNotProven,
    Backend(SelectionError),
}
pub fn select_model_task_backend(
    task:ModelTaskKind,
    evidence:ModelTaskEvidence,
    probes:&[BackendProbe],
    preference:BackendPreference,
    auto_order:&[ComputeBackend],
)->Result<BackendSelection,ModelRoutingError>{
    if !evidence.model_file_admitted {
        return Err(ModelRoutingError::ModelNotAdmitted)
    }
    if task==ModelTaskKind::Vision{
        if !evidence.projector_file_admitted{
            return Err(ModelRoutingError::ProjectorNotAdmitted)
        }
        if !evidence.live_vision_probe_passed{
            return Err(ModelRoutingError::VisionNotProven)
        }
    }
    select_backend(probes,preference,auto_order).map_err(ModelRoutingError::Backend)
}
#[cfg(test)]
mod ai_operations_routing_tests{
    use super::*;
    fn probe(backend:ComputeBackend,ready:bool)->BackendProbe{
        BackendProbe{
            backend,package_present:ready,driver_compatible:ready,
            model_compatible:ready,detail:"verified-only-test-fixture".into()
        }
    }
    #[test]
    fn strict_backend_never_falls_back_from_unverified_vulkan(){
        let probes=[probe(ComputeBackend::Vulkan,false),probe(ComputeBackend::Cpu,true)];
        let e=ModelTaskEvidence{model_file_admitted:true,
            projector_file_admitted:false,live_vision_probe_passed:false};
        assert_eq!(
            select_model_task_backend(ModelTaskKind::Text,e,&probes,
                BackendPreference::Require(ComputeBackend::Vulkan),
                &[ComputeBackend::Vulkan,ComputeBackend::Cpu]),
            Err(ModelRoutingError::Backend(
                SelectionError::RequestedBackendUnavailable(ComputeBackend::Vulkan)))
        );
        assert_eq!(
            select_model_task_backend(ModelTaskKind::Text,e,&probes,
                BackendPreference::Automatic,
                &[ComputeBackend::Vulkan,ComputeBackend::Cpu])
                .unwrap().backend,
            ComputeBackend::Cpu
        );
    }
    #[test]
    fn header_mmproj_and_gpu_hint_do_not_prove_vision(){
        let probes=[probe(ComputeBackend::Cpu,true)];
        let evidence=ModelTaskEvidence{
            model_file_admitted:true,projector_file_admitted:true,
            live_vision_probe_passed:false,
        };
        assert_eq!(
            select_model_task_backend(ModelTaskKind::Vision,evidence,&probes,
                BackendPreference::Automatic,&[ComputeBackend::Cpu]),
            Err(ModelRoutingError::VisionNotProven)
        );
    }
}
