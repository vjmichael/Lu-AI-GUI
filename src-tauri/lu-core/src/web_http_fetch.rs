//! TOOLS-10A: security-constrained HTTPS retrieval.
//! This host-only adapter calls a host-pinned curl executable, never a shell.
//! It does not grant the model a URL, authority, CA path or network access.
use std::{
    net::{IpAddr, Ipv4Addr, ToSocketAddrs},
    path::Path,
    process::Command,
    time::Duration,
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
}
#[derive(Debug)]
pub struct FetchedPage {
    pub canonical_url: String,
    pub http_status: u16,
    pub bytes: Vec<u8>,
    pub sha256: String,
}
fn public_ipv4(v: Ipv4Addr) -> bool {
    let [a,b,c,d] = v.octets();
    if a == 0 || a == 10 || a == 127 || a >= 224
        || (a == 100 && (64..=127).contains(&b))
        || (a == 169 && b == 254)
        || (a == 172 && (16..=31).contains(&b))
        || (a == 192 && (b == 0 || b == 168))
        || (a == 198 && (b == 18 || b == 19))
        || (a == 192 && b == 88 && c == 99)
        || (a == 192 && b == 0 && c == 0)
        || (a == 198 && b == 51 && c == 100)
        || (a == 203 && b == 0 && c == 113)
        || (a == 255 && b == 255 && c == 255 && d == 255)
    { return false; }
    true
}
fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => public_ipv4(v),
        // Initial adapter is IPv4-only. Reject v6 rather than risking
        // mapped, link-local, ULA or IPv6 literal SSRF bypasses.
        IpAddr::V6(_) => false,
    }
}
fn validated_host(url: &str) -> Result<(String,String),FetchError> {
    if !url.starts_with("https://") || url.len()>2048 || url.chars().any(char::is_control) {
        return Err(FetchError::InvalidUrl);
    }
    let remainder=&url[8..];
    let end=remainder.find(['/', '?', '#']).unwrap_or(remainder.len());
    let authority=&remainder[..end];
    if authority.is_empty() || authority.len()>MAX_HOST_LEN || authority.contains([':', '@', '[', ']', '%', '\\']) {
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
    Ok((host,format!("https://{}{}",authority,&remainder[end..])))
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
        let (host,canonical_url)=validated_host(url)?;
        let resolved:Vec<IpAddr>=(host.as_str(),443).to_socket_addrs()
            .map_err(|_| FetchError::ResolverFailed)?
            .map(|address| address.ip()).collect();
        let address=trusted_resolution(resolved)?;
        self.fetch_pinned(&canonical_url,&host,address)
    }
    fn fetch_pinned(&self,url:&str,host:&str,address:Ipv4Addr)
        -> Result<FetchedPage,FetchError> {
        if !self.curl_binary.is_absolute() || !self.curl_binary.is_file() {
            return Err(FetchError::BinaryUnavailable);
        }
        let mut cmd=Command::new(self.curl_binary);
        cmd.args(["--silent","--show-error","--no-progress-meter","--ipv4",
            "--proto","=https","--proto-redir","=https","--max-redirs","0",
            "--noproxy","*","--connect-timeout","8","--max-time","25",
            "--max-filesize","1048576","--request","GET","--output","-",
            "--write-out","%{http_code}"]);
        cmd.arg("--resolve").arg(format!("{host}:443:{address}"));
        if let Some(ca)=self.ca_bundle {
            if !ca.is_absolute() || !ca.is_file() {
                return Err(FetchError::InvalidUrl);
            }
            cmd.arg("--cacert").arg(ca);
        }
        cmd.arg("--").arg(url);
        let output=cmd.output().map_err(|_| FetchError::TransportFailed)?;
        if !output.status.success() { return Err(FetchError::TransportFailed); }
        if output.stdout.len()<3 || output.stdout.len()>MAX_RESPONSE_BYTES+3 {
            return Err(FetchError::ResponseTooLarge);
        }
        let body_len=output.stdout.len()-3;
        let http_status=std::str::from_utf8(&output.stdout[body_len..])
            .ok().and_then(|x|x.parse::<u16>().ok())
            .ok_or(FetchError::InvalidResponse)?;
        let bytes=output.stdout[..body_len].to_vec();
        let sha256=format!("{:x}",Sha256::digest(&bytes));
        Ok(FetchedPage { canonical_url:url.into(),http_status,bytes,sha256 })
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
    fn reject_private_and_mixed_dns() {
        assert_eq!(trusted_resolution([IpAddr::V4(Ipv4Addr::new(10,0,0,1))]),
                   Err(FetchError::UnsafeAddress));
        assert_eq!(trusted_resolution([IpAddr::V4(Ipv4Addr::new(8,8,8,8)),
                   IpAddr::V4(Ipv4Addr::new(127,0,0,1))]),Err(FetchError::UnsafeAddress));
        assert!(trusted_resolution([IpAddr::V4(Ipv4Addr::new(8,8,8,8))]).is_ok());
    }
}
