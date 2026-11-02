//! Safe local compute inventory: CPU capacity and feature hints plus platform GPU hints.
//! A snapshot is diagnostic only; inference engines must still validate startup.
use crate::{
    cpu_instructions::{detect_cpu_instructions, CpuInstructionHints},
    cpu_topology::CpuTopology,
    platform_compute::{collect_platform_gpu_hints, PlatformGpuHints},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComputeInventory {
    pub os: &'static str,
    pub arch: &'static str,
    pub cpu: CpuTopology,
    pub instructions: CpuInstructionHints,
    pub gpu: PlatformGpuHints,
}

pub fn collect_compute_inventory() -> ComputeInventory {
    ComputeInventory {
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        cpu: CpuTopology::discover(),
        instructions: detect_cpu_instructions(),
        gpu: collect_platform_gpu_hints(),
    }
}
