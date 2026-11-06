//! WAVE4-RESEARCH-02 — explicit native OpenAlex discovery through a bundled
//! hash-pinned HTTPS client and the existing verified-evidence ledger.
//! Search leads remain UNVERIFIED. No model-selected URL, network tool grant,
//! arbitrary curl, browser session or research agent runtime is created.
use std::{
    fs::{self,File},io::Read,path::{Path,PathBuf},
};
use serde::Serialize;
use sha2::{Digest,Sha256};
use lu_core::{
    web_http_fetch::SecureHttpFetcher,
    web_research_evidence::{Candidate,EvidenceLedger},
    web_search_openalex::search_openalex,
};

#[derive(Serialize,Clone)]
#[serde(rename_all="camelCase")]
pub(crate) struct ResearchLeadView{
    pub source_id:String,
    pub title:String,
    pub authors:String,
    pub doi:String,
    pub catalog_url:String,
    /// Discovery is NOT a bibliographic or document citation receipt.
    pub verification_status:&'static str,
}
#[derive(Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct ResearchSearchView{
    pub query:String,
    pub provider:&'static str,
    pub observed_sha256:String,
    pub source_count:usize,
    pub leads:Vec<ResearchLeadView>,
    pub message:&'static str,
}
pub(crate) struct ResearchDesktop{
    root:PathBuf,
    curl:PathBuf,
    expected_sha256:Option<&'static str>,
}
fn safe_id(s:&str)->bool{
    s.starts_with("c-")&&s.len()<=128
        &&s.bytes().all(|c|c.is_ascii_alphanumeric()||c==b'-'||c==b'_')
}
fn valid_digest(s:&str)->bool{
    s.len()==64&&s.bytes().all(|b|b.is_ascii_hexdigit())
}
fn verified_file(path:&Path,expected:&str)->bool{
    if !valid_digest(expected)||!path.is_absolute(){return false;}
    let Ok(meta)=fs::symlink_metadata(path) else{return false;};
    if !meta.file_type().is_file()||meta.len()<16||meta.len()>30*1024*1024{
        return false;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 !=0{return false;}
    }
    let Ok(mut f)=File::open(path) else{return false;};
    let mut hash=Sha256::new();
    let mut chunk=[0u8;32*1024];
    loop {
        let Ok(n)=f.read(&mut chunk) else{return false;};
        if n==0{break;}
        hash.update(&chunk[..n]);
    }
    format!("{:x}",hash.finalize())==expected.to_ascii_lowercase()
}
impl ResearchDesktop{
    pub(crate) fn new(resources:PathBuf,app_data:PathBuf)->Self{
        #[cfg(windows)]
        let filename="curl.exe";
        #[cfg(not(windows))]
        let filename="curl";
        Self{
            root:app_data.join("lu-research-evidence"),
            curl:resources.join("lu-research").join(filename),
            expected_sha256:option_env!("LU_RESEARCH_CURL_SHA256"),
        }
    }
    pub(crate) fn available(&self)->bool{
        self.expected_sha256.is_some_and(|sha|verified_file(&self.curl,sha))
    }
    pub(crate) fn search(
        &self,chat_id:&str,query:&str
    )->Result<ResearchSearchView,String>{
        if !safe_id(chat_id)||query.trim().is_empty()||query.len()>512
            ||query.chars().any(char::is_control){
            return Err("Invalid research query or conversation".into());
        }
        // Never trust a second host-selected command or user-specified binary
        // path; reverify the same pinned curl executable before each task.
        if !self.available(){
            return Err("Reviewed Lu research HTTPS runtime not installed".into());
        }
        let fetcher=SecureHttpFetcher{
            curl_binary:&self.curl,
            ca_bundle:None,
        };
        let page=search_openalex(&fetcher,query,1)
            .map_err(|_|"OpenAlex discovery unavailable or response invalid")?;
        fs::create_dir_all(&self.root)
            .map_err(|_|"Research evidence directory unavailable")?;
        let dir=fs::symlink_metadata(&self.root)
            .map_err(|_|"Research evidence directory unavailable")?;
        if !dir.file_type().is_dir()||dir.file_type().is_symlink(){
            return Err("Research evidence directory is not trusted".into());
        }
        let mut ledger=EvidenceLedger::open(&self.root.join(format!("{chat_id}.sqlite")))
            .map_err(|_|"Research evidence ledger unavailable")?;
        let mut leads=Vec::new();
        for lead in page.leads {
            if lead.provider_id.len()>128||lead.authors.len()>512
                ||lead.title.len()>1024||lead.doi.len()>512{
                continue;
            }
            ledger.discover(&Candidate{
                id:lead.provider_id.clone(),title:lead.title.clone(),
                author:lead.authors.clone(),identifier:lead.doi.clone()
            }).map_err(|_|"Could not persist discovered source")?;
            leads.push(ResearchLeadView{
                source_id:lead.provider_id,title:lead.title,
                authors:lead.authors,doi:lead.doi,
                catalog_url:lead.catalog_url,
                verification_status:"discovered_unverified",
            });
        }
        Ok(ResearchSearchView{
            query:query.to_owned(),provider:"OpenAlex",
            observed_sha256:page.observed_sha256,
            source_count:leads.len(),leads,
            message:"Discovered leads only. No bibliographic corroboration, full-text passage or citation has been verified.",
        })
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn no_runtime_or_unapproved_binary_cannot_be_research_network_authority(){
        let host=ResearchDesktop::new(
            PathBuf::from("/nonexistent-runtime"),PathBuf::from("/nonexistent-data")
        );
        assert!(!host.available());
        assert!(host.search("c-approved","science").is_err());
        assert!(host.search("other-chat","science").is_err());
        assert!(host.search("c-approved","science\nhttp://localhost").is_err());
        assert!(!verified_file(Path::new("/nonexistent"),&"a".repeat(64)));
    }
}
