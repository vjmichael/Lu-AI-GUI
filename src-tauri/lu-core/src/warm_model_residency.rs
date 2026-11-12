//! PERF-01 P1: host-only, fail-closed warm GGUF residency decisions.
//! This module never loads a model, grants authority, or interrupts inference.
//! The desktop owner may poll eviction; the supervisor rechecks the generation
//! reservation before touching the engine.
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WarmResidencyPolicy {
    /// Retain a verified ready model for this long after the last generation.
    pub idle_grace: Duration,
    /// Below this measured available RAM, evict an otherwise idle model.
    pub critical_available_ram_mib: u64,
}

impl Default for WarmResidencyPolicy {
    fn default() -> Self {
        Self {
            idle_grace: Duration::from_secs(15 * 60),
            critical_available_ram_mib: 1024,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarmDecision {
    RetainActive,
    RetainUnmeasured,
    RetainWithinGrace,
    Retain,
    EvictIdleTimeout,
    EvictMeasuredPressure,
}

impl WarmResidencyPolicy {
    /// Caller must supply a host-measured RAM sample; None does not imply free
    /// capacity. Eviction is permitted only for a verified idle session.
    pub fn decide(
        self,
        has_session: bool,
        has_reserved_generation: bool,
        idle_for: Option<Duration>,
        available_ram_mib: Option<u64>,
    ) -> WarmDecision {
        if !has_session || has_reserved_generation || idle_for.is_none() {
            return WarmDecision::RetainActive;
        }
        // Give newly loaded models a short first-turn opportunity even if the\n        // host is already under pressure. Never thrash GPU initialization.\n        if idle_for.is_some_and(|elapsed| elapsed >= Duration::from_secs(30))\n            && available_ram_mib.is_some_and(|mib| mib < self.critical_available_ram_mib) {
            return WarmDecision::EvictMeasuredPressure;
        }
        if idle_for.is_some_and(|elapsed| elapsed >= self.idle_grace) {
            return WarmDecision::EvictIdleTimeout;
        }
        if available_ram_mib.is_none() {
            return WarmDecision::RetainUnmeasured;
        }
        WarmDecision::RetainWithinGrace
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pressure_never_interrupts_reserved_generation() {
        let p = WarmResidencyPolicy::default();
        assert_eq!(p.decide(true, true, None, Some(1)), WarmDecision::RetainActive);
        assert_eq!(p.decide(true, true, Some(Duration::from_secs(3600)), Some(1)), WarmDecision::RetainActive);
    }
    #[test]
    fn only_measured_pressure_or_elapsed_grace_evicts() {
        let p = WarmResidencyPolicy::default();
        assert_eq!(p.decide(true, false, Some(Duration::ZERO), None), WarmDecision::RetainUnmeasured);
        assert_eq!(p.decide(true, false, Some(Duration::from_secs(30)), Some(512)), WarmDecision::EvictMeasuredPressure);
        assert_eq!(p.decide(true, false, Some(Duration::from_secs(900)), None), WarmDecision::EvictIdleTimeout);
        assert_eq!(p.decide(false, false, Some(Duration::from_secs(900)), Some(1)), WarmDecision::RetainActive);
        assert_eq!(p.decide(true, false, Some(Duration::from_secs(899)), Some(2048)), WarmDecision::RetainWithinGrace);
    }
}
