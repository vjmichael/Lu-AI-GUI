//! Linux developer-only L3 resilience smoke: in-flight cancellation, unload,
//! intentional termination of the exact test-owned PID, and explicit reload.
//! The required opt-in env only grants this test permission to kill its own engine.
use lu_core::inference_host::{Cancellation, EngineError, GenerationRequest, InferenceHost};
use lu_core::model_admission::ModelAdmissionManager;
use lu_core::persistent_llama_backend::{PersistentEngineConfig, PersistentLlamaBackend};
use std::{env, num::NonZeroUsize, path::PathBuf, time::Duration};

fn digest(hex: &str) -> Result<[u8; 32], String> {
    if hex.len() != 64 {
        return Err("expected 64 SHA-256 hex characters".into());
    }
    let mut bytes = [0u8; 32];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16)
            .map_err(|_| "invalid SHA-256 hex string")?;
    }
    Ok(bytes)
}

#[cfg(target_os = "linux")]
fn terminate_test_child(pid: u32) -> Result<(), String> {
    let status = std::process::Command::new("/usr/bin/kill")
        .args(["-KILL", &pid.to_string()])
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("could not terminate test-owned engine".into());
    }
    Ok(())
}
#[cfg(not(target_os = "linux"))]
fn terminate_test_child(_pid: u32) -> Result<(), String> {
    Err("resilience acceptance is Linux-only".into())
}

fn request(text: &str, max_new_tokens: u32) -> GenerationRequest {
    GenerationRequest {
        prompt: format!("<|im_start|>user\n{text}<|im_end|>\n<|im_start|>assistant\n"),
        max_new_tokens,
    }
}
fn generate_text(host: &mut InferenceHost<PersistentLlamaBackend>) -> Result<(), String> {
    let mut response = String::new();
    host.generate(
        request("Write a brief greeting.", 48),
        Cancellation::default(),
        |chunk| {
            response.push_str(&chunk.text);
            Ok(())
        },
    )
    .map_err(|e| format!("{e:?}"))?;
    if response.trim().is_empty() {
        return Err("empty response after recovery".into());
    }
    Ok(())
}

fn run() -> Result<(), String> {
    if !cfg!(target_os = "linux")
        || env::var("LU_LINUX_TEST_ALLOW_KILL").ok().as_deref() != Some("yes")
    {
        return Err("Linux test-owned process kill requires LU_LINUX_TEST_ALLOW_KILL=yes".into());
    }
    let executable = PathBuf::from(env::var("LU_SERVER_EXE").map_err(|e| e.to_string())?);
    let model = PathBuf::from(env::var("LU_TEST_GGUF").map_err(|e| e.to_string())?);
    let filename = model
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("model filename")?;
    let mut admissions = ModelAdmissionManager::new();
    admissions
        .admit(model.parent().ok_or("model directory")?, filename)
        .map_err(|e| format!("{e:?}"))?;
    let engine = PersistentLlamaBackend::new(PersistentEngineConfig {
        executable,
        expected_sha256: digest(&env::var("LU_SERVER_SHA256").map_err(|e| e.to_string())?)?,
        threads: NonZeroUsize::new(2).unwrap(),
        context_tokens: 512,
        startup_timeout: Duration::from_secs(120),
        request_timeout: Duration::from_secs(180),
        api_key: env::var("LU_SERVER_API_KEY").map_err(|e| e.to_string())?,
    })?;
    let mut host = InferenceHost::new(engine);
    host.load(admissions.current().unwrap())
        .map_err(|e| format!("{e:?}"))?;
    let first_pid = host.process_id().ok_or("missing initial engine PID")?;
    generate_text(&mut host)?;

    // Cancel *inside* the first received output callback. This is
    // deterministic in-flight cancellation, not a pre-cancelled request.
    let cancellation = Cancellation::default();
    let callback_cancellation = cancellation.clone();
    let mut saw_output = false;
    let result = host.generate(
        request("Write a long story about a wandering astronaut.", 512),
        cancellation,
        |chunk| {
            if !chunk.text.is_empty() {
                saw_output = true;
                callback_cancellation.cancel();
            }
            Ok(())
        },
    );
    if !saw_output || result != Err(EngineError::Cancelled) {
        return Err(format!(
            "in-flight cancellation failed: output={saw_output}, result={result:?}"
        ));
    }
    if host.process_id() != Some(first_pid) {
        return Err("cancellation unexpectedly replaced the engine process".into());
    }
    host.unload().map_err(|e| format!("{e:?}"))?;
    if host.process_id().is_some() {
        return Err("unload left a process handle registered".into());
    }

    // Start a fresh engine, prove it generates, then kill that *specific*
    // host-supervised PID and require a detected failure and explicit reload.
    host.load(admissions.current().unwrap())
        .map_err(|e| format!("{e:?}"))?;
    generate_text(&mut host)?;
    let doomed_pid = host.process_id().ok_or("missing second engine PID")?;
    terminate_test_child(doomed_pid)?;
    let crashed = host.generate(
        request("Reply with the word hello.", 24),
        Cancellation::default(),
        |_| Ok(()),
    );
    if crashed.is_ok() {
        return Err("terminated engine was incorrectly treated as healthy".into());
    }
    host.unload().map_err(|e| format!("{e:?}"))?;
    host.load(admissions.current().unwrap())
        .map_err(|e| format!("{e:?}"))?;
    generate_text(&mut host)?;
    host.unload().map_err(|e| format!("{e:?}"))?;
    println!(
        "LU_LINUX_PERSISTENT_GGUF_RESILIENCE_PASS cancel=inflight unload=pass crash=detected reload=pass model=gguf_smoke"
    );
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("LU_PERSISTENT_GGUF_RESILIENCE_FAIL: {error}");
        std::process::exit(1);
    }
}
