//! Real, CI-opted-in Hugging Face network test. Fixture-only, not user
//! consent, installed GGUF admission, or model semantic-inference evidence.
use lu_core::{
    model_provider_catalog as catalog,
    model_provider_staging::{ExactFilePlan,TransferError,publish_verified_part},
    model_provider_transport::{download_approved_part,TransportError},
    web_http_fetch::SecureHttpFetcher,
};
use sha2::{Digest,Sha256};
use std::{fs::{self,File},io::{Read,Write},path::PathBuf,sync::atomic::AtomicBool,time::{SystemTime,UNIX_EPOCH}};

#[test]
#[ignore="test-owned real HTTPS permission and public immutable small GGUF required"]
fn hf_p05_real_network_download_cancel_resume_publish_sha(){
    assert_eq!(std::env::var("LU_HF_P05_REAL_TEST").as_deref(),Ok("ci-test-only"));
    let curl=PathBuf::from(std::env::var("LU_HF_P05_TEST_CURL").expect("Explicit test curl path"));
    let fetcher=SecureHttpFetcher{curl_binary:&curl,ca_bundle:None};
    const REPO:&str="ggml-org/tinyopenjev-for-testing-gguf";
    let url=format!("https://huggingface.co/api/models/{REPO}?cardData=true");
    let page=fetcher.fetch_bounded(&url,512*1024).unwrap();
    assert_eq!(page.http_status,200);assert!(page.redirect_target.is_none());
    let meta:serde_json::Value=serde_json::from_slice(&page.bytes).unwrap();
    assert_eq!(meta["id"],REPO);
    let revision=meta["sha"].as_str().expect("Immutable upstream revision");
    assert!(catalog::valid_sha40(revision));
    let info=fetcher.fetch_bounded(
        &catalog::model_info_url(REPO,revision).unwrap(),512*1024).unwrap();
    assert_eq!(info.http_status,200);assert!(info.redirect_target.is_none());
    let license=catalog::parse_repo_license(&info.bytes,REPO,revision).unwrap();
    let listing=fetcher.fetch_bounded(
        &catalog::tree_url(REPO,revision).unwrap(),768*1024).unwrap();
    assert_eq!(listing.http_status,200);assert!(listing.redirect_target.is_none());
    let review=catalog::parse_tree(&listing.bytes,REPO,revision,&license).unwrap();
    let file=review.files.iter().filter(|f|f.bytes<=90*1024*1024)
        .min_by_key(|f|f.bytes).expect("Small test GGUF missing");
    let plan=ExactFilePlan{
        repo_id:REPO.into(),revision:revision.into(),filename:file.name.clone(),
        expected_bytes:file.bytes,sha256:file.sha256.clone(),
    };
    plan.validated().unwrap();
    let root=std::env::temp_dir().join(format!("lu-hf-p05-real-{}-{}",
        std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir(&root).unwrap();
    let first="hflivenonce0000001";let second="hflivenonce0000002";
    let interrupted="hflivenonce0000003";
    for id in [first,second,interrupted]{fs::create_dir(root.join(id)).unwrap();}
    let cancelled=AtomicBool::new(true);
    assert_eq!(download_approved_part(
        &curl,&plan,&root.join(first).join("received.part"),0,&cancelled,|_|{},
    ),Err(TransportError::Cancelled));
    assert!(!root.join(first).join("received.part").exists());
    let running=AtomicBool::new(false);
    let full=download_approved_part(
        &curl,&plan,&root.join(first).join("received.part"),0,&running,|_|{},
    ).expect("Real original HTTPS transfer failed");
    assert_eq!(full.http_code,200);
    let original=publish_verified_part(&root,first,&plan,&running)
        .expect("Real original SHA-256 readback failed");
    assert_eq!(original.sha256,plan.sha256);
    let prefix_size=std::cmp::min(plan.expected_bytes/3,1024*1024);
    assert!(prefix_size>0&&prefix_size<plan.expected_bytes);
    let mut input=File::open(&original.file).unwrap();
    let mut prefix=vec![0u8;prefix_size as usize];input.read_exact(&mut prefix).unwrap();
    let mut part=File::create(root.join(second).join("received.part")).unwrap();
    part.write_all(&prefix).unwrap();part.sync_all().unwrap();drop(part);
    let resumed=download_approved_part(
        &curl,&plan,&root.join(second).join("received.part"),prefix_size,&running,|_|{},
    ).expect("Real HTTP 206 exact-Content-Range resume failed");
    assert_eq!(resumed.http_code,206);assert!(resumed.was_resumed);
    let proof=publish_verified_part(&root,second,&plan,&running)
        .expect("Real resumed SHA-256 readback failed");
    assert_eq!(proof.sha256,plan.sha256);
    assert_eq!(proof.sha256,format!("{:x}",Sha256::digest(fs::read(&original.file).unwrap())));
    assert_eq!(publish_verified_part(&root,second,&plan,&running),
        Err(TransferError::ReadbackMismatch));
    // On Linux, throttle only this TEST OWNED forwarding wrapper so the
    // real HTTPS curl child is demonstrably in flight before cancellation.
    // The production Lu client remains the original independently pinned
    // executable; this wrapper is never staged or approved for users.
    #[cfg(unix)]
    {
        use std::{os::unix::fs::PermissionsExt,sync::atomic::Ordering};
        let shell_curl=curl.to_str().expect("Test curl must be UTF-8");
        assert!(!shell_curl.contains('\''));
        let throttle=root.join("ci-test-throttled-curl");
        fs::write(&throttle,format!(
            "#!/bin/sh\nexec '{}' --limit-rate 32768 \"$@\"\n",shell_curl
        )).unwrap();
        let mut permissions=fs::metadata(&throttle).unwrap().permissions();
        permissions.set_mode(0o700);fs::set_permissions(&throttle,permissions).unwrap();
        let part_path=root.join(interrupted).join("received.part");
        let midflight_cancel=AtomicBool::new(false);
        let mut witnessed_bytes=false;
        assert_eq!(download_approved_part(
            &throttle,&plan,&part_path,0,&midflight_cancel,|size|{
                if size>0 && size<plan.expected_bytes{
                    witnessed_bytes=true;
                    midflight_cancel.store(true,Ordering::Release);
                }
            }
        ),Err(TransportError::Cancelled));
        assert!(witnessed_bytes,"Only genuine live streamed bytes may trigger cancellation");
        let stopped_bytes=fs::metadata(&part_path).unwrap().len();
        assert!(stopped_bytes>0&&stopped_bytes<plan.expected_bytes);
        let fresh_process_consent=AtomicBool::new(false);
        let restarted=download_approved_part(
            &curl,&plan,&part_path,stopped_bytes,&fresh_process_consent,|_|{}
        ).expect("Fresh HTTPS 206 resume after an actually killed child must pass");
        assert_eq!(restarted.http_code,206);
        assert!(restarted.was_resumed);
        let checked=publish_verified_part(
            &root,interrupted,&plan,&fresh_process_consent
        ).expect("Independently re-read complete cancelled-then-resumed source");
        assert_eq!(checked.sha256,plan.sha256);
        println!("HF_P05_CANCEL_MID_TRANSFER_RESTART_RESUME_VERIFIED bytes_before_cancel={} http=206 sha256={}",
            stopped_bytes,checked.sha256);
    }
    println!("HF_P05_REAL_NETWORK_VERIFIED repo={} revision={} bytes={} sha256={} http=200,206 cancel_before_spawn=PASS replay=DENIED",
        REPO,revision,plan.expected_bytes,plan.sha256);
    fs::remove_dir_all(root).unwrap();
}
