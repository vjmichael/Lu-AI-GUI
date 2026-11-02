use lu_core::compute_runtime::{
    select_backend, BackendPreference, BackendProbe, ComputeBackend, SelectionError,
};

fn probe(backend: ComputeBackend, driver: bool) -> BackendProbe {
    BackendProbe {
        backend,
        package_present: true,
        driver_compatible: driver,
        model_compatible: true,
        detail: "fixture".into(),
    }
}

#[test]
fn automatic_selection_skips_missing_gpu_driver_and_falls_back_to_cpu() {
    let probes = [
        probe(ComputeBackend::Rocm, false),
        probe(ComputeBackend::Cpu, true),
    ];
    let chosen = select_backend(
        &probes,
        BackendPreference::Automatic,
        &[ComputeBackend::Rocm, ComputeBackend::Cpu],
    )
    .unwrap();
    assert_eq!(chosen.backend, ComputeBackend::Cpu);
}

#[test]
fn explicit_backend_never_silently_falls_back() {
    let probes = [
        probe(ComputeBackend::Vulkan, false),
        probe(ComputeBackend::Cpu, true),
    ];
    assert_eq!(
        select_backend(
            &probes,
            BackendPreference::Require(ComputeBackend::Vulkan),
            &[ComputeBackend::Cpu]
        ),
        Err(SelectionError::RequestedBackendUnavailable(
            ComputeBackend::Vulkan
        ))
    );
}

#[test]
fn refuses_duplicate_and_missing_probes() {
    let duplicate = [
        probe(ComputeBackend::Cuda, true),
        probe(ComputeBackend::Cuda, true),
    ];
    assert_eq!(
        select_backend(
            &duplicate,
            BackendPreference::Automatic,
            &[ComputeBackend::Cuda]
        ),
        Err(SelectionError::DuplicateProbe(ComputeBackend::Cuda))
    );
    assert_eq!(
        select_backend(&[], BackendPreference::Automatic, &[ComputeBackend::Cpu]),
        Err(SelectionError::NoCompatibleBackend)
    );
}

#[test]
fn model_incompatibility_blocks_backend_even_with_driver() {
    let mut gpu = probe(ComputeBackend::Vulkan, true);
    gpu.model_compatible = false;
    let selection = select_backend(
        &[gpu],
        BackendPreference::Automatic,
        &[ComputeBackend::Vulkan],
    );
    assert_eq!(selection, Err(SelectionError::NoCompatibleBackend));
}
