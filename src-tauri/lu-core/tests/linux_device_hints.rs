use lu_core::linux_device_hints::inspect_linux_pci;
use std::{fs, path::PathBuf};

#[test]
fn reads_only_well_formed_pci_hints_and_sorts() {
    let root = std::env::temp_dir().join(format!("lu-pci-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    for (name, vendor, device) in [
        ("0000:05:00.0", "0x1002\n", "0x7550\n"),
        ("0000:01:00.0", "0x10DE\n", "0x2684\n"),
        ("untrusted-name", "0x1234", "0x1234"),
        ("0000:02:00.0", "oops", "0x1234"),
    ] {
        let path = root.join(name);
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("vendor"), vendor).unwrap();
        fs::write(path.join("device"), device).unwrap();
    }
    let found = inspect_linux_pci(&root);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].vendor_id, "10de");
    assert_eq!(found[1].vendor_id, "1002");
    assert!(inspect_linux_pci(&PathBuf::from("/nonexistent-lu-pci-root")).is_empty());
    fs::remove_dir_all(root).unwrap();
}
