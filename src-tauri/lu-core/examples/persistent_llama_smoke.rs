//! Real Windows validation of a resident Lu-owned GGUF process.
//! The same llama-server PID must serve two generations without reloading.
use lu_core::inference_host::{Cancellation, GenerationRequest, InferenceHost};
use lu_core::model_admission::ModelAdmissionManager;
use lu_core::persistent_llama_backend::{PersistentEngineConfig, PersistentLlamaBackend};
use std::{env, num::NonZeroUsize, path::PathBuf, time::Duration};

fn digest(hex: &str) -> Result<[u8; 32], String> {
    if hex.len() != 64 {
        return Err("expected 64 hex chars".into());
    }
    let mut data = [0u8; 32];
    for (i, byte) in data.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).map_err(|_| "invalid checksum")?;
    }
    Ok(data)
}
fn run() -> Result<(), String> {
    let exe = PathBuf::from(env::var("LU_SERVER_EXE").map_err(|e| e.to_string())?);
    let model = PathBuf::from(env::var("LU_TEST_GGUF").map_err(|e| e.to_string())?);
    let parent = model.parent().ok_or("no model directory")?;
    let filename = model
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("bad model filename")?;
    let mut admitted = ModelAdmissionManager::new();
    admitted
        .admit(parent, filename)
        .map_err(|e| format!("{e:?}"))?;
    let engine = PersistentLlamaBackend::new(PersistentEngineConfig {
        executable: exe,
        expected_sha256: digest(&env::var("LU_SERVER_SHA256").map_err(|e| e.to_string())?)?,
        threads: NonZeroUsize::new(2).unwrap(),
        context_tokens: 512,
        startup_timeout: Duration::from_secs(120),
        request_timeout: Duration::from_secs(180),
        // CI ephemeral key, never a model argument. Production uses the OS CSPRNG.
        api_key: env::var("LU_SERVER_API_KEY").map_err(|e| e.to_string())?,
    })?;
    let mut host = InferenceHost::new(engine);
    host.load(admitted.current().unwrap())
        .map_err(|e| format!("{e:?}"))?;
    let initial_pid = host.process_id().ok_or("missing supervised engine PID")?;
    let mut outputs = Vec::new();
    // SmolLM2-Instruct uses ChatML. Raw untokenized plain sentences can
    // generate an immediate EOS in the /completion API and are not a valid
    // proof that the persistent server can answer a conversation.
    for prompt in [
        "<|im_start|>user\nReply with the word hello.<|im_end|>\n<|im_start|>assistant\n",
        "<|im_start|>user\nReply with the word goodbye.<|im_end|>\n<|im_start|>assistant\n",
    ] {
        let mut text = String::new();
        host.generate(
            GenerationRequest {
                prompt: prompt.into(),
                max_new_tokens: 32,
            },
            Cancellation::default(),
            |chunk| {
                text.push_str(&chunk.text);
                Ok(())
            },
        )
        .map_err(|e| format!("{e:?}"))?;
        if text.trim().len() < 2 {
            return Err("missing real generated answer".into());
        }
        if host.process_id() != Some(initial_pid) {
            return Err("engine restarted between conversations".into());
        }
        outputs.push(text);
    }
    host.unload().map_err(|e| format!("{e:?}"))?;
    println!(
        "LU_PERSISTENT_GGUF_TWO_TURNS_PASS pid={} turns={} bytes={}",
        initial_pid,
        outputs.len(),
        outputs.iter().map(String::len).sum::<usize>()
    );
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("LU_PERSISTENT_GGUF_FAIL: {e}");
        std::process::exit(1);
    }
}
