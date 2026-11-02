//! Read-only Linux GPU device-node visibility hints.
//! Device-node presence is NOT proof of backend compatibility or usable access.
use std::{fs, path::Path};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LinuxGpuNodeHints {
    pub render_nodes: Vec<String>,
    pub kfd_present: bool,
}

pub fn inspect_linux_gpu_nodes(dri_root: &Path, kfd_path: &Path) -> LinuxGpuNodeHints {
    let mut result = LinuxGpuNodeHints::default();
    if let Ok(nodes) = fs::read_dir(dri_root) {
        for node in nodes.flatten() {
            let name = node.file_name();
            let Some(name) = name.to_str() else { continue };
            if name.len() > 7
                && name.starts_with("renderD")
                && name[7..].bytes().all(|byte| byte.is_ascii_digit())
                && fs::symlink_metadata(node.path()).is_ok_and(|m| !m.file_type().is_symlink())
            {
                result.render_nodes.push(name.to_owned());
            }
        }
    }
    result.render_nodes.sort();
    result.kfd_present =
        fs::symlink_metadata(kfd_path).is_ok_and(|meta| !meta.file_type().is_symlink());
    result
}
