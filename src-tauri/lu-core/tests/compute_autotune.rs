use lu_core::compute_runtime::{
    choose_verified_engine, AutoTuneError, ComputeBackend, VerifiedEngineCandidate,
};

fn candidate(
    backend: ComputeBackend,
    initialized: bool,
    inference_passed: bool,
    tps: u64,
) -> VerifiedEngineCandidate {
    VerifiedEngineCandidate {
        backend,
        initialized,
        inference_passed,
        median_tokens_per_second_milli: Some(tps),
    }
}

#[test]
fn picks_fastest_verified_engine_not_fastest_unverified_one() {
    let results = [
        candidate(ComputeBackend::Rocm, false, false, 999999),
        candidate(ComputeBackend::Vulkan, true, true, 42000),
        candidate(ComputeBackend::Cpu, true, true, 12000),
    ];
    assert_eq!(choose_verified_engine(&results), Ok(ComputeBackend::Vulkan));
}

#[test]
fn never_selects_uninitialized_or_unbenchmarked_backend() {
    let incomplete = [candidate(ComputeBackend::Cuda, true, false, 123000)];
    assert_eq!(
        choose_verified_engine(&incomplete),
        Err(AutoTuneError::NoVerifiedEngine)
    );
    assert_eq!(
        choose_verified_engine(&[]),
        Err(AutoTuneError::NoVerifiedEngine)
    );
    let duplicate = [
        candidate(ComputeBackend::Cpu, true, true, 5000),
        candidate(ComputeBackend::Cpu, true, true, 6000),
    ];
    assert_eq!(
        choose_verified_engine(&duplicate),
        Err(AutoTuneError::DuplicateEngine(ComputeBackend::Cpu))
    );
}
