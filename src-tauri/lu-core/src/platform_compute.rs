//! Read-only platform-specific device hints. No driver or runtime execution.
use crate::linux_device_hints::LinuxDeviceHint;
use crate::linux_gpu_nodes::LinuxGpuNodeHints;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlatformGpuHints {
    Linux {
        pci: Vec<LinuxDeviceHint>,
        nodes: LinuxGpuNodeHints,
    },
    Windows {
        diagnostic: String,
    },
    Unsupported {
        diagnostic: String,
    },
}

/// Fixed, host-owned Linux paths. Windows never attempts to inspect /sys or
/// /dev and reports its discovery adapter as unavailable until implemented.
pub fn collect_platform_gpu_hints() -> PlatformGpuHints {
    #[cfg(target_os = "linux")]
    {
        PlatformGpuHints::Linux {
            pci: crate::linux_device_hints::inspect_linux_pci(std::path::Path::new(
                "/sys/bus/pci/devices",
            )),
            nodes: crate::linux_gpu_nodes::inspect_linux_gpu_nodes(
                std::path::Path::new("/dev/dri"),
                std::path::Path::new("/dev/kfd"),
            ),
        }
    }
    #[cfg(target_os = "windows")]
    {
        PlatformGpuHints::Windows {
            diagnostic: "Windows GPU probing requires a dedicated adapter; no backend initialized"
                .into(),
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        PlatformGpuHints::Unsupported {
            diagnostic: "Platform GPU probing not implemented".into(),
        }
    }
}
