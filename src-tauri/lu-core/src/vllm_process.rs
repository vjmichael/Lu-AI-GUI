//! Linux-only, Lu-owned vLLM V1 sidecar. No Docker or externally supplied
//! daemon is ever killed. The owner supplies pinned interpreter + worker
//! script identities; Python/vLLM environment attestation remains an
//! independent release gate. Lu internal inference IPC stays Protobuf.
#![cfg(target_os = "linux")]
use crate::vllm_control::VllmControlTransport;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{BufRead, BufReader, Read, Write},
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::{Arc, Mutex},
};

const MAX_CONTROL: usize = 64 * 1024;
const SIGKILL: i32 = 9;
unsafe extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
}

#[derive(Debug, PartialEq, Eq)]
pub enum WorkerError {
    UntrustedProgram,
    InvalidConfig,
    Unavailable,
    Protocol,
    Io,
    Poisoned,
    ReapFailed,
}

pub struct VllmWorkerConfig {
    pub python_binary: PathBuf,
    pub python_sha256: [u8; 32],
    pub worker_script: PathBuf,
    pub script_sha256: [u8; 32],
    /// Verified host selection, not a model-provided path.
    pub host_model_ref: String,
    pub max_model_len: u32,
}
fn pin_file(path: &Path, expected: &[u8; 32]) -> Result<PathBuf, WorkerError> {
    if expected.iter().all(|v| *v == 0) {
        return Err(WorkerError::UntrustedProgram);
    }
    let canonical = path.canonicalize().map_err(|_| WorkerError::UntrustedProgram)?;
    if !canonical.is_file() { return Err(WorkerError::UntrustedProgram); }
    let mut file = File::open(&canonical).map_err(|_| WorkerError::UntrustedProgram)?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer).map_err(|_| WorkerError::UntrustedProgram)?;
        if n == 0 { break; }
        hash.update(&buffer[..n]);
    }
    if hash.finalize().as_slice() != expected { return Err(WorkerError::UntrustedProgram); }
    Ok(canonical)
}

pub struct VllmWorkerProcess {
    child: Arc<Mutex<Child>>,
    input: Arc<Mutex<ChildStdin>>,
    output: Arc<Mutex<BufReader<ChildStdout>>>,
}
impl VllmWorkerProcess {
    pub fn launch(config: VllmWorkerConfig) -> Result<Self, WorkerError> {
        if !(256..=262_144).contains(&config.max_model_len)
            || config.host_model_ref.is_empty()
            || config.host_model_ref.len() > 4096
            || config.host_model_ref.chars().any(|c| c == '\0' || c.is_control()) {
            return Err(WorkerError::InvalidConfig);
        }
        let python = pin_file(&config.python_binary, &config.python_sha256)?;
        let script = pin_file(&config.worker_script, &config.script_sha256)?;
        let mut command = Command::new(python);
        command.arg(script).arg("--model").arg(config.host_model_ref)
            .arg("--max-model-len").arg(config.max_model_len.to_string())
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
        // Own a distinct process group, including vLLM engine subprocesses.
        // Only Lu's retained *owned* child handle can trigger a group kill.
        command.process_group(0);
        let mut child = command.spawn().map_err(|_| WorkerError::Unavailable)?;
        let input = child.stdin.take().ok_or(WorkerError::Unavailable)?;
        let output = child.stdout.take().ok_or(WorkerError::Unavailable)?;
        Ok(Self {
            child: Arc::new(Mutex::new(child)),
            input: Arc::new(Mutex::new(input)),
            output: Arc::new(Mutex::new(BufReader::new(output))),
        })
    }
    pub fn pid(&self) -> Result<u32, WorkerError> {
        self.child.lock().map(|c| c.id()).map_err(|_| WorkerError::Poisoned)
    }
    /// The requesting thread may block reading output. Another trusted thread
    /// may still send abort or kill the process group without its read lock.
    pub fn read_event(&self) -> Result<Value, WorkerError> {
        let mut guard = self.output.lock().map_err(|_| WorkerError::Poisoned)?;
        let mut bounded = (&mut *guard).take((MAX_CONTROL + 1) as u64);
        let mut bytes = Vec::new();
        bounded.read_until(b'\n', &mut bytes).map_err(|_| WorkerError::Io)?;
        if bytes.is_empty() || bytes.len() > MAX_CONTROL || !bytes.ends_with(b"\n") {
            return Err(WorkerError::Protocol);
        }
        let value: Value = serde_json::from_slice(&bytes).map_err(|_| WorkerError::Protocol)?;
        if !value.is_object() { return Err(WorkerError::Protocol); }
        Ok(value)
    }
    pub fn is_running(&self) -> Result<bool, WorkerError> {
        let mut child = self.child.lock().map_err(|_| WorkerError::Poisoned)?;
        Ok(child.try_wait().map_err(|_| WorkerError::Io)?.is_none())
    }
    pub fn force_terminate_owned_group(&self) -> Result<(), WorkerError> {
        let mut child = self.child.lock().map_err(|_| WorkerError::Poisoned)?;
        // Check retained process ownership before invoking kill(-pgid).
        // A PID/process-group reuse race remains possible without pidfd/cgroup;
        // production release requires an OS-strong containment proof.
        if child.try_wait().map_err(|_| WorkerError::Io)?.is_some() { return Ok(()); }
        let pgid = i32::try_from(child.id()).map_err(|_| WorkerError::InvalidConfig)?;
        if pgid <= 1 { return Err(WorkerError::InvalidConfig); }
        // SAFETY: negative PGID was assigned to the owned child at spawn.
        let result = unsafe { kill(-pgid, SIGKILL) };
        if result != 0 {
            let _ = child.kill();
        }
        child.wait().map_err(|_| WorkerError::ReapFailed)?;
        Ok(())
    }
}
impl VllmControlTransport for VllmWorkerProcess {
    fn terminate_owned_worker(&mut self) -> Result<(), ()> {
        self.force_terminate_owned_group().map_err(|_| ())
    }
    fn read_host_event(&mut self) -> Result<Value, ()> {
        self.read_event().map_err(|_| ())
    }
    fn send_host_control(&mut self, message: &Value) -> Result<(), ()> {
        let data = serde_json::to_vec(message).map_err(|_| ())?;
        if data.is_empty() || data.len() + 1 > MAX_CONTROL { return Err(()); }
        let mut input = self.input.lock().map_err(|_| ())?;
        input.write_all(&data).map_err(|_| ())?;
        input.write_all(b"\n").map_err(|_| ())?;
        input.flush().map_err(|_| ())
    }
}
impl Drop for VllmWorkerProcess {
    fn drop(&mut self) {
        let _ = self.force_terminate_owned_group();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn linux_group_ownership_kills_real_stuck_worker() {
        // No vLLM or GPU required. This is the OS containment primitive only.
        let mut command=Command::new("/bin/sh");
        command.arg("-c").arg("sleep 30 & wait").stdin(Stdio::piped())
            .stdout(Stdio::piped()).stderr(Stdio::null()).process_group(0);
        let mut child=command.spawn().unwrap();
        let input=child.stdin.take().unwrap();
        let output=child.stdout.take().unwrap();
        let worker=VllmWorkerProcess {
            child: Arc::new(Mutex::new(child)),
            input: Arc::new(Mutex::new(input)),
            output: Arc::new(Mutex::new(BufReader::new(output))),
        };
        assert!(worker.is_running().unwrap());
        worker.force_terminate_owned_group().unwrap();
        assert!(!worker.is_running().unwrap());
    }
}
