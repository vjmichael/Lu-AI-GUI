use lu_core::linux_gpu_nodes::inspect_linux_gpu_nodes;
use std::fs;

#[test]
fn reports_hints_without_declaring_backend_readiness() {
    let root = std::env::temp_dir().join(format!(
        "lu-gpu-nodes-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let dri = root.join("dri");
    fs::create_dir_all(&dri).unwrap();
    fs::write(dri.join("renderD128"), "").unwrap();
    fs::write(dri.join("renderD129"), "").unwrap();
    fs::write(dri.join("renderDX"), "").unwrap();
    fs::write(root.join("kfd"), "").unwrap();
    let result = inspect_linux_gpu_nodes(&dri, &root.join("kfd"));
    assert_eq!(result.render_nodes, vec!["renderD128", "renderD129"]);
    assert!(result.kfd_present);
    fs::remove_dir_all(root).unwrap();
}
