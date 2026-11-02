//! Conservative host observations translated into backend compatibility.
//! Presence of a GPU or driver alone never proves an inference engine works.
use crate::compute_runtime::{BackendProbe, ComputeBackend};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostComputeObservation {
    pub os: String,
    pub backend: ComputeBackend,
    pub device_name: Option<String>,
    pub driver_version: Option<String>,
    pub runtime_package_version: Option<String>,
    /// Verified by a trusted host backend probe, not from model assertions.
    pub initialization_succeeded: bool,
    pub supported_model: bool,
    pub diagnostic: String,
}

pub fn evaluate_observation(observation: &HostComputeObservation) -> BackendProbe {
    let package_present = observation.runtime_package_version.is_some();
    let driver_compatible = match observation.backend {
        ComputeBackend::Cpu => observation.initialization_succeeded,
        _ => {
            observation.device_name.is_some()
                && observation.driver_version.is_some()
                && observation.initialization_succeeded
        }
    };
    BackendProbe {
        backend: observation.backend,
        package_present,
        driver_compatible,
        model_compatible: observation.supported_model,
        detail: observation.diagnostic.clone(),
    }
}
