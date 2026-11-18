//! P03 model-provider discovery: untrusted public metadata is rendered as
//! operator-review candidates only. No paths, credentials, download grants,
//! engine selection, license acceptance or executable authority are produced.
use serde::{Deserialize,Serialize};
use serde_json::Value;

pub const PROVIDER_ROOT:&str="https://huggingface.co";
const MAX_SEARCH:usize=96;
const MAX_METADATA:usize=1024*1024;
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum ProviderCatalogError{
    InvalidQuery,InvalidRepository,InvalidRevision,InvalidResponse,
    PrivateOrGated,Unlicensed,NoVerifiedFile,TooManyResults,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize)]
#[serde(rename_all="camelCase")]
pub struct ModelLead{
    pub repo_id:String,
    pub revision:String,
    pub license_label:String,
    pub requires_review:bool,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize)]
#[serde(rename_all="camelCase")]
pub struct ModelFileLead{
    pub name:String,
    pub bytes:u64,
    pub sha256:String,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize)]
#[serde(rename_all="camelCase")]
pub struct ModelRepoReview{
    pub repo_id:String,
    pub revision:String,
    pub license_label:String,
    pub files:Vec<ModelFileLead>,
    pub download_available:bool,
    pub source_url:String,
}
fn strict_segment(s:&str)->bool{
    !s.is_empty() && s.len()<=96 && !s.starts_with('.') &&
    !s.ends_with('.') && !s.contains("..") &&
    s.bytes().all(|c|c.is_ascii_alphanumeric()||
        matches!(c,b'-'|b'_'|b'.'))
}
pub fn valid_repo(s:&str)->bool{
    let mut p=s.split('/');
    let a=p.next().unwrap_or("");
    let b=p.next().unwrap_or("");
    p.next().is_none()&&strict_segment(a)&&strict_segment(b)
}
pub fn valid_sha40(s:&str)->bool{
    s.len()==40 && s.bytes().all(|b|b.is_ascii_hexdigit())
}
fn valid_digest(s:&str)->bool{
    s.len()==64&&s.bytes().all(|b|b.is_ascii_hexdigit())
}
fn safe_license(label:&str)->bool{
    !label.is_empty()&&label.len()<=80&&
    label.bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'-'|b'_'|b'.'))
}
fn valid_name(s:&str)->bool{
    s.len()>5&&s.len()<=200&&!s.starts_with('.')&&!s.contains("..")&&
    s.to_ascii_lowercase().ends_with(".gguf") &&
    s.bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'-'|b'_'|b'.'))
}
fn is_restricted(v:&Value)->bool{
    v.get("private").and_then(Value::as_bool).unwrap_or(false)||
    v.get("gated").is_some_and(|g|g!=&Value::Bool(false)&&!g.is_null())
}
fn read_license(v:&Value)->Option<String>{
    let label=v.pointer("/cardData/license")
        .or_else(||v.pointer("/card_data/license"))
        .and_then(Value::as_str)?;
    if safe_license(label){Some(label.to_owned())}else{None}
}
pub fn search_url(query:&str)->Result<String,ProviderCatalogError>{
    let q=query.trim();
    if q.is_empty()||q.len()>MAX_SEARCH||
        !q.bytes().all(|b|b.is_ascii_alphanumeric()||
            matches!(b,b' '|b'-'|b'_'|b'.')){
        return Err(ProviderCatalogError::InvalidQuery)
    }
    let encoded=q.split_whitespace().collect::<Vec<_>>().join("%20");
    Ok(format!("{PROVIDER_ROOT}/api/models?search={encoded}&limit=12&full=true&cardData=true"))
}
pub fn parse_search(raw:&[u8])->Result<Vec<ModelLead>,ProviderCatalogError>{
    if raw.len()>MAX_METADATA{return Err(ProviderCatalogError::InvalidResponse)}
    let v:Value=serde_json::from_slice(raw)
        .map_err(|_|ProviderCatalogError::InvalidResponse)?;
    let arr=v.as_array().ok_or(ProviderCatalogError::InvalidResponse)?;
    if arr.len()>12{return Err(ProviderCatalogError::TooManyResults)}
    let mut out=Vec::new();
    for item in arr{
        let Some(id)=item.get("id").and_then(Value::as_str)else{continue};
        let Some(revision)=item.get("sha").and_then(Value::as_str)else{continue};
        if !valid_repo(id)||!valid_sha40(revision)||is_restricted(item){continue}
        let license=read_license(item).unwrap_or_else(||"unknown".into());
        out.push(ModelLead{
            repo_id:id.into(),revision:revision.to_ascii_lowercase(),
            requires_review:true,license_label:license,
        });
    }
    Ok(out)
}
pub fn model_info_url(repo_id:&str,sha:&str)->Result<String,ProviderCatalogError>{
    if !valid_repo(repo_id){return Err(ProviderCatalogError::InvalidRepository)}
    if !valid_sha40(sha){return Err(ProviderCatalogError::InvalidRevision)}
    Ok(format!("{PROVIDER_ROOT}/api/models/{repo_id}/revision/{sha}?cardData=true"))
}
pub fn tree_url(repo_id:&str,sha:&str)->Result<String,ProviderCatalogError>{
    if !valid_repo(repo_id){return Err(ProviderCatalogError::InvalidRepository)}
    if !valid_sha40(sha){return Err(ProviderCatalogError::InvalidRevision)}
    Ok(format!("{PROVIDER_ROOT}/api/models/{repo_id}/tree/{sha}?recursive=false"))
}
pub fn parse_repo_license(
    raw:&[u8],repo_id:&str,revision:&str
)->Result<String,ProviderCatalogError>{
    if raw.len()>MAX_METADATA||!valid_repo(repo_id)||!valid_sha40(revision){
        return Err(ProviderCatalogError::InvalidResponse)
    }
    let v:Value=serde_json::from_slice(raw)
        .map_err(|_|ProviderCatalogError::InvalidResponse)?;
    if v.get("id").and_then(Value::as_str)!=Some(repo_id) ||
       v.get("sha").and_then(Value::as_str)!=Some(revision) ||
       is_restricted(&v){
        return Err(ProviderCatalogError::PrivateOrGated)
    }
    read_license(&v).ok_or(ProviderCatalogError::Unlicensed)
}
pub fn parse_tree(
    raw:&[u8],repo_id:&str,revision:&str,license:&str
)->Result<ModelRepoReview,ProviderCatalogError>{
    if !valid_repo(repo_id)||!valid_sha40(revision)||!safe_license(license) ||
       raw.len()>MAX_METADATA{return Err(ProviderCatalogError::InvalidResponse)}
    let v:Value=serde_json::from_slice(raw)
        .map_err(|_|ProviderCatalogError::InvalidResponse)?;
    let arr=v.as_array().ok_or(ProviderCatalogError::InvalidResponse)?;
    if arr.len()>512{return Err(ProviderCatalogError::TooManyResults)}
    let mut files=Vec::new();
    for item in arr{
        if item.get("type").and_then(Value::as_str)!=Some("file"){continue}
        let Some(name)=item.get("path").and_then(Value::as_str)else{continue};
        if !valid_name(name){continue}
        let Some(size)=item.get("size").and_then(Value::as_u64)else{continue};
        let digest=item.pointer("/lfs/sha256").and_then(Value::as_str)
            .or_else(||item.pointer("/lfs/oid").and_then(Value::as_str));
        let Some(digest)=digest else{continue};
        if !(24..=128*1024*1024*1024u64).contains(&size) || !valid_digest(digest) {
            continue
        }
        files.push(ModelFileLead{
            name:name.into(),bytes:size,sha256:digest.to_ascii_lowercase(),
        });
        if files.len()==40{break}
    }
    files.sort_by(|a,b|a.name.cmp(&b.name));
    if files.is_empty(){return Err(ProviderCatalogError::NoVerifiedFile)}
    Ok(ModelRepoReview{
        repo_id:repo_id.into(),revision:revision.into(),
        license_label:license.into(),files,download_available:false,
        source_url:format!("{PROVIDER_ROOT}/{repo_id}/tree/{revision}"),
    })
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test] fn denies_query_injection_and_unsafe_repo(){
        assert_eq!(search_url("qwen 3").unwrap(),
            "https://huggingface.co/api/models?search=qwen%203&limit=12&full=true&cardData=true");
        for q in ["","x&private=true","../secret","x#fragment","üñicode"]{
            assert!(search_url(q).is_err());
        }
        for r in ["x/../z","a/b/c","a/b?q=1","a/%2fb","a/.secret"]{
            assert!(!valid_repo(r))
        }
    }
    #[test] fn filters_unverified_private_and_mutable_search_leads(){
        let raw=format!(r#"[
          {{"id":"open/one","sha":"{}","cardData":{{"license":"apache-2.0"}}}},
          {{"id":"private/one","sha":"{}","private":true}},
          {{"id":"unverified/one","sha":"main"}},
          {{"id":"gated/one","sha":"{}","gated":"manual"}}
        ]"#,"a".repeat(40),"a".repeat(40),"a".repeat(40));
        let leads=parse_search(raw.as_bytes()).unwrap();
        assert_eq!(leads.len(),1);
        assert_eq!(leads[0].license_label,"apache-2.0");
    }
    #[test] fn accepts_only_matching_pinned_license_and_lfs_sha(){
        let rev="b".repeat(40);
        let info=serde_json::json!({"id":"owner/model","sha":rev,
            "cardData":{"license":"mit"}});
        assert_eq!(parse_repo_license(info.to_string().as_bytes(),
            "owner/model",&rev).unwrap(),"mit");
        let tree=serde_json::json!([
            {"type":"file","path":"weights.gguf","size":4096,
              "lfs":{"sha256":"c".repeat(64)}},
            {"type":"file","path":"../../escape.gguf","size":4096,
              "lfs":{"sha256":"c".repeat(64)}},
            {"type":"file","path":"bad.gguf","size":4096},
            {"type":"directory","path":"other.gguf","size":4096,
              "lfs":{"sha256":"c".repeat(64)}}
        ]);
        let got=parse_tree(tree.to_string().as_bytes(),"owner/model",&rev,"mit").unwrap();
        assert_eq!(got.files.len(),1);
        assert!(!got.download_available);
        assert_eq!(got.files[0].name,"weights.gguf");
    }
}
