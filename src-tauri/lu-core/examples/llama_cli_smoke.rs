//! Real, unattended Windows GGUF smoke using Lu-owned llama-cli adapter.
//! Invoked only on disposable CI runner with a pinned model and engine build.
use lu_core::inference_host::{Cancellation, GenerationRequest, InferenceHost};
use lu_core::llama_cli_backend::{LlamaCliBackend, LlamaCliConfig};
use lu_core::model_admission::ModelAdmissionManager;
use std::{env, num::NonZeroUsize, path::PathBuf, time::Duration};

fn parse_sha256(hex: &str) -> Result<[u8; 32], String> {
    if hex.len() != 64 {
        return Err("expected 64 hexadecimal characters".into());
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
            .map_err(|_| "invalid engine SHA-256 hex")?;
    }
    Ok(out)
}

fn run() -> Result<(), String> {
    let engine = PathBuf::from(env::var("LU_ENGINE_EXE").map_err(|e| e.to_string())?);
    let model = PathBuf::from(env::var("LU_TEST_GGUF").map_err(|e| e.to_string())?);
    let prompt_dir = PathBuf::from(env::var("LU_PRIVATE_PROMPT_DIR").map_err(|e| e.to_string())?);
    let sha = parse_sha256(&env::var("LU_ENGINE_SHA256").map_err(|e| e.to_string())?)?;
    let filename = model
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("invalid test model path")?;
    let directory = model.parent().ok_or("missing test model directory")?;
    let mut admissions = ModelAdmissionManager::new();
    admissions
        .admit(directory, filename)
        .map_err(|e| format!("{e:?}"))?;

    let backend = LlamaCliBackend::new(LlamaCliConfig {
        executable: engine,
        expected_sha256: sha,
        private_prompt_dir: prompt_dir,
        threads: NonZeroUsize::new(2).unwrap(),
        timeout: Duration::from_secs(180),
    })?;
    let mut host = InferenceHost::new(backend);
    host.load(admissions.current().unwrap())
        .map_err(|e| format!("{e:?}"))?;
    let mut answer = String::new();
    host.generate(
        GenerationRequest {
            prompt: "Give a short English greeting.".into(),
            max_new_tokens: 32,
        },
        Cancellation::default(),
        |chunk| {
            answer.push_str(&chunk.text);
            Ok(())
        },
    )
    .map_err(|e| format!("{e:?}"))?;
    host.unload().map_err(|e| format!("{e:?}"))?;
    if answer.trim().len() < 2 {
        return Err("real GGUF generation returned no useful text".into());
    }
    println!("LU_NATIVE_GGUF_INFERENCE_PASS bytes={}", answer.len());
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("LU_NATIVE_GGUF_INFERENCE_FAIL: {error}");
        std::process::exit(1);
    }
}
