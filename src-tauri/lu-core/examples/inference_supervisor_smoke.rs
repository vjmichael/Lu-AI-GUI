//! Standalone Windows acceptance for the host-owned P3 Protobuf supervisor.
//! Uses the real, already pinned llama-server and checksum-verified GGUF.
use lu_core::{
    inference_bridge::BridgeError,
    inference_supervisor::TrustedInferenceSupervisor,
    inference_wire::decode_frame,
    model_admission::ModelAdmissionManager,
    persistent_llama_backend::{PersistentEngineConfig, PersistentLlamaBackend},
    wire::proto,
};
use std::{env, num::NonZeroUsize, path::PathBuf, time::Duration};

fn digest(hex: &str) -> Result<[u8; 32], String> {
    if hex.len() != 64 {
        return Err("invalid engine SHA256".into());
    }
    let mut out = [0u8; 32];
    for (index, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16)
            .map_err(|_| "invalid engine checksum")?;
    }
    Ok(out)
}
fn run() -> Result<(), String> {
    let executable = PathBuf::from(env::var("LU_SERVER_EXE").map_err(|e| e.to_string())?);
    let gguf = PathBuf::from(env::var("LU_TEST_GGUF").map_err(|e| e.to_string())?);
    let sha = digest(&env::var("LU_SERVER_SHA256").map_err(|e| e.to_string())?)?;
    let api_key = env::var("LU_SERVER_API_KEY").map_err(|e| e.to_string())?;
    let mut catalog = ModelAdmissionManager::new();
    let filename = gguf
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("invalid model name")?;
    let model = catalog
        .admit(gguf.parent().ok_or("missing model directory")?, filename)
        .map_err(|e| format!("{e:?}"))?
        .clone();
    let supervisor = TrustedInferenceSupervisor::new(
        "llama-cpu".into(),
        512,
        vec![("model-1".into(), model)],
        move || {
            PersistentLlamaBackend::new(PersistentEngineConfig {
                executable: executable.clone(),
                expected_sha256: sha,
                threads: NonZeroUsize::new(2).unwrap(),
                context_tokens: 512,
                startup_timeout: Duration::from_secs(120),
                request_timeout: Duration::from_secs(180),
                api_key: api_key.clone(),
            })
        },
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(supervisor.model_ids(), vec!["model-1"]);
    let loaded = supervisor
        .load("model-1", "session-1".into(), "load-1")
        .map_err(|e| format!("{e:?}"))?;
    decode_frame(&loaded).map_err(|e| format!("{e:?}"))?;
    let initial = supervisor.status().map_err(|e| format!("{e:?}"))?;
    let pid = initial.pid.ok_or("native model worker has no process ID")?;
    let mut total_text = String::new();
    for (i, prompt) in [
        "<|im_start|>user\nSay hello.<|im_end|>\n<|im_start|>assistant\n",
        "<|im_start|>user\nSay goodbye.<|im_end|>\n<|im_start|>assistant\n",
    ]
    .iter()
    .enumerate()
    {
        let generation = format!("g-{}", i + 1);
        let mut states = 0;
        let mut pieces = 0;
        let mut terminal = 0;
        supervisor
            .reserve_generation("session-1", &generation, &generation, prompt, 32)
            .map_err(|e| format!("{e:?}"))?;
        supervisor
            .run_reserved(
                "session-1",
                &generation,
                &generation,
                (*prompt).into(),
                32,
                |frame| {
                    let decoded = decode_frame(&frame).map_err(|_| BridgeError::Consumer)?;
                    if let Some(proto::inference_frame::Body::Event(event)) = decoded.body {
                        match event.kind {
                            Some(proto::inference_event::Kind::State(_)) => states += 1,
                            Some(proto::inference_event::Kind::TextChunk(chunk)) => {
                                total_text.push_str(&chunk.text);
                                pieces += 1;
                            }
                            Some(proto::inference_event::Kind::Finished(_)) => terminal += 1,
                            _ => {}
                        }
                    }
                    Ok(())
                },
            )
            .map_err(|e| format!("{e:?}"))?;
        if states != 1 || pieces == 0 || terminal != 1 {
            return Err("missing native Protobuf lifecycle or text events".into());
        }
        if supervisor.status().map_err(|e| format!("{e:?}"))?.pid != Some(pid) {
            return Err("the persistent model process restarted between turns".into());
        }
    }
    if total_text.trim().len() < 2 {
        return Err("inference stream was empty".into());
    }
    let unloaded = supervisor
        .unload("session-1", "unload-1")
        .map_err(|e| format!("{e:?}"))?;
    decode_frame(&unloaded).map_err(|e| format!("{e:?}"))?;
    if supervisor
        .status()
        .map_err(|e| format!("{e:?}"))?
        .pid
        .is_some()
    {
        return Err("unload retained an engine process ID".into());
    }
    println!(
        "LU_P3_SUPERVISOR_GGUF_PASS pid={pid} turns=2 bytes={}",
        total_text.len()
    );
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("LU_P3_SUPERVISOR_GGUF_FAIL: {error}");
        std::process::exit(1);
    }
}
