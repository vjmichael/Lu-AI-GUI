//! Portable compilation of Lu's actual desktop load diagnostics.
//! This is the production source module, not a mocked rewrite.
//! The harness intentionally has no Tauri, WebView, GPU or engine dependency.
#[path = "../../../src-tauri/src/load_trace.rs"]
mod load_trace;

#[cfg(test)]
mod integration_tests {
    use super::load_trace::LoadTrace;

    #[test]
    fn restarts_do_not_inherit_active_load_state() {
        let mut first = LoadTrace::default();
        let load_id = format!("load-{}", "d".repeat(32));
        let model_id = format!("m-{}", "e".repeat(32));
        first.begin(load_id, model_id).unwrap();
        first.record_engine("engine_spawned", None, Some(123));
        assert!(first.snapshot().loading);

        // A fresh host must not claim that an old process is still loading.
        let restarted = LoadTrace::default();
        assert!(!restarted.snapshot().loading);
        assert!(restarted.snapshot().events.is_empty());
    }
}
