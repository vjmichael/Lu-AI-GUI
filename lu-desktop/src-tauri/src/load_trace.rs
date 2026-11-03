//! WIN-LOAD-01: bounded, host-owned model load trace.
//! This records lifecycle *observations*, never guessed bytes/percent or driver cause.
//! No model paths, prompt contents, API keys, or engine stderr are stored here.
use serde::Serialize;
use std::{
    collections::VecDeque,
    time::Instant,
};

const MAX_EVENTS: usize = 96;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LoadEvent {
    pub sequence: u64,
    pub operation_id: String,
    pub model_ref: String,
    pub phase: &'static str,
    pub elapsed_ms: u64,
    pub outcome: Option<&'static str>,
}

#[derive(Debug, Clone)]
struct ActiveLoad {
    operation_id: String,
    model_ref: String,
    started: Instant,
}

/// Only the trusted host may append events. Lock scope is constant and small;
/// loading the model never holds this mutex.
#[derive(Debug, Default)]
pub(crate) struct LoadTrace {
    sequence: u64,
    active: Option<ActiveLoad>,
    events: VecDeque<LoadEvent>,
}

impl LoadTrace {
    pub fn begin(&mut self, operation_id: String, model_ref: String) -> Result<(), &'static str> {
        if self.active.is_some() {
            return Err("another model load is already in progress");
        }
        self.active = Some(ActiveLoad { operation_id, model_ref, started: Instant::now() });
        self.record("accepted", None);
        Ok(())
    }

    pub fn record(&mut self, phase: &'static str, outcome: Option<&'static str>) {
        let Some(active) = self.active.as_ref() else { return };
        self.sequence = self.sequence.saturating_add(1);
        let event = LoadEvent {
            sequence: self.sequence,
            operation_id: active.operation_id.clone(),
            model_ref: active.model_ref.clone(),
            phase,
            elapsed_ms: active.started.elapsed().as_millis().min(u64::MAX as u128) as u64,
            outcome,
        };
        if self.events.len() == MAX_EVENTS {
            self.events.pop_front();
        }
        self.events.push_back(event);
    }

    pub fn finish(&mut self, outcome: &'static str) {
        if self.active.is_none() { return; }
        self.record("finished", Some(outcome));
        self.active = None;
    }

    pub fn snapshot(&self) -> LoadTraceView {
        LoadTraceView {
            loading: self.active.is_some(),
            latest_sequence: self.sequence,
            events: self.events.iter().cloned().collect(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LoadTraceView {
    pub loading: bool,
    pub latest_sequence: u64,
    pub events: Vec<LoadEvent>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_monotonic_and_no_guessed_progress() {
        let mut trace = LoadTrace::default();
        trace.begin("load-1".into(), "m-opaque".into()).unwrap();
        for _ in 0..MAX_EVENTS + 5 {
            trace.record("supervisor_wait", None);
        }
        trace.finish("ok");
        let snapshot = trace.snapshot();
        assert!(!snapshot.loading);
        assert_eq!(snapshot.events.len(), MAX_EVENTS);
        assert!(snapshot.events.windows(2).all(|w| w[0].sequence < w[1].sequence));
        assert_eq!(snapshot.events.last().unwrap().outcome, Some("ok"));
        let json = serde_json::to_string(&snapshot).unwrap();
        assert!(!json.contains("percent"));
        assert!(!json.contains("filePath"));
    }

    #[test]
    fn overlapping_operations_are_rejected_without_overwrite() {
        let mut trace = LoadTrace::default();
        trace.begin("load-1".into(), "m-first".into()).unwrap();
        assert!(trace.begin("load-2".into(), "m-second".into()).is_err());
        trace.finish("error");
        assert!(!trace.snapshot().loading);
        assert!(trace.begin("load-3".into(), "m-third".into()).is_ok());
        assert_eq!(trace.snapshot().events.last().unwrap().operation_id, "load-3");
    }

    #[test]
    fn finish_is_safe_without_active_operation() {
        let mut trace = LoadTrace::default();
        trace.finish("error");
        assert_eq!(trace.snapshot().latest_sequence, 0);
    }
}
