use lu_core::cpu_topology::{CpuTopology, CpuWorkload};

#[test]
fn bounds_workers_to_available_cpu_threads() {
    let single = CpuTopology {
        available_logical_threads: 1,
    };
    assert_eq!(single.worker_budget(CpuWorkload::Interactive, None), 1);
    assert_eq!(single.worker_budget(CpuWorkload::Background, Some(99)), 1);
    let twelve = CpuTopology {
        available_logical_threads: 12,
    };
    assert_eq!(twelve.worker_budget(CpuWorkload::Interactive, None), 11);
    assert_eq!(twelve.worker_budget(CpuWorkload::Background, None), 6);
    assert_eq!(
        twelve.worker_budget(CpuWorkload::DedicatedBenchmark, None),
        12
    );
    assert_eq!(twelve.worker_budget(CpuWorkload::Interactive, Some(0)), 1);
    assert_eq!(twelve.worker_budget(CpuWorkload::Interactive, Some(20)), 12);
}

#[test]
fn operating_system_discovery_returns_at_least_one() {
    let topology = CpuTopology::discover();
    assert!(topology.available_logical_threads >= 1);
}
