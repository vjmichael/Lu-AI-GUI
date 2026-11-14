//! DEV-04.6A: review-only debugger operations. NO DAP mutation or authority.
//! A real native D5 owner event is required to construct the private stop
//! binding; model/DAP/WebView claims do not create execution permission.
use sha2::{Digest, Sha256};

const MAX_REVIEWS_PER_STOP: u8 = 8;
const MAX_SOURCE_PATH: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugReviewDenial {
    BadStop,
    BadOperation,
    WrongSource,
    WrongThread,
    UnverifiedFrame,
    ReviewAlreadyPending,
    StaleOrRevoked,
    TooManyReviews,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepKind {
    Over,
    Into,
    Out,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DebugReviewIntent {
    Breakpoint { source: String, line: u32 },
    Step { kind: StepKind, thread_id: u32 },
    InspectVariables { frame_id: u64 },
}

/// Private native stop binding. The only non-test constructor accepts the
/// independently protected Windows D5 event-owner witness, CI-only.
#[derive(Debug, Clone)]
struct NativeStop {
    job_id: String,
    debug_plan_sha: String,
    build_completed_sha: String,
    debug_running_sha: String,
    owner_event_sha: String,
    source_snapshot_sha: String,
    source: String,
    line: u32,
    process_id: u32,
    thread_id: u32,
    rip: u64,
    event_sequence: u64,
    verified_frame_id: Option<u64>,
}

fn is_sha(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|c| c.is_ascii_hexdigit())
}
fn is_source(path: &str) -> bool {
    if path.is_empty()
        || path.len() > MAX_SOURCE_PATH
        || path.starts_with('/')
        || path.starts_with('\\')
        || path.contains(':')
        || path.contains('\\')
        || path.chars().any(char::is_control)
    {
        return false;
    }
    let mut count = 0;
    for p in path.split('/') {
        if p.is_empty()
            || p == "."
            || p == ".."
            || p.starts_with('.')
            || p.ends_with(' ')
            || !p
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.'))
        {
            return false;
        }
        count += 1;
    }
    (2..=12).contains(&count)
}
impl NativeStop {
    fn validate(&self) -> Result<(), DebugReviewDenial> {
        if !self.job_id.starts_with("job-")
            || self.job_id.len() != 36
            || !self.job_id[4..].bytes().all(|b| b.is_ascii_hexdigit())
            || !is_sha(&self.debug_plan_sha)
            || !is_sha(&self.build_completed_sha)
            || !is_sha(&self.debug_running_sha)
            || !is_sha(&self.owner_event_sha)
            || !is_sha(&self.source_snapshot_sha)
            || !is_source(&self.source)
            || self.line == 0
            || self.line > 1_000_000
            || self.process_id == 0
            || self.thread_id == 0
            || self.rip == 0
            || self.event_sequence == 0
            || self.event_sequence > 4096
        {
            return Err(DebugReviewDenial::BadStop);
        }
        Ok(())
    }
}

/// This is compiled ONLY inside the D5 disposable native CI feature.
/// Its result is a candidate review surface, never a live debugger grant.
#[cfg(all(windows, debug_assertions, feature = "dev045d5-ci-event-owner"))]
pub(crate) fn review_from_verified_native_owner_for_ci(
    witness: &crate::windows_worker::lsp_native::WindowsOwnerEventAccepted,
    source_snapshot_sha256: &str,
) -> Result<DebugReviewSession, DebugReviewDenial> {
    if !witness.lldb_event_loop_owner_attested
        || !witness.protected_pe_pdb_verified
        || !witness.durable_receipt_verified
        || witness.verified_frame_id == 0
        || witness.verified_frame_id > 1_000_000
        || witness.debugger_commands_authorized
        || witness.installed_app_accepted
    {
        return Err(DebugReviewDenial::BadStop);
    }
    DebugReviewSession::from_stop(NativeStop {
        job_id: witness.debugger_job_id.clone(),
        debug_plan_sha: witness.debugger_plan_sha256.clone(),
        build_completed_sha: witness.build_terminal_receipt_sha256.clone(),
        debug_running_sha: witness.debug_running_receipt_sha256.clone(),
        owner_event_sha: witness.native_owner_receipt_sha256.clone(),
        source_snapshot_sha: source_snapshot_sha256.to_owned(),
        // D5's fixed first native project maps exactly this source. General
        // source locations require their own independent PDB/source proof.
        source: "src/main.rs".into(),
        line: witness.verified_source_line,
        process_id: witness.debuggee_pid,
        thread_id: witness.debuggee_thread_id,
        rip: witness.stopped_rip,
        event_sequence: witness.owner_sequence,
        verified_frame_id: Some(witness.verified_frame_id),
    })
}

/// FIN-04R-1: construct a *review-only* action identity from the current
/// protected first-party LLDB event plus an independent protected PE/PDB
/// source map. This is neither a reusable consent nor executable authority.
/// Every field is from the native Windows Job and its existing receipts,
/// never an adapter stop packet, WebView or recovered action journal.
#[cfg(all(windows, feature="fin04-release-owner-evidence"))]
pub(crate) fn review_from_protected_release_owner(
    witness:&crate::windows_worker::lsp_native::ReleaseOwnerObservation,
)->Result<DebugReviewSession,DebugReviewDenial>{
    if !witness.os_event_owner_verified ||
       !witness.original_debug_job_verified ||
       !witness.original_build_receipt_verified ||
       !witness.source_line_independently_verified ||
       witness.debugger_control_authorized || witness.recovery_authorized ||
       witness.independently_verified_source_line.is_none() ||
       witness.independently_verified_source_sha256.as_deref()
           .is_none_or(|sha|!is_sha(sha)) ||
       witness.independent_pdb_sha256.as_deref()
           .is_none_or(|sha|!is_sha(sha)) ||
       witness.source_frame_id==0 || witness.source_frame_id>1_000_000
    {
        return Err(DebugReviewDenial::BadStop);
    }
    DebugReviewSession::from_stop(NativeStop{
        job_id:witness.debugger_job_id.clone(),
        debug_plan_sha:witness.debugger_plan_sha256.clone(),
        build_completed_sha:witness.build_terminal_receipt_sha256.clone(),
        debug_running_sha:witness.debug_running_receipt_sha256.clone(),
        owner_event_sha:witness.receipt_sha256.clone(),
        source_snapshot_sha:witness.source_snapshot_sha256.clone(),
        source:"src/main.rs".into(),
        line:witness.independently_verified_source_line
            .ok_or(DebugReviewDenial::BadStop)?,
        process_id:witness.process_id,thread_id:witness.thread_id,
        rip:witness.stopped_rip,event_sequence:witness.owner_sequence,
        verified_frame_id:Some(witness.source_frame_id),
    })
}
/// Preview-only. It is not a Tauri command or an executable DAP request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugReviewPreview {
    pub preview_sha256: String,
    pub intent: DebugReviewIntent,
    pub owner_event_sha256: String,
    pub source_snapshot_sha256: String,
    pub stopped_rip: u64,
    pub owner_sequence: u64,
    pub executable_authority: bool,
    pub installed_app_accepted: bool,
}

/// A private, non-transferable association between a reviewed operation
/// and the original one-use OS event. It is never an execution grant.
#[derive(Debug, Clone)]
pub(crate) struct NativeReviewBinding {
    pub(crate) job_id: String,
    pub(crate) plan_sha256: String,
    pub(crate) build_receipt_sha256: String,
    pub(crate) running_receipt_sha256: String,
    pub(crate) owner_event_sha256: String,
    pub(crate) source_snapshot_sha256: String,
    pub(crate) owner_sequence: u64,
    pub(crate) process_id: u32,
    pub(crate) thread_id: u32,
    pub(crate) stopped_rip: u64,
    pub(crate) source_line: u32,
    pub(crate) relative_source: String,
}

pub struct DebugReviewSession {
    stop: NativeStop,
    live: bool,
    reviewed: u8,
    pending: Option<DebugReviewPreview>,
}
impl DebugReviewSession {
    fn from_stop(stop: NativeStop) -> Result<Self, DebugReviewDenial> {
        stop.validate()?;
        Ok(Self {
            stop,
            live: true,
            reviewed: 0,
            pending: None,
        })
    }
    /// Called on continued event, cancellation, crash, source mutation, Job
    /// custody loss or restart. A disk receipt CANNOT reanimate this state.
    pub fn invalidate(&mut self) {
        self.live = false;
        self.pending = None;
    }
    pub fn preview(
        &mut self,
        intent: DebugReviewIntent,
        current_owner_sequence: u64,
    ) -> Result<DebugReviewPreview, DebugReviewDenial> {
        if !self.live || current_owner_sequence != self.stop.event_sequence {
            self.invalidate();
            return Err(DebugReviewDenial::StaleOrRevoked);
        }
        if self.pending.is_some() {
            return Err(DebugReviewDenial::ReviewAlreadyPending);
        }
        if self.reviewed >= MAX_REVIEWS_PER_STOP {
            return Err(DebugReviewDenial::TooManyReviews);
        }
        let operation = match &intent {
            DebugReviewIntent::Breakpoint { source, line } => {
                if !is_source(source) {
                    return Err(DebugReviewDenial::BadOperation);
                }
                if source != &self.stop.source || *line != self.stop.line {
                    return Err(DebugReviewDenial::WrongSource);
                }
                format!("breakpoint:{source}:{line}")
            }
            DebugReviewIntent::Step { kind, thread_id } => {
                if *thread_id != self.stop.thread_id {
                    return Err(DebugReviewDenial::WrongThread);
                }
                let action = match kind {
                    StepKind::Over => "over",
                    StepKind::Into => "into",
                    StepKind::Out => "out",
                };
                format!("step:{action}:{thread_id}")
            }
            DebugReviewIntent::InspectVariables { frame_id } => {
                // D5 does not independently attest frame IDs; reject rather
                // than querying scopes/variables on adapter testimony alone.
                if *frame_id == 0 || Some(*frame_id) != self.stop.verified_frame_id {
                    return Err(DebugReviewDenial::UnverifiedFrame);
                }
                format!("variables:{frame_id}")
            }
        };
        self.reviewed += 1;
        let body = format!(
            "lu-debug-review-v1\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
            self.stop.job_id,
            self.stop.debug_plan_sha,
            self.stop.build_completed_sha,
            self.stop.debug_running_sha,
            self.stop.owner_event_sha,
            self.stop.source_snapshot_sha,
            self.stop.source,
            self.stop.line,
            self.stop.process_id,
            self.stop.thread_id,
            self.stop.rip,
            self.stop.event_sequence,
            self.reviewed,
            current_owner_sequence,
            operation,
        );
        let preview = DebugReviewPreview {
            preview_sha256: format!("{:x}", Sha256::digest(body.as_bytes())),
            intent,
            owner_event_sha256: self.stop.owner_event_sha.clone(),
            source_snapshot_sha256: self.stop.source_snapshot_sha.clone(),
            stopped_rip: self.stop.rip,
            owner_sequence: self.stop.event_sequence,
            executable_authority: false,
            installed_app_accepted: false,
        };
        self.pending = Some(preview.clone());
        Ok(preview)
    }
    pub fn decline(&mut self, sha: &str) -> Result<(), DebugReviewDenial> {
        if !self.live
            || !self
                .pending
                .as_ref()
                .is_some_and(|p| p.preview_sha256 == sha)
        {
            return Err(DebugReviewDenial::BadOperation);
        }
        self.pending = None;
        Ok(())
    }
    /// Bind the exactly pending review to the native owner and journal.
    /// There is deliberately no source-level executor in D6B.
    pub(crate) fn pending_native_binding(
        &self,
        hash: &str,
        current_sequence: u64,
    ) -> Result<NativeReviewBinding, DebugReviewDenial> {
        if !self.live || current_sequence != self.stop.event_sequence {
            return Err(DebugReviewDenial::StaleOrRevoked);
        }
        if !self
            .pending
            .as_ref()
            .is_some_and(|x| x.preview_sha256 == hash)
        {
            return Err(DebugReviewDenial::BadOperation);
        }
        Ok(NativeReviewBinding {
            job_id: self.stop.job_id.clone(),
            plan_sha256: self.stop.debug_plan_sha.clone(),
            build_receipt_sha256: self.stop.build_completed_sha.clone(),
            running_receipt_sha256: self.stop.debug_running_sha.clone(),
            owner_event_sha256: self.stop.owner_event_sha.clone(),
            source_snapshot_sha256: self.stop.source_snapshot_sha.clone(),
            owner_sequence: self.stop.event_sequence,
            process_id: self.stop.process_id,
            thread_id: self.stop.thread_id,
            stopped_rip: self.stop.rip,
            source_line: self.stop.line,
            relative_source: self.stop.source.clone(),
        })
    }
    /// There is deliberately no approve/dispatch method in D6A.
    pub fn has_executable_authority(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> NativeStop {
        NativeStop {
            job_id: format!("job-{}", "a".repeat(32)),
            debug_plan_sha: "b".repeat(64),
            build_completed_sha: "c".repeat(64),
            debug_running_sha: "d".repeat(64),
            owner_event_sha: "e".repeat(64),
            source_snapshot_sha: "f".repeat(64),
            source: "src/main.rs".into(),
            line: 7,
            process_id: 22,
            thread_id: 33,
            rip: 0x140001000,
            event_sequence: 5,
            verified_frame_id: None,
        }
    }
    #[test]
    fn precise_breakpoint_review_requires_new_step_review_and_is_not_a_command() {
        let mut s = DebugReviewSession::from_stop(sample()).unwrap();
        let first = s
            .preview(
                DebugReviewIntent::Breakpoint {
                    source: "src/main.rs".into(),
                    line: 7,
                },
                5,
            )
            .unwrap();
        assert!(is_sha(&first.preview_sha256));
        assert!(!first.executable_authority);
        assert!(!s.has_executable_authority());
        assert_eq!(
            s.preview(
                DebugReviewIntent::Step {
                    kind: StepKind::Into,
                    thread_id: 33,
                },
                5
            ),
            Err(DebugReviewDenial::ReviewAlreadyPending)
        );
        s.decline(&first.preview_sha256).unwrap();
        let second = s
            .preview(
                DebugReviewIntent::Step {
                    kind: StepKind::Over,
                    thread_id: 33,
                },
                5,
            )
            .unwrap();
        assert_ne!(first.preview_sha256, second.preview_sha256);
        s.invalidate();
        assert_eq!(
            s.preview(
                DebugReviewIntent::Step {
                    kind: StepKind::Out,
                    thread_id: 33,
                },
                5
            ),
            Err(DebugReviewDenial::StaleOrRevoked)
        );
    }
    #[test]
    fn rejects_wrong_files_lines_threads_and_unverified_variables() {
        let mut s = DebugReviewSession::from_stop(sample()).unwrap();
        for path in [
            "/src/main.rs",
            "../src/main.rs",
            "src\\main.rs",
            "src/./main.rs",
            "C:/src/main.rs",
            "src//main.rs",
            "src/main.rs\n",
        ] {
            assert!(!is_source(path));
        }
        assert_eq!(
            s.preview(
                DebugReviewIntent::Breakpoint {
                    source: "src/other.rs".into(),
                    line: 7,
                },
                5
            ),
            Err(DebugReviewDenial::WrongSource)
        );
        assert_eq!(
            s.preview(
                DebugReviewIntent::Breakpoint {
                    source: "src/main.rs".into(),
                    line: 8,
                },
                5
            ),
            Err(DebugReviewDenial::WrongSource)
        );
        assert_eq!(
            s.preview(
                DebugReviewIntent::Step {
                    kind: StepKind::Into,
                    thread_id: 11,
                },
                5
            ),
            Err(DebugReviewDenial::WrongThread)
        );
        assert_eq!(
            s.preview(DebugReviewIntent::InspectVariables { frame_id: 1 }, 5),
            Err(DebugReviewDenial::UnverifiedFrame)
        );
    }
    #[test]
    fn changed_event_revokes_even_previous_review() {
        let mut s = DebugReviewSession::from_stop(sample()).unwrap();
        assert_eq!(
            s.preview(
                DebugReviewIntent::Step {
                    kind: StepKind::Over,
                    thread_id: 33,
                },
                6
            ),
            Err(DebugReviewDenial::StaleOrRevoked)
        );
        assert_eq!(
            s.preview(
                DebugReviewIntent::Step {
                    kind: StepKind::Over,
                    thread_id: 33,
                },
                5
            ),
            Err(DebugReviewDenial::StaleOrRevoked)
        );
    }
    #[test]
    fn malformed_owner_receipt_refused_at_construction() {
        let mut bad = sample();
        bad.owner_event_sha = "0".repeat(63);
        assert!(matches!(
            DebugReviewSession::from_stop(bad),
            Err(DebugReviewDenial::BadStop)
        ));
    }
    #[test]
    fn eight_reviews_max_and_no_replay() {
        let mut s = DebugReviewSession::from_stop(sample()).unwrap();
        for _ in 0..MAX_REVIEWS_PER_STOP {
            let p = s
                .preview(
                    DebugReviewIntent::Step {
                        kind: StepKind::Over,
                        thread_id: 33,
                    },
                    5,
                )
                .unwrap();
            s.decline(&p.preview_sha256).unwrap();
        }
        assert_eq!(
            s.preview(
                DebugReviewIntent::Step {
                    kind: StepKind::Over,
                    thread_id: 33,
                },
                5
            ),
            Err(DebugReviewDenial::TooManyReviews)
        );
        assert!(!s.has_executable_authority());
    }
}
