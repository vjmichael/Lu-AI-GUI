//! CPU topology and conservative worker budget; independent of GPU discovery.
//! available_parallelism reflects process affinity / OS restrictions when available.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuTopology {
    pub available_logical_threads: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuWorkload {
    Interactive,
    Background,
    DedicatedBenchmark,
}

impl CpuTopology {
    pub fn discover() -> Self {
        Self {
            available_logical_threads: std::thread::available_parallelism()
                .map(|threads| threads.get())
                .unwrap_or(1),
        }
    }

    pub fn worker_budget(&self, workload: CpuWorkload, requested: Option<usize>) -> usize {
        let available = self.available_logical_threads.max(1);
        let default = match workload {
            CpuWorkload::Interactive => available.saturating_sub(1).max(1),
            CpuWorkload::Background => (available / 2).max(1),
            CpuWorkload::DedicatedBenchmark => available,
        };
        requested.unwrap_or(default).clamp(1, available)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThreadBenchmark {
    pub threads: usize,
    pub tokens_per_second_milli: u64,
    pub p95_latency_millis: u64,
}

/// Choose measured threading with latency guard, never exceed host limits.
/// A tie prefers fewer threads to leave compute available to the desktop.
pub fn choose_measured_threads(
    topology: &CpuTopology,
    measurements: &[ThreadBenchmark],
    max_p95_latency_millis: u64,
) -> Option<usize> {
    measurements
        .iter()
        .filter(|r| {
            r.threads > 0
                && r.threads <= topology.available_logical_threads.max(1)
                && r.tokens_per_second_milli > 0
                && r.p95_latency_millis <= max_p95_latency_millis
        })
        .max_by(|a, b| {
            a.tokens_per_second_milli
                .cmp(&b.tokens_per_second_milli)
                .then_with(|| b.threads.cmp(&a.threads))
        })
        .map(|r| r.threads)
}
