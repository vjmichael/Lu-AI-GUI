//! PERF-01 P2 owner-only real GGUF, pinned-engine warm-context benchmark.
//! Run manually with: cargo test -p lu-core --test perf01_kv_cache_real
//!   -- --ignored --nocapture
//! No network provider and no per-commit CI. Requires explicit environment
//! model path, engine hash, random 256-bit localhost API key and original SHA.
//! NO performance threshold is asserted; distributions are evidence, not a PASS.
use lu_core::{
    inference_host::{Cancellation, ChatTurn, EngineError, GenerationRequest,
        InferenceBackend, TokenChunk},
    model_admission::ModelAdmissionManager,
    persistent_llama_backend::{PersistentEngineConfig, PersistentLlamaBackend},
};
use sha2::{Digest, Sha256};
use std::{
    env,
    fs::File,
    io::Read,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

#[derive(Clone,Copy,Debug)]
struct Sample { ttft_ms:u64, total_ms:u64, pieces:u32 }
fn digest_file(file:&Path)->[u8;32]{
    let mut reader=File::open(file).expect("benchmark model file must exist");
    let mut buffer=[0u8;131072];
    let mut sha=Sha256::new();
    loop {
        let n=reader.read(&mut buffer).expect("benchmark digest read failed");
        if n==0 {break;}
        sha.update(&buffer[..n]);
    }
    sha.finalize().into()
}
fn decode_digest(value:&str)->[u8;32]{
    assert_eq!(value.len(),64,"expected 64-character SHA-256 digest");
    let mut out=[0u8;32];
    for (i,slot) in out.iter_mut().enumerate(){
        *slot=u8::from_str_radix(&value[i*2..i*2+2],16)
            .expect("invalid hex in expected identity");
    }
    out
}
fn metric_ms(start:Instant)->u64{
    start.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
}
fn percentile(samples:&[u64], pct:usize)->u64{
    assert!(!samples.is_empty());
    let mut sorted=samples.to_vec(); sorted.sort_unstable();
    let index=((sorted.len()-1)*pct+99)/100;
    sorted[index]
}
fn measure(backend:&mut PersistentLlamaBackend,scope:&str)->Result<Sample,String>{
    // Fixed transcript and stable appended current request across all lanes.
    // Never benchmark tool execution or infer authority from these messages.
    let history=vec![
        ChatTurn{role:"user".into(),text:
            "Give a short, precise explanation of idempotence in distributed workflows.".into()},
        ChatTurn{role:"assistant".into(),text:
            "Idempotence means repeated execution has the same effect as one execution; an operation needs deduplication or a stable effect identity.".into()},
    ];
    let request=GenerationRequest{
        history,
        prompt:"Give one concrete example of an idempotent status lookup.".into(),
        max_new_tokens:128,
    };
    let started=Instant::now();
    let mut first=None;
    let mut pieces=0u32;
    backend.set_trusted_cache_scope(Some(scope));
    backend.generate(&request,&Cancellation::default(),
        &mut |_chunk:TokenChunk|->Result<(),EngineError>{
            if first.is_none(){first=Some(metric_ms(started));}
            pieces=pieces.saturating_add(1);
            Ok(())
        })?;
    let ttft=first.ok_or("backend returned no visible model chunks")?;
    Ok(Sample{ttft_ms:ttft,total_ms:metric_ms(started),pieces})
}
#[test]
#[ignore = "owner must supply genuine GGUF, engine hash, API key and hardware"]
fn real_gguf_cold_vs_eligible_warm_context() {
    let executable=PathBuf::from(env::var_os("LU_PERF01_BENCH_ENGINE")
        .expect("LU_PERF01_BENCH_ENGINE is required"));
    let expected_engine=decode_digest(&env::var("LU_PERF01_BENCH_ENGINE_SHA256")
        .expect("LU_PERF01_BENCH_ENGINE_SHA256 is required"));
    let dir=PathBuf::from(env::var_os("LU_PERF01_BENCH_MODEL_DIR")
        .expect("LU_PERF01_BENCH_MODEL_DIR is required"));
    let name=env::var("LU_PERF01_BENCH_MODEL_NAME")
        .expect("LU_PERF01_BENCH_MODEL_NAME is required");
    assert!(!name.contains('/')&&!name.contains('\\'),
        "model is a basename admitted within the host directory");
    let expected_model=decode_digest(&env::var("LU_PERF01_BENCH_MODEL_SHA256")
        .expect("LU_PERF01_BENCH_MODEL_SHA256 is required"));
    assert_eq!(digest_file(&dir.join(&name)),expected_model,
        "real model identity differs from the pinned benchmark fixture");
    let api_key=env::var("LU_PERF01_BENCH_API_KEY")
        .expect("host-generated 64-char random localhost key is required");
    assert!(api_key.len()==64 && api_key.bytes().all(|ch|ch.is_ascii_hexdigit()),
        "invalid localhost API key");
    let repetitions=env::var("LU_PERF01_BENCH_REPEATS").ok()
        .and_then(|v|v.parse::<usize>().ok()).unwrap_or(7);
    assert!((3..=20).contains(&repetitions),"use 3 to 20 repeated pairs");
    let mut catalog=ModelAdmissionManager::new();
    catalog.admit(&dir,&name).expect("original model admission failed");
    let mut backend=PersistentLlamaBackend::new(PersistentEngineConfig{
        executable,expected_sha256:expected_engine,
        threads:NonZeroUsize::new(4).unwrap(),context_tokens:4096,
        startup_timeout:Duration::from_secs(300),
        request_timeout:Duration::from_secs(180),
        api_key,tuning:None,load_observer:None,
    }).expect("engine digest/manifest is not admitted");
    backend.load(catalog.current().unwrap()).expect("real GGUF load failed");
    let original_pid=backend.process_id().expect("missing real supervised child");
    let mut cold=Vec::new();
    let mut warm=Vec::new();
    for i in 0..repetitions {
        // Each scoped cold request uses the exact same constant system prefix,
        // but a *different* identity ensures cache_prompt=false.
        let cold_scope=format!("benchmark-cold-{i}");
        let warm_scope=format!("benchmark-warm-{i}");
        let a=measure(&mut backend,&cold_scope).expect("cold request failed");
        let _seed=measure(&mut backend,&warm_scope).expect("warm seed failed");
        let b=measure(&mut backend,&warm_scope).expect("warm candidate failed");
        assert_eq!(backend.process_id(),Some(original_pid),
            "restarted engine invalidates this paired comparison");
        cold.push(a); warm.push(b);
    }
    backend.unload();
    let cold_first:Vec<_>=cold.iter().map(|s|s.ttft_ms).collect();
    let warm_first:Vec<_>=warm.iter().map(|s|s.ttft_ms).collect();
    let cold_total:Vec<_>=cold.iter().map(|s|s.total_ms).collect();
    let warm_total:Vec<_>=warm.iter().map(|s|s.total_ms).collect();
    // No prompts, output, private file paths, localhost key or unverified
    // cache-hit percentages appear in machine-readable diagnostic evidence.
    let receipt=serde_json::json!({
        "task":"PERF-01-P2-KV","evidence":"REAL_LOCAL_GGUF_IF_EXECUTED",
        "source_sha":option_env!("LU_PERF01_SOURCE_HEAD"),
        "os":std::env::consts::OS,"arch":std::env::consts::ARCH,
        "engine_sha256":expected_engine.iter().map(|b|format!("{b:02x}")).collect::<String>(),
        "model_sha256":expected_model.iter().map(|b|format!("{b:02x}")).collect::<String>(),
        "trials":repetitions,
        "context_tokens":4096,
        "cold_ttft_p50_ms":percentile(&cold_first,50),
        "cold_ttft_p95_ms":percentile(&cold_first,95),
        "warm_eligible_ttft_p50_ms":percentile(&warm_first,50),
        "warm_eligible_ttft_p95_ms":percentile(&warm_first,95),
        "cold_total_p50_ms":percentile(&cold_total,50),
        "warm_eligible_total_p50_ms":percentile(&warm_total,50),
        "cold_tokens_chunks":cold.iter().map(|s|s.pieces).collect::<Vec<_>>(),
        "warm_tokens_chunks":warm.iter().map(|s|s.pieces).collect::<Vec<_>>(),
        "actual_backend_cache_hits":"NOT_MEASURED",
        "gpu_vram":"NOT_MEASURED",
        "quality_equivalence":"NOT_VERIFIED"
    });
    println!("{}",receipt);
}
