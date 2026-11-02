//! Read-only Linux device hints. These are not GPU readiness attestations.
//! Trustworthy Vulkan/ROCm/CUDA availability requires real backend init probes.
use std::{fs, path::Path};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxDeviceHint {
    pub vendor_id: String,
    pub device_id: String,
    pub pci_address: String,
}

fn pci_hex(value: &str) -> Option<String> {
    let value = value.trim();
    let hex = value.strip_prefix("0x")?;
    if hex.len() != 4 || !hex.bytes().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    Some(hex.to_ascii_lowercase())
}

/// Enumerates PCI render/display device hints from Linux sysfs without
/// launching a subprocess or invoking any driver. Best effort only.
pub fn inspect_linux_pci(root: &Path) -> Vec<LinuxDeviceHint> {
    let mut hints = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return hints;
    };
    for entry in entries.flatten() {
        let Some(address) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if address.len() > 32
            || !address
                .bytes()
                .all(|b| b.is_ascii_hexdigit() || matches!(b, b':' | b'.'))
        {
            continue;
        }
        let Ok(vendor) = fs::read_to_string(entry.path().join("vendor")) else {
            continue;
        };
        let Ok(device) = fs::read_to_string(entry.path().join("device")) else {
            continue;
        };
        if let (Some(vendor_id), Some(device_id)) = (pci_hex(&vendor), pci_hex(&device)) {
            hints.push(LinuxDeviceHint {
                vendor_id,
                device_id,
                pci_address: address,
            });
        }
    }
    hints.sort_by(|a, b| a.pci_address.cmp(&b.pci_address));
    hints
}
