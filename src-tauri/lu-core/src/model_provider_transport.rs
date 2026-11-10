//! Verified HF model-asset transport. The caller supplies ONLY an immutable,
//! operator-approved ExactFilePlan and Lu's previously SHA-pinned curl binary.
//! No renderer-supplied URL/argv/path enters this module. Cross-origin signed
//! redirects must be observed directly from the official HTTPS resolve URL;
//! every download DNS answer is independently checked and pinned to a public IP.
//! Interrupted files NEVER auto-restart, publish, register or load a GGUF.
use crate::{
    model_provider_staging::{ExactFilePlan,TransferError},
    web_http_fetch::{validated_host,trusted_resolution,SecureHttpFetcher},
};
use std::{
    fs::{self,File,OpenOptions},
    io::Read,
    net::{IpAddr,ToSocketAddrs},
    path::Path,
    process::{Command,Stdio},
    sync::atomic::{AtomicBool,Ordering},
    thread,time::Duration,
};

#[derive(Debug,Clone,PartialEq,Eq)]
pub enum TransportError{
    InvalidPlan,ClientUnavailable,MetadataUnavailable,
    RedirectUntrusted,UnsafeAddress,StageUnavailable,StageMismatch,
    Cancelled,TooLarge,CurlFailed,HttpRejected,RangeRejected,ReadbackFailed,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct TransportReceipt{
    pub bytes_on_disk:u64,
    pub http_code:u16,
    pub origin_hf_revision:String,
    pub was_resumed:bool,
    pub needs_independent_sha256:true_marker::Yes,
}
// A typed "transport complete" marker does NOT assert a correct asset digest.
// This is deliberately distinct from model_provider_staging::VerifiedTransfer.
mod true_marker{
    #[derive(Debug,Clone,PartialEq,Eq)]
    pub struct Yes;
}
pub(crate) fn source_url(plan:&ExactFilePlan)->Result<String,TransportError>{
    plan.validated().map_err(|_|TransportError::InvalidPlan)?;
    Ok(format!("https://huggingface.co/{}/resolve/{}/{}",
        plan.repo_id,plan.revision,plan.filename))
}
fn signed_redirect(
    curl:&Path,plan:&ExactFilePlan
)->Result<String,TransportError>{
    let url=source_url(plan)?;
    let fetcher=SecureHttpFetcher{curl_binary:curl,ca_bundle:None};
    let page=fetcher.fetch_bounded(&url,65536)
        .map_err(|_|TransportError::MetadataUnavailable)?;
    if page.canonical_url!=url ||
        !matches!(page.http_status,302|303|307|308){
        return Err(TransportError::MetadataUnavailable)
    }
    let redirect=page.redirect_target.ok_or(TransportError::RedirectUntrusted)?;
    if redirect.len()>2048||redirect.contains('#')||redirect.contains('\\'){
        return Err(TransportError::RedirectUntrusted)
    }
    let (_,normalized)=validated_host(&redirect)
        .map_err(|_|TransportError::RedirectUntrusted)?;
    if normalized!=redirect{return Err(TransportError::RedirectUntrusted)}
    Ok(redirect)
}
pub(crate) fn parsed_http_receipt(
    marker:&str,offset:u64,total:u64
)->Result<u16,TransportError>{
    let line=marker.trim().strip_prefix("LU_HF_HTTP_V1:")
        .ok_or(TransportError::CurlFailed)?;
    let (code,range)=line.split_once('|').ok_or(TransportError::CurlFailed)?;
    let code=code.parse::<u16>().map_err(|_|TransportError::CurlFailed)?;
    if offset==0{
        if code!=200{return Err(TransportError::HttpRejected)}
        if !range.trim().is_empty(){return Err(TransportError::RangeRejected)}
    }else{
        if code!=206{return Err(TransportError::RangeRejected)}
        let expected=format!("bytes {}-{}/{}",offset,total-1,total);
        if range.trim()!=expected{return Err(TransportError::RangeRejected)}
    }
    Ok(code)
}
fn check_regular(path:&Path,expected_len:u64)->Result<(),TransportError>{
    let info=fs::symlink_metadata(path).map_err(|_|TransportError::StageUnavailable)?;
    if !info.file_type().is_file()||info.len()!=expected_len{
        return Err(TransportError::StageMismatch)
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if info.file_attributes()&0x400!=0{return Err(TransportError::StageMismatch)}
    }
    Ok(())
}
fn public_target(url:&str)->Result<(String,String,std::net::Ipv4Addr),TransportError>{
    let (host,canonical)=validated_host(url).map_err(|_|TransportError::RedirectUntrusted)?;
    let all:Vec<IpAddr>=(host.as_str(),443).to_socket_addrs()
        .map_err(|_|TransportError::UnsafeAddress)?
        .map(|sock|sock.ip()).collect();
    let ip=trusted_resolution(all).map_err(|_|TransportError::UnsafeAddress)?;
    Ok((host,canonical,ip))
}
/// A network operation starts only after a separately witnessed human review.
/// The file handle is opened by Rust BEFORE curl; stdout goes to that pinned
/// handle, never to a curl-controlled filesystem path. On resume the exact
/// existing .part size is checked and the server MUST answer with a matching
/// 206 Content-Range (not 200). Final digest/publication is a separate step.
pub fn download_approved_part(
    pinned_curl:&Path,plan:&ExactFilePlan,part:&Path,offset:u64,
    cancel:&AtomicBool,mut progress:impl FnMut(u64),
)->Result<TransportReceipt,TransportError>{
    plan.validated().map_err(|_|TransportError::InvalidPlan)?;
    if !pinned_curl.is_absolute() ||
       !fs::symlink_metadata(pinned_curl).is_ok_and(|m|m.file_type().is_file()){
        return Err(TransportError::ClientUnavailable)
    }
    if offset>=plan.expected_bytes{return Err(TransportError::StageMismatch)}
    if cancel.load(Ordering::Acquire){return Err(TransportError::Cancelled)}
    let parent=part.parent().ok_or(TransportError::StageUnavailable)?;
    let parent_info=fs::symlink_metadata(parent).map_err(|_|TransportError::StageUnavailable)?;
    if !parent_info.file_type().is_dir(){return Err(TransportError::StageUnavailable)}
    // Obtain a short-lived signed asset redirect. The link is issued by the
    // independently pinned official HF HTTPS response, not by the model.
    let target=signed_redirect(pinned_curl,plan)?;
    let (host,canonical,ip)=public_target(&target)?;
    if cancel.load(Ordering::Acquire){return Err(TransportError::Cancelled)}
    let output=if offset==0{
        OpenOptions::new().write(true).create_new(true).open(part)
            .map_err(|_|TransportError::StageUnavailable)?
    }else{
        check_regular(part,offset)?;
        OpenOptions::new().append(true).open(part)
            .map_err(|_|TransportError::StageUnavailable)?
    };
    // Pre-opened OS handle means the worker never sees a writable path.
    let stdout=Stdio::from(output);
    let mut cmd=Command::new(pinned_curl);
    cmd.env_clear().args([
        "-q","--silent","--no-progress-meter","--noproxy","*",
        "--globoff","--proto","=https","--proto-redir","=https",
        "--max-redirs","0","--ipv4","--http1.1",
        "--connect-timeout","8","--speed-time","40","--speed-limit","1024",
        "--max-time","43200",
        "--output","-",
        "--write-out","%{stderr}LU_HF_HTTP_V1:%{http_code}|%header{content-range}\n",
    ]);
    cmd.arg("--max-filesize").arg(plan.expected_bytes.to_string());
    cmd.arg("--resolve").arg(format!("{host}:443:{ip}"));
    if offset>0{cmd.arg("--range").arg(format!("{offset}-"));}
    cmd.arg("--").arg(&canonical);
    let mut child=cmd.stdout(stdout).stderr(Stdio::piped()).stdin(Stdio::null())
        .spawn().map_err(|_|TransportError::ClientUnavailable)?;
    let status=loop{
        if cancel.load(Ordering::Acquire){
            let _=child.kill();let _=child.wait();
            return Err(TransportError::Cancelled)
        }
        let size=fs::metadata(part).map(|m|m.len()).unwrap_or(0);
        progress(size);
        if size>plan.expected_bytes{
            let _=child.kill();let _=child.wait();
            return Err(TransportError::TooLarge)
        }
        match child.try_wait(){
            Ok(Some(exit))=>break exit,
            Ok(None)=>thread::sleep(Duration::from_millis(150)),
            Err(_)=>{let _=child.kill();let _=child.wait();return Err(TransportError::CurlFailed)},
        }
    };
    let mut output=Vec::new();
    let mut stderr=child.stderr.take().ok_or(TransportError::CurlFailed)?;
    stderr.take(4097).read_to_end(&mut output)
        .map_err(|_|TransportError::CurlFailed)?;
    if output.len()>4096||!status.success(){return Err(TransportError::CurlFailed)}
    let marker=std::str::from_utf8(&output).map_err(|_|TransportError::CurlFailed)?;
    let code=parsed_http_receipt(marker,offset,plan.expected_bytes)?;
    check_regular(part,plan.expected_bytes)?;
    Ok(TransportReceipt{
        bytes_on_disk:plan.expected_bytes,http_code:code,
        origin_hf_revision:plan.revision.clone(),was_resumed:offset>0,
        needs_independent_sha256:true_marker::Yes,
    })
}
#[cfg(test)]
mod tests{
    use super::*;
    fn plan()->ExactFilePlan{
        ExactFilePlan{
            repo_id:"model-org/model-name".into(),
            revision:"0123456789abcdef0123456789abcdef01234567".into(),
            filename:"model-q4.gguf".into(),expected_bytes:1024,
            sha256:"0".repeat(64),
        }
    }
    #[test]
    fn source_is_immutable_and_never_model_selected(){
        let x=plan();
        assert_eq!(source_url(&x).unwrap(),
            "https://huggingface.co/model-org/model-name/resolve/0123456789abcdef0123456789abcdef01234567/model-q4.gguf");
        let mut bad=x.clone();bad.filename="../escape.gguf".into();
        assert_eq!(source_url(&bad),Err(TransportError::InvalidPlan));
    }
    #[test]
    fn resume_rejects_200_and_inconsistent_content_ranges(){
        assert_eq!(parsed_http_receipt("LU_HF_HTTP_V1:200|",0,1024),Ok(200));
        assert_eq!(parsed_http_receipt("LU_HF_HTTP_V1:206|bytes 256-1023/1024",256,1024),Ok(206));
        for output in ["LU_HF_HTTP_V1:200|","LU_HF_HTTP_V1:206|bytes 1-1023/1024",
            "LU_HF_HTTP_V1:206|bytes 256-1023/1000","LU_HF_HTTP_V1:302|"]{
            assert!(parsed_http_receipt(output,256,1024).is_err());
        }
        assert_eq!(parsed_http_receipt("LU_HF_HTTP_V1:403|",0,1024),Err(TransportError::HttpRejected));
    }
    #[test]
    fn no_network_without_pinned_binary_or_user_cancellable_scope(){
        let p=plan();
        assert_eq!(download_approved_part(
            Path::new("curl"),&p,Path::new("any.part"),0,
            &AtomicBool::new(false),|_|{}),
            Err(TransportError::ClientUnavailable));
    }
}