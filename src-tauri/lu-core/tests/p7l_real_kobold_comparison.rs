//! PERF-01 P7L, explicit owner-only REAL pinned GGUF A/B tests.
//! Running this ignored test can take a long time and loads weights
//! separately, but MUST never install/change default Lu backend selection.
//! cargo test -p lu-core --test p7l_real_kobold_comparison -- --ignored --nocapture
//! Tests are intentionally skipped until the actual owner supplies pinned
//! installed llama-server, KoboldCpp and original GGUF, and confirms that
//! the comparison does not confuse the baseline Vulkan offload with CPU.
use lu_core::{
    browser_model_codec::{BrowserModelCodec,BrowserModelKind},
    inference_host::{Cancellation,EngineError,GenerationRequest,InferenceBackend,TokenChunk},
    model_admission::{AdmittedModel,ModelAdmissionManager},
    p7_backend_evaluation::{CandidateKind,CandidateManifest},
    p7_koboldcpp_backend::{KoboldCppBackend,KoboldCppConfig,KoboldCppLimits},
    persistent_llama_backend::{PersistentLlamaBackend,PersistentEngineConfig},
    inference_tuning::{TuningProfile,KvType,Speculative},
};
use sha2::{Digest,Sha256};
use serde_json::{json,Value};
use std::{
    env,fs::{self,File,OpenOptions},io::{Read,Write},
    num::NonZeroUsize,path::{Path,PathBuf},
    time::{Duration,Instant},
};

fn required(name:&str)->String{
    env::var(name).unwrap_or_else(|_|panic!("{name} is mandatory"))
}
fn hash(path:&Path)->[u8;32]{
    let mut file=File::open(path).expect("owner original path unavailable");
    let mut sha=Sha256::new();
    let mut buf=[0u8;1024*1024];
    loop{
        let n=file.read(&mut buf).expect("original file changed while hashing");
        if n==0{break}
        sha.update(&buf[..n]);
    }
    sha.finalize().into()
}
fn hex(digest:&[u8;32])->String{
    digest.iter().map(|b|format!("{b:02x}")).collect()
}
fn parse_sha(input:&str)->[u8;32]{
    assert_eq!(input.len(),64,"require hex digest");
    let mut out=[0u8;32];
    for (i,val) in out.iter_mut().enumerate(){
        *val=u8::from_str_radix(&input[i*2..i*2+2],16)
            .expect("invalid SHA-256 character");
    }
    assert!(out!=[0;32],"zero digest prohibited");
    out
}
fn bounded(name:&str,default:u32,min:u32,max:u32)->u32{
    let n=match env::var(name){
        Ok(s)=>s.parse().expect("invalid owner benchmark numeric value"),
        Err(env::VarError::NotPresent)=>default,
        Err(_)=>panic!("invalid owner benchmark environment"),
    };
    assert!((min..=max).contains(&n),"{name} out of range");n
}
fn context()->(AdmittedModel,[u8;32]){
    let folder=PathBuf::from(required("LU_P7L_MODEL_DIR"));
    let name=required("LU_P7L_MODEL_NAME");
    assert!(!name.contains('/')&&!name.contains('\\')&&
        name.ends_with(".gguf"),"original GGUF basename only");
    let expected=parse_sha(&required("LU_P7L_MODEL_SHA256"));
    assert_eq!(hash(&folder.join(&name)),expected,"GGUF SHA changed");
    let mut manager=ModelAdmissionManager::new();
    let admitted=manager.admit(&folder,&name)
        .expect("original GGUF admission denied").clone();
    (admitted,expected)
}
fn owner_dir()->PathBuf{
    let dir=PathBuf::from(required("LU_P7L_EVAL_DIR"));
    assert!(dir.is_absolute()&&dir.is_dir(),"owner trusted absolute eval directory required");
    dir
}
fn kobold(
    model_sha:[u8;32],session:&str,owner:&Path,
)->KoboldCppBackend{
    let candidate_path=PathBuf::from(required("LU_P7L_KCPP_ENGINE"));
    let engine_sha=parse_sha(&required("LU_P7L_KCPP_ENGINE_SHA256"));
    assert_eq!(hash(&candidate_path),engine_sha,"candidate engine hash changed");
    let (model,_)=context();
    let manifest=CandidateManifest{
        kind:CandidateKind::KoboldCpp,executable:candidate_path,
        expected_executable_sha256:engine_sha,
        expected_model_sha256:model_sha,
        expected_projector_sha256:None,
        operator_reviewed:true,local_only:true,
    };
    let candidate=manifest.preflight_for_model(&model)
        .expect("original candidate admission preflight failed");
    let threads=bounded("LU_P7L_THREADS",4,1,64);
    KoboldCppBackend::new(KoboldCppConfig{
        candidate,
        limits:KoboldCppLimits{
            threads:NonZeroUsize::new(threads as usize).unwrap(),
            context_tokens:4096,
            startup_timeout:Duration::from_secs(360),
            request_timeout:Duration::from_secs(180),
        },
        host_secret:required("LU_P7L_KCPP_SECRET"),
        session_id:session.into(),
        journal_path:owner.join("p7l-kcpp-original-custody.sqlite"),
    }).expect("unresolved Kobold child or invalid owner-only configuration")
}
fn llama()->PersistentLlamaBackend{
    let executable=PathBuf::from(required("LU_P7L_LLAMA_ENGINE"));
    let expected=parse_sha(&required("LU_P7L_LLAMA_ENGINE_SHA256"));
    assert_eq!(hash(&executable),expected,"llama baseline engine digest changed");
    PersistentLlamaBackend::new(PersistentEngineConfig{
        executable,expected_sha256:expected,
        threads:NonZeroUsize::new(4).unwrap(),
        context_tokens:4096,
        startup_timeout:Duration::from_secs(360),
        request_timeout:Duration::from_secs(180),
        api_key:required("LU_P7L_LLAMA_SECRET"),
        // P7L-1: comparing a Windows default Vulkan-offloaded baseline with
        // a CPU-only Kobold candidate is NOT an equal-device A/B test.
        // Opt-in owner benchmark forces original llama.cpp CPU layers 0.
        // Exact pinned llama-server must support these experimental flags;
        // otherwise the native run fails instead of emitting false speedups.
        tuning:Some(TuningProfile{
            threads_batch:NonZeroUsize::new(4).unwrap(),
            batch_size:512,ubatch_size:128,
            gpu_layers:0,gpu_placement:None,
            key_cache:KvType::F16,value_cache:KvType::F16,
            speculative:Speculative::Disabled,
        }),
        load_observer:None,
    }).expect("original pinned llama baseline cannot start")
}
fn sample(
    backend:&mut dyn InferenceBackend,case_id:&str,
    prompt:&str,expected:&str,trial:usize,runner:&str,
)->Value{
    let start=Instant::now();
    let mut first:Option<u64>=None;
    let mut text=String::new();
    let mut chunks=0usize;
    backend.generate(
        &GenerationRequest{
            prompt:prompt.into(),history:vec![],max_new_tokens:96
        },&Cancellation::default(),
        &mut |chunk:TokenChunk|->Result<(),EngineError>{
            if first.is_none(){
                first=Some(start.elapsed().as_millis() as u64)
            }
            chunks=chunks.saturating_add(1);
            if text.len().saturating_add(chunk.text.len())>1024*1024{
                return Err(EngineError::InvalidRequest)
            }
            text.push_str(&chunk.text);
            Ok(())
        }
    ).expect("candidate or baseline real streamed original-GGUF test failed");
    assert!(chunks>0,"empty actual model streaming reply");
    let duration=start.elapsed().as_millis() as u64;
    assert!(first.is_some_and(|ms|ms<=duration));
    let sha:[u8;32]=Sha256::digest(text.as_bytes()).into();
    // The original exact full output is never printed or persisted.
    // These are strict three-word/expression rubrics, not reasoning parity.
    json!({
        "caseId":case_id,"runner":runner,"trial":trial,
        "firstForwardMs":first,"totalMs":duration,
        "generatedTokens":null,"decodeMs":null,"decodeTokens":null,
        "qualityPass":text.trim()==expected,
        "cancelReaped":false,"restartedSafely":false,
        "outputSha256":hex(&sha),
        "textChunkCount":chunks,
    })
}
fn cases()->[(&'static str,&'static str,&'static str);2]{
    [
        ("text_exact","Reply with only the word ALPHA.","ALPHA"),
        ("text_code","Reply with exactly this Rust expression: 2 + 3","2 + 3"),
    ]
}
#[test]
#[ignore = "requires reviewed physical engine/model identities, actual installed artifacts and OS"]
fn actual_koboldcpp_vs_original_llama_paired_gguf_evaluation(){
    let (model,model_sha)=context();
    let directory=owner_dir();
    let baseline_sha=parse_sha(&required("LU_P7L_LLAMA_ENGINE_SHA256"));
    let alternative_sha=parse_sha(&required("LU_P7L_KCPP_ENGINE_SHA256"));
    let head=required("LU_P7L_SOURCE_HEAD");
    assert!(head.len()==40&&head.bytes().all(|b|b.is_ascii_hexdigit()),
        "owner must pin exact source head");
    let hardware=required("LU_P7L_HARDWARE_ID");
    assert!(hardware.len()>=3&&hardware.len()<=80&&
        hardware.bytes().all(|b|b.is_ascii_alphanumeric()||
            matches!(b,b'.'|b'_'|b'-')),"sanitized hardware identity");
    let suite_sha:[u8;32]=Sha256::digest(
        b"P7L original fixed text_exact/text_code 4096/96 t0 2026-11-12"
    ).into();
    let trials=bounded("LU_P7L_TRIALS",5,5,20) as usize;
    let mut runs=Vec::new();
    for i in 1..=trials{
        // Alternate A/B then B/A to constrain ordering/thermal effects.
        for runner in if i%2==1{
            ["baseline","candidate"]
        }else{["candidate","baseline"]}{
            if runner=="baseline"{
                let mut engine=llama();
                engine.load(&model).expect("original GGUF baseline load failed");
                for (id,prompt,expected) in cases(){
                    runs.push(sample(&mut engine,id,prompt,expected,i,runner));
                }
                engine.unload();
            }else{
                let session=format!("p7l-{i}");
                let mut engine=kobold(model_sha,&session,&directory);
                engine.load(&model).expect("KoboldCpp original GGUF load failed");
                for (id,prompt,expected) in cases(){
                    runs.push(sample(&mut engine,id,prompt,expected,i,runner));
                }
                engine.unload();
                assert!(!engine.journal_pending()
                    .expect("missing original custody receipt"),
                    "original child not reaped; refuse further trials");
            }
        }
    }
    let report=json!({
        "schema":"lu-perf01-p7-receipts-v1",
        "sourceHead":head.to_ascii_lowercase(),
        "hardwareId":hardware,
        "modelSha256":hex(&model_sha),
        "projectorSha256":null,
        "suiteSha256":hex(&suite_sha),
        "contextTokens":4096,
        "baseline":{
            "backend":"llama_cpp","engineSha256":hex(&baseline_sha),
            "requestedGpuLayersOnWindows":0,
            "baselineRequestedBackend":"CPU",
            "baselineCpuVsKoboldCpuEquivalenceProven":false
        },
        "candidate":{
            "backend":"koboldcpp","engineSha256":hex(&alternative_sha),
            "requestedBackend":"CPU"
        },
        "samples":runs,
        "physicalOwnerAcceptance":false,
    });
    let filename=format!("p7l-original-paired-{trials}-{}.json",
        &hex(&suite_sha)[..12]);
    let destination=directory.join(filename);
    let mut out=OpenOptions::new().write(true).create_new(true)
        .open(&destination).expect("comparison receipt already exists");
    out.write_all(serde_json::to_string_pretty(&report).unwrap().as_bytes())
        .expect("cannot persist exact original timings");
    out.sync_all().expect("comparison receipt is not durable");
    println!("P7L_REAL_GGUF_SAMPLES_RECORDED_NOT_INDEPENDENTLY_ACCEPTED {}",
        destination.display());
}
#[test]
#[ignore = "requires original Kobold executable/GGUF and native OS"]
fn actual_cancellation_kills_owned_child_and_reloads_after_clean_reap(){
    let (model,sha)=context();
    let directory=owner_dir();
    let mut engine=kobold(sha,"p7l-cancel",&directory);
    engine.load(&model).expect("real original alternative did not load");
    let cancel=Cancellation::default();
    let signal=cancel.clone();
    let result=engine.generate(&GenerationRequest{
        prompt:"Write the entire alphabet repeatedly for 2048 tokens.".into(),
        history:vec![],max_new_tokens:2048,
    },&cancel,&mut |_token|{
        signal.cancel();Ok(())
    });
    assert!(result.is_err(),"cancellation must not return a successful turn");
    assert!(engine.pid().is_none(),"original child not cleaned up");
    assert!(!engine.custody_uncertain(),"native child/process group uncertain");
    assert!(!engine.journal_pending().unwrap(),"durable original custody still active");
    drop(engine);
    let mut restarted=kobold(sha,"p7l-reload",&directory);
    restarted.load(&model)
        .expect("verified reaped original cannot relaunch");
    restarted.unload();
    assert!(!restarted.journal_pending().unwrap());
}
#[test]
#[ignore = "requires real KoboldCpp read-only tool streaming and host-approved schema"]
fn actual_original_read_only_tool_sse_never_executes_commands(){
    let (model,sha)=context();
    let directory=owner_dir();
    let mut engine=kobold(sha,"p7l-tool",&directory);
    engine.load(&model).expect("real original Kobold load failed");
    let schema=BrowserModelCodec(BrowserModelKind::Read).schema();
    let tools=vec![json!({"type":"function","function":{
        "name":"lu_browser_read","description":"Read an approved public source only",
        "parameters":schema
    }})];
    let calls=engine.generate_browser_function_calls(
        &GenerationRequest{
            prompt:"Use lu_browser_read to navigate to https://example.org, then stop.".into(),
            history:vec![],max_new_tokens:256,
        },
        &tools,&Cancellation::default()
    ).expect("real candidate structured tool SSE incompatible");
    assert!(!calls.is_empty(),"candidate did not produce an original structured tool call");
    assert!(calls.iter().all(|c|c.tool_name=="lu_browser_read"));
    assert!(calls.iter().all(|c|
        serde_json::from_str::<Value>(&c.arguments_json).is_ok()));
    // NO browser commands are dispatched. This only tests model proposals.
    engine.unload();
    assert!(!engine.journal_pending().unwrap());
}
