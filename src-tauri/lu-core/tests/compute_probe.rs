use lu_core::compute_probe::{evaluate_observation, HostComputeObservation};
use lu_core::compute_runtime::{select_backend, BackendPreference, ComputeBackend, SelectionError};

fn observation(backend: ComputeBackend) -> HostComputeObservation {
    HostComputeObservation {
        os: "ubuntu".into(),
        backend,
        device_name: Some("gpu".into()),
        driver_version: Some("driver".into()),
        runtime_package_version: Some("engine".into()),
        initialization_succeeded: true,
        supported_model: true,
        diagnostic: "verified probe".into(),
    }
}

#[test]
fn gpu_requires_driver_and_successful_initialization() {
    let mut gpu = observation(ComputeBackend::Vulkan);
    gpu.driver_version = None;
    assert!(!evaluate_observation(&gpu).ready());
    gpu.driver_version = Some("driver".into());
    gpu.initialization_succeeded = false;
    assert!(!evaluate_observation(&gpu).ready());
    gpu.initialization_succeeded = true;
    assert!(evaluate_observation(&gpu).ready());
}

#[test]
fn probe_rejection_preserves_cpu_fallback() {
    let mut rocm = observation(ComputeBackend::Rocm);
    rocm.initialization_succeeded = false;
    let cpu = observation(ComputeBackend::Cpu);
    let probes = [evaluate_observation(&rocm), evaluate_observation(&cpu)];
    assert_eq!(
        select_backend(
            &probes,
            BackendPreference::Automatic,
            &[ComputeBackend::Rocm, ComputeBackend::Cpu]
        )
        .unwrap()
        .backend,
        ComputeBackend::Cpu
    );
    assert_eq!(
        select_backend(
            &probes,
            BackendPreference::Require(ComputeBackend::Rocm),
            &[ComputeBackend::Cpu]
        ),
        Err(SelectionError::RequestedBackendUnavailable(
            ComputeBackend::Rocm
        ))
    );
}
