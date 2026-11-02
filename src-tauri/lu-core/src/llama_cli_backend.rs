//! A concrete Lu-owned CPU GGUF execution adapter using a trusted llama.cpp
//! `llama-cli` binary. This is a bounded, one-shot subprocess backend,
//! not Jan's model manager, Jan's worker, an MCP server, or an HTTP service.
//!
//! Model weights are loaded by llama.cpp when generation starts (not at
//! InferenceBackend::load). Reuse of KV cache/weights across turns needs a
//! future long-lived native engine. This adapter is Windows-first.
//! A host-signed package manifest must supply the executable SHA-256.
use crate::inference_host::{
    Cancellation, EngineError, GenerationRequest, InferenceBackend, TokenChunk,
};
use crate::model_admission::AdmittedModel;
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const OUTPUT_LIMIT: usize = 16 * 1024 * 1024;
static NEXT_PROMPT: AtomicU64 = AtomicU64::new(1);

/// Host policy supplies these values. Never accept the executable path,
/// expected digest or scratch directory from model-generated arguments.
pub struct LlamaCliConfig {
    pub executable: PathBuf,
    pub expected_sha256: [u8; 32],
    pub private_prompt_dir: PathBuf,
    pub threads: NonZeroUsize,
    pub timeout: Duration,
}

pub struct LlamaCliBackend {
    executable: PathBuf,
    expected_sha256: [u8; 32],
    private_prompt_dir: PathBuf,
    threads: NonZeroUsize,
    timeout: Duration,
    admitted_path: Option<PathBuf>,
    pinned_model_file: Option<File>,
}

fn checked_executable(path: &Path, expected: &[u8; 32]) -> Result<PathBuf, String> {
    if expected.iter().all(|byte| *byte == 0) {
        return Err("missing trusted llama.cpp executable digest".into());
    }
    let metadata = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !metadata.file_type().is_file() {
        return Err("llama.cpp executable must be a regular, non-symlink file".into());
    }
    let path = path.canonicalize().map_err(|e| e.to_string())?;
    let mut f = File::open(&path).map_err(|e| e.to_string())?;
    let mut digest = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        digest.update(&buf[..n]);
    }
    if digest.finalize().as_slice() != expected {
        return Err("llama.cpp executable checksum mismatch".into());
    }
    Ok(path)
}

impl LlamaCliBackend {
    pub fn new(config: LlamaCliConfig) -> Result<Self, String> {
        if config.timeout.is_zero() || config.timeout > Duration::from_secs(3600) {
            return Err("invalid inference deadline".into());
        }
        if config.threads.get() > 256 {
            return Err("excessive CPU thread count".into());
        }
        let metadata =
            fs::symlink_metadata(&config.private_prompt_dir).map_err(|e| e.to_string())?;
        if !metadata.file_type().is_dir() {
            return Err("host prompt directory must be an ordinary directory".into());
        }
        let private_prompt_dir = config
            .private_prompt_dir
            .canonicalize()
            .map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&private_prompt_dir)
                .map_err(|e| e.to_string())?
                .permissions()
                .mode();
            if mode & 0o077 != 0 {
                return Err("host prompt directory must be private (0700)".into());
            }
        }
        Ok(Self {
            executable: checked_executable(&config.executable, &config.expected_sha256)?,
            expected_sha256: config.expected_sha256,
            private_prompt_dir,
            threads: config.threads,
            timeout: config.timeout,
            admitted_path: None,
            pinned_model_file: None,
        })
    }
}

/// A private, create-new prompt file avoids exposing user text in the process
/// command line. Drop removes it, including after backend errors/cancellation.
struct PromptFile(PathBuf);
impl PromptFile {
    fn create(directory: &Path, prompt: &str) -> Result<Self, String> {
        for _ in 0..32 {
            let serial = NEXT_PROMPT.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| e.to_string())?
                .as_nanos();
            let path = directory.join(format!(
                "lu-prompt-{}-{nanos}-{serial}.txt",
                std::process::id()
            ));
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            match options.open(&path) {
                Ok(mut file) => {
                    let allocation = Self(path);
                    file.write_all(prompt.as_bytes())
                        .map_err(|e| e.to_string())?;
                    file.flush().map_err(|e| e.to_string())?;
                    return Ok(allocation);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.to_string()),
            }
        }
        Err("unable to allocate private prompt file".into())
    }
}
impl Drop for PromptFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// Kills an unfinished child if streaming exits early or a callback panics.
struct KillUnfinishedChild(Arc<Mutex<Child>>);
impl Drop for KillUnfinishedChild {
    fn drop(&mut self) {
        if let Ok(mut process) = self.0.lock() {
            if matches!(process.try_wait(), Ok(None)) {
                let _ = process.kill();
                let _ = process.wait();
            }
        }
    }
}

fn emit_utf8(
    pending: &mut Vec<u8>,
    index: &mut u32,
    emit: &mut dyn FnMut(TokenChunk) -> Result<(), EngineError>,
) -> Result<(), String> {
    loop {
        let valid = match std::str::from_utf8(pending) {
            Ok(_) => pending.len(),
            Err(error) if error.error_len().is_none() => error.valid_up_to(),
            Err(_) => return Err("llama.cpp emitted invalid UTF-8".into()),
        };
        if valid == 0 {
            break;
        }
        let text = std::str::from_utf8(&pending[..valid])
            .map_err(|e| e.to_string())?
            .to_owned();
        pending.drain(..valid);
        emit(TokenChunk {
            text,
            index: *index,
        })
        .map_err(|e| format!("consumer rejected inference output: {e:?}"))?;
        *index = index.saturating_add(1);
    }
    Ok(())
}

impl InferenceBackend for LlamaCliBackend {
    fn load(&mut self, admitted: &AdmittedModel) -> Result<(), String> {
        if self.pinned_model_file.is_some() {
            return Err("model already admitted into llama.cpp backend".into());
        }
        admitted.recheck().map_err(|e| format!("{e:?}"))?;
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            // Deny writes and delete/rename while this file is in use.
            options.share_mode(0x00000001); // FILE_SHARE_READ
        }
        let mut f = options.open(admitted.path()).map_err(|e| e.to_string())?;
        if f.metadata().map_err(|e| e.to_string())?.len() != admitted.header().file_bytes {
            return Err("GGUF changed while acquiring model handle".into());
        }
        let mut magic = [0; 4];
        f.read_exact(&mut magic).map_err(|e| e.to_string())?;
        if &magic != b"GGUF" {
            return Err("GGUF header changed while acquiring model handle".into());
        }
        admitted.recheck().map_err(|e| format!("{e:?}"))?;
        self.admitted_path = Some(admitted.path().to_path_buf());
        self.pinned_model_file = Some(f);
        Ok(())
    }

    fn unload(&mut self) {
        self.admitted_path = None;
        self.pinned_model_file = None;
    }

    fn generate(
        &mut self,
        request: &GenerationRequest,
        cancel: &Cancellation,
        emit: &mut dyn FnMut(TokenChunk) -> Result<(), EngineError>,
    ) -> Result<(), String> {
        let model = self.admitted_path.as_ref().ok_or("GGUF not admitted")?;
        if self.pinned_model_file.is_none() {
            return Err("missing pinned model handle".into());
        }
        if cancel.is_cancelled() {
            return Err("generation cancelled before startup".into());
        }
        checked_executable(&self.executable, &self.expected_sha256)?;
        let prompt = PromptFile::create(&self.private_prompt_dir, &request.prompt)?;
        let mut command = Command::new(&self.executable);
        command
            .arg("--model")
            .arg(model)
            .arg("--file")
            .arg(&prompt.0)
            .arg("--n-predict")
            .arg(request.max_new_tokens.to_string())
            .arg("--threads")
            .arg(self.threads.get().to_string())
            .arg("--n-gpu-layers")
            .arg("0")
            .arg("--temp")
            .arg("0")
            .arg("--single-turn")
            .arg("--simple-io")
            .arg("--no-display-prompt")
            .arg("--no-show-timings")
            .arg("--no-warmup")
            .arg("--color")
            .arg("off")
            .current_dir(self.executable.parent().ok_or("invalid engine path")?)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // Do not open an interactive console window for every generation.
            command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        // Disable implicit llama.cpp flags inherited from LM Studio/Jan/shell.
        for (name, _) in std::env::vars_os() {
            if name.to_string_lossy().starts_with("LLAMA_ARG_") {
                command.env_remove(name);
            }
        }
        let mut child = command.spawn().map_err(|e| e.to_string())?;
        let mut stdout = child.stdout.take().ok_or("missing engine stdout")?;
        let mut stderr = child.stderr.take().ok_or("missing engine stderr")?;
        let child = Arc::new(Mutex::new(child));
        let _kill_on_drop = KillUnfinishedChild(Arc::clone(&child));
        let stderr_thread = thread::spawn(move || {
            let mut tail = Vec::new();
            let mut buf = [0u8; 4096];
            while let Ok(n) = stderr.read(&mut buf) {
                if n == 0 {
                    break;
                }
                tail.extend_from_slice(&buf[..n]);
                if tail.len() > 4096 {
                    tail.drain(..tail.len() - 4096);
                }
            }
            String::from_utf8_lossy(&tail).into_owned()
        });
        let done = Arc::new(AtomicBool::new(false));
        let timed_out = Arc::new(AtomicBool::new(false));
        let watched = Arc::clone(&child);
        let finished = Arc::clone(&done);
        let token = cancel.clone();
        let timeout_record = Arc::clone(&timed_out);
        let deadline = Instant::now() + self.timeout;
        let watchdog = thread::spawn(move || {
            while !finished.load(Ordering::Acquire) {
                if token.is_cancelled() || Instant::now() >= deadline {
                    if !token.is_cancelled() {
                        timeout_record.store(true, Ordering::Release);
                    }
                    if let Ok(mut process) = watched.lock() {
                        let _ = process.kill();
                    }
                    break;
                }
                thread::sleep(Duration::from_millis(20));
            }
        });
        let mut pending = Vec::new();
        let mut next_index = 0u32;
        let mut total = 0usize;
        let stream_result = (|| -> Result<(), String> {
            let mut bytes = [0u8; 4096];
            loop {
                let n = stdout.read(&mut bytes).map_err(|e| e.to_string())?;
                if n == 0 {
                    break;
                }
                total += n;
                if total > OUTPUT_LIMIT {
                    return Err("llama.cpp output limit exceeded".into());
                }
                pending.extend_from_slice(&bytes[..n]);
                emit_utf8(&mut pending, &mut next_index, emit)?;
            }
            if !pending.is_empty() {
                return Err("llama.cpp output ended with incomplete UTF-8".into());
            }
            Ok(())
        })();
        if stream_result.is_err() {
            if let Ok(mut process) = child.lock() {
                let _ = process.kill();
            }
        }
        let status = child
            .lock()
            .map_err(|_| "engine process lock poisoned".to_string())?
            .wait()
            .map_err(|e| e.to_string())?;
        done.store(true, Ordering::Release);
        let _ = watchdog.join();
        let diagnostic = stderr_thread.join().unwrap_or_default();
        if cancel.is_cancelled() {
            return Err("generation cancelled".into());
        }
        if timed_out.load(Ordering::Acquire) {
            return Err("generation deadline exceeded".into());
        }
        stream_result?;
        if !status.success() {
            return Err(format!(
                "llama.cpp exited unsuccessfully ({status}); stderr tail: {diagnostic}"
            ));
        }
        if next_index == 0 {
            return Err("llama.cpp completed without text output".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refuses_missing_trusted_binary_digest() {
        assert!(checked_executable(Path::new("missing"), &[0u8; 32]).is_err());
    }
    #[test]
    fn utf8_chunks_preserve_split_multibyte_codepoint() {
        let mut pending = vec![0xf0, 0x9f];
        let mut idx = 0;
        let mut chunks = vec![];
        let mut emit = |c: TokenChunk| {
            chunks.push(c.text);
            Ok(())
        };
        emit_utf8(&mut pending, &mut idx, &mut emit).unwrap();
        assert_eq!(idx, 0);
        pending.extend([0x98, 0x80]);
        emit_utf8(&mut pending, &mut idx, &mut emit).unwrap();
        assert_eq!(chunks, vec!["😀"]);
    }
}
