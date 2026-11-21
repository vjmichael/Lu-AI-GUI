//! IDE-CORE-IPC-01B: private native one-shot Core Health operation custody.
//!
//! The installed native credential owner, never a WebView, model or project,
//! supplies `ProtectedCoreHealthBroker` and a pre-created private journal root.
//! This host consumes exactly one broker enrollment and records durable
//! preflight/start/terminal markers. An interrupted operation is uncertain:
//! no automatic replay or reuse of its credential is allowed after restart.
//! Observing Core Health confers NO tool, execution or session capabilities.
//!
//! Neither a production OS credential issuer nor a Windows signed pipe
//! broker is installed by this module. Missing owner or unsafe journal = DENY.
use crate::core_ipc_host::{
    probe_core_health_guarded, CoreHealthObservation, HostCoreHealthError,
    ProtectedCoreHealthBroker,
};
use rand::{rngs::OsRng, RngCore};
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Terminal {
    Observed,
    Refused,
    Cancelled,
    Revoked,
    Uncertain,
}
impl Terminal {
    fn label(self) -> &'static str {
        match self {
            Self::Observed => "observed",
            Self::Refused => "refused",
            Self::Cancelled => "cancelled",
            Self::Revoked => "revoked",
            Self::Uncertain => "uncertain",
        }
    }
    fn from_label(v: &str) -> Option<Self> {
        match v {
            "observed" => Some(Self::Observed),
            "refused" => Some(Self::Refused),
            "cancelled" => Some(Self::Cancelled),
            "revoked" => Some(Self::Revoked),
            "uncertain" => Some(Self::Uncertain),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JournalState {
    UncertainAfterRestart,
    Final(Terminal),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LifecycleError {
    UnsafeJournal,
    JournalIo,
    AlreadyConsumed,
    Cancelled,
    Revoked,
}

/// A private OS-owner directory MUST already exist with exact mode 0700 and
/// pinned UID. No path from app settings, client frames or project catalog.
/// All entries are write-once 0600 files; fsync both record and directory.
/// Credentials, paths, model prompts and Core responses never enter the log.
pub(crate) struct HealthJournal {
    root: PathBuf,
    expected_uid: u32,
}
impl HealthJournal {
    pub(crate) fn open(root: &Path, expected_uid: u32) -> Result<Self, LifecycleError> {
        let journal = Self {
            root: root.to_owned(),
            expected_uid,
        };
        journal.verify_root()?;
        Ok(journal)
    }

    fn verify_root(&self) -> Result<(), LifecycleError> {
        if !self.root.is_absolute()
            || self
                .root
                .components()
                .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
        {
            return Err(LifecycleError::UnsafeJournal);
        }
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};
            let meta =
                fs::symlink_metadata(&self.root).map_err(|_| LifecycleError::UnsafeJournal)?;
            if !meta.is_dir()
                || meta.file_type().is_symlink()
                || meta.uid() != self.expected_uid
                || meta.permissions().mode() & 0o777 != 0o700
                || fs::canonicalize(&self.root).ok().as_deref() != Some(self.root.as_path())
            {
                return Err(LifecycleError::UnsafeJournal);
            }
            Ok(())
        }
        #[cfg(not(target_os = "linux"))]
        {
            // Windows requires an OS-backed protected directory/ACL attestor.
            Err(LifecycleError::UnsafeJournal)
        }
    }

    fn verify_marker(&self, path: &Path, max_bytes: u64) -> Result<Vec<u8>, LifecycleError> {
        self.verify_root()?;
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::MetadataExt;
            // Inspect a single opened inode rather than checking a pathname
            // and then following it with File::open(). A revoked/stale marker
            // cannot be replaced with a symlink or hardlink between checks.
            // O_NOFOLLOW blocks the final-component symlink even in a race.
            let f = rustix::fs::open(
                path,
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::CLOEXEC
                    | rustix::fs::OFlags::NOFOLLOW,
                rustix::fs::Mode::empty(),
            )
            .map_err(|_| LifecycleError::UnsafeJournal)?;
            let mut f = File::from(f);
            let meta = f.metadata().map_err(|_| LifecycleError::JournalIo)?;
            if !meta.is_file()
                || meta.uid() != self.expected_uid
                || meta.mode() & 0o777 != 0o600
                || meta.len() > max_bytes
                || meta.nlink() != 1
            {
                return Err(LifecycleError::UnsafeJournal);
            }
            // Refuse a pathname that was swapped between opening and the
            // verifying read, including a substitute hardlink or symlink.
            let current = fs::symlink_metadata(path).map_err(|_| LifecycleError::UnsafeJournal)?;
            if !current.file_type().is_file()
                || current.dev() != meta.dev()
                || current.ino() != meta.ino()
            {
                return Err(LifecycleError::UnsafeJournal);
            }
            let mut bytes = Vec::with_capacity(max_bytes as usize);
            f.take(max_bytes + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| LifecycleError::JournalIo)?;
            if bytes.len() as u64 > max_bytes {
                return Err(LifecycleError::UnsafeJournal);
            }
            Ok(bytes)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (path, max_bytes);
            Err(LifecycleError::UnsafeJournal)
        }
    }

    fn write_once(&self, path: &Path, data: &[u8]) -> Result<(), LifecycleError> {
        self.verify_root()?;
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let mut f = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(path)
                .map_err(|_| LifecycleError::JournalIo)?;
            f.write_all(data).map_err(|_| LifecycleError::JournalIo)?;
            f.sync_all().map_err(|_| LifecycleError::JournalIo)?;
            File::open(&self.root)
                .and_then(|dir| dir.sync_all())
                .map_err(|_| LifecycleError::JournalIo)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (path, data);
            Err(LifecycleError::UnsafeJournal)
        }
    }

    fn entry(&self, op: &str, kind: &str) -> Result<PathBuf, LifecycleError> {
        if op.len() != 32 || !op.bytes().all(|ch| ch.is_ascii_hexdigit()) {
            return Err(LifecycleError::UnsafeJournal);
        }
        Ok(self.root.join(format!("{op}.{kind}")))
    }

    /// Reserves an unpredictable operation reference durably BEFORE borrowing
    /// any protected key or contacting the broker; no previous op may resume.
    fn reserve(&self) -> Result<String, LifecycleError> {
        let mut nonce = [0_u8; 16];
        OsRng.fill_bytes(&mut nonce);
        let id: String = nonce.iter().map(|b| format!("{b:02x}")).collect();
        self.write_once(&self.entry(&id, "reserved")?, b"health/v1\n")?;
        Ok(id)
    }

    fn start(&self, op: &str) -> Result<(), LifecycleError> {
        if self.verify_marker(&self.entry(op, "reserved")?, 16)? != b"health/v1\n" {
            return Err(LifecycleError::UnsafeJournal);
        }
        self.write_once(&self.entry(op, "started")?, b"health/v1\n")
    }

    fn finish(&self, op: &str, state: Terminal) -> Result<(), LifecycleError> {
        if self.verify_marker(&self.entry(op, "reserved")?, 16)? != b"health/v1\n" {
            return Err(LifecycleError::UnsafeJournal);
        }
        // A single terminal receipt for an operation, never overwritten.
        self.write_once(
            &self.entry(op, "final")?,
            format!("health/v1:{}\n", state.label()).as_bytes(),
        )
    }

    /// Inspection is read-only. A missing or torn final marker is UNKNOWN,
    /// not permission to retry. A hostile marker/symlink fails closed.
    pub(crate) fn inspect(&self, op: &str) -> Result<JournalState, LifecycleError> {
        if self.verify_marker(&self.entry(op, "reserved")?, 16)? != b"health/v1\n" {
            return Err(LifecycleError::UnsafeJournal);
        }
        let started = self.entry(op, "started")?;
        // Only an actual missing entry may be interpreted as incomplete.
        // Path::exists() follows symlinks and treats a dangling hostile link
        // as absent. Reject it even on direct per-operation inspection.
        let started_exists = match fs::symlink_metadata(&started) {
            Ok(_) => true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            Err(_) => return Err(LifecycleError::JournalIo),
        };
        if started_exists && self.verify_marker(&started, 16)? != b"health/v1\n" {
            return Err(LifecycleError::UnsafeJournal);
        }
        let terminal = self.entry(op, "final")?;
        match fs::symlink_metadata(&terminal) {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(JournalState::UncertainAfterRestart);
            }
            Err(_) => return Err(LifecycleError::JournalIo),
        }
        let wire = self.verify_marker(&terminal, 64)?;
        let value = std::str::from_utf8(&wire).map_err(|_| LifecycleError::UnsafeJournal)?;
        let label = value
            .strip_prefix("health/v1:")
            .and_then(|s| s.strip_suffix('\n'))
            .ok_or(LifecycleError::UnsafeJournal)?;
        let final_state = Terminal::from_label(label).ok_or(LifecycleError::UnsafeJournal)?;
        if !started_exists
            && matches!(
                final_state,
                Terminal::Observed | Terminal::Refused | Terminal::Uncertain
            )
        {
            return Err(LifecycleError::UnsafeJournal);
        }
        Ok(JournalState::Final(final_state))
    }

    /// Restart inspection enumerates all durable operations, including those
    /// whose ID was lost with the crashed process. No operation is retried.
    /// Unexpected filenames, symlinks, marker contents or excessive entries
    /// fail the entire scan closed instead of silently accepting a subset.
    pub(crate) fn reconcile(&self) -> Result<Vec<(String, JournalState)>, LifecycleError> {
        self.verify_root()?;
        const MAX_MARKERS: usize = 3072;
        let mut ops = BTreeSet::new();
        let mut records = 0usize;
        for entry in fs::read_dir(&self.root).map_err(|_| LifecycleError::JournalIo)? {
            let entry = entry.map_err(|_| LifecycleError::JournalIo)?;
            records += 1;
            if records > MAX_MARKERS {
                return Err(LifecycleError::UnsafeJournal);
            }
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| LifecycleError::UnsafeJournal)?;
            let (id, kind) = name.rsplit_once('.').ok_or(LifecycleError::UnsafeJournal)?;
            if id.len() != 32
                || !id
                    .bytes()
                    .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
                || !matches!(kind, "reserved" | "started" | "final")
            {
                return Err(LifecycleError::UnsafeJournal);
            }
            let content =
                self.verify_marker(&entry.path(), if kind == "final" { 64 } else { 16 })?;
            if kind != "final" && content != b"health/v1\n" {
                return Err(LifecycleError::UnsafeJournal);
            }
            ops.insert(id.to_owned());
        }
        if ops.len() > MAX_MARKERS / 2 {
            return Err(LifecycleError::UnsafeJournal);
        }
        ops.into_iter()
            .map(|id| {
                let state = self.inspect(&id)?;
                Ok((id, state))
            })
            .collect()
    }
}

/// One native operation per session object, with no auto retry. The installed
/// owner may cancel or revoke the handle; a late Core reply is then refused.
/// Neither a Health success nor an installed credential enters a model route.
pub(crate) struct CoreHealthSession {
    journal: HealthJournal,
    used: AtomicBool,
    cancelled: AtomicBool,
    revoked: AtomicBool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CoreHealthOutcome {
    pub operation_id: String,
    pub state: Terminal,
    pub observation: Result<CoreHealthObservation, HostCoreHealthError>,
}
impl CoreHealthSession {
    pub(crate) fn new(journal: HealthJournal) -> Self {
        Self {
            journal,
            used: AtomicBool::new(false),
            cancelled: AtomicBool::new(false),
            revoked: AtomicBool::new(false),
        }
    }
    pub(crate) fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
    pub(crate) fn revoke(&self) {
        self.revoked.store(true, Ordering::Release);
    }
    fn active(&self) -> bool {
        !self.cancelled.load(Ordering::Acquire) && !self.revoked.load(Ordering::Acquire)
    }
    pub(crate) fn run_once(
        &self,
        broker: &dyn ProtectedCoreHealthBroker,
    ) -> Result<CoreHealthOutcome, LifecycleError> {
        if self.used.swap(true, Ordering::AcqRel) {
            return Err(LifecycleError::AlreadyConsumed);
        }
        let op = self.journal.reserve()?;
        let observation = if self.revoked.load(Ordering::Acquire) {
            Err(HostCoreHealthError::CancelledOrRevoked)
        } else if self.cancelled.load(Ordering::Acquire) {
            Err(HostCoreHealthError::CancelledOrRevoked)
        } else {
            // Durable start before the first possible broker callback/Hello.
            self.journal.start(&op)?;
            probe_core_health_guarded(broker, &op, &|| self.active())
        };
        // Never publish a late positive observation across cancellation.
        let (state, observation) = if self.revoked.load(Ordering::Acquire) {
            (
                Terminal::Revoked,
                Err(HostCoreHealthError::CancelledOrRevoked),
            )
        } else if self.cancelled.load(Ordering::Acquire) {
            (
                Terminal::Cancelled,
                Err(HostCoreHealthError::CancelledOrRevoked),
            )
        } else {
            match observation {
                // A successfully authenticated Core response may still report
                // disconnected/unhealthy state. Never turn that response into
                // a positive native menu result or a durable observed-success
                // marker. The diagnostic remains read-only and grants nothing.
                Ok(report) => (connected_health_state(&report), Ok(report)),
                Err(e @ HostCoreHealthError::TransportUnavailable) => (Terminal::Uncertain, Err(e)),
                Err(e @ HostCoreHealthError::CancelledOrRevoked) => (Terminal::Uncertain, Err(e)),
                Err(e) => (Terminal::Refused, Err(e)),
            }
        };
        self.journal.finish(&op, state)?;
        Ok(CoreHealthOutcome {
            operation_id: op,
            state,
            observation,
        })
    }
}

/// A valid authenticated receipt with connected=false is diagnostic evidence
/// of a refused/unavailable Core, not a positive Health observation. The Core
/// epoch, status and HMAC are still independently checked by the codec.
fn connected_health_state(report: &CoreHealthObservation) -> Terminal {
    if report.connected {
        Terminal::Observed
    } else {
        Terminal::Refused
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_health_disconnected_status_never_becomes_observed_success() {
        let report = CoreHealthObservation {
            connected: false,
            status_code: "disconnected".into(),
            epoch: 41,
        };
        assert_eq!(connected_health_state(&report), Terminal::Refused);
        assert_eq!(
            connected_health_state(&CoreHealthObservation {
                connected: true,
                status_code: "ready".into(),
                epoch: 42,
            }),
            Terminal::Observed
        );
    }

    #[cfg(target_os = "linux")]
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::path::Path;

    #[cfg(target_os = "linux")]
    fn fixture() -> (PathBuf, HealthJournal) {
        let mut token = [0u8; 16];
        OsRng.fill_bytes(&mut token);
        let name: String = token.iter().map(|b| format!("{b:02x}")).collect();
        let root = std::env::temp_dir().join(format!("ide-core-health-01b-{name}"));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let uid = fs::symlink_metadata(&root).unwrap().uid();
        let journal = HealthJournal::open(&root, uid).unwrap();
        (root, journal)
    }
    struct CountingBroker<'a>(&'a std::sync::atomic::AtomicUsize);
    impl ProtectedCoreHealthBroker for CountingBroker<'_> {
        fn with_enrollment(
            &self,
            _operation_ref: &str,
            _use_once: &mut dyn FnMut(&Path, u32, &[u8; 32], &[u8; 32]),
        ) -> Result<(), HostCoreHealthError> {
            self.0.fetch_add(1, Ordering::AcqRel);
            Err(HostCoreHealthError::BrokerUnavailable)
        }
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn native_one_use_denial_is_durable_and_never_replayed() {
        let (root, j) = fixture();
        let count = std::sync::atomic::AtomicUsize::new(0);
        let session = CoreHealthSession::new(j);
        let outcome = session.run_once(&CountingBroker(&count)).unwrap();
        assert_eq!(outcome.state, Terminal::Refused);
        assert_eq!(
            outcome.observation,
            Err(HostCoreHealthError::BrokerUnavailable)
        );
        assert_eq!(count.load(Ordering::Acquire), 1);
        assert_eq!(
            session.run_once(&CountingBroker(&count)),
            Err(LifecycleError::AlreadyConsumed)
        );
        assert_eq!(count.load(Ordering::Acquire), 1);
        let reopened = HealthJournal::open(&root, fs::metadata(&root).unwrap().uid()).unwrap();
        assert_eq!(
            reopened.inspect(&outcome.operation_id),
            Ok(JournalState::Final(Terminal::Refused))
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn cancellation_and_revocation_refuse_before_broker_borrow() {
        for revoke in [false, true] {
            let (root, j) = fixture();
            let count = std::sync::atomic::AtomicUsize::new(0);
            let session = CoreHealthSession::new(j);
            if revoke {
                session.revoke();
            } else {
                session.cancel();
            }
            let outcome = session.run_once(&CountingBroker(&count)).unwrap();
            assert_eq!(
                outcome.state,
                if revoke {
                    Terminal::Revoked
                } else {
                    Terminal::Cancelled
                }
            );
            assert_eq!(count.load(Ordering::Acquire), 0);
            assert_eq!(
                session.journal.inspect(&outcome.operation_id),
                Ok(JournalState::Final(outcome.state))
            );
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn dangling_terminal_symlink_is_tampering_not_a_missing_outcome() {
        let (root, journal) = fixture();
        let id = journal.reserve().unwrap();
        journal.start(&id).unwrap();
        let final_path = journal.entry(&id, "final").unwrap();
        std::os::unix::fs::symlink(root.join("absent-final-target"), &final_path).unwrap();
        assert_eq!(journal.inspect(&id), Err(LifecycleError::UnsafeJournal));
        assert_eq!(journal.reconcile(), Err(LifecycleError::UnsafeJournal));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn hardlinked_terminal_marker_is_not_a_unique_durable_receipt() {
        let (root, journal) = fixture();
        let id = journal.reserve().unwrap();
        journal.start(&id).unwrap();
        journal.finish(&id, Terminal::Refused).unwrap();
        let final_path = journal.entry(&id, "final").unwrap();
        let other = root.join("duplicate-receipt-link");
        fs::hard_link(&final_path, &other).unwrap();
        assert_eq!(journal.inspect(&id), Err(LifecycleError::UnsafeJournal));
        assert_eq!(journal.reconcile(), Err(LifecycleError::UnsafeJournal));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn normal_reserved_started_and_final_receipts_still_reconcile() {
        let (root, journal) = fixture();
        let id = journal.reserve().unwrap();
        assert_eq!(
            journal.inspect(&id),
            Ok(JournalState::UncertainAfterRestart)
        );
        journal.start(&id).unwrap();
        assert_eq!(
            journal.inspect(&id),
            Ok(JournalState::UncertainAfterRestart)
        );
        journal.finish(&id, Terminal::Refused).unwrap();
        assert_eq!(
            journal.inspect(&id),
            Ok(JournalState::Final(Terminal::Refused))
        );
        assert_eq!(journal.reconcile().unwrap().len(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn crash_after_reservation_or_start_is_uncertain_and_never_retried() {
        let (root, j) = fixture();
        let id = j.reserve().unwrap();
        assert_eq!(j.inspect(&id), Ok(JournalState::UncertainAfterRestart));
        j.start(&id).unwrap();
        drop(j);
        let journal = HealthJournal::open(&root, fs::metadata(&root).unwrap().uid()).unwrap();
        assert_eq!(
            journal.inspect(&id),
            Ok(JournalState::UncertainAfterRestart)
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn hostile_journal_mode_wrong_uid_and_symlink_fail_closed() {
        let (root, j) = fixture();
        assert_eq!(
            HealthJournal::open(&root, fs::metadata(&root).unwrap().uid().wrapping_add(1)).err(),
            Some(LifecycleError::UnsafeJournal)
        );
        fs::set_permissions(&root, fs::Permissions::from_mode(0o777)).unwrap();
        assert_eq!(j.reserve(), Err(LifecycleError::UnsafeJournal));
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let alias = root.with_extension("symlink");
        std::os::unix::fs::symlink(&root, &alias).unwrap();
        assert_eq!(
            HealthJournal::open(&alias, fs::metadata(&root).unwrap().uid()).err(),
            Some(LifecycleError::UnsafeJournal)
        );
        fs::remove_file(alias).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn restart_enumeration_exposes_abandoned_operation_and_refuses_unknown_markers() {
        let (root, j) = fixture();
        let unfinished = j.reserve().unwrap();
        j.start(&unfinished).unwrap();
        let cancelled = j.reserve().unwrap();
        j.finish(&cancelled, Terminal::Cancelled).unwrap();
        let reopened = HealthJournal::open(&root, fs::metadata(&root).unwrap().uid()).unwrap();
        let states = reopened.reconcile().unwrap();
        assert!(states.contains(&(unfinished, JournalState::UncertainAfterRestart)));
        assert!(states.contains(&(cancelled, JournalState::Final(Terminal::Cancelled))));
        j.write_once(&root.join("unexpected-entry"), b"health/v1\n")
            .unwrap();
        assert_eq!(reopened.reconcile(), Err(LifecycleError::UnsafeJournal));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn malformed_terminal_refuses_to_claim_success() {
        let (root, j) = fixture();
        let id = j.reserve().unwrap();
        j.write_once(&j.entry(&id, "final").unwrap(), b"health/v1:unknown\n")
            .unwrap();
        assert_eq!(j.inspect(&id), Err(LifecycleError::UnsafeJournal));
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn cancellation_during_issuer_wait_rejects_late_credential_without_core_connect() {
        use std::{sync::Arc, thread, time::Duration};
        struct SlowIssuer<'a>(&'a std::sync::atomic::AtomicUsize);
        impl ProtectedCoreHealthBroker for SlowIssuer<'_> {
            fn with_enrollment(
                &self,
                _op: &str,
                callback: &mut dyn FnMut(&Path, u32, &[u8; 32], &[u8; 32]),
            ) -> Result<(), HostCoreHealthError> {
                thread::sleep(Duration::from_millis(100));
                self.0.fetch_add(1, Ordering::AcqRel);
                callback(Path::new("/no/core/permission"), 0, &[3; 32], &[4; 32]);
                Ok(())
            }
        }
        let (root, journal) = fixture();
        let session = Arc::new(CoreHealthSession::new(journal));
        let call_count = std::sync::atomic::AtomicUsize::new(0);
        let observer = Arc::clone(&session);
        thread::scope(|scope| {
            scope.spawn(|| {
                let outcome = observer.run_once(&SlowIssuer(&call_count)).unwrap();
                assert_eq!(outcome.state, Terminal::Cancelled);
                assert_eq!(
                    outcome.observation,
                    Err(HostCoreHealthError::CancelledOrRevoked)
                );
                assert_eq!(
                    observer.journal.inspect(&outcome.operation_id),
                    Ok(JournalState::Final(Terminal::Cancelled))
                );
            });
            thread::sleep(Duration::from_millis(20));
            session.cancel();
        });
        assert_eq!(call_count.load(Ordering::Acquire), 1);
        fs::remove_dir_all(root).unwrap();
    }
}
