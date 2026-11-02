//! Portable CPU instruction-capability hints, not backend compatibility claims.
//! Runtime-specific kernels must still perform their own supported dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CpuInstructionHints {
    pub x86_sse42: bool,
    pub x86_avx2: bool,
    pub x86_fma: bool,
    pub x86_avx512f: bool,
    pub arm_neon: bool,
}

pub fn detect_cpu_instructions() -> CpuInstructionHints {
    let mut hints = CpuInstructionHints::default();
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        hints.x86_sse42 = std::arch::is_x86_feature_detected!("sse4.2");
        hints.x86_avx2 = std::arch::is_x86_feature_detected!("avx2");
        hints.x86_fma = std::arch::is_x86_feature_detected!("fma");
        hints.x86_avx512f = std::arch::is_x86_feature_detected!("avx512f");
    }
    #[cfg(target_arch = "aarch64")]
    {
        hints.arm_neon = std::arch::is_aarch64_feature_detected!("neon");
    }
    hints
}
