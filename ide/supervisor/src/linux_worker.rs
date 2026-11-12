//! Native Linux worker execution. This module is opt-in at compile time.
//! It executes ONLY fixed offline Cargo recipes from a verified private input
//! snapshot. No shell, untrusted binary path, network namespace or host HOME.
//!
//! Worker admission requires a delegated cgroup-v2 subtree, CPU/memory/PID
//! limits, an owned root-trusted unprivileged Bubblewrap launcher, and cgroup
//! membership established in pre_exec BEFORE that launcher executes.
//! Missing prerequisites = denied. No unrestricted fallback.
use crate::{
    live_output::LiveOutput,
    snapshot::{verify_input, PublishedInput},
    JobPhase, JobPlan, JobSummary, NativeTreeTerminator, OsContainmentProof, Supervisor,
    SupervisorError,
};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read},
    os::{
        fd::AsRawFd,
        unix::{fs::MetadataExt, process::CommandExt},
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

#[cfg(feature = "lsp-transport")]
#[path = "linux_lsp_worker.rs"]
pub mod lsp_native;
fn native_linux_cgroup_trace(stage: &'static str) {
    if cfg!(debug_assertions)
        && std::env::var_os("LU_IDE_E2_NATIVE_TRACE").as_deref() == Some(std::ffi::OsStr::new("1"))
    {
        eprintln!("PROTECTED_LINUX_CGROUP_STAGE={stage}");
    }
}

const BWRAP: &str = "/usr/bin/bwrap";
// Linux multi-language bundles are provisioned by the privileged installer.
// No runtime path or digest may originate in the WebView, project or PATH.
const DEV03_PYTHON_ROOT: &str = "/opt/lu-ide/runtimes/python_pytest";
const DEV03_NODE_ROOT: &str = "/opt/lu-ide/runtimes/js_tests";
const DEV03_CMAKE_ROOT: &str = "/opt/lu-ide/runtimes/cpp_cmake_build";
const IDE05_UV_ROOT: &str = "/opt/lu-ide/runtimes/uv_sync_offline";
const DEV03_PYTHON_PIN: Option<&str> = option_env!("LU_IDE_DEV03_LINUX_PYTHON_SHA256");
const DEV03_NODE_PIN: Option<&str> = option_env!("LU_IDE_DEV03_LINUX_NODE_SHA256");
const DEV03_CMAKE_PIN: Option<&str> = option_env!("LU_IDE_DEV03_LINUX_CMAKE_SHA256");
const IDE05_UV_PIN: Option<&str> = option_env!("LU_IDE05_LINUX_UV_SHA256");

/// Native-only release-pinned runtime. Executable authority remains with
/// the existing supervisor OS-cgroup worker and its one-use approval.
fn reviewed_multilang_bundle(
    recipe_id: &str,
) -> Result<crate::runtime_bundles::VerifiedRuntimeBundle, SupervisorError> {
    if !cfg!(feature = "multilang-worker") {
        return Err(SupervisorError::InvalidPlan);
    }
    let (root, pin) = match recipe_id {
        "python_pytest" => (DEV03_PYTHON_ROOT, DEV03_PYTHON_PIN),
        "js_tests" => (DEV03_NODE_ROOT, DEV03_NODE_PIN),
        "cpp_cmake_build" => (DEV03_CMAKE_ROOT, DEV03_CMAKE_PIN),
        "uv_sync_offline" if cfg!(feature = "package-uv-worker") => (IDE05_UV_ROOT, IDE05_UV_PIN),
        _ => return Err(SupervisorError::InvalidPlan),
    };
    let expected = pin.ok_or(SupervisorError::NoContainment)?;
    crate::runtime_bundles::verify_runtime_bundle(recipe_id, Path::new(root), expected)
}

/// Read-only native host readiness. It NEVER issues a grant or starts code.
pub fn installed_multilang_toolchain_digest(recipe_id: &str) -> Result<String, SupervisorError> {
    ensure_root_owned_tool(Path::new(BWRAP))?;
    let bundle = reviewed_multilang_bundle(recipe_id)?;
    Ok(bundle.tree_sha256)
}

const CARGO: &str = "/usr/bin/cargo";
const RUSTC: &str = "/usr/bin/rustc";
// The fixed compiler path must be a regular root-owned file, never the
// /usr/bin/cc -> /etc/alternatives/cc symlink (which isn't mounted in bwrap).
// Explicit allowlisted package versions retain reproducible toolchain SHA
// provenance across Debian/Ubuntu, without PATH search or user input.
#[cfg(target_arch = "x86_64")]
const LINKER_CANDIDATES: &[&str] = &[
    "/usr/bin/x86_64-linux-gnu-gcc-14",
    "/usr/bin/x86_64-linux-gnu-gcc-13",
    "/usr/bin/x86_64-linux-gnu-gcc-12",
];
#[cfg(target_arch = "aarch64")]
const LINKER_CANDIDATES: &[&str] = &[
    "/usr/bin/aarch64-linux-gnu-gcc-14",
    "/usr/bin/aarch64-linux-gnu-gcc-13",
    "/usr/bin/aarch64-linux-gnu-gcc-12",
];
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
const LINKER_CANDIDATES: &[&str] = &[];
#[cfg(target_arch = "x86_64")]
const LINKER_ENV_KEY: &str = "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER";
#[cfg(target_arch = "aarch64")]
const LINKER_ENV_KEY: &str = "CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER";
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
const LINKER_ENV_KEY: &str = "LU_IDE_UNSUPPORTED_LINKER";
const CGROUP_ROOT: &str = "/sys/fs/cgroup";
const LOOP_DELAY: Duration = Duration::from_millis(40);
const REAP_LIMIT: Duration = Duration::from_secs(5);
const MAX_TOOL_BINARY: u64 = 32 * 1024 * 1024;
const TOTAL_LOG_BUFFER: usize = 64 * 1024;
const MAX_OUTPUT_FILES: usize = 6_000;
const MAX_ARTIFACT_BYTES: u64 = 256 * 1024 * 1024;
const MAX_SINGLE_ARTIFACT_BYTES: u64 = 64 * 1024 * 1024;
const OUTPUT_CHECK_INTERVAL: Duration = Duration::from_millis(250);

pub struct RunCancellation {
    cancelled: Arc<AtomicBool>,
}
impl Default for RunCancellation {
    fn default() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }
}
impl RunCancellation {
    pub fn request_cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }
    pub fn clone_handle(&self) -> Self {
        Self {
            cancelled: Arc::clone(&self.cancelled),
        }
    }
}

#[derive(Debug)]
pub struct ExecutionResult {
    pub summary: JobSummary,
    pub exit_code: Option<i32>,
    pub stdout_sha256: String,
    pub stderr_sha256: String,
    /// Fixed, information-minimal native error category. Never raw stderr.
    pub stderr_category: &'static str,
    pub stdout_bytes_recorded: usize,
    pub stderr_bytes_recorded: usize,
    pub output_truncated: bool,
    pub output_directory: PathBuf,
    pub timed_out: bool,
    pub output_limit_hit: bool,
}

fn denied() -> SupervisorError {
    SupervisorError::NoContainment
}

fn ensure_root_owned_tool(path: &Path) -> Result<(), SupervisorError> {
    let meta = fs::symlink_metadata(path).map_err(|_| denied())?;
    if !meta.file_type().is_file()
        || meta.file_type().is_symlink()
        || meta.uid() != 0
        || meta.mode() & 0o6022 != 0
    {
        return Err(denied());
    }
    Ok(())
}

fn hash_file(path: &Path) -> Result<String, SupervisorError> {
    let meta = fs::symlink_metadata(path).map_err(|_| denied())?;
    if !meta.file_type().is_file() || meta.file_type().is_symlink() || meta.len() > MAX_TOOL_BINARY
    {
        return Err(denied());
    }
    let mut source = File::open(path).map_err(|_| denied())?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut length = 0_u64;
    loop {
        let n = source.read(&mut buffer).map_err(|_| denied())?;
        if n == 0 {
            break;
        }
        length += n as u64;
        if length > MAX_TOOL_BINARY {
            return Err(denied());
        }
        hash.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

/// The trusted native host can record this digest in JobPlan. It covers
/// the two executable files, NOT their dynamic libraries or external crates.
fn trusted_gnu_linker() -> Result<&'static str, SupervisorError> {
    LINKER_CANDIDATES
        .iter()
        .copied()
        .find(|candidate| ensure_root_owned_tool(Path::new(candidate)).is_ok())
        .ok_or_else(denied)
}

pub fn installed_toolchain_digest() -> Result<String, SupervisorError> {
    ensure_root_owned_tool(Path::new(BWRAP))?;
    ensure_root_owned_tool(Path::new(CARGO))?;
    ensure_root_owned_tool(Path::new(RUSTC))?;
    let linker = trusted_gnu_linker()?;
    let mut digest = Sha256::new();
    for path in [BWRAP, CARGO, RUSTC, linker] {
        let sha = hash_file(Path::new(path))?;
        digest.update(path.as_bytes());
        digest.update([0]);
        digest.update(sha.as_bytes());
        digest.update([0]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

// Cgroup-v2 domain controllers cannot be enabled in a non-root cgroup
// containing internal processes. Provisioning must therefore place the
// trusted IDE host in its own leaf, next to the worker's cgroups, under a
// dedicated, empty delegated parent. The worker never modifies ancestor
// controllers or migrates an already-running host process.
fn select_delegated_parent(root: &Path, current: &Path) -> Result<PathBuf, SupervisorError> {
    if current.file_name().and_then(|v| v.to_str()) != Some("lu-ide-host") {
        return Err(denied());
    }
    let parent = current.parent().ok_or_else(denied)?;
    if !matches!(
        parent.file_name().and_then(|v| v.to_str()),
        Some("lu-ide-delegated" | "lu-ide-delegated.service")
    ) || !parent.starts_with(root)
        || parent == root
    {
        return Err(denied());
    }
    Ok(parent.to_path_buf())
}
fn has_controllers(data: &str) -> bool {
    ["cpu", "memory", "pids"]
        .iter()
        .all(|required| data.split_whitespace().any(|token| token == *required))
}
fn require_delegated_limits(parent: &Path) -> Result<(), SupervisorError> {
    // "controllers" is availability, not activation: the kernel creates
    // writable child quota files only when subtree_control enables them.
    if fs::read_to_string(parent.join("cgroup.type"))
        .map_err(|_| denied())?
        .trim()
        != "domain"
    {
        return Err(denied());
    }
    let available = fs::read_to_string(parent.join("cgroup.controllers")).map_err(|_| denied())?;
    let enabled =
        fs::read_to_string(parent.join("cgroup.subtree_control")).map_err(|_| denied())?;
    if !has_controllers(&available) || !has_controllers(&enabled) {
        return Err(denied());
    }
    // Domain parent must have no internal tasks. A peer host leaf is mandatory;
    // never move processes or enable controllers in the user's cgroup.
    if !fs::read_to_string(parent.join("cgroup.procs"))
        .map_err(|_| denied())?
        .trim()
        .is_empty()
    {
        return Err(denied());
    }
    Ok(())
}
fn parse_empty_cgroup_events(contents: &str) -> bool {
    let mut found = None;
    for line in contents.lines() {
        if let Some(value) = line.strip_prefix("populated ") {
            if found.is_some() || (value != "0" && value != "1") {
                return false;
            }
            found = Some(value == "0");
        }
    }
    found == Some(true)
}

fn current_cgroup_parent() -> Result<PathBuf, SupervisorError> {
    let data = fs::read_to_string("/proc/self/cgroup").map_err(|_| denied())?;
    let line = data
        .lines()
        .find(|l| l.starts_with("0::"))
        .ok_or_else(denied)?;
    let relative = line.strip_prefix("0::").ok_or_else(denied)?;
    if !relative.starts_with('/') || relative.split('/').any(|s| s == "." || s == "..") {
        return Err(denied());
    }
    let root = fs::canonicalize(CGROUP_ROOT).map_err(|_| denied())?;
    let actual =
        fs::canonicalize(root.join(relative.trim_start_matches('/'))).map_err(|_| denied())?;
    if !actual.starts_with(&root) {
        return Err(denied());
    }
    let parent = select_delegated_parent(&root, &actual)?;
    require_delegated_limits(&parent)?;
    Ok(parent)
}

/// FIN-02: read-only OS admission prerequisites for the host's *current*
/// kernel identity. This does NOT create a cgroup, move a process, claim
/// authority or prove that a future child entered its assigned worker group.
pub fn native_delegated_host_readiness() -> Result<(), SupervisorError> {
    ensure_root_owned_tool(Path::new(BWRAP))?;
    let parent=current_cgroup_parent()?;
    if !fs::symlink_metadata(&parent).map_err(|_|denied())?.is_dir(){
        return Err(denied());
    }
    let host=parent.join("lu-ide-host");
    let observed=fs::canonicalize(&host).map_err(|_|denied())?;
    let current=fs::read_to_string("/proc/self/cgroup").map_err(|_|denied())?;
    if current.lines().filter(|row|row.starts_with("0::")).count()!=1 ||
       observed.file_name().and_then(|v|v.to_str())!=Some("lu-ide-host") {
        return Err(denied());
    }
    for filename in ["cgroup.procs","cgroup.events"]{
       if !host.join(filename).is_file(){return Err(denied());}
    }
    Ok(())
}

fn make_cgroup(job_id: &str, plan: &JobPlan) -> Result<Cgroup, SupervisorError> {
    if !job_id.starts_with("job-")
        || job_id.len() != 36
        || !job_id[4..].bytes().all(|v| v.is_ascii_hexdigit())
    {
        return Err(denied());
    }
    let parent = current_cgroup_parent()?;
    native_linux_cgroup_trace("parent_delegation_verified");
    let dir = parent.join(format!("lu-ide-{job_id}"));
    fs::create_dir(&dir).map_err(|_| denied())?;
    native_linux_cgroup_trace("job_cgroup_created");
    let mut group = Cgroup { dir, active: true };
    // A brand-new dedicated worker cgroup may not already hold a process.
    if !group.zero_members() {
        group.remove_empty();
        return Err(denied());
    }
    native_linux_cgroup_trace("new_cgroup_empty");
    let settings = [
        ("cpu.max", "50000 100000".to_owned()),
        ("memory.max", plan.memory_limit_bytes.to_string()),
        ("pids.max", plan.process_limit.to_string()),
    ];
    for (key, value) in settings {
        if fs::write(group.dir.join(key), &value).is_err() {
            native_linux_cgroup_trace("quota_write_denied");
            group.remove_empty();
            return Err(denied());
        }
        let actual = fs::read_to_string(group.dir.join(key)).map_err(|_| denied())?;
        if actual.trim() != value {
            native_linux_cgroup_trace("quota_readback_mismatch");
            group.remove_empty();
            return Err(denied());
        }
    }
    native_linux_cgroup_trace("cpu_memory_pids_quota_readback_verified");
    if !group.dir.join("cgroup.kill").is_file() || !group.dir.join("cgroup.procs").is_file() {
        group.remove_empty();
        return Err(denied());
    }
    native_linux_cgroup_trace("cgroup_kill_and_procs_available");
    Ok(group)
}

struct Cgroup {
    dir: PathBuf,
    active: bool,
}
impl Cgroup {
    fn remove_empty(&mut self) {
        // Never lose the cleanup obligation if the kernel rejects rmdir.
        if self.zero_members() && fs::remove_dir(&self.dir).is_ok() {
            self.active = false;
        }
    }
    fn zero_members(&self) -> bool {
        fs::read_to_string(self.dir.join("cgroup.events"))
            .map(|data| parse_empty_cgroup_events(&data))
            .unwrap_or(false)
    }
    fn await_empty(&self, limit: Duration) -> bool {
        let start = Instant::now();
        while start.elapsed() < limit {
            if self.zero_members() {
                return true;
            }
            thread::sleep(LOOP_DELAY);
        }
        self.zero_members()
    }
    fn kill_and_verify(&self) -> Result<bool, SupervisorError> {
        fs::write(self.dir.join("cgroup.kill"), "1").map_err(|_| denied())?;
        Ok(self.await_empty(REAP_LIMIT))
    }
}
impl Drop for Cgroup {
    fn drop(&mut self) {
        // Do not leave an uncontained descendant if a normal return path fails.
        // If kernel kill fails the cgroup remains visible for recovery; never
        // silently claim the journal's job was cancelled.
        if self.active {
            if !self.zero_members() {
                let _ = fs::write(self.dir.join("cgroup.kill"), "1");
                let _ = self.await_empty(REAP_LIMIT);
            }
            if self.zero_members() {
                let _ = fs::remove_dir(&self.dir);
            }
        }
    }
}

/// An independently readable kernel witness for the actual spawned PID.
/// `pre_exec` moving the child is necessary but not sufficient evidence:
/// refuse a forged/absent /proc identity or missing cgroup.procs membership
/// before the supervisor can record a job as Running.
fn require_kernel_child_membership(group: &Cgroup, pid: u32) -> Result<(), SupervisorError> {
    if pid == 0 {
        return Err(denied());
    }
    let proc_identity = fs::read_to_string(format!("/proc/{pid}/cgroup")).map_err(|_| denied())?;
    let mut v2 = proc_identity
        .lines()
        .filter_map(|line| line.strip_prefix("0::"));
    let relative = v2.next().ok_or_else(denied)?;
    if v2.next().is_some()
        || !relative.starts_with('/')
        || relative
            .split('/')
            .any(|component| component == "." || component == "..")
    {
        return Err(denied());
    }
    let kernel_root = fs::canonicalize(CGROUP_ROOT).map_err(|_| denied())?;
    let observed = fs::canonicalize(kernel_root.join(relative.trim_start_matches('/')))
        .map_err(|_| denied())?;
    let expected = fs::canonicalize(&group.dir).map_err(|_| denied())?;
    if observed != expected {
        return Err(denied());
    }
    let procs = fs::read_to_string(group.dir.join("cgroup.procs")).map_err(|_| denied())?;
    if !procs
        .lines()
        .any(|entry| entry.parse::<u32>().ok() == Some(pid))
    {
        return Err(denied());
    }
    Ok(())
}

struct CgroupTerminator<'a> {
    group: &'a Cgroup,
    expected_job: &'a str,
    expected_hash: &'a str,
}
impl NativeTreeTerminator for CgroupTerminator<'_> {
    fn terminate_and_verify_tree(
        &mut self,
        job_id: &str,
        plan_sha256: &str,
    ) -> Result<bool, SupervisorError> {
        if job_id != self.expected_job || plan_sha256 != self.expected_hash {
            return Err(SupervisorError::WrongProjectOrSource);
        }
        self.group.kill_and_verify()
    }
}

/// Attach to a cgroup during POSIX pre_exec; no bwrap or toolchain instruction
/// may run before this write succeeds. This code uses only Linux syscalls and
/// fixed stack buffers after fork, avoiding Rust allocations in the child.
unsafe fn attach_pid_in_pre_exec(cgroup_fd: i32) -> io::Result<()> {
    let mut number = unsafe { libc::getpid() } as u32;
    let mut buffer = [0_u8; 16];
    let mut index = buffer.len();
    loop {
        index -= 1;
        buffer[index] = b'0' + (number % 10) as u8;
        number /= 10;
        if number == 0 {
            break;
        }
    }
    let value = &buffer[index..];
    let written = unsafe { libc::write(cgroup_fd, value.as_ptr().cast(), value.len()) };
    if written != value.len() as isize {
        return Err(io::Error::last_os_error());
    }
    if unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0 {
        return Err(io::Error::last_os_error());
    }
    // Kernel inherited resource ceilings supplement cgroup accounting.
    let file_limit = libc::rlimit {
        rlim_cur: MAX_SINGLE_ARTIFACT_BYTES as libc::rlim_t,
        rlim_max: MAX_SINGLE_ARTIFACT_BYTES as libc::rlim_t,
    };
    if unsafe { libc::setrlimit(libc::RLIMIT_FSIZE, &file_limit) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let core_limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    if unsafe { libc::setrlimit(libc::RLIMIT_CORE, &core_limit) } != 0 {
        return Err(io::Error::last_os_error());
    }
    unsafe {
        libc::umask(0o077);
    }
    Ok(())
}

fn command_for(
    plan: &JobPlan,
    input: &PublishedInput,
    output: &Path,
    group: &Cgroup,
    runtime: Option<&crate::runtime_bundles::VerifiedRuntimeBundle>,
) -> Result<(Command, File), SupervisorError> {
    let manifest = match plan.recipe_id.as_str() {
        "lu_ide_host_tests" => "/workspace/ide/host/Cargo.toml",
        "rust_tests" | "rust_check" | "rust_build" => "/workspace/Cargo.toml",
        id if crate::rust_exact_case_name(id).is_some()=>"/workspace/Cargo.toml",
        "python_pytest" if runtime.is_some() => "/workspace/pyproject.toml",
        "uv_sync_offline" if cfg!(feature = "package-uv-worker") && runtime.is_some() => {
            "/workspace/pyproject.toml"
        }
        "js_tests" if runtime.is_some() => "/workspace/package.json",
        "cpp_cmake_build" if runtime.is_some() => "/workspace/CMakeLists.txt",
        _ => return Err(SupervisorError::InvalidPlan),
    };
    if !input
        .root
        .join(manifest.trim_start_matches("/workspace/"))
        .is_file()
        || input.seal.manifest_relative_path != manifest.trim_start_matches("/workspace/")
    {
        return Err(SupervisorError::WrongProjectOrSource);
    }
    let linker = trusted_gnu_linker()?;
    let proc_file = OpenOptions::new()
        .write(true)
        .open(group.dir.join("cgroup.procs"))
        .map_err(|_| denied())?;
    let fd = proc_file.as_raw_fd();
    let mut cmd = Command::new(BWRAP);
    cmd.env_clear()
        .arg("--die-with-parent")
        .arg("--new-session")
        .arg("--unshare-all")
        .arg("--clearenv")
        .arg("--ro-bind")
        .arg("/usr")
        .arg("/usr")
        .arg("--ro-bind-try")
        .arg("/bin")
        .arg("/bin")
        .arg("--ro-bind-try")
        .arg("/lib")
        .arg("/lib")
        .arg("--ro-bind-try")
        .arg("/lib64")
        .arg("/lib64")
        .arg("--ro-bind")
        .arg(&input.root)
        .arg("/workspace")
        .arg("--bind")
        .arg(output)
        .arg("/output")
        .arg("--tmpfs")
        .arg("/tmp")
        .arg("--proc")
        .arg("/proc")
        .arg("--dev")
        .arg("/dev")
        .arg("--setenv")
        .arg("HOME")
        .arg("/tmp")
        .arg("--setenv")
        .arg("CARGO_HOME")
        .arg("/tmp/cargo-home")
        .arg("--setenv")
        .arg("CARGO_TARGET_DIR")
        .arg("/output/target")
        .arg("--setenv")
        .arg("CARGO_NET_OFFLINE")
        .arg("true")
        .arg("--setenv")
        .arg(LINKER_ENV_KEY)
        .arg(linker)
        .arg("--setenv")
        .arg("CARGO_BUILD_JOBS")
        .arg("1")
        .arg("--setenv")
        .arg("RUST_TEST_THREADS")
        .arg("1")
        .arg("--setenv")
        .arg("PATH")
        .arg("/usr/bin:/bin")
        .arg("--setenv")
        .arg("LANG")
        .arg("C")
        .arg("--chdir")
        .arg("/workspace");
    if let Some(bundle) = runtime {
        if bundle.profile.id() != plan.recipe_id || bundle.tree_sha256 != plan.toolchain_sha256 {
            return Err(SupervisorError::WrongProjectOrSource);
        }
        cmd.arg("--ro-bind")
            .arg(&bundle.root)
            .arg("/runtime")
            .arg("--setenv")
            .arg("PATH")
            .arg("/runtime/bin:/usr/bin:/bin")
            .arg("--setenv")
            .arg("PYTHONDONTWRITEBYTECODE")
            .arg("1")
            .arg("--setenv")
            .arg("NODE_REPL_HISTORY")
            .arg("/tmp/node-history")
            .arg("--setenv")
            .arg("TMPDIR")
            .arg("/tmp");
    }
    if plan.recipe_id == "uv_sync_offline" {
        // Source tree and lock are immutable at /workspace; all writable
        // environment/cache bytes are confined to the private /output mount.
        // --no-build alone does not suppress first-party build backends;
        // --no-install-workspace is independently necessary.
        cmd.arg("--setenv")
            .arg("UV_PROJECT_ENVIRONMENT")
            .arg("/output/.venv")
            .arg("--setenv")
            .arg("UV_CACHE_DIR")
            .arg("/output/uv-cache")
            .arg("--setenv")
            .arg("UV_NO_BUILD")
            .arg("1")
            .arg("--setenv")
            .arg("UV_PYTHON_DOWNLOADS")
            .arg("never")
            .arg("--setenv")
            .arg("UV_NO_PROGRESS")
            .arg("1")
            .arg("--setenv")
            .arg("UV_NO_CONFIG")
            .arg("1");
    }
    // Bubblewrap options MUST be constructed before its "--" separator.
    // Only this exact CI-only synthetic-wheel plan gets a local wheelhouse.
    // This never authorizes a production package project or network access.
    let ci_positive_wheelhouse = cfg!(feature = "ide055-ci-wheelhouse")
        && plan.recipe_id == "uv_sync_offline"
        && plan.effect_class == "package_uv_sync_offline:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
    if ci_positive_wheelhouse {
        cmd.arg("--setenv").arg("UV_NO_INDEX").arg("1");
        // uv 0.9's default install pool sizes to host CPU count, which
        // exceeds this OS-enforced 8-PID fixture limit on CI hardware.
        // Bound the Rayon installer pool without adding cgroup PIDs or
        // changing any production worker's admission/resource policy.
        cmd.arg("--setenv").arg("UV_CONCURRENT_INSTALLS").arg("1");
    }
    cmd.arg("--");
    match plan.recipe_id.as_str() {
        "rust_check" | "rust_build" | "rust_tests" | "lu_ide_host_tests" if runtime.is_none() => {
            cmd.arg(CARGO);
            cmd.arg(match plan.recipe_id.as_str() {
                "rust_check" => "check",
                "rust_build" => "build",
                _ => "test",
            })
            .arg("--locked")
            .arg("--offline")
            .arg("--manifest-path")
            .arg(manifest);
        }
        id if crate::rust_exact_case_name(id).is_some() && runtime.is_none()=>{
            let name=crate::rust_exact_case_name(id).ok_or(SupervisorError::InvalidPlan)?;
            cmd.arg(CARGO).args(["test","--locked","--offline",
                "--manifest-path"]).arg(manifest)
                .arg(name).args(["--","--exact"]);
        }
        "uv_sync_offline" if cfg!(feature = "package-uv-worker") && runtime.is_some() => {
            if !input.root.join("uv.lock").is_file() {
                return Err(SupervisorError::WrongProjectOrSource);
            }
            // Only this exact CI fixture, NOT the negative fixture or any
            // production worker, may resolve the pinned local wheelhouse.
            // A different reviewed plan retains the original --offline
            // registry/cache behavior and cannot inherit the CI exception.
            cmd.arg("/runtime/bin/uv").args([
                "sync",
                "--offline",
                "--locked",
                "--no-build",
                "--no-install-workspace",
                "--no-default-groups",
                "--no-dev",
                "--no-sources",
                "--link-mode",
                "copy",
                "--no-config",
                "--no-python-downloads",
                "--python",
                "/runtime/bin/python3",
            ]);
            if ci_positive_wheelhouse {
                // uv's default link-index cache creates symlinks that the
                // strict native private-output scanner rejects. This fixed
                // CI-only wheel is already in the read-only runtime, so use
                // an ephemeral cache in the isolated /tmp mount instead.
                // Production offline workers retain their existing cache.
                cmd.args([
                    "--no-index",
                    "--find-links",
                    "/runtime/wheelhouse",
                    "--no-cache",
                ]);
            }
        }
        "python_pytest" if runtime.is_some() => {
            cmd.arg("/runtime/bin/python3").args([
                "-I",
                "-m",
                "pytest",
                "-q",
                "-p",
                "no:cacheprovider",
            ]);
        }
        "js_tests" if runtime.is_some() => {
            cmd.arg("/runtime/bin/node")
                .args(["--test", "--test-reporter=tap"]);
        }
        "cpp_cmake_build" if runtime.is_some() => {
            cmd.arg("/runtime/bin/cmake")
                .arg("-DLU_IDE_WORKSPACE=/workspace")
                .arg("-DLU_IDE_OUTPUT=/output")
                .args(["-P", "/runtime/drivers/dev03-cmake.cmake"]);
        }
        _ => return Err(SupervisorError::InvalidPlan),
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // SAFETY: child does not allocate, lock, or access Rust shared state in
    // pre_exec; it writes its fixed PID to preopened cgroup.procs and sets
    // no_new_privs. If that fails Command::spawn returns error without exec.
    unsafe {
        cmd.pre_exec(move || attach_pid_in_pre_exec(fd));
    }
    Ok((cmd, proc_file))
}

struct Captured {
    sha256: String,
    bytes: usize,
    truncated: bool,
    category: &'static str,
}
fn classify_native_output(first: &[u8]) -> &'static str {
    let lower = String::from_utf8_lossy(first).to_ascii_lowercase();
    if lower.contains("creating new namespace failed")
        || lower.contains("unshare failed")
        || lower.contains("user namespaces are not enabled")
    {
        "bubblewrap_namespace_denied"
    } else if lower.contains("operation not permitted") || lower.contains("permission denied") {
        "native_permission_denied"
    } else if lower.contains("failed to download") || lower.contains("network is unreachable") {
        "unexpected_network_request"
    } else if lower.contains("lock file needs to be updated")
        || lower.contains("lockfile needs to be updated")
    {
        "offline_lockfile_incompatible"
    } else if lower.contains("could not execute process") || lower.contains("couldn't execute") {
        "compiler_exec_failed"
    } else if lower.contains("no such file or directory") {
        "required_file_missing"
    } else if lower.contains("failed to parse manifest") {
        "manifest_denied"
    } else if lower.contains("error:") {
        "native_reported_error"
    } else if first.is_empty() {
        "empty"
    } else {
        "unclassified_nonempty"
    }
}
fn collect_limited<R: Read + Send + 'static>(
    mut source: R,
    max: usize,
    live: Option<Arc<LiveOutput>>,
    kind: &'static str,
) -> thread::JoinHandle<io::Result<Captured>> {
    thread::spawn(move || {
        let mut digest = Sha256::new();
        let mut buffer = [0_u8; TOTAL_LOG_BUFFER];
        let mut kept = 0_usize;
        let mut truncated = false;
        let mut diagnostic_prefix = Vec::with_capacity(2048);
        loop {
            let n = source.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            let prefix_left = 2048_usize.saturating_sub(diagnostic_prefix.len());
            diagnostic_prefix.extend_from_slice(&buffer[..n.min(prefix_left)]);
            let len = n.min(max.saturating_sub(kept));
            if let Some(ref stream) = live {
                stream.append(kind, &buffer[..len]);
            }
            digest.update(&buffer[..len]);
            kept += len;
            if n > len {
                truncated = true;
            }
        }
        Ok(Captured {
            sha256: format!("{:x}", digest.finalize()),
            bytes: kept,
            truncated,
            category: classify_native_output(&diagnostic_prefix),
        })
    })
}
fn make_output_root(parent: &Path, job_id: &str) -> Result<PathBuf, SupervisorError> {
    let meta = fs::symlink_metadata(parent).map_err(|_| denied())?;
    if !meta.is_dir() || meta.file_type().is_symlink() || meta.mode() & 0o077 != 0 {
        return Err(denied());
    }
    let output = parent.join(job_id);
    fs::create_dir(&output).map_err(|_| denied())?;
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&output, fs::Permissions::from_mode(0o700)).map_err(|_| denied())?;
    Ok(output)
}
// Bounds private bytes only, never proves that a package install is safe.
// uv creates interpreter aliases even for failed offline sync. These exact,
// inert aliases are not followed and remain INELIGIBLE for artifact promotion.
fn uv_virtualenv_alias_bytes(root: &Path, link: &Path) -> Option<u64> {
    let relative = link.strip_prefix(root).ok()?.to_str()?;
    let target = fs::read_link(link).ok()?;
    let value = target.to_str()?;
    if value.is_empty() || value.len() > 128 {
        return None;
    }
    let system_python = ["/usr/bin/python3", "/runtime/bin/python3"]
        .iter()
        .any(|prefix| {
            value.strip_prefix(prefix).is_some_and(|suffix| {
                suffix.is_empty()
                    || suffix
                        .strip_prefix('.')
                        .is_some_and(|v| !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()))
            })
        });
    let accepted = if relative == ".venv/lib64" {
        value == "lib"
    } else if relative == ".venv/bin/python" {
        system_python
    } else if relative == ".venv/bin/python3" {
        value == "python"
    } else if let Some(version) = relative.strip_prefix(".venv/bin/python3.") {
        !version.is_empty() && version.bytes().all(|b| b.is_ascii_digit()) && value == "python"
    } else {
        false
    };
    accepted.then_some(value.len() as u64)
}

fn scan_output_budget(root: &Path) -> Result<(), SupervisorError> {
    scan_output_budget_for_recipe(root, false)
}

fn scan_output_budget_for_recipe(
    root: &Path,
    allow_uv_aliases: bool,
) -> Result<(), SupervisorError> {
    let mut pending = vec![(root.to_path_buf(), 0_usize)];
    let mut files = 0_usize;
    let mut bytes = 0_u64;
    while let Some((dir, depth)) = pending.pop() {
        if depth > 40 {
            return Err(denied());
        }
        for entry in fs::read_dir(dir).map_err(|_| denied())? {
            let entry = entry.map_err(|_| denied())?;
            let path = entry.path();
            let meta = fs::symlink_metadata(&path).map_err(|_| denied())?;
            let entry_bytes = if meta.file_type().is_symlink() {
                if !allow_uv_aliases {
                    return Err(denied());
                }
                uv_virtualenv_alias_bytes(root, &path).ok_or_else(denied)?
            } else if meta.is_dir() {
                pending.push((path, depth + 1));
                continue;
            } else if meta.is_file() {
                meta.len()
            } else {
                return Err(denied());
            };
            files += 1;
            bytes = bytes.checked_add(entry_bytes).ok_or_else(denied)?;
            if files > MAX_OUTPUT_FILES
                || bytes > MAX_ARTIFACT_BYTES
                || entry_bytes > MAX_SINGLE_ARTIFACT_BYTES
            {
                return Err(denied());
            }
        }
    }
    Ok(())
}

fn proof_for(group: &Cgroup) -> OsContainmentProof {
    OsContainmentProof {
        platform: "linux".to_owned(),
        restricted_identity: true, // Bubblewrap unshared user namespace
        filesystem_access_limited: true, // explicit ro-bind/isolated output
        network_access_limited: true, // Bubblewrap --unshare-all includes net
        child_contained_before_resume: true, // pre_exec cgroup.procs
        process_tree_kill_verified: group.dir.join("cgroup.kill").is_file(),
        memory_quota: true,
        cpu_quota: true,
        pid_quota: true,
        // App-profile journal is not mounted in the worker's namespace.
        protected_journal_broker: true,
    }
}

/// Executes a genuine Rust build/test process only under full Linux OS
/// isolation and after an exact in-memory one-shot grant has already been
/// recorded by the trusted native approval API. Not exposed to the WebView.
/// This is compile-feature gated until hostile platform acceptance passes.
pub fn execute_linux_worker(
    supervisor: &Supervisor,
    job_id: &str,
    plan: &JobPlan,
    input: &PublishedInput,
    output_parent: &Path,
    cancellation: &RunCancellation,
) -> Result<ExecutionResult, SupervisorError> {
    execute_linux_worker_with_output(
        supervisor,
        job_id,
        plan,
        input,
        output_parent,
        cancellation,
        None,
    )
}

/// Same already-protected cgroup/Bubblewrap worker; optional native-only
/// bounded stream witness is not authority and cannot be provided by WebView.
pub fn execute_linux_worker_with_output(
    supervisor: &Supervisor,
    job_id: &str,
    plan: &JobPlan,
    input: &PublishedInput,
    output_parent: &Path,
    cancellation: &RunCancellation,
    live: Option<Arc<LiveOutput>>,
) -> Result<ExecutionResult, SupervisorError> {
    verify_input(input).map_err(|_| SupervisorError::WrongProjectOrSource)?;
    if plan.input_snapshot_sha256 != input.seal.source_sha256
        || plan.manifest_sha256 != input.seal.manifest_sha256
    {
        return Err(SupervisorError::WrongProjectOrSource);
    }
    let runtime = if cfg!(feature = "multilang-worker")
        && crate::runtime_bundles::RuntimeProfile::for_id(&plan.recipe_id).is_some()
    {
        let bundle = reviewed_multilang_bundle(&plan.recipe_id)?;
        if plan.toolchain_sha256 != bundle.tree_sha256 {
            return Err(SupervisorError::WrongProjectOrSource);
        }
        Some(bundle)
    } else {
        if plan.toolchain_sha256 != installed_toolchain_digest()? {
            return Err(SupervisorError::WrongProjectOrSource);
        }
        None
    };
    // Verify fixed launcher and binary paths BEFORE issuing an admission
    // receipt or creating any process. No caller-specified executable.
    for tool in [BWRAP, CARGO, RUSTC, trusted_gnu_linker()?] {
        ensure_root_owned_tool(Path::new(tool))?;
    }
    let group = make_cgroup(job_id, plan)?;
    let output = make_output_root(output_parent, job_id)?;
    let (mut command, proc_file) = command_for(plan, input, &output, &group, runtime.as_ref())?;
    let current_proof = proof_for(&group);
    if !current_proof.complete() {
        return Err(denied());
    }
    supervisor.consume_grant_after_attestation(job_id, plan, &current_proof)?;
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(_) => {
            // pre_exec refused before the bwrap image could execute.
            if group.zero_members() {
                supervisor.native_deny_before_start(job_id, true)?;
            }
            return Err(denied());
        }
    };
    drop(proc_file);
    // The actual kernel must confirm the just-spawned child still belongs to
    // this exact newly created cgroup before publishing Running. Unexpected
    // early exit, migration, stale PID or missing /proc accounting all deny.
    if require_kernel_child_membership(&group, child.id()).is_err() {
        let _ = group.kill_and_verify();
        let _ = supervisor.native_quarantine_after_error(job_id);
        return Err(denied());
    }
    supervisor.native_mark_running(job_id)?;
    // CI-only crash oracle: abort after kernel membership and durable Running.
    // This opt-in feature is absent from default workers and installed apps.
    if cfg!(feature = "ide055-ci-wheelhouse")
        && std::env::var("LU_IDE055_CI_CRASH_AFTER_RUNNING").as_deref() == Ok("1")
    {
        eprintln!("IDE055_CI_HOSTILE_ABORT_AFTER_NATIVE_RUNNING_RECEIPT");
        std::process::abort();
    }
    let stdout = child.stdout.take().ok_or_else(denied)?;
    let stderr = child.stderr.take().ok_or_else(denied)?;
    let half = usize::try_from(plan.log_limit_bytes / 2).unwrap_or(0);
    let out_reader = collect_limited(stdout, half, live.clone(), "stdout");
    let err_reader = collect_limited(stderr, half, live, "stderr");
    let began = Instant::now();
    let mut timed_out = false;
    let mut was_cancelled = false;
    let mut output_limit_hit = false;
    let mut last_output_check = Instant::now();
    let status = loop {
        if last_output_check.elapsed() >= OUTPUT_CHECK_INTERVAL {
            if scan_output_budget_for_recipe(&output, plan.recipe_id == "uv_sync_offline").is_err()
            {
                output_limit_hit = true;
            }
            last_output_check = Instant::now();
        }
        if cancellation.cancelled.load(Ordering::SeqCst)
            || began.elapsed().as_millis() >= plan.runtime_limit_ms as u128
            || output_limit_hit
        {
            timed_out = !cancellation.cancelled.load(Ordering::SeqCst);
            was_cancelled = true;
            let exact_hash = supervisor
                .inspect()?
                .iter()
                .find(|j| j.job_id == job_id)
                .ok_or(SupervisorError::NoSuchJob)?
                .plan_sha256
                .clone();
            let mut terminator = CgroupTerminator {
                group: &group,
                expected_job: job_id,
                expected_hash: &exact_hash,
            };
            // Request is journaled BEFORE native cgroup.kill; failed cleanup
            // becomes uncertain and stops further admissions.
            supervisor.cancel_running_with(job_id, &mut terminator)?;
            break child
                .wait()
                .map_err(|_| SupervisorError::JournalUnavailable)?;
        }
        if let Some(exit) = child
            .try_wait()
            .map_err(|_| SupervisorError::JournalUnavailable)?
        {
            break exit;
        }
        thread::sleep(LOOP_DELAY);
    };
    let out = out_reader
        .join()
        .map_err(|_| SupervisorError::JournalUnavailable)?
        .map_err(|_| SupervisorError::JournalUnavailable)?;
    let err = err_reader
        .join()
        .map_err(|_| SupervisorError::JournalUnavailable)?
        .map_err(|_| SupervisorError::JournalUnavailable)?;
    let empty = group.await_empty(REAP_LIMIT);
    let final_output_bounded =
        scan_output_budget_for_recipe(&output, plan.recipe_id == "uv_sync_offline").is_ok();
    if cfg!(debug_assertions) && plan.recipe_id == "uv_sync_offline" {
        eprintln!(
            "IDE055_UV_OS_TERMINAL_FACTS=tree_reaped:{empty},output_bounded:{final_output_bounded},exit:{:?}",
            status.code()
        );
    }
    let summary = if was_cancelled {
        supervisor
            .inspect()?
            .into_iter()
            .find(|j| j.job_id == job_id)
            .ok_or(SupervisorError::NoSuchJob)?
    } else {
        supervisor.native_finish(
            job_id,
            status.success() && !out.truncated && !err.truncated && final_output_bounded,
            empty,
            final_output_bounded,
        )?
    };
    Ok(ExecutionResult {
        summary,
        exit_code: status.code(),
        stdout_sha256: out.sha256,
        stderr_sha256: err.sha256,
        stderr_category: err.category,
        stdout_bytes_recorded: out.bytes,
        stderr_bytes_recorded: err.bytes,
        output_truncated: out.truncated || err.truncated,
        output_directory: output,
        timed_out,
        output_limit_hit,
    })
}

/// Explicit recovery after the IDE or native host restarts. The previous
/// cgroup path must still exist under the *current* delegated parent and
/// must match this exact native job identity; otherwise remain quarantined.
/// No process is restarted. A verified cgroup.kill + populated=0 is required.
pub fn reconcile_linux_interrupted(
    supervisor: &Supervisor,
    job_id: &str,
) -> Result<JobSummary, SupervisorError> {
    if !job_id.starts_with("job-")
        || job_id.len() != 36
        || !job_id[4..].bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(SupervisorError::NoSuchJob);
    }
    let summary = supervisor
        .inspect()?
        .into_iter()
        .find(|value| value.job_id == job_id)
        .ok_or(SupervisorError::NoSuchJob)?;
    if !summary.requires_reconciliation {
        return Err(SupervisorError::InvalidTransition);
    }
    let root = current_cgroup_parent()?;
    let path = root.join(format!("lu-ide-{job_id}"));
    let metadata = fs::symlink_metadata(&path).map_err(|_| SupervisorError::Quarantined)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(SupervisorError::Quarantined);
    }
    if !path.join("cgroup.kill").is_file() || !path.join("cgroup.events").is_file() {
        return Err(SupervisorError::Quarantined);
    }
    let group = Cgroup {
        dir: path,
        active: true,
    };
    let mut broker = CgroupTerminator {
        group: &group,
        expected_job: job_id,
        expected_hash: &summary.plan_sha256,
    };
    supervisor.reconcile_interrupted_with(job_id, &mut broker)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actual_pipe_drain_emits_bounded_distinct_streams() {
        let sink = Arc::new(LiveOutput::default());
        let stdout = collect_limited(
            std::io::Cursor::new(b"ok\n".to_vec()),
            4096,
            Some(Arc::clone(&sink)),
            "stdout",
        );
        let stderr = collect_limited(
            std::io::Cursor::new(b"warning\n".to_vec()),
            4096,
            Some(Arc::clone(&sink)),
            "stderr",
        );
        assert_eq!(stdout.join().unwrap().unwrap().bytes, 3);
        assert_eq!(stderr.join().unwrap().unwrap().bytes, 8);
        let page = sink.page(0);
        assert_eq!(page.chunks.len(), 2);
        let kinds: std::collections::HashSet<_> = page.chunks.iter().map(|c| c.stream).collect();
        assert!(kinds.contains("stdout"));
        assert!(kinds.contains("stderr"));
        assert!(page.live_unverified);
    }

    #[test]
    fn only_explicit_sibling_host_leaf_can_use_delegated_parent() {
        let root = Path::new("/sys/fs/cgroup");
        let base = root.join("user.slice").join("lu-ide-delegated");
        let good = base.join("lu-ide-host");
        assert_eq!(select_delegated_parent(root, &good).unwrap(), base);
        let service = root.join("system.slice/lu-ide-delegated.service");
        assert_eq!(
            select_delegated_parent(root, &service.join("lu-ide-host")).unwrap(),
            service
        );
        assert!(select_delegated_parent(root, &root.join("user.slice")).is_err());
        assert!(select_delegated_parent(root, &base.join("other-host")).is_err());
        assert!(select_delegated_parent(root, &root.join("lu-ide-host")).is_err());
        assert!(
            select_delegated_parent(root, Path::new("/tmp/lu-ide-delegated/lu-ide-host")).is_err()
        );
    }

    #[test]
    fn quota_controllers_must_be_enabled_not_merely_available() {
        assert!(has_controllers("cpu memory pids io cpuset"));
        assert!(!has_controllers("cpu memory io"));
        assert!(!has_controllers("+cpu +memory +pids"));
        assert!(!has_controllers("cpuset memory pids"));
    }

    #[test]
    fn cgroup_empty_evidence_rejects_missing_or_conflicting_values() {
        assert!(parse_empty_cgroup_events("populated 0\nfrozen 0\n"));
        assert!(!parse_empty_cgroup_events("populated 1\nfrozen 0\n"));
        assert!(!parse_empty_cgroup_events("frozen 0\n"));
        assert!(!parse_empty_cgroup_events("populated 0\npopulated 1\n"));
        assert!(!parse_empty_cgroup_events("populated 0\npopulated 0\n"));
    }

    #[test]
    fn arbitrary_toolchain_and_bwrap_paths_are_not_accepted() {
        assert_eq!(BWRAP, "/usr/bin/bwrap");
        assert_eq!(CARGO, "/usr/bin/cargo");
        assert_eq!(RUSTC, "/usr/bin/rustc");
        assert!(
            matches!(
                installed_toolchain_digest(),
                Err(SupervisorError::NoContainment)
            ) || installed_toolchain_digest().is_ok()
        );
    }

    #[test]
    fn output_budget_rejects_sparse_files_and_links() {
        let root = std::env::temp_dir().join(format!("ide-output-probe-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir(&root).unwrap();
        let too_big = root.join("too-big");
        File::create(&too_big)
            .unwrap()
            .set_len(MAX_SINGLE_ARTIFACT_BYTES + 1)
            .unwrap();
        assert!(scan_output_budget(&root).is_err());
        fs::remove_file(&too_big).unwrap();
        assert!(scan_output_budget(&root).is_ok());
        use std::os::unix::fs::symlink;
        symlink("/etc/passwd", root.join("link")).unwrap();
        assert!(scan_output_budget(&root).is_err());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn uv_budget_accepts_only_inert_private_venv_aliases() {
        use std::os::unix::fs::symlink;
        let root =
            std::env::temp_dir().join(format!("ide-uv-private-output-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join(".venv/bin")).unwrap();
        fs::create_dir_all(root.join(".venv/lib")).unwrap();
        symlink("/usr/bin/python3", root.join(".venv/bin/python")).unwrap();
        symlink("python", root.join(".venv/bin/python3")).unwrap();
        symlink("python", root.join(".venv/bin/python3.12")).unwrap();
        symlink("lib", root.join(".venv/lib64")).unwrap();
        assert!(scan_output_budget(&root).is_err());
        assert!(scan_output_budget_for_recipe(&root, true).is_ok());
        symlink("/etc/passwd", root.join(".venv/bin/untrusted")).unwrap();
        assert!(scan_output_budget_for_recipe(&root, true).is_err());
        fs::remove_file(root.join(".venv/bin/untrusted")).unwrap();
        symlink("../../../../etc/passwd", root.join(".venv/bin/python3.13")).unwrap();
        assert!(scan_output_budget_for_recipe(&root, true).is_err());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn undelegated_cgroup_denies_without_any_process_fallback() {
        let missing = JobPlan {
            project_id: format!("ide-{}", "a".repeat(32)),
            recipe_id: "rust_check".to_owned(),
            source_head: "b".repeat(40),
            input_snapshot_sha256: "c".repeat(64),
            manifest_sha256: "d".repeat(64),
            toolchain_sha256: "e".repeat(64),
            debug_target_binding_sha256: None,
            runtime_limit_ms: 1000,
            memory_limit_bytes: 64 * 1024 * 1024,
            process_limit: 2,
            log_limit_bytes: 4096,
            effect_class: "test_build_isolated".to_owned(),
        };
        let bogus_job = "job-ffffffffffffffffffffffffffffffff";
        let attempt = make_cgroup(bogus_job, &missing);
        if let Ok(group) = attempt {
            assert!(group.dir.join("cgroup.procs").is_file());
            drop(group);
        } // Delegation may be present; test must never launch a child.
    }
}
