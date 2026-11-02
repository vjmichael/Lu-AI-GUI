use lu_core::cpu_topology::{choose_measured_threads, CpuTopology, ThreadBenchmark};
#[test]
fn measured_threads_respect_cpu_capacity_and_latency() {
    let host = CpuTopology {
        available_logical_threads: 12,
    };
    let runs = [
        ThreadBenchmark {
            threads: 6,
            tokens_per_second_milli: 42000,
            p95_latency_millis: 70,
        },
        ThreadBenchmark {
            threads: 12,
            tokens_per_second_milli: 50000,
            p95_latency_millis: 130,
        },
        ThreadBenchmark {
            threads: 24,
            tokens_per_second_milli: 90000,
            p95_latency_millis: 10,
        },
        ThreadBenchmark {
            threads: 8,
            tokens_per_second_milli: 44000,
            p95_latency_millis: 90,
        },
    ];
    assert_eq!(choose_measured_threads(&host, &runs, 100), Some(8));
    assert_eq!(choose_measured_threads(&host, &runs, 50), None);
}
