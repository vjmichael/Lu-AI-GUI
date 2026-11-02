use lu_core::platform_compute::{collect_platform_gpu_hints, PlatformGpuHints};

#[test]
fn platform_gpu_hints_never_claim_runtime_initialization() {
    let hints = collect_platform_gpu_hints();
    #[cfg(target_os = "linux")]
    assert!(matches!(hints, PlatformGpuHints::Linux { .. }));
    #[cfg(target_os = "windows")]
    assert!(matches!(hints, PlatformGpuHints::Windows { .. }));
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    assert!(matches!(hints, PlatformGpuHints::Unsupported { .. }));
}
