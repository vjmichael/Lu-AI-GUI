//! BACKEND-CTRL-01C — request-scoped vLLM control dispatch.
//! The vLLM worker is a separately supervised Linux process. This module
//! only emits bounded host-issued requests and never treats abort ACKs as
//! verified scheduler/worker cessation. Model text cannot choose request IDs.
use crate::backend_control::{BackendController, BackendCapabilities, ControlError, RecoveryPolicy, StopState};
use serde_json::{json, Value};
use std::time::Instant;

#[derive(Debug, PartialEq, Eq)]
pub enum VllmControlError {
    InvalidHostRequest,
    InvalidFeedback,
    Transport,
    State(ControlError),
}
impl From<ControlError> for VllmControlError {
    fn from(value: ControlError) -> Self { Self::State(value) }
}

/// Implemented only by a host-supervised transport bound to the exact vLLM
/// worker. Do not implement this for arbitrary OpenAI-compatible servers.
pub trait VllmControlTransport {
    fn send_host_control(&mut self, request: &Value) -> Result<(), ()>;
    /// Only implemented by the process supervisor retaining a real owned OS
    /// child handle. Never use this for an externally managed vLLM server.
    fn terminate_owned_worker(&mut self) -> Result<(), ()> { Err(()) }
    fn read_host_event(&mut self) -> Result<Value, ()> { Err(()) }
}

pub struct VllmRequestController<T: VllmControlTransport> {
    transport: T,
    state: BackendController,
}
impl<T: VllmControlTransport> VllmRequestController<T> {
    pub fn new(transport: T) -> Self {
        let state = BackendController::new(BackendCapabilities {
            request_scoped_cancel: true,
            independent_stop_verification: true,
            lu_owned_process: true,
        }, RecoveryPolicy::default()).expect("valid host policy");
        Self { transport, state }
    }
    pub fn state(&self) -> StopState { self.state.state() }
    pub fn next_event(&mut self) -> Result<Value, VllmControlError> {
        let event=self.transport.read_host_event()
            .map_err(|_| VllmControlError::Transport)?;
        self.observe_feedback(&event)?;
        Ok(event)
    }
    pub fn generate(&mut self, host_request_id: &str, prompt: &str, max_tokens: u32)
        -> Result<(), VllmControlError> {
        if prompt.trim().is_empty() || prompt.len() > 16 * 1024
            || !(1..=32768).contains(&max_tokens) {
            return Err(VllmControlError::InvalidHostRequest);
        }
        self.state.start(host_request_id)?;
        // Failure after this point leaves an active/uncertain host state;
        // caller must reconcile/terminate worker, NOT retry blindly.
        self.transport.send_host_control(&json!({
            "op":"generate","id":host_request_id,
            "prompt":prompt,"max_new_tokens":max_tokens
        })).map_err(|_| VllmControlError::Transport)
    }
    pub fn abort(&mut self, host_request_id: &str, now: Instant)
        -> Result<(), VllmControlError> {
        self.state.cancel(host_request_id,now)?;
        self.transport.send_host_control(&json!({"op":"abort","id":host_request_id}))
            .map_err(|_| VllmControlError::Transport)
    }
    /// Handle *external* adapter feedback as data, not trusted authority.
    /// Only the host can establish verified scheduler/request stop.
    pub fn observe_feedback(&mut self, feedback: &Value)
        -> Result<(), VllmControlError> {
        let id = feedback.get("id").and_then(Value::as_str)
            .ok_or(VllmControlError::InvalidFeedback)?;
        if self.state.active_request() != Some(id) {
            return Err(VllmControlError::InvalidFeedback);
        }
        match feedback.get("event").and_then(Value::as_str) {
            Some("abort_ack_unverified") => {
                self.state.cancel_acknowledged(id)?;
                Ok(())
            }
            Some("abort_verified_empty_engine") => {
                // Version-pinned Lu worker awaited native AsyncLLM.abort and
                // independently observed zero unfinished engine requests.
                // Only a single request is admitted to this managed worker.
                self.state.verify_stopped(id,false)?;
                Ok(())
            }
            // This is a terminal event for a *normally finishing* request.
            // During cancellation, a final message may race the abort; it
            // cannot independently verify the scheduler has released it.
            Some("finished") if self.state.state() == StopState::Generating => {
                self.state.verify_stopped(id, false)?;
                Ok(())
            }
            Some("text") | Some("status") | Some("stream_closed_without_finish")
                | Some("reader_cancelled") | Some("abort_failed") | Some("failed") => Ok(()),
            _ => Err(VllmControlError::InvalidFeedback),
        }
    }
    pub fn verify_host_observed_stop(&mut self, host_request_id: &str,
        verified_scheduler_inactive: bool) -> Result<(), VllmControlError> {
        if !verified_scheduler_inactive {
            return Err(VllmControlError::State(ControlError::UnsupportedVerification));
        }
        self.state.verify_stopped(host_request_id,false)?;
        Ok(())
    }
    pub fn check_escalation(&mut self, now: Instant) -> Result<bool, VllmControlError> {
        Ok(self.state.check_escalation(now)?)
    }
    pub fn mark_worker_terminated(&mut self, confirmed: bool) -> Result<(), VllmControlError> {
        self.state.mark_worker_terminated(confirmed)?;
        Ok(())
    }
    /// Host watchdog calls this on its own thread after the native abort grace
    /// expires; a failing kill is not treated as a verified stopped worker.
    pub fn escalate_owned_if_stalled(&mut self, now: Instant)
        -> Result<bool, VllmControlError> {
        if !self.state.check_escalation(now)? { return Ok(false); }
        let confirmed=self.transport.terminate_owned_worker().is_ok();
        self.state.mark_worker_terminated(confirmed)?;
        Ok(confirmed)
    }
    /// The caller must independently reconcile the effect journal and resource
    /// admission before supplying a new Lu-owned worker. This does not replay
    /// the original generation or reset the restart history.
    pub fn replace_after_recovery(&mut self, transport: T, now: Instant,
        reconciled: bool) -> Result<(), VllmControlError> {
        self.state.permit_restart(now,reconciled)?;
        self.transport=transport;
        Ok(())
    }
    pub fn permit_restart(&mut self, now: Instant, reconciled: bool)
        -> Result<(), VllmControlError> {
        self.state.permit_restart(now,reconciled)?;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Capturing(Vec<Value>);
    impl VllmControlTransport for Capturing {
        fn send_host_control(&mut self, value:&Value)->Result<(),()> {
            self.0.push(value.clone());
            Ok(())
        }
    }
    #[test]
    fn abort_is_bound_to_exact_host_request_id_and_ack_is_not_completion() {
        let mut c=VllmRequestController::new(Capturing(vec![]));
        let now=Instant::now();
        c.generate("req_1","hello",32).unwrap();
        assert_eq!(c.abort("req_2",now),
            Err(VllmControlError::State(ControlError::WrongRequest)));
        c.abort("req_1",now).unwrap();
        c.observe_feedback(&json!({"id":"req_1","event":"abort_ack_unverified"})).unwrap();
        assert_eq!(c.state(),StopState::CancelAcknowledged);
        assert_eq!(c.verify_host_observed_stop("req_1",false),
            Err(VllmControlError::State(ControlError::UnsupportedVerification)));
        assert!(c.check_escalation(now+std::time::Duration::from_secs(1)).unwrap());
    }
    #[test]
    fn killing_unowned_transport_is_never_marked_verified() {
        let mut c=VllmRequestController::new(Capturing(vec![]));
        let now=Instant::now();
        c.generate("req_1","hello",16).unwrap();
        c.abort("req_1",now).unwrap();
        assert!(!c.escalate_owned_if_stalled(now).unwrap());
        assert!(!c.escalate_owned_if_stalled(
            now+std::time::Duration::from_secs(1)).unwrap());
        assert_eq!(c.state(),StopState::FailedStop);
    }
    #[test]
    fn owned_abort_escalation_requires_confirmed_worker_exit_then_replacement() {
        use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
        struct Owned(Arc<AtomicBool>);
        impl VllmControlTransport for Owned {
            fn send_host_control(&mut self, _:&Value)->Result<(),()> { Ok(()) }
            fn terminate_owned_worker(&mut self)->Result<(),()> {
                self.0.store(true,Ordering::SeqCst);
                Ok(())
            }
        }
        let flag=Arc::new(AtomicBool::new(false));
        let mut c=VllmRequestController::new(Owned(Arc::clone(&flag)));
        let now=Instant::now();
        c.generate("req_1","hello",10).unwrap();
        c.abort("req_1",now).unwrap();
        assert!(c.escalate_owned_if_stalled(
            now+std::time::Duration::from_secs(1)).unwrap());
        assert!(flag.load(Ordering::SeqCst));
        assert_eq!(c.state(),StopState::WorkerTerminated);
        assert_eq!(c.replace_after_recovery(Owned(Arc::clone(&flag)),
            now+std::time::Duration::from_secs(2),false),
            Err(VllmControlError::State(ControlError::InvalidTransition)));
        c.replace_after_recovery(Owned(Arc::clone(&flag)),
            now+std::time::Duration::from_secs(2),true).unwrap();
        c.generate("req_2","fresh user request",10).unwrap();
    }
    #[test]
    fn vllm_native_abort_with_zero_unfinished_requests_preserves_model() {
        let mut c=VllmRequestController::new(Capturing(vec![]));
        let now=Instant::now();
        c.generate("req_1","hello",16).unwrap();
        c.abort("req_1",now).unwrap();
        c.observe_feedback(&json!({"id":"req_1",
            "event":"abort_verified_empty_engine"})).unwrap();
        assert_eq!(c.state(),StopState::StopVerified);
        assert!(!c.check_escalation(now+std::time::Duration::from_secs(3)).unwrap());
        c.generate("req_2","next",16).unwrap();
    }
    #[test]
    fn normal_finish_releases_exact_request_for_next_generation() {
        let mut c=VllmRequestController::new(Capturing(vec![]));
        c.generate("req_1","hello",32).unwrap();
        assert_eq!(c.observe_feedback(&json!({"id":"req_2","event":"finished","reason":"stop"})),
            Err(VllmControlError::InvalidFeedback));
        c.observe_feedback(&json!({"id":"req_1","event":"finished","reason":"stop"})).unwrap();
        assert_eq!(c.state(),StopState::StopVerified);
        c.generate("req_2","next",32).unwrap();
    }
}
