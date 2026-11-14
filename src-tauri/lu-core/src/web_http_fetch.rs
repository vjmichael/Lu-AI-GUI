//! TOOLS-10A: security-constrained HTTPS retrieval.
//! This host-only adapter calls a host-pinned curl executable, never a shell.
//! It does not grant the model a URL, authority, CA path or network access.
use std::{
    net::{IpAddr, Ipv4Addr, ToSocketAddrs},
    path::Path,
    io::Read,
    process::{Command,Stdio},
    sync::atomic::{AtomicBool,Ordering},
    thread,
    time::{Duration,Instant},
};
use sha2::{Digest, Sha256};

const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_HOST_LEN: usize = 253;
#[derive(Debug, PartialEq, Eq)]
pub enum FetchError {
    InvalidUrl,
    UnsafeAddress,
    ResolverFailed,
    BinaryUnavailable,
    TransportFailed,
    ResponseTooLarge,
    InvalidResponse,
    Cancelled,
}
#[derive(Debug)]
pub struct FetchedPage {
    pub canonical_url: String,
    pub http_status: u16,
    pub bytes: Vec<u8>,
    pub sha256: String,
    /// Trusted curl's parsed response redirect target; never auto-followed.
    pub redirect_target:Option<String>,
}
#[derive(Debug)]
pub struct RedirectHop{
    pub url:String,
    pub status:u16,
    pub response_sha256:String,
}
#[derive(Debug)]
pub struct RetrievedRedirectChain{
    pub hops:Vec<RedirectHop>,
    pub final_page:FetchedPage,
    pub chain_sha256:String,
    pub catalog_url:String,
    pub redirected:bool,
}
fn public_ipv4(v: Ipv4Addr) -> bool {
    let [a,b,c,d] = v.octets();
    if a == 0 || a == 10 || a == 127 || a >= 224
        || (a == 100 && (64..=127).contains(&b))
        || (a == 169 && b == 254)
        || (a == 172 && (16..=31).contains(&b))
        || (a == 192 && (b == 0 || b == 2 || b == 168))
        || (a == 198 && (b == 18 || b == 19 || (b == 51 && c == 100)))
        || (a == 192 && b == 88 && c == 99)
        || (a == 192 && b == 0 && c == 0)
        || (a == 203 && b == 0 && c == 113)
        || (a == 255 && b == 255 && c == 255 && d == 255)
    { return false; }
    true
}
fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => public_ipv4(v),
        IpAddr::V6(v) => {
            let seg = v.segments();
            !v.is_unspecified() && !v.is_loopback() && !v.is_multicast()
                && (seg[0] & 0xfe00) != 0xfc00
                && (seg[0] & 0xffc0) != 0xfe80
                && !(seg[0] == 0x2001 && seg[1] == 0xdb8)
                && v.to_ipv4_mapped().is_none()
        },
    }
}
fn validated_host(url: &str) -> Result<(String,String),FetchError> {
    if !url.starts_with("https://") || url.len()>2048 || url.chars().any(char::is_control) {
        return Err(FetchError::InvalidUrl);
    }
    let remainder=&url[8..];
    let end=remainder.find(|c| matches!(c, '/' | '?' | '#')).unwrap_or(remainder.len());
    let authority=&remainder[..end];
    if authority.is_empty() || authority.len()>MAX_HOST_LEN || authority.chars().any(|c| matches!(c, ':' | '@' | '[' | ']' | '%' | '\\')) {
        return Err(FetchError::InvalidUrl);
    }
    let host=authority.to_ascii_lowercase();
    if host == "localhost" || host.ends_with(".localhost") || host.ends_with(".local")
        || host.ends_with(".internal") || host.ends_with(".test")
        || host.parse::<IpAddr>().is_ok() || host.starts_with('.') || host.ends_with('.')
        || host.split('.').count()<2 || host.split('.').any(|label|
            label.is_empty() || label.len()>63 || label.starts_with('-') || label.ends_with('-')
            || !label.bytes().all(|c| c.is_ascii_alphanumeric() || c==b'-')
        )
    { return Err(FetchError::InvalidUrl); }
    Ok((host.clone(),format!("https://{}{}",host,&remainder[end..])))
}
fn trusted_resolution(addresses: impl IntoIterator<Item=IpAddr>) -> Result<Ipv4Addr,FetchError> {
    let addresses:Vec<_>=addresses.into_iter().collect();
    if addresses.is_empty() || addresses.iter().any(|ip| !public_ip(*ip)) {
        return Err(FetchError::UnsafeAddress);
    }
    addresses.iter().find_map(|ip| match ip { IpAddr::V4(v)=>Some(*v), _=>None })
        .ok_or(FetchError::UnsafeAddress)
}
pub struct SecureHttpFetcher<'a> {
    /// Fixed host-approved absolute executable. Must be installed/bundled by
    /// Lu and identity-checked before this adapter is admitted.
    pub curl_binary: &'a Path,
    /// Host-approved trusted enterprise/dev CA bundle, not supplied by model.
    pub ca_bundle: Option<&'a Path>,
}
impl SecureHttpFetcher<'_> {
    pub fn fetch(&self, url: &str) -> Result<FetchedPage,FetchError> {
        self.fetch_bounded(url,MAX_RESPONSE_BYTES)
    }
    /// Only trusted host adapters may request the larger research-document
    /// limit. Every response still has a hard streaming cap and no redirects.
    pub fn fetch_bounded(
        &self,url:&str,max_bytes:usize
    )->Result<FetchedPage,FetchError>{
        self.fetch_admitted(url,max_bytes,None)
    }
    /// A host-held cancellation flag can immediately terminate a hung
    /// in-flight curl, unlike a database-only cancel marker. This flag never
    /// comes from model arguments.
    pub fn fetch_cancellable(
        &self,url:&str,cancel:&AtomicBool
    )->Result<FetchedPage,FetchError>{
        self.fetch_bounded_cancellable(url,MAX_RESPONSE_BYTES,cancel)
    }
    pub fn fetch_bounded_cancellable(
        &self,url:&str,max_bytes:usize,cancel:&AtomicBool
    )->Result<FetchedPage,FetchError>{
        self.fetch_admitted(url,max_bytes,Some(cancel))
    }
    fn fetch_admitted(
        &self,url:&str,max_bytes:usize,cancel:Option<&AtomicBool>
    )->Result<FetchedPage,FetchError>{
        if max_bytes==0||max_bytes>12*1024*1024 {
            return Err(FetchError::ResponseTooLarge);
        }
        if cancel.is_some_and(|c|c.load(Ordering::Acquire)){
            return Err(FetchError::Cancelled);
        }
        let (host,canonical_url)=validated_host(url)?;
        let resolved:Vec<IpAddr>=(host.as_str(),443).to_socket_addrs()
            .map_err(|_|FetchError::ResolverFailed)?
            .map(|address|address.ip()).collect();
        let address=trusted_resolution(resolved)?;
        self.fetch_pinned(&canonical_url,&host,address,max_bytes,cancel)
    }
    /// Only follows independently observed HTTPS response redirects, never
    /// curl -L. Every hop repeats public DNS admission and exact IP pinning;
    /// all hop identities are hashed and returned for Research provenance.
    pub fn fetch_document_chain(
        &self,catalog_url:&str,max_bytes:usize
    )->Result<RetrievedRedirectChain,FetchError>{
        self.fetch_document_chain_admitted(catalog_url,max_bytes,None)
    }
    pub fn fetch_document_chain_cancellable(
        &self,catalog_url:&str,max_bytes:usize,cancel:&AtomicBool
    )->Result<RetrievedRedirectChain,FetchError>{
        self.fetch_document_chain_admitted(catalog_url,max_bytes,Some(cancel))
    }
    fn fetch_document_chain_admitted(
        &self,catalog_url:&str,max_bytes:usize,cancel:Option<&AtomicBool>
    )->Result<RetrievedRedirectChain,FetchError>{
        let (_,catalog)=validated_host(catalog_url)?;
        let mut current=catalog.clone();
        let mut seen=std::collections::BTreeSet::new();
        let mut hops=Vec::new();
        for _ in 0..=3usize{
            if !seen.insert(current.clone()){return Err(FetchError::InvalidResponse);}
            let page=self.fetch_admitted(&current,max_bytes,cancel)?;
            hops.push(RedirectHop{
                url:page.canonical_url.clone(),status:page.http_status,
                response_sha256:page.sha256.clone()
            });
            if page.http_status==200 {
                let mut signed=String::new();
                for hop in &hops{
                    signed.push_str(&format!(
                        "{}\n{}\n{}\n",hop.url,hop.status,hop.response_sha256
                    ));
                }
                return Ok(RetrievedRedirectChain{
                    redirected:hops.len()>1,
                    catalog_url:catalog,
                    chain_sha256:format!("{:x}",Sha256::digest(signed.as_bytes())),
                    hops,final_page:page,
                });
            }
            if !matches!(page.http_status,301|302|303|307|308){
                return Err(FetchError::InvalidResponse);
            }
            let target=page.redirect_target.ok_or(FetchError::InvalidResponse)?;
            let (_,next)=validated_host(&target)?;
            // A same-URL loop is not a verified redirect chain.
            if next==current{return Err(FetchError::InvalidResponse);}
            current=next;
        }
        Err(FetchError::InvalidResponse)
    }
    fn fetch_pinned(
        &self,url:&str,host:&str,address:Ipv4Addr,max_bytes:usize,
        cancel:Option<&AtomicBool>
    )->Result<FetchedPage,FetchError>{
        if !self.curl_binary.is_absolute() || !self.curl_binary.is_file() {
            return Err(FetchError::BinaryUnavailable);
        }
        let mut cmd=Command::new(self.curl_binary);
        // The downloaded/installed runtime may not inherit a user's proxy,
        // CA override, dynamic-loader preload, curl-specific environment or
        // network credentials. This is an explicit public-HTTPS read.
        // TLS verification uses this reviewed binary's system certificate
        // configuration or separately host-approved CA bundle.
        cmd.env_clear();
        // -q must be the first curl argument: ignore ambient .curlrc flags.
        cmd.args(["-q","--globoff","--silent","--show-error","--no-progress-meter","--ipv4",
            "--proto","=https","--proto-redir","=https","--max-redirs","0",
            "--noproxy","*","--connect-timeout","8","--max-time","25",
            "--request","GET","--output","-",
            "--write-out","\n__LU_CURL_META_V1__%{http_code}|%{redirect_url}|__END__"]);
        cmd.arg("--max-filesize").arg(max_bytes.to_string());
        cmd.arg("--resolve").arg(format!("{host}:443:{address}"));
        if let Some(ca)=self.ca_bundle {
            if !ca.is_absolute() || !ca.is_file() {
                return Err(FetchError::InvalidUrl);
            }
            cmd.arg("--cacert").arg(ca);
        }
        cmd.arg("--").arg(url);
        // curl's --max-filesize may rely on Content-Length. Enforce a
        // host-side streaming cap too, so chunked/unknown-size responses
        // cannot exhaust Lu's memory before curl returns.
        let mut child=cmd.stdout(Stdio::piped()).stderr(Stdio::null())
            .spawn().map_err(|_| FetchError::TransportFailed)?;
        let stdout=child.stdout.take().ok_or(FetchError::TransportFailed)?;
        // A dedicated bounded reader keeps stdout from blocking the parent.
        // Host cancellation and timeout can now kill the *actual curl child*
        // instead of waiting indefinitely on read_to_end.
        const META_LIMIT:usize=4096;
        let reader=thread::spawn(move||{
            let mut output=Vec::new();
            let outcome=stdout.take((max_bytes+META_LIMIT+1) as u64)
                .read_to_end(&mut output);
            (outcome,output)
        });
        let deadline=Instant::now()+Duration::from_secs(27);
        let status=loop{
            if cancel.is_some_and(|c|c.load(Ordering::Acquire)){
                let _=child.kill();let _=child.wait();
                let _=reader.join();
                return Err(FetchError::Cancelled);
            }
            if Instant::now()>=deadline{
                let _=child.kill();let _=child.wait();
                let _=reader.join();
                return Err(FetchError::TransportFailed);
            }
            match child.try_wait(){
                Ok(Some(result))=>break result,
                Ok(None)=>thread::sleep(Duration::from_millis(20)),
                Err(_)=>{
                    let _=child.kill();let _=child.wait();
                    let _=reader.join();
                    return Err(FetchError::TransportFailed);
                },
            }
        };
        let (read_result,output)=reader.join()
            .map_err(|_|FetchError::TransportFailed)?;
        if read_result.is_err(){return Err(FetchError::TransportFailed);}
        if output.len()>max_bytes+META_LIMIT{
            return Err(FetchError::ResponseTooLarge);
        }
        if !status.success(){return Err(FetchError::TransportFailed);}
        const MARKER:&[u8]=b"\n__LU_CURL_META_V1__";
        const END:&[u8]=b"|__END__";
        let start=output.windows(MARKER.len())
            .rposition(|x|x==MARKER).ok_or(FetchError::InvalidResponse)?;
        let meta=&output[start+MARKER.len()..];
        if !meta.ends_with(END) || meta.len()>2100 {
            return Err(FetchError::InvalidResponse);
        }
        let actual=&meta[..meta.len()-END.len()];
        let separator=actual.iter().position(|b|*b==b'|')
            .ok_or(FetchError::InvalidResponse)?;
        let http_status=std::str::from_utf8(&actual[..separator])
            .ok().and_then(|x|x.parse::<u16>().ok())
            .ok_or(FetchError::InvalidResponse)?;
        let redirect_raw=&actual[separator+1..];
        let redirect_target=if redirect_raw.is_empty(){None}else{
            let candidate=std::str::from_utf8(redirect_raw)
                .map_err(|_|FetchError::InvalidResponse)?;
            if candidate.contains('|')||candidate.chars().any(char::is_control)
                ||candidate.len()>2048{return Err(FetchError::InvalidResponse);}
            Some(candidate.to_owned())
        };
        if start>max_bytes {return Err(FetchError::ResponseTooLarge);}
        let bytes=output[..start].to_vec();
        let sha256=format!("{:x}",Sha256::digest(&bytes));
        Ok(FetchedPage {
            canonical_url:url.into(),http_status,bytes,sha256,redirect_target
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reject_local_unsafe_urls_and_credentials() {
        for url in ["http://example.org","https://127.0.0.1","https://localhost",
                    "https://example.org@127.0.0.1","https://[::1]/",
                    "https://example.org:8443","https://foo.local/",
                    "https://example.org\\@evil.test"] {
            assert!(validated_host(url).is_err(), "{url}");
        }
        assert!(validated_host("https://example.org/path?q=1").is_ok());
    }
    #[test]
    fn the_large_document_fetch_remains_host_bounded_and_https_only(){
        let fetcher=SecureHttpFetcher{curl_binary:Path::new("/nonexistent"),ca_bundle:None};
        assert_eq!(fetcher.fetch_bounded("https://public.example/doc.pdf",0).err(),
            Some(FetchError::ResponseTooLarge));
        assert_eq!(fetcher.fetch_bounded("https://public.example/doc.pdf",
            13*1024*1024).err(),Some(FetchError::ResponseTooLarge));
        assert_eq!(fetcher.fetch_bounded("file:///etc/passwd",1024).err(),
            Some(FetchError::InvalidUrl));
    }
    #[test]
    fn reject_reserved_documentation_and_local_network_addresses() {
        for address in ["192.0.2.5","198.51.100.8","203.0.113.24","169.254.3.1",
                        "172.19.1.2","192.168.1.9","127.0.0.1"] {
            let address=address.parse::<IpAddr>().unwrap();
            assert_eq!(trusted_resolution([address]),Err(FetchError::UnsafeAddress));
        }
    }
    #[test]
    fn https_redirects_are_never_permitted_to_resolve_private_targets(){
        assert!(validated_host("https://publisher.example/article").is_ok());
        assert!(validated_host("http://publisher.example/article").is_err());
        assert!(validated_host("https://localhost/private").is_err());
        assert!(validated_host("https://alice@publisher.example/private").is_err());
    }
    #[test]
    fn reject_private_and_mixed_dns() {
        assert_eq!(trusted_resolution([IpAddr::V4(Ipv4Addr::new(10,0,0,1))]),
                   Err(FetchError::UnsafeAddress));
        assert_eq!(trusted_resolution([IpAddr::V4(Ipv4Addr::new(8,8,8,8)),
                   IpAddr::V4(Ipv4Addr::new(127,0,0,1))]),Err(FetchError::UnsafeAddress));
        assert!(trusted_resolution([IpAddr::V4(Ipv4Addr::new(8,8,8,8))]).is_ok());
    }
}
