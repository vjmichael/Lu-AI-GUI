//! PERF-01 P7: ignored owner-only synthetic loopback protocol probe.
//!
//! Run after independently launching a reviewed native candidate executable
//! under an OS-supervised test host, NOT as part of Lu desktop startup:
//! cargo test -p lu-core --test perf01_p7_candidate_protocol_real -- --ignored --nocapture
//!
//! Candidate process custody and port ownership cannot be proven here. This
//! probe deliberately sends only fixed synthetic messages and records no
//! prompts, user history, image bytes, shell commands or tool permissions.
use lu_core::{
    model_admission::ModelAdmissionManager,
    multimodal_saved_turn::build_structured_chat_request,
    multimodal_http::generate_chat_stream_body,
    p7_backend_evaluation::{CandidateKind,CandidateManifest},
    inference_host::ChatTurn,
};
use std::{
    env,path::PathBuf,sync::{Arc,atomic::{AtomicBool,Ordering}},
    time::{Duration,Instant},
};
use sha2::{Digest,Sha256};

fn env_required(key:&str)->String{
    env::var(key).unwrap_or_else(|_|panic!("Missing required P7 owner-only {key}"))
}
fn hex_digest(value:&str)->[u8;32]{
    assert_eq!(value.len(),64,"expected original SHA-256 hex");
    let mut out=[0u8;32];
    for (i,byte) in out.iter_mut().enumerate(){
        *byte=u8::from_str_radix(&value[i*2..i*2+2],16)
            .expect("invalid native SHA-256");
    }
    assert!(out.iter().any(|v|*v!=0),"zero SHA is not original evidence");
    out
}
fn sha_hex(value:&[u8])->String{
    value.iter().map(|b|format!("{b:02x}")).collect()
}
#[test]
#[ignore = "requires separately reviewed, pinned, OS-supervised local candidate"]
fn synthetic_bounded_sse_and_client_abort_on_original_local_candidate(){
    let kind=match env_required("LU_P7_CANDIDATE_KIND").as_str(){
        "mistral_rs"=>CandidateKind::MistralRs,
        "koboldcpp"=>CandidateKind::KoboldCpp,
        "vllm_gguf"=>CandidateKind::VllmGguf,
        _=>panic!("unknown candidate backend"),
    };
    let path=PathBuf::from(env_required("LU_P7_CANDIDATE_ENGINE"));
    let original_exe=hex_digest(&env_required("LU_P7_CANDIDATE_ENGINE_SHA256"));
    let model_dir=PathBuf::from(env_required("LU_P7_MODEL_DIR"));
    let model_name=env_required("LU_P7_MODEL_NAME");
    assert!(model_name.ends_with(".gguf"),"original GGUF basename required");
    let model_sha=hex_digest(&env_required("LU_P7_MODEL_SHA256"));
    let mut manager=ModelAdmissionManager::new();
    let model=manager.admit(&model_dir,&model_name)
        .expect("original local GGUF admission denied").clone();
    let reviewed=CandidateManifest{
        kind,executable:path,expected_executable_sha256:original_exe,
        expected_model_sha256:model_sha,
        expected_projector_sha256:None,
        operator_reviewed:true,local_only:true,
    };
    let candidate=reviewed.preflight_for_model(&model)
        .expect("candidate binary/GGUF file pin mismatch");
    let port=env_required("LU_P7_CANDIDATE_PORT").parse::<u16>()
        .expect("candidate port must be numeric");
    assert!(port!=0&&port>=1024,"synthetic test requires nonprivileged port");
    let api_key=env_required("LU_P7_CANDIDATE_API_KEY");
    assert!(api_key.len()==64&&api_key.bytes().all(|b|b.is_ascii_hexdigit()),
        "synthetic loopback protocol needs host-generated local test secret");
    let mut receipts=Vec::new();
    let cases=[
        ("text_exact","Reply with exactly the word PINE."),
        ("text_reason","Compute 19 + 23 and reply with the integer only."),
        ("code_literal","Write the exact Rust expression: 2 + 3"),
    ];
    for (case_id,prompt) in cases {
        let history=vec![ChatTurn{role:"user".into(),text:"Hello".into()},
            ChatTurn{role:"assistant".into(),text:"Hello.".into()}];
        let body=build_structured_chat_request(prompt,&history,&[],64)
            .expect("synthetic original Lu chat builder failed");
        let started=Instant::now();
        let mut first_forward_ms=None;
        let mut text=String::new();
        let mut events=0usize;
        let done=generate_chat_stream_body(
            port,&api_key,&body,Duration::from_secs(120),||false,
            |part|{
                if first_forward_ms.is_none(){
                    first_forward_ms=Some(started.elapsed().as_millis() as u64);
                }
                events=events.saturating_add(1);
                text.push_str(&part);
                if text.len()>1024*1024 {
                    return Err("unbounded candidate reply".into())
                }
                Ok(())
            },
        );
        let total_ms=started.elapsed().as_millis() as u64;
        done.expect("candidate lacks compatible bounded chunked chat SSE");
        assert!(events>0&&first_forward_ms.is_some()&&
            !text.trim().is_empty(),"empty candidate output");
        let output_sha:[u8;32]=Sha256::digest(text.as_bytes()).into();
        receipts.push(serde_json::json!({
            "caseId":case_id,"events":events,"firstForwardMs":first_forward_ms,
            "totalMs":total_ms,"outputSha256":sha_hex(&output_sha)
        }));
    }
    // This abort exercises CLIENT transport handling only; it is not proof
    // that an unowned listener killed a model worker, or that Lu can rejoin it.
    let cancel=Arc::new(AtomicBool::new(false));
    let cancelled=Arc::clone(&cancel);
    let signal=Arc::clone(&cancel);
    let body=build_structured_chat_request(
        "Write a long alphabetical sequence from A through Z.",
        &[],&[],2048
    ).unwrap();
    let aborted=generate_chat_stream_body(
        port,&api_key,&body,Duration::from_secs(120),
        ||cancelled.load(Ordering::Acquire),
        |_part|{
            signal.store(true,Ordering::Release);
            Ok(())
        }
    );
    assert!(aborted.is_err(),"client cancellation was ignored");
    println!("{}",serde_json::json!({
        "task":"PERF-01-P7",
        "mode":"OWNER_SYNTHETIC_LOOPBACK_PROTOCOL_PROBE",
        "candidateKind":kind.id(),
        "engineSha256":sha_hex(&candidate.engine_sha256()),
        "modelSha256":sha_hex(&candidate.model_sha256()),
        "sourceHead":option_env!("LU_PERF01_SOURCE_HEAD"),
        "platform":std::env::consts::OS,
        "cases":receipts,
        "syntheticClientAbortReceived":aborted.is_err(),
        "candidateProcessIdentityIndependentlyEstablished":false,
        "nativeChildReapedOrCancellationProven":false,
        "toolAuthorizationTransferred":false,
        "mmprojAccuracyProven":false,
        "productionPromotionAllowed":false
    }));
}
