use lu_core::compute_inventory::collect_compute_inventory;

#[test]
fn inventory_is_nonempty_and_matches_build_target() {
    let inventory = collect_compute_inventory();
    assert!(inventory.cpu.available_logical_threads >= 1);
    assert_eq!(inventory.os, std::env::consts::OS);
    assert_eq!(inventory.arch, std::env::consts::ARCH);
}
