//! IDE-CORE-IPC-01B-R2: installed Linux IDE startup admission, IDE side only.
//!
//! This is an optional, native-only authority bridge. It DOES NOT install or
//! implement the Lu Core-owned issuer, and does not expose an IPC capability or
//! a Tauri/WebView invocation. Missing or unverified OS installation = DENY.
//! A root-installed read-only profile pins the exact executable, issuer UID/GID,
//! package digest, Core owner and private journal. Neither preferences nor an
//! environment variable may select an alternative profile or endpoint.
//! The separately installed issuer MUST independently verify the caller PID,
//! installed executable and package identity, and durably burn each operation.
#![cfg(target_os = "linux")]

use crate::core_ipc_issuer::{InstalledIssuerAnchor, UnixCredentialIssuer};
use crate::core_ipc_lifecycle::{CoreHealthSession, HealthJournal};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, Metadata},
    io::Read,
    os::unix::fs::MetadataExt,
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

const INSTALLED_PROFILE: &str = "/etc/lu-ide/native-health-v1.json";
const PROFILE_MAX: u64 = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstalledAdmissionError {
    /// No OS-administered installed enrollment; absence never enables probing.
    NotInstalled,
    /// Untrusted install metadata or unexpected native process identity.
    UntrustedInstall,
    /// The private journal or fixed native issuer is unavailable.
    Unavailable,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InstalledProfile {
    version: u32,
    client_uid: u32,
    executable: PathBuf,
    executable_sha256: String,
    package_sha256: String,
    issuer_socket: PathBuf,
    issuer_uid: u32,
    issuer_group_gid: u32,
    core_uid: u32,
    journal_directory: PathBuf,
}

pub struct InstalledCoreHealthRuntime {
    session: CoreHealthSession,
    issuer: UnixCredentialIssuer,
    native_action_claimed: AtomicBool,
}

/// A native-host-only, non-cloneable one-shot invitation. Constructed only
/// after the installed OS profile has been admitted. It is deliberately not
/// serializable, does not carry credential bytes and has no WebView path.
#[must_use = "dropping the invitation denies the operation"]
pub struct NativeHealthPermit {
    runtime: Arc<InstalledCoreHealthRuntime>,
    completed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeHealthDisposition {
    Observed,
    Refused,
    Cancelled,
    Revoked,
    Uncertain,
    JournalUnavailable,
}
impl NativeHealthDisposition {
    /// Fixed presentation strings only: no credentials, endpoint names,
    /// operation identifiers or model-authorizing information leave the host.
    pub fn message(self) -> &'static str {
        match self {
            Self::Observed => {
                "Lu Core Health responded. This is read-only status, not an execution grant."
            }
            Self::Refused => "Lu Core Health was refused; no authorization was granted.",
            Self::Cancelled => "Lu Core Health request was cancelled without approval.",
            Self::Revoked => "Lu Core Health authorization was revoked.",
            Self::Uncertain => {
                "Lu Core Health outcome is uncertain. It will not be retried automatically."
            }
            Self::JournalUnavailable => "Protected Health journal unavailable. Request denied.",
        }
    }
}
impl NativeHealthPermit {
    /// Called strictly after a native host OS dialog result, never from JS or
    /// a model-visible Tauri command. Denial is journaled without consulting
    /// the issuer. A positive click authorizes only a single read-only probe.
    pub fn finish(mut self, human_confirmed: bool) -> NativeHealthDisposition {
        self.completed = true;
        if !human_confirmed {
            self.runtime.session.cancel();
        }
        let result = self.runtime.session.run_once(&self.runtime.issuer);
        match result {
            Ok(r) => match r.state {
                crate::core_ipc_lifecycle::Terminal::Observed => NativeHealthDisposition::Observed,
                crate::core_ipc_lifecycle::Terminal::Refused => NativeHealthDisposition::Refused,
                crate::core_ipc_lifecycle::Terminal::Cancelled => {
                    NativeHealthDisposition::Cancelled
                }
                crate::core_ipc_lifecycle::Terminal::Revoked => NativeHealthDisposition::Revoked,
                crate::core_ipc_lifecycle::Terminal::Uncertain => {
                    NativeHealthDisposition::Uncertain
                }
            },
            Err(_) => NativeHealthDisposition::JournalUnavailable,
        }
    }
}
impl Drop for NativeHealthPermit {
    fn drop(&mut self) {
        if !self.completed {
            // A crashed or abandoned dialog may never leave an enrollment.
            // The shared in-memory action stays consumed and future requests
            // require a new verified installed native startup.
            self.runtime.session.cancel();
        }
    }
}

fn native_uid() -> u32 {
    // rustix consults the OS identity, never an environment variable.
    rustix::process::getuid().as_raw()
}
fn canonical_absolute(path: &Path) -> Result<(), InstalledAdmissionError> {
    if !path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
        || fs::canonicalize(path).ok().as_deref() != Some(path)
    {
        return Err(InstalledAdmissionError::UntrustedInstall);
    }
    Ok(())
}
fn owned_root_directory(path: &Path) -> Result<(), InstalledAdmissionError> {
    canonical_absolute(path)?;
    let meta = fs::symlink_metadata(path).map_err(|_| InstalledAdmissionError::UntrustedInstall)?;
    if !meta.is_dir()
        || meta.uid() != 0
        || meta.mode() & 0o022 != 0
        || meta.file_type().is_symlink()
    {
        return Err(InstalledAdmissionError::UntrustedInstall);
    }
    Ok(())
}
fn secure_file(path: &Path, exact_mode: u32) -> Result<Metadata, InstalledAdmissionError> {
    canonical_absolute(path)?;
    let meta = fs::symlink_metadata(path).map_err(|_| InstalledAdmissionError::NotInstalled)?;
    if !meta.is_file()
        || meta.uid() != 0
        || meta.mode() & 0o777 != exact_mode
        || meta.file_type().is_symlink()
    {
        return Err(InstalledAdmissionError::UntrustedInstall);
    }
    Ok(meta)
}
fn decode_hash(s: &str) -> Result<[u8; 32], InstalledAdmissionError> {
    if s.len() != 64 {
        return Err(InstalledAdmissionError::UntrustedInstall);
    }
    let mut out = [0u8; 32];
    for i in 0..32 {
        let hi = s.as_bytes()[2 * i];
        let lo = s.as_bytes()[2 * i + 1];
        fn hex(n: u8) -> Option<u8> {
            match n {
                b'0'..=b'9' => Some(n - b'0'),
                b'a'..=b'f' => Some(n - b'a' + 10),
                _ => None,
            }
        }
        out[i] = (hex(hi).ok_or(InstalledAdmissionError::UntrustedInstall)? << 4)
            | hex(lo).ok_or(InstalledAdmissionError::UntrustedInstall)?;
    }
    if out.iter().all(|b| *b == 0) {
        return Err(InstalledAdmissionError::UntrustedInstall);
    }
    Ok(out)
}
fn executable_hash(path: &Path) -> Result<[u8; 32], InstalledAdmissionError> {
    let mut f = File::open(path).map_err(|_| InstalledAdmissionError::UntrustedInstall)?;
    let mut h = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = f
            .read(&mut buf)
            .map_err(|_| InstalledAdmissionError::UntrustedInstall)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().into())
}
impl InstalledCoreHealthRuntime {
    /// Called only by the native desktop startup, never a command. This does
    /// not borrow an enrollment or initiate a Core network/IPC exchange.
    pub fn from_installed_linux() -> Result<Self, InstalledAdmissionError> {
        Self::load_profile(Path::new(INSTALLED_PROFILE))
    }

    fn load_profile(profile_path: &Path) -> Result<Self, InstalledAdmissionError> {
        // A root-owned exact installer profile cannot be forged by a normal
        // IDE process. Require 0755/0700 parent without group write; reject
        // symlinks and inode swaps during the read.
        let parent = profile_path
            .parent()
            .ok_or(InstalledAdmissionError::UntrustedInstall)?;
        owned_root_directory(parent)?;
        let before = secure_file(profile_path, 0o644)?;
        if before.len() == 0 || before.len() > PROFILE_MAX {
            return Err(InstalledAdmissionError::UntrustedInstall);
        }
        let mut contents = Vec::new();
        File::open(profile_path)
            .map_err(|_| InstalledAdmissionError::NotInstalled)?
            .take(PROFILE_MAX + 1)
            .read_to_end(&mut contents)
            .map_err(|_| InstalledAdmissionError::UntrustedInstall)?;
        let after = secure_file(profile_path, 0o644)?;
        if contents.len() as u64 > PROFILE_MAX
            || before.dev() != after.dev()
            || before.ino() != after.ino()
            || before.len() != after.len()
        {
            return Err(InstalledAdmissionError::UntrustedInstall);
        }
        let pinned: InstalledProfile = serde_json::from_slice(&contents)
            .map_err(|_| InstalledAdmissionError::UntrustedInstall)?;
        if pinned.version != 1
            || pinned.client_uid == 0
            || pinned.client_uid != native_uid()
            || native_uid() != rustix::process::geteuid().as_raw()
            || pinned.issuer_uid == pinned.client_uid
        {
            // Same-user issuer can be impersonated by the IDE account. No
            // privileged installed admission claim without a distinct UID.
            return Err(InstalledAdmissionError::UntrustedInstall);
        }
        let actual_exe =
            std::env::current_exe().map_err(|_| InstalledAdmissionError::UntrustedInstall)?;
        canonical_absolute(&pinned.executable)?;
        if actual_exe != pinned.executable {
            return Err(InstalledAdmissionError::UntrustedInstall);
        }
        let exe_meta = secure_file(&actual_exe, 0o755)?;
        if exe_meta.len() == 0
            || executable_hash(&actual_exe)? != decode_hash(&pinned.executable_sha256)?
        {
            return Err(InstalledAdmissionError::UntrustedInstall);
        }
        // The package hash is a separate installer commitment; issuer checks
        // that value itself and must attest executable/publisher independently.
        let package = decode_hash(&pinned.package_sha256)?;
        if !pinned.issuer_socket.is_absolute() || matches!(pinned.issuer_group_gid, 0 | u32::MAX) {
            return Err(InstalledAdmissionError::UntrustedInstall);
        }
        let journal = HealthJournal::open(&pinned.journal_directory, pinned.client_uid)
            .map_err(|_| InstalledAdmissionError::Unavailable)?;
        // Always reconcile bounded durable state before admitting a new run.
        journal
            .reconcile()
            .map_err(|_| InstalledAdmissionError::Unavailable)?;
        let issuer = UnixCredentialIssuer::new(InstalledIssuerAnchor {
            issuer_socket: pinned.issuer_socket,
            issuer_uid: pinned.issuer_uid,
            issuer_group_gid: Some(pinned.issuer_group_gid),
            core_uid: pinned.core_uid,
            package_sha256: package,
        })
        .map_err(|_| InstalledAdmissionError::Unavailable)?;
        Ok(Self {
            session: CoreHealthSession::new(journal),
            issuer,
            native_action_claimed: AtomicBool::new(false),
        })
    }

    /// Reserve exactly one native UI authorization attempt per installed
    /// runtime lifetime, before showing any dialog. Duplicate clicks,
    /// concurrent callbacks and late prompts never borrow another lease.
    pub fn reserve_native_action(
        self: &Arc<Self>,
    ) -> Result<NativeHealthPermit, InstalledAdmissionError> {
        self.native_action_claimed
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| InstalledAdmissionError::Unavailable)?;
        Ok(NativeHealthPermit {
            runtime: Arc::clone(self),
            completed: false,
        })
    }

    /// Native host owns this cancellation handle; no model/WebView interface.
    pub fn cancel(&self) {
        self.session.cancel();
    }
    pub fn revoke(&self) {
        self.session.revoke();
    }

    // There is intentionally no public probe() or frontend invoke command.
    // Only a consumed NativeHealthPermit can reach session.run_once.
    // An accepted health response never confers broader Core capabilities.
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hash_parser_refuses_noncanonical_or_zero_material() {
        assert_eq!(
            decode_hash(&"0".repeat(64)),
            Err(InstalledAdmissionError::UntrustedInstall)
        );
        assert_eq!(
            decode_hash(&"F".repeat(64)),
            Err(InstalledAdmissionError::UntrustedInstall)
        );
        assert_eq!(
            decode_hash("0102"),
            Err(InstalledAdmissionError::UntrustedInstall)
        );
        assert_eq!(decode_hash(&"ab".repeat(32)), Ok([0xab; 32]));
    }
    fn denied_runtime_for_fixture() -> (Arc<InstalledCoreHealthRuntime>, PathBuf) {
        use crate::core_ipc_issuer::InstalledIssuerAnchor;
        use rand::{rngs::OsRng, RngCore};
        use std::os::unix::fs::PermissionsExt;
        let mut seed = [0u8; 12];
        OsRng.fill_bytes(&mut seed);
        let root = std::env::temp_dir().join(format!(
            "ide-01b-r3-{}",
            seed.iter().map(|v| format!("{v:02x}")).collect::<String>()
        ));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let journal = HealthJournal::open(&root, native_uid()).unwrap();
        let issuer = UnixCredentialIssuer::new(InstalledIssuerAnchor {
            issuer_socket: root.join("never-issue.sock"),
            issuer_uid: native_uid(),
            issuer_group_gid: None,
            core_uid: native_uid(),
            package_sha256: [0x91; 32],
        })
        .unwrap();
        (
            Arc::new(InstalledCoreHealthRuntime {
                session: CoreHealthSession::new(journal),
                issuer,
                native_action_claimed: AtomicBool::new(false),
            }),
            root,
        )
    }

    #[test]
    fn native_human_denial_is_durable_one_use_without_issuer_contact() {
        use crate::core_ipc_lifecycle::{JournalState, Terminal};
        let (runtime, root) = denied_runtime_for_fixture();
        let action = runtime.reserve_native_action().unwrap();
        assert!(runtime.reserve_native_action().is_err());
        assert_eq!(action.finish(false), NativeHealthDisposition::Cancelled);
        assert!(runtime.reserve_native_action().is_err());
        let journal = HealthJournal::open(&root, native_uid()).unwrap();
        let records = journal.reconcile().unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].1, JournalState::Final(Terminal::Cancelled));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn native_health_revoke_during_dialog_fails_closed() {
        use crate::core_ipc_lifecycle::{JournalState, Terminal};
        let (runtime, root) = denied_runtime_for_fixture();
        let action = runtime.reserve_native_action().unwrap();
        runtime.revoke();
        assert_eq!(action.finish(true), NativeHealthDisposition::Revoked);
        let journal = HealthJournal::open(&root, native_uid()).unwrap();
        assert_eq!(
            journal.reconcile().unwrap()[0].1,
            JournalState::Final(Terminal::Revoked)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn dropped_native_dialog_permit_prevents_future_credentials() {
        let (runtime, root) = denied_runtime_for_fixture();
        drop(runtime.reserve_native_action().unwrap());
        assert!(runtime.reserve_native_action().is_err());
        // Abandoning the dialog is not an issuer operation: no journal
        // reservation or credential is generated, even on restart.
        let journal = HealthJournal::open(&root, native_uid()).unwrap();
        assert!(journal.reconcile().unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn native_confirmed_no_issuer_is_refused_and_not_retried() {
        use crate::core_ipc_lifecycle::{JournalState, Terminal};
        let (runtime, root) = denied_runtime_for_fixture();
        let action = runtime.reserve_native_action().unwrap();
        assert_eq!(action.finish(true), NativeHealthDisposition::Refused);
        assert!(runtime.reserve_native_action().is_err());
        let journal = HealthJournal::open(&root, native_uid()).unwrap();
        assert_eq!(
            journal.reconcile().unwrap()[0].1,
            JournalState::Final(Terminal::Refused)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn no_installed_owner_profile_refuses_without_issuer_contact() {
        let absent = Path::new("/etc/lu-ide/never-installed-ide-core-01b-r2.json");
        assert!(InstalledCoreHealthRuntime::load_profile(absent).is_err());
    }
    // This test runs as a different real Linux UID after the parent installs
    // a root-owned profile, group-gated socket and a user-owned 0700 journal.
    // Environment passes only the TEST fixture location, never production
    // authority; production startup always uses INSTALLED_PROFILE.
    #[test]
    #[ignore = "restricted child fixture; invoked by explicit root-only OS test"]
    fn child_real_installed_profile_one_use_and_restart() {
        use crate::core_ipc_host::{HostCoreHealthError, ProtectedCoreHealthBroker};
        use crate::core_ipc_lifecycle::{JournalState, LifecycleError, Terminal};
        let Some(path) = std::env::var_os("IDE_CORE_01B_R2_CHILD_PROFILE") else {
            return;
        };
        let runtime = InstalledCoreHealthRuntime::load_profile(Path::new(&path)).unwrap();
        let result = runtime.session.run_once(&runtime.issuer).unwrap();
        assert_eq!(result.state, Terminal::Uncertain);
        assert_eq!(
            result.observation,
            Err(HostCoreHealthError::TransportUnavailable)
        );
        assert_eq!(
            runtime.session.run_once(&runtime.issuer),
            Err(LifecycleError::AlreadyConsumed)
        );
        let reloaded = InstalledCoreHealthRuntime::load_profile(Path::new(&path)).unwrap();
        let pinned: InstalledProfile = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let scanned = HealthJournal::open(&pinned.journal_directory, 65534)
            .unwrap()
            .reconcile()
            .unwrap();
        assert_eq!(
            scanned,
            vec![(
                result.operation_id.clone(),
                JournalState::Final(Terminal::Uncertain)
            )]
        );
        // A fresh adapter attempting to replay the same operation MUST be
        // rejected by the separately owned issuer's persisted burn journal.
        let mut calls = 0;
        assert_eq!(
            reloaded
                .issuer
                .with_enrollment(&result.operation_id, &mut |_, _, _, _| {
                    calls += 1;
                }),
            Err(HostCoreHealthError::BrokerUnavailable)
        );
        assert_eq!(calls, 0);
        // Cancellation and revocation are independently durable even across
        // new native session objects; neither opens another issuer socket.
        let cancelled = InstalledCoreHealthRuntime::load_profile(Path::new(&path)).unwrap();
        cancelled.cancel();
        let cancelled_state = cancelled.session.run_once(&cancelled.issuer).unwrap();
        assert_eq!(cancelled_state.state, Terminal::Cancelled);
        let revoked = InstalledCoreHealthRuntime::load_profile(Path::new(&path)).unwrap();
        revoked.revoke();
        let revoked_state = revoked.session.run_once(&revoked.issuer).unwrap();
        assert_eq!(revoked_state.state, Terminal::Revoked);
        let recovered = InstalledCoreHealthRuntime::load_profile(Path::new(&path)).unwrap();
        let _ = recovered; // Startup reconciles; it never auto-reissues.
        let journal = HealthJournal::open(&pinned.journal_directory, 65534).unwrap();
        let records = journal.reconcile().unwrap();
        assert_eq!(records.len(), 3);
        assert!(records.contains(&(
            result.operation_id,
            JournalState::Final(Terminal::Uncertain)
        )));
        assert!(records.contains(&(
            cancelled_state.operation_id,
            JournalState::Final(Terminal::Cancelled)
        )));
        assert!(records.contains(&(
            revoked_state.operation_id,
            JournalState::Final(Terminal::Revoked)
        )));
    }

    #[test]
    #[ignore = "explicit privileged Debian OS custody qualification; never automatic CI"]
    fn root_owned_real_uid_issuer_custody_and_durable_burn() {
        use crate::core_ipc_issuer::proto::{NativeHealthLeaseReply, NativeHealthLeaseRequest};
        use prost::Message;
        use rand::{rngs::OsRng, RngCore};
        use std::{
            io::{Read, Write},
            os::unix::{
                fs::{OpenOptionsExt, PermissionsExt},
                net::UnixListener,
            },
            process::Command,
            thread,
            time::{Duration, Instant, SystemTime, UNIX_EPOCH},
        };
        // Running as root is required to create a genuinely independent UID
        // boundary. On unprivileged CI this test only checks source contracts;
        // it is not positive installed acceptance on that worker.
        if native_uid() != 0 {
            eprintln!("CROSS_UID_NOT_RUN: requires privileged disposable Debian host");
            return;
        }
        assert!(Command::new("setpriv").arg("--help").output().is_ok());
        let mut nonce = [0u8; 8];
        OsRng.fill_bytes(&mut nonce);
        let root = PathBuf::from(format!(
            "/run/lu-ide-ipc-r2-{}",
            nonce.iter().map(|b| format!("{b:02x}")).collect::<String>()
        ));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(root.clone());
        let issuer_dir = root.join("issuer");
        let journal_dir = root.join("journal");
        let ledger_dir = root.join("issuer-burn");
        for dir in [&issuer_dir, &journal_dir, &ledger_dir] {
            fs::create_dir(dir).unwrap();
        }
        // Distinct OS accounts: privileged issuer UID0; IDE UID/GID 65534.
        let chown = |owner: &str, path: &Path| {
            assert!(Command::new("chown")
                .arg(owner)
                .arg(path)
                .status()
                .unwrap()
                .success());
        };
        chown("0:65534", &issuer_dir);
        fs::set_permissions(&issuer_dir, fs::Permissions::from_mode(0o750)).unwrap();
        chown("65534:65534", &journal_dir);
        fs::set_permissions(&journal_dir, fs::Permissions::from_mode(0o700)).unwrap();
        fs::set_permissions(&ledger_dir, fs::Permissions::from_mode(0o700)).unwrap();
        let socket = issuer_dir.join("lease.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        chown("0:65534", &socket);
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o660)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let server_ledger = ledger_dir.clone();
        let server = thread::spawn(move || {
            for attempt in 0..2 {
                let started = Instant::now();
                let (mut stream, _) = loop {
                    match listener.accept() {
                        Ok(v) => break v,
                        Err(e)
                            if e.kind() == std::io::ErrorKind::WouldBlock
                                && started.elapsed() < Duration::from_secs(8) =>
                        {
                            thread::sleep(Duration::from_millis(20));
                        }
                        Err(e) => panic!("issuer did not receive request {attempt}: {e}"),
                    }
                };
                let peer = rustix::net::sockopt::socket_peercred(&stream).unwrap();
                assert_eq!(peer.uid.as_raw(), 65534); // actual unprivileged child
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut head = [0u8; 4];
                stream.read_exact(&mut head).unwrap();
                let n = u32::from_be_bytes(head) as usize;
                assert!((1..=4096).contains(&n));
                let mut body = vec![0u8; n];
                stream.read_exact(&mut body).unwrap();
                let request = NativeHealthLeaseRequest::decode(body.as_slice()).unwrap();
                assert_eq!(request.schema_version, 1);
                assert_eq!(request.scope, "core.health.readonly");
                assert_eq!(request.installed_package_sha256, vec![0x74; 32]);
                assert_eq!(request.journal_operation_ref.len(), 32);
                // Privileged owner burns the journal ID DURABLY before making
                // even a synthetic lease visible. A second request is denied.
                let marker = server_ledger.join(format!("{}.used", request.journal_operation_ref));
                if attempt == 0 {
                    let mut f = fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .mode(0o600)
                        .open(&marker)
                        .unwrap();
                    f.write_all(b"burned\n").unwrap();
                    f.sync_all().unwrap();
                    File::open(&server_ledger).unwrap().sync_all().unwrap();
                    let expires = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs()
                        + 30;
                    let reply = NativeHealthLeaseReply {
                        schema_version: 1,
                        operation_nonce: request.operation_nonce,
                        installed_package_sha256: request.installed_package_sha256,
                        scope: request.scope,
                        core_socket_path: "/run/lu-core/not-installed.sock".into(),
                        core_owner_uid: 65534,
                        credential: vec![0x57; 32],
                        expires_unix_seconds: expires,
                        journal_operation_ref: request.journal_operation_ref,
                    }
                    .encode_to_vec();
                    stream
                        .write_all(&(reply.len() as u32).to_be_bytes())
                        .unwrap();
                    stream.write_all(&reply).unwrap();
                } else {
                    assert!(marker.is_file());
                    // Refuse replay without sending any second lease.
                }
            }
        });
        let exe = std::env::current_exe().unwrap();
        assert_eq!(fs::metadata(&exe).unwrap().uid(), 0);
        let exe_digest = executable_hash(&exe).unwrap();
        let tohex = |bytes: &[u8]| -> String { bytes.iter().map(|b| format!("{b:02x}")).collect() };
        let profile = root.join("profile.json");
        let config = serde_json::json!({
            "version":1,"client_uid":65534,"executable":exe,
            "executable_sha256":tohex(&exe_digest),"package_sha256":"74".repeat(32),
            "issuer_socket":socket,"issuer_uid":0,"issuer_group_gid":65534,
            "core_uid":65534,"journal_directory":journal_dir
        });
        fs::write(&profile, serde_json::to_vec(&config).unwrap()).unwrap();
        fs::set_permissions(&profile, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(fs::metadata(&profile).unwrap().uid(), 0);
        let test_name =
            "installed_core_health::tests::child_real_installed_profile_one_use_and_restart";
        let output = Command::new("setpriv")
            .args(["--reuid", "65534", "--regid", "65534", "--clear-groups"])
            .arg(std::env::current_exe().unwrap())
            .args(["--exact", test_name, "--ignored", "--nocapture"])
            .env("IDE_CORE_01B_R2_CHILD_PROFILE", &profile)
            .output()
            .unwrap();
        server.join().unwrap();
        assert!(
            output.status.success(),
            "cross-UID child failed: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(fs::read_dir(&ledger_dir).unwrap().count(), 1);
    }

    #[test]
    fn native_startup_does_not_accept_unowned_profile_paths() {
        // The checked loader may only accept an OS-root-owned installation;
        // arbitrary content under a nonexistent directory is never authority.
        assert!(InstalledCoreHealthRuntime::load_profile(Path::new(
            "/run/never-installed-ide-core-r2/profile.json"
        ))
        .is_err());
    }
}
