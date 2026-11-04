//! BACKEND-CTRL-01 — host-only stop verification and bounded worker recovery.
//! This state machine is independent of llama.cpp/vLLM wire formats. A native
//! adapter must provide an authentic observation before calling verify_stopped.
//! It never grants tools or automatically resubmits an interrupted prompt.
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopState {
    Idle,
    Generating,
    CancelRequested,
    CancelAcknowledged,
    StopVerified,
    EscalationRequired,
    WorkerTerminated,
    FailedStop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlError {
    Busy,
    NotGenerating,
    WrongRequest,
    InvalidTransition,
    RecoveryCircuitOpen,
    UnsupportedVerification,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackendCapabilities {
    pub request_scoped_cancel: bool,
    pub independent_stop_verification: bool,
    pub lu_owned_process: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct RecoveryPolicy {
    pub cancel_grace: Duration,
    pub restart_window: Duration,
    pub max_restarts: usize,
}
impl Default for RecoveryPolicy {
    fn default() -> Self {
        Self {
            cancel_grace: Duration::from_millis(750),
            restart_window: Duration::from_secs(300),
            max_restarts: 2,
        }
    }
}
impl RecoveryPolicy {
    pub fn validate(self) -> Result<Self, ControlError> {
        if self.cancel_grace.is_zero() || self.cancel_grace > Duration::from_secs(30)
            || self.restart_window.is_zero() || self.restart_window > Duration::from_secs(86400)
            || self.max_restarts == 0 || self.max_restarts > 10 {
            return Err(ControlError::InvalidTransition);
        }
        Ok(self)
    }
}

/// Host-issued request IDs are bound outside this controller; callers cannot
/// use text supplied by a model as a request identifier.
pub struct BackendController {
    caps: BackendCapabilities,
    policy: RecoveryPolicy,
    state: StopState,
    request_id: Option<String>,
    cancel_started: Option<Instant>,
    restart_history: Vec<Instant>,
}
impl BackendController {
    pub fn new(caps: BackendCapabilities, policy: RecoveryPolicy)
        -> Result<Self, ControlError> {
        Ok(Self {
            caps, policy: policy.validate()?, state: StopState::Idle,
            request_id: None, cancel_started: None, restart_history: Vec::new(),
        })
    }
    pub fn state(&self) -> StopState { self.state }
    pub fn active_request(&self) -> Option<&str> { self.request_id.as_deref() }

    pub fn start(&mut self, host_request_id: &str) -> Result<(), ControlError> {
        if self.state != StopState::Idle && self.state != StopState::StopVerified {
            return Err(ControlError::Busy);
        }
        if host_request_id.is_empty() || host_request_id.len() > 128 {
            return Err(ControlError::InvalidTransition);
        }
        self.request_id = Some(host_request_id.to_string());
        self.state = StopState::Generating;
        self.cancel_started = None;
        Ok(())
    }
    fn matches(&self, request_id: &str) -> Result<(), ControlError> {
        if self.request_id.as_deref() != Some(request_id) {
            Err(ControlError::WrongRequest)
        } else { Ok(()) }
    }
    pub fn cancel(&mut self, request_id: &str, now: Instant)
        -> Result<(), ControlError> {
        self.matches(request_id)?;
        if self.state != StopState::Generating { return Err(ControlError::InvalidTransition); }
        self.state = StopState::CancelRequested;
        self.cancel_started = Some(now);
        Ok(())
    }
    /// An ACK is never considered proof that the request stopped decoding.
    pub fn cancel_acknowledged(&mut self, request_id: &str)
        -> Result<(), ControlError> {
        self.matches(request_id)?;
        if self.state != StopState::CancelRequested { return Err(ControlError::InvalidTransition); }
        self.state = StopState::CancelAcknowledged;
        Ok(())
    }
    /// Only call with trustworthy backend-specific independent slot/request
    /// liveness evidence or a verified normal terminal EOS event.
    pub fn verify_stopped(&mut self, request_id: &str, is_active: bool)
        -> Result<(), ControlError> {
        self.matches(request_id)?;
        if !matches!(self.state, StopState::Generating |
            StopState::CancelRequested | StopState::CancelAcknowledged) {
            return Err(ControlError::InvalidTransition);
        }
        if is_active { return Err(ControlError::UnsupportedVerification); }
        if !self.caps.independent_stop_verification && self.state != StopState::Generating {
            return Err(ControlError::UnsupportedVerification);
        }
        self.state = StopState::StopVerified;
        self.cancel_started = None;
        self.request_id = None;
        Ok(())
    }
    pub fn check_escalation(&mut self, now: Instant) -> Result<bool, ControlError> {
        if !matches!(self.state, StopState::CancelRequested |
            StopState::CancelAcknowledged) { return Ok(false); }
        if self.cancel_started.is_some_and(|since| now.saturating_duration_since(since) >= self.policy.cancel_grace) {
            self.state = StopState::EscalationRequired;
            return Ok(true);
        }
        Ok(false)
    }
    pub fn mark_worker_terminated(&mut self, exit_confirmed: bool)
        -> Result<(), ControlError> {
        if self.state != StopState::EscalationRequired || !self.caps.lu_owned_process {
            return Err(ControlError::InvalidTransition);
        }
        self.state = if exit_confirmed { StopState::WorkerTerminated }
            else { StopState::FailedStop };
        self.request_id = None;
        Ok(())
    }
    /// A forced restart is allowed only once the prior owned worker exited
    /// and the host has separately reconciled uncertain effects and resources.
    /// This function doesn't launch any process or retry the failed prompt.
    pub fn permit_restart(&mut self, now: Instant, reconciled: bool)
        -> Result<(), ControlError> {
        if self.state != StopState::WorkerTerminated || !reconciled {
            return Err(ControlError::InvalidTransition);
        }
        self.restart_history.retain(|at|
            now.saturating_duration_since(*at) < self.policy.restart_window);
        if self.restart_history.len() >= self.policy.max_restarts {
            return Err(ControlError::RecoveryCircuitOpen);
        }
        self.restart_history.push(now);
        self.state = StopState::Idle;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn owned() -> BackendController {
        BackendController::new(BackendCapabilities {
            request_scoped_cancel: true, independent_stop_verification: true,
            lu_owned_process: true,
        }, RecoveryPolicy::default()).unwrap()
    }
    #[test]
    fn abort_ack_is_not_stop_and_escalates_after_deadline() {
        let mut c=owned(); let now=Instant::now();
        c.start("request-a").unwrap(); c.cancel("request-a",now).unwrap();
        c.cancel_acknowledged("request-a").unwrap();
        assert_eq!(c.state(),StopState::CancelAcknowledged);
        assert!(!c.check_escalation(now+Duration::from_millis(100)).unwrap());
        assert!(c.check_escalation(now+Duration::from_secs(1)).unwrap());
        assert_eq!(c.state(),StopState::EscalationRequired);
        c.mark_worker_terminated(false).unwrap();
        assert_eq!(c.state(),StopState::FailedStop);
    }
    #[test]
    fn verified_cancel_preserves_worker_without_escalation() {
        let mut c=owned(); let now=Instant::now();
        c.start("request-a").unwrap(); c.cancel("request-a",now).unwrap();
        assert_eq!(c.verify_stopped("request-a",true),Err(ControlError::UnsupportedVerification));
        c.verify_stopped("request-a",false).unwrap();
        assert_eq!(c.state(),StopState::StopVerified);
        assert!(!c.check_escalation(now+Duration::from_secs(10)).unwrap());
    }
    #[test]
    fn restart_circuit_breaker_blocks_the_third_recovery() {
        let mut c=owned(); let now=Instant::now();
        for i in 0..3 {
            let rid=format!("request-{i}");
            c.start(&rid).unwrap(); c.cancel(&rid,now).unwrap();
            assert!(c.check_escalation(now+Duration::from_secs(1)).unwrap());
            c.mark_worker_terminated(true).unwrap();
            let result=c.permit_restart(now+Duration::from_secs(2),true);
            if i==2 {
                assert_eq!(result,Err(ControlError::RecoveryCircuitOpen));
            } else { assert!(result.is_ok()); }
        }
    }
    #[test]
    fn cannot_abort_another_request_or_restart_before_reconciliation() {
        let mut c=owned(); let now=Instant::now();
        c.start("request-a").unwrap();
        assert_eq!(c.cancel("request-b",now),Err(ControlError::WrongRequest));
        c.cancel("request-a",now).unwrap();
        assert_eq!(c.permit_restart(now,true),Err(ControlError::InvalidTransition));
        c.check_escalation(now+Duration::from_secs(1)).unwrap();
        c.mark_worker_terminated(true).unwrap();
        assert_eq!(c.permit_restart(now,false),Err(ControlError::InvalidTransition));
    }
}
