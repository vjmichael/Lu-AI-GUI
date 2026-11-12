//! Owner-opt-in PERF-01 P5 pinned-engine comparative experiment.
//! cargo test -p lu-core --test perf01_p5_real_tuning -- --ignored --nocapture
//! Never part of normal app startup. All output is bounded digest/latency
//! evidence, NOT independent performance/quality acceptance or a GPU oracle.
use lu_core::{
    inference_host::{Cancellation,EngineError,GenerationRequest,InferenceBackend,TokenChunk},
    inference_tuning::{KvType,Speculative,TuningProfile},
    model_admission::{AdmittedModel,ModelAdmissionManager},
    persistent_llama_backend::{PersistentEngineConfig,PersistentLlamaBackend},
};
use sha2::{Digest,Sha256};
use std::{
    env,fs::File,io::Read,num::NonZeroUsize,path::{Path,PathBuf},
    time::{Duration,Instant},
};

fn digest(file:&Path)->[u8;32]{
    let mut input=File::open(file).expect("real trusted GGUF/engine is missing");
    let mut sha=Sha256::new();let mut buffer=[0u8;131072];
    loop {
        let n=input.read(&mut buffer).expect("digest failed");
        if n==0 {break}
        sha.update(&buffer[..n]);
    }
    sha.finalize().into()
}
fn decode_sha(value:&str)->[u8;32]{
    assert_eq!(value.len(),64,"expected 64 hex digest characters");
    let mut bytes=[0u8;32];
    for i in 0..32 {
        bytes[i]=u8::from_str_radix(&value[2*i..2*i+2],16)
            .expect("bad SHA-256 hex");
    }
    bytes
}
fn hex(value:&[u8])->String{
    value.iter().map(|x|format!("{x:02x}")).collect()
}
fn required(key:&str)->String{
    env::var(key).unwrap_or_else(|_|panic!("{key} is required for native acceptance"))
}
fn bounded(key:&str,default:u32,low:u32,high:u32)->u32{
    let result=match env::var(key){
        Ok(value)=>value.parse::<u32>()
            .unwrap_or_else(|_|panic!("{key} must be a valid numeric tuning value")),
        Err(env::VarError::NotPresent)=>default,
        Err(_)=>panic!("{key} cannot be decoded safely"),
    };
    assert!((low..=high).contains(&result),"{key} outside admitted bounds");
    result
}
fn verified_model(dir_key:&str,name_key:&str,sha_key:&str)->(AdmittedModel,[u8;32]){
    let dir=PathBuf::from(required(dir_key));
    let name=required(name_key);
    assert!(!name.contains('/')&&!name.contains('\\')
        &&name.ends_with(".gguf"),"GGUF name must be a basename");
    let expected=decode_sha(&required(sha_key));
    assert_eq!(digest(&dir.join(&name)),expected,"model SHA changed");
    let mut manager=ModelAdmissionManager::new();
    manager.admit(&dir,&name).expect("native GGUF admission failed");
    (manager.current().unwrap().clone(),expected)
}
#[derive(Debug,Clone)]
struct Sample{
    mode:&'static str,
    load_ms:u64,
    ttft_ms:u64,
    total_ms:u64,
    output_sha256:String,
    exact_rubric_pass:bool,
}
fn ms(t:Instant)->u64{t.elapsed().as_millis().min(u128::from(u64::MAX)) as u64}
fn pct(data:&[u64],p:usize)->u64{
    assert!(!data.is_empty());let mut s=data.to_vec();s.sort_unstable();
    s[((s.len()-1)*p+99)/100]
}
fn measure(
    backend:&mut PersistentLlamaBackend,
    prompt:&str,
    expected:&str,
)->(u64,u64,String,bool){
    let began=Instant::now();
    let mut first=None;
    let mut response=String::new();
    backend.set_trusted_cache_scope(None); // no prefetched KV bias
    backend.generate(&GenerationRequest{
        prompt:prompt.into(),history:vec![],max_new_tokens:96,
    },&Cancellation::default(),&mut |chunk:TokenChunk|->Result<(),EngineError>{
        if first.is_none(){first=Some(ms(began))}
        response.push_str(&chunk.text);
        Ok(())
    }).expect("pinned llama-server completion failed (unsupported tuning flags?)");
    let first=first.expect("no visible token emitted");
    let fingerprint=hex(&Sha256::digest(response.as_bytes()));
    (first,ms(began),fingerprint,response.trim()==expected)
}
fn mode_sample(
    engine:&Path,engine_sha:[u8;32],model:&AdmittedModel,
    api_key:&str,tuning:Option<TuningProfile>,candidate_threads:usize,
    mode:&'static str,
)->Vec<Sample>{
    let started=Instant::now();
    let mut backend=PersistentLlamaBackend::new(PersistentEngineConfig{
        executable:engine.to_path_buf(),expected_sha256:engine_sha,
        threads:NonZeroUsize::new(candidate_threads).unwrap(),
        context_tokens:4096,startup_timeout:Duration::from_secs(300),
        request_timeout:Duration::from_secs(180),
        api_key:api_key.into(),tuning,load_observer:None,
    }).expect("pinned tuning configuration rejected");
    backend.load(model).expect("admitted GGUF/engine trial failed closed");
    let load_ms=ms(started);
    // Identical deliberate warm-up, excluded from performance samples.
    let _=measure(&mut backend,
        "Reply with exactly READY if you can answer a short deterministic question.",
        "READY");
    let mut results=Vec::new();
    for (prompt,expected) in [
        ("Compute 11 + 13. Reply only with the integer.","24"),
        ("Reply with only the word RIGHT in uppercase.","RIGHT"),
        ("Reply YES or NO only: Is every square a rectangle?","YES"),
    ] {
        let (first,total,sha,rubric)=measure(&mut backend,prompt,expected);
        results.push(Sample{
            mode,load_ms,ttft_ms:first,total_ms:total,
            output_sha256:sha,exact_rubric_pass:rubric,
        });
    }
    backend.unload();
    results
}
#[test]
#[ignore = "physical owner must supply pinned native llama-server and genuine GGUF"]
fn compare_baseline_and_explicit_tuning_on_real_gguf() {
    let engine=PathBuf::from(required("LU_P5_ENGINE"));
    let engine_sha=decode_sha(&required("LU_P5_ENGINE_SHA256"));
    assert_eq!(digest(&engine),engine_sha,"engine changed during benchmark");
    let (model,model_sha)=verified_model(
        "LU_P5_MODEL_DIR","LU_P5_MODEL_NAME","LU_P5_MODEL_SHA256");
    let key=required("LU_P5_API_KEY");
    assert!(key.len()==64&&key.bytes().all(|x|x.is_ascii_hexdigit()),
        "P5 API key requires host generated 256-bit local secret");
    let mode=required("LU_P5_SPEC");
    let speculation=match mode.as_str() {
        "off"=>Speculative::Disabled,
        "ngram"=>Speculative::NgramSimple,
        "draft"=>{
            let (draft,_)=verified_model(
                "LU_P5_DRAFT_DIR","LU_P5_DRAFT_NAME","LU_P5_DRAFT_SHA256");
            Speculative::DraftSimple(draft)
        }
        _=>panic!("LU_P5_SPEC must be off, ngram or draft"),
    };
    let kv=|name:&str|match required(name).as_str(){
        "f16"=>KvType::F16,"q8_0"=>KvType::Q8_0,
        "q4_0"=>KvType::Q4_0,_=>panic!("{name} unsupported KV type")
    };
    let candidate_threads=bounded("LU_P5_THREADS",4,1,64) as usize;
    let profile=TuningProfile{
        threads_batch:NonZeroUsize::new(
            bounded("LU_P5_THREADS_BATCH",4,1,64) as usize).unwrap(),
        batch_size:bounded("LU_P5_BATCH",512,1,2048),
        ubatch_size:bounded("LU_P5_UBATCH",128,1,512),
        gpu_layers:bounded("LU_P5_GPU_LAYERS",
            if cfg!(target_os="windows"){99}else{0},0,99) as u16,
        key_cache:kv("LU_P5_KV_K"),value_cache:kv("LU_P5_KV_V"),
        speculative:speculation,
    };
    profile.validate_numbers(NonZeroUsize::new(candidate_threads).unwrap(),4096)
        .expect("P5 numerical resource checks failed");
    profile.validate_for_model(&model,false)
        .expect("draft/target tokenizer or admission mismatch");
    let trials=bounded("LU_P5_TRIALS",5,5,20) as usize;
    let mut baseline=Vec::new();
    let mut candidate=Vec::new();
    for trial in 0..trials {
        // Alternate A/B and B/A to limit (not eliminate) thermal/order bias.
        for mode in if trial%2==0 {["baseline","candidate"]}
            else{["candidate","baseline"]}{
            if mode=="baseline"{
                baseline.extend(mode_sample(&engine,engine_sha,&model,&key,
                    None,4,"baseline"));
            }else{
                candidate.extend(mode_sample(&engine,engine_sha,&model,&key,
                    Some(profile.clone()),candidate_threads,"candidate"));
            }
        }
    }
    let summarize=|group:&[Sample]|{
        let first=group.iter().map(|x|x.ttft_ms).collect::<Vec<_>>();
        let total=group.iter().map(|x|x.total_ms).collect::<Vec<_>>();
        let load=group.iter().map(|x|x.load_ms).collect::<Vec<_>>();
        serde_json::json!({
            "samples":group.len(),
            "ttft_p50_ms":pct(&first,50),"ttft_p95_ms":pct(&first,95),
            "total_p50_ms":pct(&total,50),"total_p95_ms":pct(&total,95),
            "load_p50_ms":pct(&load,50),
            "exact_rubric_passes":group.iter().filter(|x|x.exact_rubric_pass).count(),
            "exact_rubric_total":group.len(),
            "output_sha256":group.iter().map(|x|x.output_sha256.as_str()).collect::<Vec<_>>(),
            "trial_modes":group.iter().map(|x|x.mode).collect::<Vec<_>>(),
        })
    };
    println!("{}",serde_json::json!({
        "task":"PERF-01-P5",
        "evidence_scope":"REAL_PINNED_ENGINE_IF_EXECUTED_NOT_INDEPENDENTLY_ACCEPTED",
        "source_head":option_env!("LU_PERF01_SOURCE_HEAD"),
        "platform":std::env::consts::OS,
        "arch":std::env::consts::ARCH,
        "engine_sha256":hex(&engine_sha),"model_sha256":hex(&model_sha),
        "candidate_mode":profile.label(),
        "candidate_threads":candidate_threads,
        "candidate_batch":profile.batch_size,
        "candidate_ubatch":profile.ubatch_size,
        "candidate_gpu_layers":profile.gpu_layers,
        "candidate_kv_k":profile.key_cache.cli(),
        "candidate_kv_v":profile.value_cache.cli(),
        "pairs":trials,
        "baseline":summarize(&baseline),
        "candidate":summarize(&candidate),
        "quality_equivalence_verified":false,
        "backend_speculative_acceptance_count":"NOT_MEASURED",
        "vram_mib":"NOT_MEASURED",
        "energy_joules":"NOT_MEASURED",
        "installed_physical_acceptance":false,
    }));
}
