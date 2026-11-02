use lu_core::cpu_instructions::detect_cpu_instructions;

#[test]
fn instruction_hints_can_be_collected_without_backend_execution() {
    let hints = detect_cpu_instructions();
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    assert!(!hints.x86_avx2 && !hints.x86_sse42 && !hints.x86_fma && !hints.x86_avx512f);
    #[cfg(not(target_arch = "aarch64"))]
    assert!(!hints.arm_neon);
    let _ = hints;
}
