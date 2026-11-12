//! DEV-04.6B — one-operation human-consent admission and crash-safe receipts.
//!
//! This module is compiled ONLY for unit tests or the disposable D5 Windows
//! kernel-event CI fixture. It never sends DAP, executes a worker or exposes a
//! model/WebView command. The native installed consent bridge is NOT present:
//! all current native test consents are expressly CI_ONLY and cannot become a
//! production privilege, even when a truthful per-action receipt is recorded.
#![cfg(any(test, any(all(windows, debug_assertions, feature = "dev045d5-ci-event-owner"), all(windows, feature = "fin04-release-owner-evidence"))))]

use crate::debug_command_review::{DebugReviewIntent, DebugReviewPreview, NativeReviewBinding};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
const MAX_LOG_BYTES: u64 = 32 * 1024;
const MAX_EVENTS: u64 = 12;
const MAX_TTL_MS: u64 = 30_000;
const MAX_ACTIONS_PER_RESTART: usize = 4096;

fn unsafe_link(meta: &fs::Metadata) -> bool {
    if meta.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    false
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ApprovalDenial {
    WrongReview,
    WrongStop,
    Expired,
    NativeConsentMissing,
    AlreadyDecided,
    GrantConsumed,
    Cancelled,
    JournalUnavailable,
    JournalCorrupt,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    AwaitingConsent,
    Approved,
    Consumed,
    Denied,
    Revoked,
}

/// A consent source is not equivalent to "confirmed=true" sent by a model.
/// The production native UI owner must eventually provide a trusted marker
/// from an independently verified OS consent action. There is no such
/// constructor in this source branch. CI may simulate it for state tests.
#[derive(Debug)]
pub(crate) struct NativeConsentMarker {
    preview_sha256: String,
    challenge_sha256: String,
    native_owner_sequence: u64,
    observed_unix_ms: u64,
    confirmed: bool,
    ci_fixture_only: bool,
}
impl NativeConsentMarker {
    /// Call only from the trusted native Windows host's blocking OS dialog
    /// callback, after deriving the challenge from the authentic D5 owner.
    /// No renderer command can construct this non-CI marker.
    #[cfg(all(windows, debug_assertions, feature = "dev046d-native-dialog"))]
    pub(crate) fn for_trusted_modal(
        preview: &str,
        challenge: &str,
        sequence: u64,
        now: u64,
        confirmed: bool,
    ) -> Self {
        Self {
            preview_sha256: preview.to_owned(),
            challenge_sha256: challenge.to_owned(),
            native_owner_sequence: sequence,
            observed_unix_ms: now,
            confirmed,
            ci_fixture_only: false,
        }
    }
    #[cfg(test)]
    fn for_unit_test(preview: &str, challenge: &str, sequence: u64, now: u64, yes: bool) -> Self {
        Self {
            preview_sha256: preview.to_owned(),
            challenge_sha256: challenge.to_owned(),
            native_owner_sequence: sequence,
            observed_unix_ms: now,
            confirmed: yes,
            ci_fixture_only: true,
        }
    }
    /// A disposable acceptance fixture can exercise the exact journal and
    /// one-shot rules; this does NOT prove actual human GUI approval.
    #[cfg(all(windows, debug_assertions, feature = "dev045d5-ci-event-owner"))]
    pub(crate) fn for_native_ci_fixture(
        preview: &str,
        challenge: &str,
        sequence: u64,
        now: u64,
        yes: bool,
    ) -> Self {
        Self {
            preview_sha256: preview.to_owned(),
            challenge_sha256: challenge.to_owned(),
            native_owner_sequence: sequence,
            observed_unix_ms: now,
            confirmed: yes,
            ci_fixture_only: true,
        }
    }
}
fn sha_text(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
fn hex64(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|x| x.is_ascii_hexdigit())
}
fn valid_job(value: &str) -> bool {
    value.len() == 36
        && value.starts_with("job-")
        && value[4..].bytes().all(|x| x.is_ascii_hexdigit())
}
pub(crate) fn op_digest(op: &DebugReviewIntent) -> String {
    let canonical = match op {
        DebugReviewIntent::Breakpoint { source, line } => format!("breakpoint:{source}:{line}"),
        DebugReviewIntent::Step { kind, thread_id } => format!("step:{kind:?}:{thread_id}"),
        DebugReviewIntent::InspectVariables { frame_id } => format!("variables:{frame_id}"),
    };
    sha_text(&canonical)
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ActionEvent {
    version: u32,
    ordinal: u64,
    action_id_sha256: String,
    preview_sha256: String,
    operation_sha256: String,
    job_id: String,
    plan_sha256: String,
    owner_event_sha256: String,
    owner_sequence: u64,
    phase: String,
    outcome_code: String,
    observed_unix_ms: u64,
    previous_hash: String,
    entry_hash: String,
}
fn event_hash(e: &ActionEvent) -> Result<String, ApprovalDenial> {
    let mut copy = e.clone();
    copy.entry_hash.clear();
    serde_json::to_vec(&copy)
        .map(|json| format!("{:x}", Sha256::digest(&json)))
        .map_err(|_| ApprovalDenial::JournalCorrupt)
}
fn verify_events(
    raw: &[u8],
    action: &str,
    preview: &str,
) -> Result<(u64, String, String), ApprovalDenial> {
    if raw.is_empty() || raw.len() as u64 > MAX_LOG_BYTES || !raw.ends_with(b"\n") {
        return Err(ApprovalDenial::JournalCorrupt);
    }
    let mut prior = "0".repeat(64);
    let mut sequence = 0u64;
    let mut phase = "";
    let mut original: Option<(String, String, String, String, u64)> = None;
    let mut last_timestamp: Option<u64> = None;
    for line in raw.split(|b| *b == b'\n').filter(|line| !line.is_empty()) {
        let e: ActionEvent =
            serde_json::from_slice(line).map_err(|_| ApprovalDenial::JournalCorrupt)?;
        if e.version != 1
            || e.ordinal != sequence + 1
            || e.ordinal > MAX_EVENTS
            || e.action_id_sha256 != action
            || e.preview_sha256 != preview
            || e.previous_hash != prior
            || !hex64(&e.entry_hash)
            || event_hash(&e)? != e.entry_hash
            || !hex64(&e.operation_sha256)
            || !hex64(&e.plan_sha256)
            || !hex64(&e.owner_event_sha256)
            || !valid_job(&e.job_id)
            || e.owner_sequence == 0
        {
            return Err(ApprovalDenial::JournalCorrupt);
        }
        // Event-by-event hashing is insufficient if a later entry can
        // silently change the original Job, plan, operation or OS owner.
        let identity = (
            e.operation_sha256.clone(),
            e.job_id.clone(),
            e.plan_sha256.clone(),
            e.owner_event_sha256.clone(),
            e.owner_sequence,
        );
        if let Some(origin) = &original {
            if origin != &identity {
                return Err(ApprovalDenial::JournalCorrupt);
            }
        } else {
            original = Some(identity);
        }
        if last_timestamp.is_some_and(|prior| e.observed_unix_ms < prior) {
            return Err(ApprovalDenial::JournalCorrupt);
        }
        last_timestamp = Some(e.observed_unix_ms);
        let known_outcome = match e.phase.as_str() {
            "Prepared" => e.outcome_code == "awaiting_fresh_native_operator",
            "Approved" => matches!(
                e.outcome_code.as_str(),
                "ci_only_not_installed_consent" | "native_human_confirmed"
            ),
            "Denied" => e.outcome_code == "native_operator_declined",
            "Consumed" => e.outcome_code == "one_use_slot_consumed_before_dispatch",
            "Revoked" => matches!(
                e.outcome_code.as_str(),
                "wrong_owner_or_review"
                    | "expired"
                    | "stale_owner_or_preview"
                    | "expired_before_consumption"
                    | "stop_or_operator_revoked"
            ),
            "ResultUncertain" => {
                e.outcome_code == "requires_independent_result_reconciliation_no_replay"
            }
            "ResultVerified" => {
                e.outcome_code.starts_with("ci_independent_os_step:")
                    || e.outcome_code
                        .starts_with("native_dialog_independent_os_step:")
            }
            _ => false,
        };
        if !known_outcome {
            return Err(ApprovalDenial::JournalCorrupt);
        }
        if e.phase == "ResultVerified" {
            let Some(hash) = e
                .outcome_code
                .strip_prefix("ci_independent_os_step:")
                .or_else(|| {
                    e.outcome_code
                        .strip_prefix("native_dialog_independent_os_step:")
                })
            else {
                return Err(ApprovalDenial::JournalCorrupt);
            };
            if !hex64(hash) {
                return Err(ApprovalDenial::JournalCorrupt);
            }
        }
        let allowed = matches!(
            (phase, e.phase.as_str()),
            ("", "Prepared")
                | ("Prepared", "Approved")
                | ("Prepared", "Denied")
                | ("Prepared", "Revoked")
                | ("Approved", "Consumed")
                | ("Approved", "Revoked")
                | ("Consumed", "ResultVerified")
                | ("Consumed", "ResultUncertain")
        );
        if !allowed {
            return Err(ApprovalDenial::JournalCorrupt);
        }
        sequence = e.ordinal;
        prior = e.entry_hash;
        phase = match e.phase.as_str() {
            "Prepared" => "Prepared",
            "Approved" => "Approved",
            "Denied" => "Denied",
            "Revoked" => "Revoked",
            "Consumed" => "Consumed",
            "ResultVerified" => "ResultVerified",
            "ResultUncertain" => "ResultUncertain",
            _ => return Err(ApprovalDenial::JournalCorrupt),
        };
    }
    if sequence == 0 {
        return Err(ApprovalDenial::JournalCorrupt);
    }
    Ok((sequence, prior, phase.to_owned()))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RestartActionRecord {
    pub(crate) action_id_sha256: String,
    pub(crate) job_id: String,
    pub(crate) plan_sha256: String,
    pub(crate) phase: String,
    pub(crate) last_receipt_sha256: String,
    pub(crate) requires_native_os_cleanup: bool,
}

/// DEV-04.6D read-only restart inventory. No claim can be recovered, issued
/// or sent to DAP here. Every in-flight approval requires OS-only cleanup.
pub(crate) fn inspect_action_receipts_after_restart(
    trusted_root: &Path,
) -> Result<Vec<RestartActionRecord>, ApprovalDenial> {
    let root =
        fs::symlink_metadata(trusted_root).map_err(|_| ApprovalDenial::JournalUnavailable)?;
    if !trusted_root.is_absolute() || !root.is_dir() || unsafe_link(&root) {
        return Err(ApprovalDenial::JournalCorrupt);
    }
    let mut result = Vec::new();
    let entries = fs::read_dir(trusted_root).map_err(|_| ApprovalDenial::JournalUnavailable)?;
    for entry in entries {
        let entry = entry.map_err(|_| ApprovalDenial::JournalUnavailable)?;
        let name = entry.file_name();
        let name = name.to_str().ok_or(ApprovalDenial::JournalCorrupt)?;
        if !name.starts_with("lu-debug-action-") {
            continue;
        }
        let action = name
            .strip_prefix("lu-debug-action-")
            .and_then(|s| s.strip_suffix(".ndjson"))
            .ok_or(ApprovalDenial::JournalCorrupt)?;
        if !hex64(action) || result.len() >= MAX_ACTIONS_PER_RESTART {
            return Err(ApprovalDenial::JournalCorrupt);
        }
        let meta =
            fs::symlink_metadata(entry.path()).map_err(|_| ApprovalDenial::JournalUnavailable)?;
        if !meta.is_file() || unsafe_link(&meta) || meta.len() == 0 || meta.len() > MAX_LOG_BYTES {
            return Err(ApprovalDenial::JournalCorrupt);
        }
        let mut bytes = Vec::new();
        File::open(entry.path())
            .map_err(|_| ApprovalDenial::JournalUnavailable)?
            .take(MAX_LOG_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| ApprovalDenial::JournalUnavailable)?;
        let first_line = bytes
            .split(|b| *b == b'\n')
            .next()
            .ok_or(ApprovalDenial::JournalCorrupt)?;
        let first: ActionEvent =
            serde_json::from_slice(first_line).map_err(|_| ApprovalDenial::JournalCorrupt)?;
        let (_, last_hash, phase) = verify_events(&bytes, action, &first.preview_sha256)?;
        result.push(RestartActionRecord {
            action_id_sha256: action.to_owned(),
            job_id: first.job_id,
            plan_sha256: first.plan_sha256,
            requires_native_os_cleanup: matches!(
                phase.as_str(),
                "Prepared" | "Approved" | "Consumed"
            ),
            phase,
            last_receipt_sha256: last_hash,
        });
    }
    result.sort_by(|a, b| a.action_id_sha256.cmp(&b.action_id_sha256));
    Ok(result)
}

/// Audit identity only. Possession of this value cannot recreate a
/// consumed in-memory grant or invoke the DAP gateway.
#[derive(Debug, Clone)]
pub(crate) struct DebugActionAuditRef {
    pub(crate) action_id_sha256: String,
    pub(crate) preview_sha256: String,
    pub(crate) last_receipt_sha256: String,
    pub(crate) final_phase: String,
}

/// Reopen the original private journal directory after the supervisor
/// restarts; verify the entire chained, transition-checked log. Never issue
/// or reconstruct an authorization from a receipt file.
pub(crate) fn verify_action_receipt_after_restart(
    trusted_root: &Path,
    reference: &DebugActionAuditRef,
) -> Result<(), ApprovalDenial> {
    if !trusted_root.is_absolute()
        || !hex64(&reference.action_id_sha256)
        || !hex64(&reference.preview_sha256)
        || !hex64(&reference.last_receipt_sha256)
    {
        return Err(ApprovalDenial::JournalCorrupt);
    }
    let root =
        fs::symlink_metadata(trusted_root).map_err(|_| ApprovalDenial::JournalUnavailable)?;
    if !root.is_dir() || unsafe_link(&root) {
        return Err(ApprovalDenial::JournalCorrupt);
    }
    let path = trusted_root.join(format!(
        "lu-debug-action-{}.ndjson",
        reference.action_id_sha256,
    ));
    let meta = fs::symlink_metadata(&path).map_err(|_| ApprovalDenial::JournalUnavailable)?;
    if !meta.is_file() || unsafe_link(&meta) || meta.len() > MAX_LOG_BYTES || meta.len() == 0 {
        return Err(ApprovalDenial::JournalCorrupt);
    }
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|_| ApprovalDenial::JournalUnavailable)?
        .take(MAX_LOG_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ApprovalDenial::JournalUnavailable)?;
    let (_, hash, phase) = verify_events(
        &bytes,
        &reference.action_id_sha256,
        &reference.preview_sha256,
    )?;
    if hash != reference.last_receipt_sha256
        || phase != reference.final_phase
        || !matches!(
            phase.as_str(),
            "Denied" | "Revoked" | "Consumed" | "ResultVerified" | "ResultUncertain"
        )
    {
        return Err(ApprovalDenial::JournalCorrupt);
    }
    Ok(())
}

/// A one-time claim is not a debugger operation. There is deliberately no
/// adapter request bytes or dispatch method in this object.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ConsumedControlClaim {
    pub(crate) action_id_sha256: String,
    pub(crate) approved_receipt_sha256: String,
    pub(crate) consumed_receipt_sha256: String,
    pub(crate) operation_sha256: String,
    pub(crate) owner_sequence: u64,
    pub(crate) ci_fixture_only: bool,
    pub(crate) execution_authorized: bool,
    pub(crate) job_id: String,
    pub(crate) plan_sha256: String,
    pub(crate) owner_event_sha256: String,
    pub(crate) source_snapshot_sha256: String,
    pub(crate) preview_sha256: String,
    pub(crate) stopped_rip: u64,
    pub(crate) process_id: u32,
    pub(crate) thread_id: u32,
    pub(crate) source_line: u32,
    pub(crate) relative_source: String,
    pub(crate) operation: DebugReviewIntent,
}
pub(crate) struct DebugActionApproval {
    binding: NativeReviewBinding,
    preview: DebugReviewPreview,
    path: PathBuf,
    file: File,
    action_id_sha256: String,
    challenge_sha256: String,
    nonce_sha256: String,
    deadline_ms: u64,
    seq: u64,
    prior: String,
    approved_sha: Option<String>,
    consent_ci_only: bool,
    state: State,
}
impl DebugActionApproval {
    /// Root is the *existing native supervisor journal parent*, never a
    /// project/source folder. Only a CI-native host method calls this outside
    /// tests; no public constructor for a renderer/model/adapter.
    pub(crate) fn prepare_native(
        trusted_journal_parent: &Path,
        binding: NativeReviewBinding,
        preview: DebugReviewPreview,
        now_ms: u64,
    ) -> Result<Self, ApprovalDenial> {
        let metadata = fs::symlink_metadata(trusted_journal_parent)
            .map_err(|_| ApprovalDenial::JournalUnavailable)?;
        if !trusted_journal_parent.is_absolute()
            || !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || !valid_job(&binding.job_id)
            || !hex64(&binding.plan_sha256)
            || !hex64(&binding.owner_event_sha256)
            || !hex64(&binding.build_receipt_sha256)
            || !hex64(&binding.running_receipt_sha256)
            || !hex64(&binding.source_snapshot_sha256)
            || !hex64(&preview.preview_sha256)
            || preview.owner_event_sha256 != binding.owner_event_sha256
            || preview.owner_sequence != binding.owner_sequence
            || preview.source_snapshot_sha256 != binding.source_snapshot_sha256
            || preview.stopped_rip != binding.stopped_rip
            || binding.process_id == 0
            || binding.thread_id == 0
            || binding.source_line == 0
            || binding.relative_source != "src/main.rs"
            || preview.executable_authority
            || preview.installed_app_accepted
        {
            return Err(ApprovalDenial::WrongStop);
        }
        // CSPRNG random ID is never serialized as a reusable grant token.
        let random = rand::random::<[u8; 32]>();
        let nonce_sha256 = format!("{:x}", Sha256::digest(random));
        let operation_sha256 = op_digest(&preview.intent);
        let material = format!(
            "lu-debug-action-v1:{}:{}:{}:{}:{}",
            binding.job_id,
            binding.plan_sha256,
            preview.preview_sha256,
            operation_sha256,
            nonce_sha256
        );
        let action_id_sha256 = sha_text(&material);
        let challenge_sha256 = sha_text(&format!(
            "lu-operator-challenge-v1:{material}:{}",
            binding.owner_sequence,
        ));
        let deadline_ms = now_ms
            .checked_add(MAX_TTL_MS)
            .ok_or(ApprovalDenial::Expired)?;
        let path =
            trusted_journal_parent.join(format!("lu-debug-action-{}.ndjson", action_id_sha256));
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|_| ApprovalDenial::JournalUnavailable)?;
        let mut this = Self {
            binding,
            preview,
            path,
            file,
            action_id_sha256,
            challenge_sha256,
            nonce_sha256,
            deadline_ms,
            seq: 0,
            prior: "0".repeat(64),
            approved_sha: None,
            consent_ci_only: true,
            state: State::AwaitingConsent,
        };
        this.write_event("Prepared", "awaiting_fresh_native_operator", now_ms)?;
        Ok(this)
    }
    fn write_event(
        &mut self,
        phase: &str,
        outcome: &str,
        now_ms: u64,
    ) -> Result<String, ApprovalDenial> {
        let mut event = ActionEvent {
            version: 1,
            ordinal: self.seq + 1,
            action_id_sha256: self.action_id_sha256.clone(),
            preview_sha256: self.preview.preview_sha256.clone(),
            operation_sha256: op_digest(&self.preview.intent),
            job_id: self.binding.job_id.clone(),
            plan_sha256: self.binding.plan_sha256.clone(),
            owner_event_sha256: self.binding.owner_event_sha256.clone(),
            owner_sequence: self.binding.owner_sequence,
            phase: phase.into(),
            outcome_code: outcome.into(),
            observed_unix_ms: now_ms,
            previous_hash: self.prior.clone(),
            entry_hash: String::new(),
        };
        event.entry_hash = event_hash(&event)?;
        let mut bytes =
            serde_json::to_vec(&event).map_err(|_| ApprovalDenial::JournalUnavailable)?;
        bytes.push(b'\n');
        if self.seq >= MAX_EVENTS || bytes.len() > 4096 {
            return Err(ApprovalDenial::JournalUnavailable);
        }
        // Write then fsync *before* changing live state or consuming a
        // claim. On any failure the caller must quarantine the operation;
        // no DAP execution fallback is permitted.
        self.file
            .write_all(&bytes)
            .map_err(|_| ApprovalDenial::JournalUnavailable)?;
        self.file
            .sync_all()
            .map_err(|_| ApprovalDenial::JournalUnavailable)?;
        self.seq += 1;
        self.prior = event.entry_hash.clone();
        Ok(event.entry_hash)
    }
    pub(crate) fn audit_reference(
        &self,
        final_phase: &str,
    ) -> Result<DebugActionAuditRef, ApprovalDenial> {
        let verified = self.verify_reopened_audit()?;
        if verified != final_phase {
            return Err(ApprovalDenial::JournalCorrupt);
        }
        Ok(DebugActionAuditRef {
            action_id_sha256: self.action_id_sha256.clone(),
            preview_sha256: self.preview.preview_sha256.clone(),
            last_receipt_sha256: self.prior.clone(),
            final_phase: verified,
        })
    }
    pub(crate) fn challenge_sha256(&self) -> &str {
        &self.challenge_sha256
    }
    pub(crate) fn journal_path_for_audit(&self) -> &Path {
        &self.path
    }
    /// Only a fresh, operation-specific OS-native consent callback may
    /// eventually construct an approved native marker. CI fixture markers
    /// are *always* labelled test-only and never enable installed authority.
    pub(crate) fn decide(
        &mut self,
        marker: NativeConsentMarker,
        actual_sequence: u64,
        now_ms: u64,
    ) -> Result<(), ApprovalDenial> {
        if self.state != State::AwaitingConsent {
            return Err(ApprovalDenial::AlreadyDecided);
        }
        if marker.preview_sha256 != self.preview.preview_sha256
            || marker.challenge_sha256 != self.challenge_sha256
            || marker.native_owner_sequence != self.binding.owner_sequence
            || actual_sequence != self.binding.owner_sequence
        {
            self.state = State::Revoked;
            let _ = self.write_event("Revoked", "wrong_owner_or_review", now_ms);
            return Err(ApprovalDenial::WrongStop);
        }
        if now_ms > self.deadline_ms
            || marker.observed_unix_ms > now_ms
            || now_ms.saturating_sub(marker.observed_unix_ms) > MAX_TTL_MS
        {
            self.state = State::Revoked;
            let _ = self.write_event("Revoked", "expired", now_ms);
            return Err(ApprovalDenial::Expired);
        }
        if !marker.confirmed {
            self.write_event("Denied", "native_operator_declined", now_ms)?;
            self.state = State::Denied;
            return Err(ApprovalDenial::NativeConsentMissing);
        }
        let sha = self.write_event(
            "Approved",
            if marker.ci_fixture_only {
                "ci_only_not_installed_consent"
            } else {
                "native_human_confirmed"
            },
            now_ms,
        )?;
        self.approved_sha = Some(sha);
        self.consent_ci_only = marker.ci_fixture_only;
        self.state = State::Approved;
        Ok(())
    }
    /// Marks the exact one-use claim consumed *durably before* any future
    /// command can be dispatched. This source branch has no dispatch API.
    pub(crate) fn consume_for_receipt_test(
        &mut self,
        preview_sha256: &str,
        actual_sequence: u64,
        now_ms: u64,
    ) -> Result<ConsumedControlClaim, ApprovalDenial> {
        if self.state == State::Consumed {
            return Err(ApprovalDenial::GrantConsumed);
        }
        if self.state != State::Approved {
            return Err(ApprovalDenial::Cancelled);
        }
        if preview_sha256 != self.preview.preview_sha256
            || actual_sequence != self.binding.owner_sequence
        {
            self.state = State::Revoked;
            let _ = self.write_event("Revoked", "stale_owner_or_preview", now_ms);
            return Err(ApprovalDenial::WrongStop);
        }
        if now_ms > self.deadline_ms {
            self.state = State::Revoked;
            let _ = self.write_event("Revoked", "expired_before_consumption", now_ms);
            return Err(ApprovalDenial::Expired);
        }
        let consumed_sha =
            self.write_event("Consumed", "one_use_slot_consumed_before_dispatch", now_ms)?;
        self.state = State::Consumed;
        Ok(ConsumedControlClaim {
            action_id_sha256: self.action_id_sha256.clone(),
            approved_receipt_sha256: self
                .approved_sha
                .clone()
                .ok_or(ApprovalDenial::JournalCorrupt)?,
            consumed_receipt_sha256: consumed_sha,
            operation_sha256: op_digest(&self.preview.intent),
            owner_sequence: self.binding.owner_sequence,
            ci_fixture_only: self.consent_ci_only,
            execution_authorized: false,
            job_id: self.binding.job_id.clone(),
            plan_sha256: self.binding.plan_sha256.clone(),
            owner_event_sha256: self.binding.owner_event_sha256.clone(),
            source_snapshot_sha256: self.binding.source_snapshot_sha256.clone(),
            preview_sha256: self.preview.preview_sha256.clone(),
            stopped_rip: self.binding.stopped_rip,
            process_id: self.binding.process_id,
            thread_id: self.binding.thread_id,
            source_line: self.binding.source_line,
            relative_source: self.binding.relative_source.clone(),
            operation: self.preview.intent.clone(),
        })
    }
    /// An active approval is revoked on continuation/cancel/timeout/Job loss.
    /// A consumed claim cannot be reused even if a later result is unknown.
    pub(crate) fn revoke(&mut self, now_ms: u64) -> Result<(), ApprovalDenial> {
        if matches!(self.state, State::Revoked | State::Denied | State::Consumed) {
            return Err(ApprovalDenial::AlreadyDecided);
        }
        let sha = self.write_event("Revoked", "stop_or_operator_revoked", now_ms)?;
        self.prior = sha;
        self.state = State::Revoked;
        Ok(())
    }
    /// There is NO D6C independently verified debugger outcome reader yet.
    /// Never accept a caller's boolean or a DAP success message as proof.
    /// The only legal terminal for today's consumed/no-dispatch claim is
    /// ResultUncertain, which cannot restore or replay execution authority.
    /// CI-only verified result may be written ONLY with a private proof
    /// generated from fresh Job-owned kernel stop + independent PE/PDB and
    /// correlated DAP reply. There is no "success: bool" caller shortcut.
    #[cfg(any(test, all(windows, debug_assertions, feature = "dev046c-ci-control")))]
    pub(crate) fn record_independent_ci_step_result(
        &mut self,
        proof: &crate::debug_control_dispatch::VerifiedCiStepResult,
        now_ms: u64,
    ) -> Result<(), ApprovalDenial> {
        if self.state != State::Consumed
            || !proof.matches_consumption(&self.action_id_sha256, &self.prior)
        {
            return Err(ApprovalDenial::WrongStop);
        }
        let label = if self.consent_ci_only {
            "ci_independent_os_step"
        } else {
            "native_dialog_independent_os_step"
        };
        let outcome = format!("{label}:{}", proof.evidence_sha256());
        self.write_event("ResultVerified", &outcome, now_ms)?;
        self.state = State::Revoked;
        Ok(())
    }
    pub(crate) fn record_uncertain_result(&mut self, now_ms: u64) -> Result<(), ApprovalDenial> {
        if self.state != State::Consumed {
            return Err(ApprovalDenial::Cancelled);
        }
        self.write_event(
            "ResultUncertain",
            "requires_independent_result_reconciliation_no_replay",
            now_ms,
        )?;
        self.state = State::Revoked;
        Ok(())
    }
    /// Audit only: validate every chained hash, phase transition, and the
    /// exact original action ID. No grant is reconstructed after restart.
    pub(crate) fn verify_reopened_audit(&self) -> Result<String, ApprovalDenial> {
        let meta =
            fs::symlink_metadata(&self.path).map_err(|_| ApprovalDenial::JournalUnavailable)?;
        if !meta.is_file() || unsafe_link(&meta) || meta.len() > MAX_LOG_BYTES {
            return Err(ApprovalDenial::JournalCorrupt);
        }
        let input = File::open(&self.path).map_err(|_| ApprovalDenial::JournalUnavailable)?;
        let mut data = Vec::new();
        input
            .take(MAX_LOG_BYTES + 1)
            .read_to_end(&mut data)
            .map_err(|_| ApprovalDenial::JournalUnavailable)?;
        let (seq, last, phase) =
            verify_events(&data, &self.action_id_sha256, &self.preview.preview_sha256)?;
        if seq != self.seq || last != self.prior {
            return Err(ApprovalDenial::JournalCorrupt);
        }
        Ok(phase)
    }
    pub(crate) fn has_live_dispatch_authority(&self) -> bool {
        false
    }
}

/// Best-effort last-resort reconciliation on a failed native command.
/// If the OS process is killed before Rust Drop, the durable terminal phase
/// remains "Consumed" and is NEVER replayable after supervisor restart.
impl Drop for DebugActionApproval {
    fn drop(&mut self) {
        if self.state == State::Consumed {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(self.deadline_ms);
            let _ = self.record_uncertain_result(now);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn preview() -> DebugReviewPreview {
        DebugReviewPreview {
            preview_sha256: "a".repeat(64),
            intent: DebugReviewIntent::Step {
                kind: crate::debug_command_review::StepKind::Over,
                thread_id: 11,
            },
            owner_event_sha256: "b".repeat(64),
            source_snapshot_sha256: "c".repeat(64),
            stopped_rip: 0x140001000,
            owner_sequence: 7,
            executable_authority: false,
            installed_app_accepted: false,
        }
    }
    fn binding() -> NativeReviewBinding {
        NativeReviewBinding {
            job_id: format!("job-{}", "d".repeat(32)),
            plan_sha256: "e".repeat(64),
            build_receipt_sha256: "f".repeat(64),
            running_receipt_sha256: "1".repeat(64),
            owner_event_sha256: "b".repeat(64),
            source_snapshot_sha256: "c".repeat(64),
            owner_sequence: 7,
            process_id: 22,
            thread_id: 11,
            stopped_rip: 0x140001000,
            source_line: 7,
            relative_source: "src/main.rs".into(),
        }
    }
    fn book() -> (DebugActionApproval, PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("lu-debug-approval-{}", rand::random::<u128>()));
        fs::create_dir(&dir).unwrap();
        (
            DebugActionApproval::prepare_native(&dir, binding(), preview(), 500).unwrap(),
            dir,
        )
    }
    fn clean(book: DebugActionApproval, dir: PathBuf) {
        drop(book);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn fresh_approval_consumes_once_before_any_future_command() {
        let (mut a, dir) = book();
        let p = a.preview.preview_sha256.clone();
        let challenge = a.challenge_sha256().to_owned();
        a.decide(
            NativeConsentMarker::for_unit_test(&p, &challenge, 7, 505, true),
            7,
            505,
        )
        .unwrap();
        let claim = a.consume_for_receipt_test(&p, 7, 506).unwrap();
        assert!(!claim.execution_authorized);
        assert!(claim.ci_fixture_only);
        assert!(hex64(&claim.approved_receipt_sha256));
        assert!(hex64(&claim.consumed_receipt_sha256));
        assert_eq!(
            a.consume_for_receipt_test(&p, 7, 507),
            Err(ApprovalDenial::GrantConsumed)
        );
        a.record_uncertain_result(508).unwrap();
        assert_eq!(a.verify_reopened_audit().unwrap(), "ResultUncertain");
        let audit = a.audit_reference("ResultUncertain").unwrap();
        verify_action_receipt_after_restart(&dir, &audit).unwrap();
        assert!(!a.has_live_dispatch_authority());
        clean(a, dir);
    }
    #[test]
    fn separate_intent_requires_new_nonce_new_approval_and_fresh_receipt() {
        let (a, dir) = book();
        let first = a.action_id_sha256.clone();
        let mut different = preview();
        different.preview_sha256 = "9".repeat(64);
        different.intent = DebugReviewIntent::Breakpoint {
            source: "src/main.rs".into(),
            line: 7,
        };
        let second = DebugActionApproval::prepare_native(&dir, binding(), different, 500).unwrap();
        assert_ne!(first, second.action_id_sha256);
        assert_ne!(a.challenge_sha256(), second.challenge_sha256());
        assert_ne!(
            op_digest(&a.preview.intent),
            op_digest(&second.preview.intent)
        );
        drop(second);
        drop(a);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn wrong_owner_or_preview_revokes_and_never_grants() {
        let (mut a, dir) = book();
        let p = a.preview.preview_sha256.clone();
        let c = a.challenge_sha256().to_owned();
        assert_eq!(
            a.decide(
                NativeConsentMarker::for_unit_test(&p, &c, 8, 501, true),
                8,
                501
            ),
            Err(ApprovalDenial::WrongStop)
        );
        assert_eq!(
            a.consume_for_receipt_test(&p, 7, 502),
            Err(ApprovalDenial::Cancelled)
        );
        assert_eq!(a.verify_reopened_audit().unwrap(), "Revoked");
        clean(a, dir);
    }
    #[test]
    fn explicit_denial_and_expiration_never_consume() {
        let (mut a, dir) = book();
        let p = a.preview.preview_sha256.clone();
        let c = a.challenge_sha256().to_owned();
        assert_eq!(
            a.decide(
                NativeConsentMarker::for_unit_test(&p, &c, 7, 502, false),
                7,
                502
            ),
            Err(ApprovalDenial::NativeConsentMissing)
        );
        assert_eq!(a.verify_reopened_audit().unwrap(), "Denied");
        clean(a, dir);
        let (mut later, dir) = book();
        let p = later.preview.preview_sha256.clone();
        let c = later.challenge_sha256().to_owned();
        assert_eq!(
            later.decide(
                NativeConsentMarker::for_unit_test(&p, &c, 7, 501, true),
                7,
                31_000
            ),
            Err(ApprovalDenial::Expired)
        );
        clean(later, dir);
    }
    #[test]
    fn tampering_breaks_reopen_audit_and_cannot_restore_authority() {
        let (mut a, dir) = book();
        let p = a.preview.preview_sha256.clone();
        let c = a.challenge_sha256().to_owned();
        a.decide(
            NativeConsentMarker::for_unit_test(&p, &c, 7, 503, true),
            7,
            503,
        )
        .unwrap();
        assert_eq!(a.verify_reopened_audit().unwrap(), "Approved");
        let path = a.journal_path_for_audit().to_path_buf();
        let audit = a.audit_reference("Approved").unwrap();
        assert!(!a.has_live_dispatch_authority());
        drop(a);
        // Close the original file HANDLE before modifying, so this tests
        // portable Windows restart/tamper detection without file-sharing
        // behavior masquerading as an integrity failure.
        fs::write(&path, b"tampered\n").unwrap();
        assert_eq!(
            verify_action_receipt_after_restart(&dir, &audit),
            Err(ApprovalDenial::JournalCorrupt)
        );
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn old_challenge_cannot_approve_another_operation_or_session() {
        let (first, dir) = book();
        let mut alternate_preview = preview();
        alternate_preview.preview_sha256 = "9".repeat(64);
        alternate_preview.intent = DebugReviewIntent::Breakpoint {
            source: "src/main.rs".into(),
            line: 7,
        };
        let mut second =
            DebugActionApproval::prepare_native(&dir, binding(), alternate_preview, 500).unwrap();
        let old_challenge = first.challenge_sha256().to_owned();
        let second_preview_sha = second.preview.preview_sha256.clone();
        assert_eq!(
            second.decide(
                NativeConsentMarker::for_unit_test(
                    &second_preview_sha,
                    &old_challenge,
                    7,
                    501,
                    true,
                ),
                7,
                501,
            ),
            Err(ApprovalDenial::WrongStop)
        );
        assert_eq!(second.verify_reopened_audit().unwrap(), "Revoked");
        assert_eq!(
            second.consume_for_receipt_test(&second_preview_sha, 7, 502,),
            Err(ApprovalDenial::Cancelled)
        );
        drop(first);
        clean(second, dir);
    }
    #[test]
    fn a_stale_event_after_approval_cannot_consume_or_replay() {
        let (mut a, dir) = book();
        let preview = a.preview.preview_sha256.clone();
        let challenge = a.challenge_sha256().to_owned();
        a.decide(
            NativeConsentMarker::for_unit_test(&preview, &challenge, 7, 501, true),
            7,
            501,
        )
        .unwrap();
        assert_eq!(
            a.consume_for_receipt_test(&preview, 8, 502),
            Err(ApprovalDenial::WrongStop)
        );
        assert_eq!(a.verify_reopened_audit().unwrap(), "Revoked");
        assert_eq!(
            a.consume_for_receipt_test(&preview, 7, 503),
            Err(ApprovalDenial::Cancelled)
        );
        assert!(!a.has_live_dispatch_authority());
        clean(a, dir);
    }
    #[test]
    fn no_consent_or_wrong_challenge_never_creates_authority() {
        let (mut a, dir) = book();
        let p = a.preview.preview_sha256.clone();
        assert_eq!(
            a.consume_for_receipt_test(&p, 7, 501),
            Err(ApprovalDenial::Cancelled)
        );
        assert_eq!(
            a.decide(
                NativeConsentMarker::for_unit_test(&p, &"9".repeat(64), 7, 502, true),
                7,
                502,
            ),
            Err(ApprovalDenial::WrongStop)
        );
        assert_eq!(a.verify_reopened_audit().unwrap(), "Revoked");
        assert_eq!(
            a.consume_for_receipt_test(&p, 7, 503),
            Err(ApprovalDenial::Cancelled)
        );
        clean(a, dir);
    }

    #[test]
    fn d6d_restart_quarantines_prepared_and_approved_actions() {
        let (a, dir) = book();
        let first = a.action_id_sha256.clone();
        let mut other = preview();
        other.preview_sha256 = "9".repeat(64);
        let mut b = DebugActionApproval::prepare_native(&dir, binding(), other, 500).unwrap();
        let challenge = b.challenge_sha256().to_owned();
        b.decide(
            NativeConsentMarker::for_unit_test(&b.preview.preview_sha256, &challenge, 7, 501, true),
            7,
            501,
        )
        .unwrap();
        drop(a);
        drop(b);
        let inventory = inspect_action_receipts_after_restart(&dir).unwrap();
        assert_eq!(inventory.len(), 2);
        assert!(inventory.iter().any(|x| x.action_id_sha256 == first
            && x.phase == "Prepared"
            && x.requires_native_os_cleanup));
        assert!(inventory
            .iter()
            .any(|x| x.phase == "Approved" && x.requires_native_os_cleanup));
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn d6d_uncertain_terminal_is_audit_only() {
        let (mut a, dir) = book();
        let p = a.preview.preview_sha256.clone();
        let c = a.challenge_sha256().to_owned();
        a.decide(
            NativeConsentMarker::for_unit_test(&p, &c, 7, 501, true),
            7,
            501,
        )
        .unwrap();
        a.consume_for_receipt_test(&p, 7, 502).unwrap();
        a.record_uncertain_result(503).unwrap();
        drop(a);
        let inventory = inspect_action_receipts_after_restart(&dir).unwrap();
        assert_eq!(inventory.len(), 1);
        assert_eq!(inventory[0].phase, "ResultUncertain");
        assert!(!inventory[0].requires_native_os_cleanup);
        assert!(hex64(&inventory[0].last_receipt_sha256));
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn d6d_rehashed_job_substitution_still_fails() {
        let (mut a, dir) = book();
        let p = a.preview.preview_sha256.clone();
        let c = a.challenge_sha256().to_owned();
        a.decide(
            NativeConsentMarker::for_unit_test(&p, &c, 7, 501, true),
            7,
            501,
        )
        .unwrap();
        let path = a.journal_path_for_audit().to_path_buf();
        drop(a);
        let data = fs::read_to_string(&path).unwrap();
        let mut events: Vec<ActionEvent> = data
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        events[1].job_id = format!("job-{}", "0".repeat(32));
        events[1].entry_hash = event_hash(&events[1]).unwrap();
        let forged = events
            .iter()
            .map(|e| format!("{}\n", serde_json::to_string(e).unwrap()))
            .collect::<String>();
        fs::write(&path, forged).unwrap();
        assert!(matches!(
            inspect_action_receipts_after_restart(&dir),
            Err(ApprovalDenial::JournalCorrupt)
        ));
        fs::remove_dir_all(dir).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn d6d_symlinked_action_journal_denied() {
        use std::os::unix::fs::symlink;
        let (a, dir) = book();
        let source = a.journal_path_for_audit().to_path_buf();
        drop(a);
        let fake = dir.join(format!("lu-debug-action-{}.ndjson", "9".repeat(64)));
        symlink(&source, &fake).unwrap();
        assert!(matches!(
            inspect_action_receipts_after_restart(&dir),
            Err(ApprovalDenial::JournalCorrupt)
        ));
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn invalid_stop_binding_and_replayed_human_decision_denied() {
        let dir =
            std::env::temp_dir().join(format!("lu-debug-bad-binding-{}", rand::random::<u128>()));
        fs::create_dir(&dir).unwrap();
        let mut invalid = binding();
        invalid.source_snapshot_sha256 = "0".repeat(64);
        assert!(matches!(
            DebugActionApproval::prepare_native(&dir, invalid, preview(), 500,),
            Err(ApprovalDenial::WrongStop)
        ));
        let mut a = DebugActionApproval::prepare_native(&dir, binding(), preview(), 500).unwrap();
        let p = a.preview.preview_sha256.clone();
        let c = a.challenge_sha256().to_owned();
        a.decide(
            NativeConsentMarker::for_unit_test(&p, &c, 7, 505, true),
            7,
            505,
        )
        .unwrap();
        assert_eq!(
            a.decide(
                NativeConsentMarker::for_unit_test(&p, &c, 7, 506, true),
                7,
                506,
            ),
            Err(ApprovalDenial::AlreadyDecided)
        );
        assert!(!a.has_live_dispatch_authority());
        clean(a, dir);
    }
}
