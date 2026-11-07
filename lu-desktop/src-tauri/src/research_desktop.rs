//! WAVE4-RESEARCH-02 — explicit native OpenAlex discovery through a bundled
//! hash-pinned HTTPS client and the existing verified-evidence ledger.
//! Search leads remain UNVERIFIED. No model-selected URL, network tool grant,
//! arbitrary curl, browser session or research agent runtime is created.
use std::{
    fs::{self,File},io::Read,path::{Path,PathBuf},
    sync::{Arc,Mutex,atomic::{AtomicBool,Ordering}},collections::HashMap,
};
use serde::Serialize;
use sha2::{Digest,Sha256};
use lu_core::{
    web_http_fetch::{SecureHttpFetcher,FetchError},
    web_research_evidence::{Candidate,EvidenceLedger,EvidenceError,SourceStatus,TrustedBibliography,ResearchCampaignStatus},
    web_research_planner::canonical_doi,
    web_research_analysis::{AnalysisStore,Interpretation,Stance},
    web_research_report::{render_verified_report,VerifiedResearchReport},
    web_research_styles::{catalog_work_from_crossref,render_correlated,render_bibliography,export_csl_json,Style},

    web_research_document::{extract_research_document,MAX_RESEARCH_DOCUMENT_BYTES},
    web_search_openalex::{search_openalex,search_openalex_cancellable},
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
    pub page:u32,
    pub next_page:Option<u32>,
    pub leads:Vec<ResearchLeadView>,
    pub message:&'static str,
}

#[derive(Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct ResearchBrowserEvidenceView {
    pub generation_id:String,
    pub call_id:String,
    pub source_url:String,
    pub observed_sha256:String,
    pub excerpt:String,
    pub excerpt_truncated:bool,
    pub verification_status:&'static str,
    pub citation_verified:bool,
}
#[derive(Serialize,Clone)]
#[serde(rename_all="camelCase")]
pub(crate) struct ResearchQuoteProofView {
    pub source_url:String,
    pub observed_sha256:String,
    pub exact_quote:String,
    pub publisher_source_id:Option<String>,
    pub publisher_link_matched:bool,
    pub document_accessed:bool,
    pub citation_verified:bool,
    pub message:&'static str,
}
#[derive(Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct ResearchCorroborationView{
    pub source_id:String,
    pub verification_status:&'static str,
    pub record_url:String,
    pub citation_verified:bool,
    pub message:&'static str,
}
#[derive(Serialize,Clone)]
#[serde(rename_all="camelCase")]
pub(crate) struct ResearchCitationPreviewView {
    pub source_id:String,
    pub style:String,
    pub bibliography:String,
    pub first_note:Option<String>,
    pub catalog_response_sha256:String,
    pub csl_json:serde_json::Value,
    pub bibliography_identity_corroborated:bool,
    pub author_identity_corroborated:bool,
    pub publication_details_verified:bool,
    pub quotation_or_page_verified:bool,
    pub factual_claim_verified:bool,
    pub coverage:String,
    pub message:&'static str,
}
/// Offline, chat-scoped output built only from sealed Crossref responses.
/// No model-authored source list or new network request can enter this view.
#[derive(Serialize,Clone)]
#[serde(rename_all="camelCase")]
pub(crate) struct ResearchBibliographyView {
    pub style:String,
    pub source_count:usize,
    pub markdown:String,
    pub csl_json:serde_json::Value,
    pub catalog_manifest_sha256:String,
    pub publication_details_verified:bool,
    pub quotation_or_page_verified:bool,
    pub factual_claim_verified:bool,
    pub message:&'static str,
}
#[derive(Serialize,Clone)]
#[serde(rename_all="camelCase")]
pub(crate) struct ResearchBibliographyExportView {
    pub saved_path:String,
    pub format:String,
    pub saved_sha256:String,
    pub catalog_manifest_sha256:String,
    pub source_count:usize,
    pub publication_details_verified:bool,
    pub message:&'static str,
}
/// The native desktop retains the exact downloaded bytes between human
/// comparison and an atomic revision; no URL or alternate bytes can be
/// substituted by the WebView, an LLM, or a stale preview.
pub(crate) struct ResearchCitationRevisionCandidate{
    pub source_id:String,
    pub expected_previous_sha256:String,
    pub new_response_sha256:String,
    pub previous_bibliography:String,
    pub proposed_bibliography:String,
    pub proposed_response:Vec<u8>,
}
#[derive(Serialize,Clone)]
#[serde(rename_all="camelCase")]
pub(crate) struct ResearchCitationRevisionReceipt{
    pub source_id:String,
    pub previous_sha256:String,
    pub current_sha256:String,
    pub revision_count:usize,
    pub publication_details_verified:bool,
    pub message:&'static str,
}
/// Percent-encode a host-selected, ledger-stored DOI as a Crossref path
/// segment. Model/UI input never supplies a network URL or a replacement DOI.
fn encode_doi_component(doi:&str)->String{
    let mut out=String::new();
    for b in doi.bytes(){
        if b.is_ascii_alphanumeric()||matches!(b,b'-'|b'_'|b'.'|b'~'){
            out.push(char::from(b));
        }else{
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}
/// Crossref's metadata is independent bibliographic evidence, never
/// evidence that a full text was obtained or a quotation actually occurs.
fn parse_crossref_metadata(
    bytes:&[u8],expected_doi:&str,record_url:&str
)->Result<TrustedBibliography,&'static str>{
    if bytes.len()>1024*1024{return Err("Crossref response too large");}
    let json:serde_json::Value=serde_json::from_slice(bytes)
        .map_err(|_|"Crossref returned invalid JSON")?;
    let work=json.get("message").ok_or("Crossref work missing")?;
    let doi=work.get("DOI").and_then(|v|v.as_str())
        .and_then(canonical_doi).ok_or("Crossref DOI missing")?;
    if doi!=expected_doi{return Err("Crossref DOI identity mismatch");}
    let title=work.get("title").and_then(|v|v.as_array())
        .and_then(|v|v.first()).and_then(|v|v.as_str())
        .ok_or("Crossref title unavailable")?;
    if title.trim().is_empty()||title.len()>1024||title.chars().any(char::is_control){
        return Err("Crossref title invalid");
    }
    let authors=work.get("author").and_then(|v|v.as_array())
        .ok_or("Crossref authors unavailable")?;
    if authors.is_empty()||authors.len()>128{return Err("Crossref authors invalid");}
    let mut names=Vec::new();
    for a in authors {
        let given=a.get("given").and_then(|v|v.as_str()).unwrap_or("");
        let family=a.get("family").and_then(|v|v.as_str()).unwrap_or("");
        let full=format!("{} {}",given.trim(),family.trim()).trim().to_owned();
        if full.is_empty()||full.len()>128||full.chars().any(char::is_control){
            return Err("Crossref author name invalid");
        }
        names.push(full);
    }
    let author=names.join("; ");
    if author.len()>512{return Err("Crossref author list exceeds evidence bounds");}
    Ok(TrustedBibliography{
        record_url:record_url.to_owned(),
        title:title.to_owned(),author,identifier:doi,
    })
}

/// Crossref's DOI-checked work record may independently specify a landing
/// URL. It is NOT proof of full-text access; later Browser evidence must
/// match this *exact* final HTTPS URL to qualify for a linked quote proof.
fn crossref_publisher_url(bytes:&[u8],expected_doi:&str)->Option<String>{
    let json:serde_json::Value=serde_json::from_slice(bytes).ok()?;
    let work=json.get("message")?;
    if work.get("DOI").and_then(|v|v.as_str()).and_then(canonical_doi)
        .as_deref()!=Some(expected_doi){return None;}
    let safe=|url:&str|{
        if !url.starts_with("https://")||url.len()>2048
            ||url.contains('@')||url.contains('\\')
            ||url.chars().any(|c:char|c.is_whitespace()||c.is_control()){
            return false;
        }
        let authority=url.strip_prefix("https://").unwrap_or("")
            .split(&['/','?','#'][..]).next().unwrap_or("");
        !authority.is_empty()&&!authority.contains(':')
            &&authority.contains('.')&&!authority.starts_with('.')
    };
    // Prefer a SINGLE independently cataloged PDF link over the generic
    // Crossref DOI URL, which usually redirects to an HTML landing page.
    // Ambiguous links do not grant a new destination: fail closed rather
    // than choose a model-preferred publisher or unverified mirror.
    let mut pdf=std::collections::BTreeSet::<String>::new();
    if let Some(links)=work.get("link").and_then(|v|v.as_array()){
        if links.len()>32{return None;}
        for link in links{
            let kind=link.get("content-type").and_then(|v|v.as_str());
            if kind!=Some("application/pdf"){continue;}
            let url=link.get("URL").and_then(|v|v.as_str())?;
            if !safe(url){return None;}
            pdf.insert(url.into());
        }
    }
    if pdf.len()>1{return None;}
    if let Some(url)=pdf.into_iter().next(){return Some(url);}
    let url=work.get("URL")?.as_str()?;
    safe(url).then(||url.to_owned())
}

#[derive(Serialize,Clone)]
#[serde(rename_all="camelCase")]
pub(crate) struct ResearchDocumentView{
    pub source_id:String,
    pub catalog_url:String,
    pub final_url:String,
    pub redirect_hops:Vec<String>,
    pub redirect_chain_sha256:String,
    pub response_sha256:String,
    pub extracted_text_sha256:String,
    pub format:String,
    pub excerpt:String,
    pub truncated:bool,
    pub document_accessed:bool,
    pub publisher_url_matched:bool,
    pub citation_verified:bool,
    pub message:&'static str,
}
#[derive(Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct ResearchDocumentPassageView{
    pub source_id:String,
    pub exact_quote:String,
    pub verification_status:&'static str,
    pub bibliographic_passage_verified:bool,
    pub interpretation_verified:bool,
}
#[derive(Serialize,Clone)]
#[serde(rename_all="camelCase")]
pub(crate) struct ResearchCampaignView{
    pub campaign_id:String,
    pub query:String,
    pub state:String,
    pub next_page:u32,
    pub candidate_ceiling:u32,
    pub distinct_dois:u32,
    pub completed_pages:u32,
    pub blocked_reason:Option<String>,
    pub message:&'static str,
}
impl From<ResearchCampaignStatus> for ResearchCampaignView{
    fn from(status:ResearchCampaignStatus)->Self{
        Self{
            campaign_id:status.id,query:status.query,
            state:status.state,next_page:status.next_page,
            candidate_ceiling:status.candidate_ceiling,
            distinct_dois:status.distinct_dois,
            completed_pages:status.completed_pages,
            blocked_reason:status.blocked_reason,
            message:"Catalog leads are discovered only; search count is not independently verified full-text evidence.",
        }
    }
}
#[derive(Serialize,Clone)]
#[serde(rename_all="camelCase")]
pub(crate) struct ResearchClaimView{
    pub claim:String,
    pub supports:usize,
    pub challenges:usize,
    pub complicates:usize,
    pub distinct_catalog_sources:usize,
    pub unresolved_disagreement:bool,
    pub interpretation_verified:bool,
    pub reason_codes:Vec<String>,
}
fn claim_view(
    row:lu_core::web_research_analysis::ClaimDebate
)->ResearchClaimView{
    ResearchClaimView{
        claim:row.thesis_claim,
        supports:row.supports.len(),
        challenges:row.challenges.len(),
        complicates:row.complicates.len(),
        distinct_catalog_sources:row.distinct_catalog_sources,
        unresolved_disagreement:row.unresolved_disagreement,
        interpretation_verified:false,
        reason_codes:row.reason_codes.into_iter().map(str::to_owned).collect(),
    }
}
#[derive(Serialize,Clone)]
#[serde(rename_all="camelCase")]
pub(crate) struct ResearchReportExportView{
    pub saved_path:String,
    pub format:String,
    pub saved_sha256:String,
    pub distinct_sources:usize,
    pub checked_passages:usize,
    pub bibliography_sources:usize,
    pub interpretation_verified:bool,
    pub message:&'static str,
}
#[derive(Serialize,Clone)]
#[serde(rename_all="camelCase")]
pub(crate) struct ResearchCampaignReviewItem{
    pub source_id:String,
    pub stage:String,
    pub reason_code:String,
    pub can_retry:bool,
}
#[derive(Serialize,Clone)]
#[serde(rename_all="camelCase")]
pub(crate) struct ResearchCampaignReviewView{
    pub campaign_id:String,
    pub discovered_dois:u32,
    pub sources_reviewed:u32,
    pub complete_document_texts:u32,
    pub pending_sources:u32,
    pub reviewed_this_action:u32,
    pub message:&'static str,
}
pub(crate) struct ResearchDesktop{
    active_campaigns:Mutex<HashMap<String,Arc<AtomicBool>>>,
    active_documents:Mutex<HashMap<String,Arc<AtomicBool>>>,
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
            active_campaigns:Mutex::new(HashMap::new()),
            active_documents:Mutex::new(HashMap::new()),
            root:app_data.join("lu-research-evidence"),
            curl:resources.join("lu-research").join(filename),
            expected_sha256:option_env!("LU_RESEARCH_CURL_SHA256"),
        }
    }
    pub(crate) fn available(&self)->bool{
        self.expected_sha256.is_some_and(|sha|verified_file(&self.curl,sha))
    }
    /// P03 explicit operator-selected public model discovery. The same
    /// hash-pinned HTTPS reader used by Research is reused rather than
    /// introducing a model-provided binary, URL, credential or network tool.
    /// Neither method downloads weights or issues a model execution grant.
    pub(crate) fn model_provider_search(
        &self,query:&str
    )->Result<Vec<lu_core::model_provider_catalog::ModelLead>,String>{
        use lu_core::model_provider_catalog::{search_url,parse_search};
        let url=search_url(query).map_err(|_|"Invalid model search query")?;
        if !self.available(){
            return Err("The hash-pinned Lu HTTPS runtime is unavailable".into())
        }
        let fetcher=SecureHttpFetcher{curl_binary:&self.curl,ca_bundle:None};
        let page=fetcher.fetch_bounded(&url,512*1024)
            .map_err(|_|"Public model catalog search unavailable")?;
        if page.http_status!=200||page.redirect_target.is_some(){
            return Err("Model catalog returned an unverified status or redirect".into())
        }
        parse_search(&page.bytes).map_err(|_|"Invalid model catalog search response".into())
    }
    pub(crate) fn model_provider_review(
        &self,repo_id:&str,revision:&str
    )->Result<lu_core::model_provider_catalog::ModelRepoReview,String>{
        use lu_core::model_provider_catalog::{
            model_info_url,tree_url,parse_repo_license,parse_tree
        };
        let info_url=model_info_url(repo_id,revision)
            .map_err(|_|"Invalid pinned model reference")?;
        let files_url=tree_url(repo_id,revision)
            .map_err(|_|"Invalid pinned model revision")?;
        if !self.available(){
            return Err("The hash-pinned Lu HTTPS runtime is unavailable".into())
        }
        let fetcher=SecureHttpFetcher{curl_binary:&self.curl,ca_bundle:None};
        let info=fetcher.fetch_bounded(&info_url,512*1024)
            .map_err(|_|"Cannot independently inspect pinned model metadata")?;
        if info.http_status!=200||info.redirect_target.is_some(){
            return Err("Pinned model identity could not be verified".into())
        }
        let license=parse_repo_license(&info.bytes,repo_id,revision)
            .map_err(|_|"Model license/access could not be verified")?;
        let tree=fetcher.fetch_bounded(&files_url,768*1024)
            .map_err(|_|"Pinned model file inventory unavailable")?;
        if tree.http_status!=200||tree.redirect_target.is_some(){
            return Err("Pinned model files cannot be independently inspected".into())
        }
        parse_tree(&tree.bytes,repo_id,revision,&license)
            .map_err(|_|"No safely identified immutable GGUF assets found".into())
    }
    pub(crate) fn search(
        &self,chat_id:&str,query:&str
    )->Result<ResearchSearchView,String>{
        self.search_page(chat_id,query,1)
    }
    /// Bound unattended pagination to five explicitly approved pages (up to
    /// 125 discovery candidates); never claim these are verified citations.
    pub(crate) fn search_page(
        &self,chat_id:&str,query:&str,number:u32
    )->Result<ResearchSearchView,String>{
        if !(1..=5).contains(&number){return Err("Invalid research page".into());}
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
        let page=search_openalex(&fetcher,query,number)
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
            source_count:leads.len(),page:number,
            next_page:page.next_page.filter(|p|*p<=5),leads,
            message:"Discovered leads only. No bibliographic corroboration, full-text passage or citation has been verified.",
        })
    }

    /// Host-only bridge from a successfully dispatched, authorized native
    /// Browser READ/ACCESSIBILITY to the same chat-scoped Research evidence DB.
    /// The caller must recheck the current Browser task and redirect origin.
    /// This NEVER promotes a bibliography record or claims a checked citation.
    pub(crate) fn record_browser_read(
        &self,chat_id:&str,generation_id:&str,call_id:&str,
        source_url:&str,kind:&str,text:&str,truncated:bool
    )->Result<ResearchBrowserEvidenceView,String>{
        if !safe_id(chat_id){
            return Err("Browser evidence requires a trusted conversation".into());
        }
        fs::create_dir_all(&self.root)
            .map_err(|_|"Research evidence directory unavailable")?;
        let root=fs::symlink_metadata(&self.root)
            .map_err(|_|"Research evidence directory unavailable")?;
        if !root.file_type().is_dir()||root.file_type().is_symlink(){
            return Err("Research evidence directory is not trusted".into());
        }
        let db=self.root.join(format!("{chat_id}.sqlite"));
        match fs::symlink_metadata(&db) {
            Ok(meta) if !meta.file_type().is_file()||meta.file_type().is_symlink()=>
                return Err("Research evidence ledger is not a regular file".into()),
            Err(err) if err.kind()!=std::io::ErrorKind::NotFound=>
                return Err("Research evidence ledger cannot be checked".into()),
            _=>{}
        }
        let mut ledger=EvidenceLedger::open(&db)
            .map_err(|_|"Research evidence ledger unavailable")?;
        let observed=ledger.observe_browser_text(
            generation_id,call_id,source_url,kind,text,truncated
        ).map_err(|_|"Browser observation rejected or conflicted with prior receipt")?;
        Ok(ResearchBrowserEvidenceView{
            generation_id:observed.generation_id,
            call_id:observed.call_id,
            source_url:observed.url,
            observed_sha256:observed.observed_sha256,
            excerpt:observed.excerpt,
            excerpt_truncated:observed.excerpt_truncated,
            verification_status:"browser_observed_untrusted",
            citation_verified:false,
        })
    }

    /// Read-only UI evidence inventory. Does not depend on curl availability,
    /// never creates a ledger merely to display an empty list and returns
    /// only records from the selected conversation's own SQLite.
    pub(crate) fn browser_evidence(
        &self,chat_id:&str
    )->Result<Vec<ResearchBrowserEvidenceView>,String>{
        if !safe_id(chat_id){return Err("Select a trusted conversation".into());}
        let root=match fs::symlink_metadata(&self.root){
            Ok(m)=>m,
            Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Ok(vec![]),
            Err(_)=>return Err("Research evidence directory unavailable".into()),
        };
        if !root.file_type().is_dir()||root.file_type().is_symlink(){
            return Err("Research evidence directory is not trusted".into());
        }
        let db=self.root.join(format!("{chat_id}.sqlite"));
        let file=match fs::symlink_metadata(&db){
            Ok(m)=>m,
            Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Ok(vec![]),
            Err(_)=>return Err("Research evidence ledger unavailable".into())
        };
        if !file.file_type().is_file()||file.file_type().is_symlink(){
            return Err("Research evidence ledger is untrusted".into());
        }
        let ledger=EvidenceLedger::open(&db).map_err(|_|"Research evidence unavailable")?;
        ledger.recent_browser_observations(24)
            .map_err(|_|"Research Browser evidence could not be read".into())
            .map(|observed|observed.into_iter().map(|record|ResearchBrowserEvidenceView{
                generation_id:record.generation_id,call_id:record.call_id,
                source_url:record.url,observed_sha256:record.observed_sha256,
                excerpt:record.excerpt,excerpt_truncated:record.excerpt_truncated,
                verification_status:"browser_observed_untrusted",citation_verified:false,
            }).collect())
    }

    /// A human reviews a proposed direct quotation from one previously
    /// observed Browser call. An optional academic source ID must already be
    /// Crossref-corroborated and have its independently fetched publisher URL
    /// exactly equal to the recorded final Browser URL.
    pub(crate) fn verify_browser_quote(
        &self,chat_id:&str,generation_id:&str,call_id:&str,quote:&str,
        publisher_source_id:Option<&str>
    )->Result<ResearchQuoteProofView,String>{
        if !safe_id(chat_id)||quote.len()>2048||quote.trim().is_empty(){
            return Err("Invalid conversation or quote".into());
        }
        let root=fs::symlink_metadata(&self.root)
            .map_err(|_|"Research evidence directory unavailable")?;
        if !root.file_type().is_dir()||root.file_type().is_symlink(){
            return Err("Research evidence directory untrusted".into());
        }
        let db=self.root.join(format!("{chat_id}.sqlite"));
        let file=fs::symlink_metadata(&db)
            .map_err(|_|"Research evidence ledger unavailable")?;
        if !file.file_type().is_file()||file.file_type().is_symlink(){
            return Err("Research evidence ledger is not a regular file".into());
        }
        let mut ledger=EvidenceLedger::open(&db)
            .map_err(|_|"Research evidence ledger unavailable")?;
        let proof=ledger.prove_browser_passage(
            generation_id,call_id,quote,publisher_source_id
        ).map_err(|err|format!("Exact Browser passage proof refused: {err:?}"))?;
        Ok(ResearchQuoteProofView{
            source_url:proof.source_url,
            observed_sha256:proof.observed_sha256,
            exact_quote:proof.exact_quote,
            publisher_source_id:proof.source_id,
            publisher_link_matched:proof.publisher_link_matched,
            document_accessed:false,
            citation_verified:false,
            message:if proof.publisher_link_matched {
                "Exact quotation matched the retained Browser excerpt and a Crossref-corroborated publisher URL. This does NOT attest the complete scholarly document or factual interpretation."
            }else{
                "Exact quotation matched the retained Browser excerpt only. No independent publication or scholarly citation was verified."
            },
        })
    }

    /// Host-approved NetworkRead for one already corroborated catalog URL.
    /// Redirect hops are individually HTTPS/DNS pinned by SecureHttpFetcher.
    /// This does not let the model supply a source URL or executable.
    /// A direct user-picked document task owns one cancellable HTTPS worker.
    /// Campaign reviews hold their own process lease and use the same
    /// lower-level cancellable redirect fetcher.
    pub(crate) fn retrieve_document(
        &self,chat_id:&str,source_id:&str
    )->Result<ResearchDocumentView,String>{
        if !safe_id(chat_id)||!source_id.starts_with("https://openalex.org/")
            ||source_id.len()>128{
            return Err("Invalid source identity for document retrieval".into());
        }
        let key=format!("{chat_id}:{source_id}");
        let cancel=Arc::new(AtomicBool::new(false));
        {
            let mut active=self.active_documents.lock()
                .map_err(|_|"Document worker lease unavailable")?;
            if active.contains_key(&key){
                return Err("This document is already being retrieved".into());
            }
            active.insert(key.clone(),Arc::clone(&cancel));
        }
        let result=self.retrieve_document_admitted(
            chat_id,source_id,Some(cancel.as_ref())
        );
        if let Ok(mut active)=self.active_documents.lock(){
            active.remove(&key);
        }
        result
    }
    pub(crate) fn cancel_document(
        &self,chat_id:&str,source_id:&str
    )->Result<bool,String>{
        if !safe_id(chat_id)||!source_id.starts_with("https://openalex.org/")
            ||source_id.len()>128{
            return Err("Invalid document cancellation identity".into());
        }
        let key=format!("{chat_id}:{source_id}");
        let active=self.active_documents.lock()
            .map_err(|_|"Document cancellation unavailable")?;
        let Some(flag)=active.get(&key) else{return Ok(false)};
        flag.store(true,Ordering::Release);
        Ok(true)
    }
    fn retrieve_document_admitted(
        &self,chat_id:&str,source_id:&str,cancel:Option<&AtomicBool>
    )->Result<ResearchDocumentView,String>{
        if !safe_id(chat_id)||!source_id.starts_with("https://openalex.org/")
            ||source_id.len()>128||source_id.chars().any(char::is_control){
            return Err("Select a corroborated source in a trusted conversation".into());
        }
        if !self.available(){return Err("Pinned Research HTTPS client unavailable".into());}
        let root=fs::symlink_metadata(&self.root)
            .map_err(|_|"Research evidence directory unavailable")?;
        if !root.file_type().is_dir()||root.file_type().is_symlink(){
            return Err("Research evidence directory untrusted".into());
        }
        let db=self.root.join(format!("{chat_id}.sqlite"));
        let file=fs::symlink_metadata(&db)
            .map_err(|_|"Research evidence ledger unavailable")?;
        if !file.file_type().is_file()||file.file_type().is_symlink(){
            return Err("Research evidence ledger untrusted".into());
        }
        let mut ledger=EvidenceLedger::open(&db)
            .map_err(|_|"Research evidence ledger unavailable")?;
        if ledger.status(source_id).map_err(|_|"Research source unknown")?
            !=SourceStatus::MetadataCorroborated{
            return Err("Research source is not in the metadata-corroborated stage".into());
        }
        let catalog_url=ledger.publisher_url_for_source(source_id)
            .map_err(|_|"Publisher identity unavailable")?
            .ok_or("Crossref did not identify a specific HTTPS landing URL")?;
        let fetcher=SecureHttpFetcher{curl_binary:&self.curl,ca_bundle:None};
        let chain=match cancel{
            Some(flag)=>fetcher.fetch_document_chain_cancellable(
                &catalog_url,MAX_RESEARCH_DOCUMENT_BYTES,flag
            ),
            None=>fetcher.fetch_document_chain(
                &catalog_url,MAX_RESEARCH_DOCUMENT_BYTES
            ),
        }.map_err(|e|match e{
            FetchError::Cancelled=>"Research document retrieval cancelled by user",
            _=>"Document HTTPS retrieval or redirect attestation refused",
        })?;
        if cancel.is_some_and(|flag|flag.load(Ordering::Acquire)){
            return Err("Research document retrieval cancelled by user".into());
        }
        let document=extract_research_document(&chain.final_page.bytes)
            .map_err(|_|"Fetched document is unreadable, unsupported or exceeds extraction limits")?;
        if cancel.is_some_and(|flag|flag.load(Ordering::Acquire)){
            return Err("Research document retrieval cancelled by user".into());
        }
        let proof=ledger.record_document_retrieval(
            source_id,&catalog_url,&chain.final_page.canonical_url,
            &document.raw_sha256,&chain.chain_sha256,document.kind.code(),
            &document.text,document.truncated
        ).map_err(|e|format!("Document evidence could not be persisted: {e:?}"))?;
        let excerpt=document.text.chars().take(4000).collect();
        Ok(ResearchDocumentView{
            source_id:source_id.into(),
            catalog_url:proof.catalog_url.clone(),
            final_url:proof.final_url.clone(),
            redirect_hops:chain.hops.into_iter().map(|h|h.url).collect(),
            redirect_chain_sha256:proof.chain_sha256,
            response_sha256:proof.raw_sha256,
            extracted_text_sha256:proof.text_sha256,
            format:proof.format,excerpt,
            truncated:proof.truncated,
            document_accessed:proof.document_accessed,
            publisher_url_matched:proof.final_url==proof.catalog_url,
            citation_verified:false,
            message:if proof.document_accessed{
                "Complete text layer extracted from a directly Crossref-linked PDF. Individual passages still require exact verification; interpretations are not certified."
            }else{
                "Observed document text only. HTML, truncated PDFs or catalog redirects do not prove complete publisher-identified publication access."
            },
        })
    }
    /// A reviewed quote can be verified against a stored PDF text layer only
    /// after catalog identity, untruncated extraction and document hash gate.
    pub(crate) fn verify_document_passage(
        &self,chat_id:&str,source_id:&str,quote:&str
    )->Result<ResearchDocumentPassageView,String>{
        if !safe_id(chat_id)||source_id.len()>128
            ||!source_id.starts_with("https://openalex.org/")
            ||quote.len()>4096||quote.trim().is_empty(){
            return Err("Invalid document passage request".into());
        }
        let root=fs::symlink_metadata(&self.root)
            .map_err(|_|"Research evidence directory unavailable")?;
        if !root.file_type().is_dir()||root.file_type().is_symlink(){
            return Err("Research evidence directory untrusted".into());
        }
        let db=self.root.join(format!("{chat_id}.sqlite"));
        let file=fs::symlink_metadata(&db)
            .map_err(|_|"Research evidence ledger unavailable")?;
        if !file.file_type().is_file()||file.file_type().is_symlink(){
            return Err("Research evidence ledger untrusted".into());
        }
        let mut ledger=EvidenceLedger::open(&db)
            .map_err(|_|"Research evidence ledger unavailable")?;
        ledger.verify_passage(source_id,"full-text",quote)
            .map_err(|e|format!("Verified document passage refused: {e:?}"))?;
        Ok(ResearchDocumentPassageView{
            source_id:source_id.into(),exact_quote:quote.into(),
            verification_status:"document_passage_verified",
            bibliographic_passage_verified:true,
            interpretation_verified:false,
        })
    }

    pub(crate) fn campaigns(&self,chat_id:&str)
    ->Result<Vec<ResearchCampaignView>,String>{
        if !safe_id(chat_id){return Err("Invalid Research conversation".into());}
        let dir=match fs::symlink_metadata(&self.root){
            Ok(m)=>m,
            Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Ok(Vec::new()),
            Err(_)=>return Err("Research evidence directory unavailable".into()),
        };
        if !dir.file_type().is_dir()||dir.file_type().is_symlink(){
            return Err("Research evidence directory untrusted".into());
        }
        let db=self.root.join(format!("{chat_id}.sqlite"));
        let file=match fs::symlink_metadata(&db){
            Ok(m)=>m,
            Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Ok(Vec::new()),
            Err(_)=>return Err("Research evidence ledger unavailable".into()),
        };
        if !file.file_type().is_file()||file.file_type().is_symlink(){
            return Err("Research evidence ledger untrusted".into());
        }
        let ledger=EvidenceLedger::open(&db)
            .map_err(|_|"Research evidence ledger unavailable")?;
        ledger.recent_campaigns(20)
            .map_err(|_|"Research campaign inventory unavailable".into())
            .map(|rows|rows.into_iter().map(Into::into).collect())
    }
    /// Bounded campaign with one fixed OpenAlex query. The caller must obtain
    /// a native grant for the campaign, not accept a model-created task ID.
    pub(crate) fn create_campaign(
        &self,chat_id:&str,campaign_id:&str,query:&str,ceiling:u32
    )->Result<ResearchCampaignView,String>{
        if !safe_id(chat_id){return Err("Invalid Research conversation".into());}
        fs::create_dir_all(&self.root)
            .map_err(|_|"Research evidence directory unavailable")?;
        let root=fs::symlink_metadata(&self.root)
            .map_err(|_|"Research evidence directory unavailable")?;
        if !root.file_type().is_dir()||root.file_type().is_symlink(){
            return Err("Research directory untrusted".into());
        }
        let db=self.root.join(format!("{chat_id}.sqlite"));
        if let Ok(meta)=fs::symlink_metadata(&db){
            if !meta.file_type().is_file()||meta.file_type().is_symlink(){
                return Err("Research evidence file untrusted".into());
            }
        }
        let mut ledger=EvidenceLedger::open(&db)
            .map_err(|_|"Research evidence ledger unavailable")?;
        ledger.create_campaign(campaign_id,query,ceiling)
            .map(Into::into)
            .map_err(|_|"Research campaign rejected".into())
    }
    pub(crate) fn campaign_status(
        &self,chat_id:&str,campaign_id:&str
    )->Result<ResearchCampaignView,String>{
        if !safe_id(chat_id){return Err("Invalid Research conversation".into());}
        let dir=fs::symlink_metadata(&self.root)
            .map_err(|_|"Research directory unavailable")?;
        if !dir.file_type().is_dir()||dir.file_type().is_symlink(){
            return Err("Research directory untrusted".into());
        }
        let db=self.root.join(format!("{chat_id}.sqlite"));
        let file=fs::symlink_metadata(&db)
            .map_err(|_|"Research evidence ledger unavailable")?;
        if !file.file_type().is_file()||file.file_type().is_symlink(){
            return Err("Research evidence ledger untrusted".into());
        }
        let ledger=EvidenceLedger::open(&db)
            .map_err(|_|"Research evidence ledger unavailable")?;
        ledger.campaign_status(campaign_id).map(Into::into)
            .map_err(|_|"Research campaign not found".into())
    }
    /// Each page is a separate public HTTPS read, atomically checkpointed
    /// with the discovered DOIs in the same chat-scoped SQLite transaction.
    /// A failed page remains resumable and cannot claim fake progress.
    pub(crate) fn advance_campaign(
        &self,chat_id:&str,campaign_id:&str,cancel:&AtomicBool
    )->Result<ResearchCampaignView,String>{
        if !self.available(){return Err("Pinned Research HTTPS client unavailable".into());}
        let db=self.root.join(format!("{chat_id}.sqlite"));
        // campaign_status verifies chat identity and trusted file before use.
        let _=self.campaign_status(chat_id,campaign_id)?;
        let (query,page,remaining)={
            let mut ledger=EvidenceLedger::open(&db)
                .map_err(|_|"Research evidence ledger unavailable")?;
            match ledger.claim_campaign_page(campaign_id)
                .map_err(|_|"Campaign cannot claim this page")?{
                Some(task)=>task,
                None=>return ledger.campaign_status(campaign_id)
                    .map(Into::into).map_err(|_|"Campaign unavailable".into()),
            }
        };
        if remaining==0{return self.campaign_status(chat_id,campaign_id);}
        let fetcher=SecureHttpFetcher{curl_binary:&self.curl,ca_bundle:None};
        let result=search_openalex_cancellable(&fetcher,&query,page,cancel);
        let mut ledger=EvidenceLedger::open(&db)
            .map_err(|_|"Research evidence ledger unavailable")?;
        let found=match result{
            Ok(page_data)=>page_data,
            Err(_)=>{
                let _=ledger.block_campaign(campaign_id,"FETCH_FAILED");
                return Err("Research page failed; progress is saved. Review and resume explicitly.".into());
            }
        };
        if found.page!=page{
            let _=ledger.block_campaign(campaign_id,"POLICY_DENIED");
            return Err("Research provider page mismatch".into());
        }
        let leads=found.leads.into_iter().map(|l|Candidate{
            id:l.provider_id,title:l.title,author:l.authors,
            identifier:l.doi
        }).collect::<Vec<_>>();
        ledger.finish_campaign_page(
            campaign_id,page,&found.observed_sha256,&leads,
            found.next_page.is_some()&&remaining>0
        ).map(Into::into)
        .map_err(|_|"Research page did not produce a durable checkpoint".into())
    }
    /// User-approved run of up to five explicitly bounded OpenAlex pages.
    /// A restart never auto-resumes network access; the user must approve
    /// a fresh request. Each completed page survives application failure.
    pub(crate) fn run_campaign(
        &self,chat_id:&str,campaign_id:&str
    )->Result<ResearchCampaignView,String>{
        // An in-flight native worker owns the one campaign process lease.
        // A post-crash RUNNING SQLite receipt is recoverable after restart
        // because the new process has no active workers. A same-process
        // concurrent Resume cannot duplicate or race the active curl task.
        let key=format!("{chat_id}:{campaign_id}");
        let cancel=Arc::new(AtomicBool::new(false));
        {
            let mut leases=self.active_campaigns.lock()
                .map_err(|_|"Research campaign worker lock unavailable")?;
            if leases.contains_key(&key){
                return Err("Research campaign is already running".into());
            }
            leases.insert(key.clone(),Arc::clone(&cancel));
        }
        let result=(||{
            let mut latest=self.campaign_status(chat_id,campaign_id)?;
            for _ in 0..5{
                if matches!(latest.state.as_str(),"DONE"|"CANCELLED"){break;}
                if cancel.load(Ordering::Acquire){break;}
                latest=self.advance_campaign(chat_id,campaign_id,&cancel)?;
            }
            Ok(latest)
        })();
        if let Ok(mut leases)=self.active_campaigns.lock(){
            leases.remove(&key);
        }
        result
    }
    pub(crate) fn resume_campaign(
        &self,chat_id:&str,campaign_id:&str
    )->Result<ResearchCampaignView,String>{
        let key=format!("{chat_id}:{campaign_id}");
        if self.active_campaigns.lock()
            .map_err(|_|"Research worker lock unavailable")?
            .contains_key(&key){
            return Err("Research campaign is still running".into());
        }
        let _=self.campaign_status(chat_id,campaign_id)?;
        let db=self.root.join(format!("{chat_id}.sqlite"));
        let mut ledger=EvidenceLedger::open(&db)
            .map_err(|_|"Research evidence ledger unavailable")?;
        let state=ledger.campaign_status(campaign_id)
            .map_err(|_|"Research campaign unknown")?;
        if state.state=="BLOCKED" {
            ledger.resume_campaign(campaign_id)
                .map_err(|_|"Research campaign resume refused")?;
        }
        ledger.campaign_status(campaign_id).map(Into::into)
            .map_err(|_|"Research campaign unavailable".into())
    }
    pub(crate) fn cancel_campaign(
        &self,chat_id:&str,campaign_id:&str
    )->Result<ResearchCampaignView,String>{
        let _=self.campaign_status(chat_id,campaign_id)?;
        let db=self.root.join(format!("{chat_id}.sqlite"));
        let mut ledger=EvidenceLedger::open(&db)
            .map_err(|_|"Research evidence ledger unavailable")?;
        ledger.cancel_campaign(campaign_id)
            .map_err(|_|"Research campaign cancel refused")?;
        // DB revocation is durable; atomically stop the owned HTTPS child,
        // rather than just marking the campaign cancelled after curl exits.
        let key=format!("{chat_id}:{campaign_id}");
        if let Ok(active)=self.active_campaigns.lock(){
            if let Some(flag)=active.get(&key){
                flag.store(true,Ordering::Release);
            }
        }
        ledger.campaign_status(campaign_id).map(Into::into)
            .map_err(|_|"Research campaign unavailable".into())
    }

    /// Following discovery, this independently authorized bounded review
    /// step corroborates DOI metadata and attempts full-text extraction.
    /// Every source outcome is a durable receipt, not a fabricated citation.
    /// The same active-campaign lease excludes simultaneous search/review.
    pub(crate) fn review_campaign_sources(
        &self,chat_id:&str,campaign_id:&str,limit:usize
    )->Result<ResearchCampaignReviewView,String>{
        if !(1..=10).contains(&limit)||!self.available(){
            return Err("Only 1–10 source reviews with a pinned HTTPS runtime are available".into());
        }
        let initial=self.campaign_status(chat_id,campaign_id)?;
        if initial.state=="CANCELLED"{
            return Err("Cancelled Research campaign cannot review more sources".into());
        }
        let key=format!("{chat_id}:{campaign_id}");
        let cancel=Arc::new(AtomicBool::new(false));
        {
            let mut active=self.active_campaigns.lock()
                .map_err(|_|"Research worker lock unavailable")?;
            if active.contains_key(&key){
                return Err("This Research campaign has an active network worker".into());
            }
            active.insert(key.clone(),Arc::clone(&cancel));
        }
        let outcome=(||{
            let db=self.root.join(format!("{chat_id}.sqlite"));
            let pending={
                let ledger=EvidenceLedger::open(&db)
                    .map_err(|_|"Research evidence ledger unavailable")?;
                ledger.campaign_pending_sources(campaign_id,limit)
                    .map_err(|_|"Research review frontier unavailable")?
            };
            let mut reviewed=0u32;
            for source_id in pending {
                let snapshot=self.campaign_status(chat_id,campaign_id)?;
                if snapshot.state=="CANCELLED"||cancel.load(Ordering::Acquire){break;}
                // Metadata corroboration is retained even after a failed
                // PDF fetch: explicit retries can go straight to the
                // independently pinned publisher URL instead of attempting
                // to re-corroborate an already checked DOI.
                let source_status=EvidenceLedger::open(&db)
                    .map_err(|_|"Research evidence ledger unavailable")?
                    .status(&source_id)
                    .map_err(|_|"Research candidate status unavailable")?;
                let ready=match source_status{
                    SourceStatus::Discovered=>match self.corroborate_crossref_admitted(
                        chat_id,&source_id,Some(&cancel)
                    ){
                        Ok(meta) if meta.verification_status=="metadata_corroborated"=>true,
                        Ok(_)=>false,
                        Err(_)=>false,
                    },
                    SourceStatus::MetadataCorroborated=>true,
                    SourceStatus::DocumentAccessed|SourceStatus::PassageVerified=>false,
                    SourceStatus::Mismatch|SourceStatus::Inaccessible=>false,
                };
                if cancel.load(Ordering::Acquire){break;}
                let final_status=EvidenceLedger::open(&db)
                    .map_err(|_|"Research evidence ledger unavailable")?
                    .status(&source_id)
                    .map_err(|_|"Research metadata status unavailable")?;
                let (stage,reason)=match final_status{
                    SourceStatus::DocumentAccessed|SourceStatus::PassageVerified=>
                        ("DOCUMENT_ACCESSED","OK"),
                    SourceStatus::Mismatch=>("METADATA_MISMATCH","CATALOG_MISMATCH"),
                    SourceStatus::Inaccessible=>("ERROR","METADATA_FAILED"),
                    _ if ready=>match self.retrieve_document_admitted(
                        chat_id,&source_id,Some(&cancel)
                    ){
                        Ok(doc) if doc.document_accessed=>("DOCUMENT_ACCESSED","OK"),
                        Ok(_)=>("METADATA_CORROBORATED","UNSUPPORTED_CONTENT"),
                        Err(_)=>("DOCUMENT_UNAVAILABLE","RETRIEVAL_FAILED"),
                    },
                    _=>("ERROR","METADATA_FAILED"),
                };
                let mut ledger=EvidenceLedger::open(&db)
                    .map_err(|_|"Research evidence ledger unavailable")?;
                ledger.record_campaign_source_review(
                    campaign_id,&source_id,stage,reason
                ).map_err(|_|"Research review receipt could not be persisted")?;
                reviewed+=1;
            }
            let ledger=EvidenceLedger::open(&db)
                .map_err(|_|"Research evidence ledger unavailable")?;
            let (discovered,checked,full)=ledger.campaign_review_progress(campaign_id)
                .map_err(|_|"Research review progress unavailable")?;
            let pending=discovered.saturating_sub(checked);
            Ok(ResearchCampaignReviewView{
                campaign_id:campaign_id.into(),
                discovered_dois:discovered,
                sources_reviewed:checked,
                complete_document_texts:full,
                pending_sources:pending,
                reviewed_this_action:reviewed,
                message:"Source review records metadata agreement, attempted documents and actual complete text. It does not certify claims or automatically admit quotations as citations.",
            })
        })();
        if let Ok(mut active)=self.active_campaigns.lock(){
            active.remove(&key);
        }
        outcome
    }

    pub(crate) fn campaign_review_rows(
        &self,chat_id:&str,campaign_id:&str
    )->Result<Vec<ResearchCampaignReviewItem>,String>{
        let _=self.campaign_status(chat_id,campaign_id)?;
        let db=self.root.join(format!("{chat_id}.sqlite"));
        let ledger=EvidenceLedger::open(&db)
            .map_err(|_|"Research evidence ledger unavailable")?;
        ledger.campaign_review_rows(campaign_id,120)
            .map_err(|_|"Research source review inventory unavailable".to_string())
            .map(|rows|rows.into_iter().map(|(source_id,stage,reason_code)|{
                ResearchCampaignReviewItem{
                    can_retry:matches!(
                        stage.as_str(),"ERROR"|"DOCUMENT_UNAVAILABLE"
                        |"METADATA_CORROBORATED"
                    ),
                    source_id,stage,reason_code,
                }
            }).collect())
    }
    pub(crate) fn retry_campaign_review(
        &self,chat_id:&str,campaign_id:&str,source_id:&str
    )->Result<(),String>{
        if self.campaign_status(chat_id,campaign_id)?.state=="CANCELLED"{
            return Err("Cancelled Research campaign cannot retry".into());
        }
        let key=format!("{chat_id}:{campaign_id}");
        if self.active_campaigns.lock()
            .map_err(|_|"Research lease unavailable")?.contains_key(&key){
            return Err("Cannot reset a Research source during active retrieval".into());
        }
        let db=self.root.join(format!("{chat_id}.sqlite"));
        let mut ledger=EvidenceLedger::open(&db)
            .map_err(|_|"Research evidence ledger unavailable")?;
        ledger.retry_campaign_source(campaign_id,source_id)
            .map_err(|_|"Research source is not eligible for retry".into())
    }

    /// User-submitted interpretation. The native host must ensure this comes
    /// from an explicit UI action; the model cannot mark its own interpretation
    /// HUMAN_REVIEWED or bypass exact-source passage verification.
    pub(crate) fn add_claim(
        &self,chat_id:&str,source_id:&str,claim:&str,quote:&str,
        stance:&str,reasoning:&str
    )->Result<Vec<ResearchClaimView>,String>{
        let _=self.browser_evidence(chat_id)?;
        let db=self.root.join(format!("{chat_id}.sqlite"));
        let ledger=EvidenceLedger::open(&db)
            .map_err(|_|"Research evidence ledger unavailable")?;
        if !ledger.has_verified_passage(source_id,quote)
            .map_err(|_|"Document passage could not be checked")?{
            return Err("Claim requires an exact verified document passage".into());
        }
        let position=match stance{
            "SUPPORTS"=>Stance::Supports,
            "CHALLENGES"=>Stance::Challenges,
            "COMPLICATES"=>Stance::Complicates,
            _=>return Err("Unknown evidence stance".into()),
        };
        let analysis_path=self.root.join(format!("{chat_id}.analysis.sqlite"));
        if let Ok(meta)=fs::symlink_metadata(&analysis_path){
            if !meta.file_type().is_file()||meta.file_type().is_symlink(){
                return Err("Research analysis database is untrusted".into());
            }
        }
        let mut analysis=AnalysisStore::open(&analysis_path)
            .map_err(|_|"Research analysis database unavailable")?;
        analysis.add_interpretation(&ledger,Interpretation{
            source_id,thesis_claim:claim,direct_quote:quote,
            stance:position,reasoning
        }).map_err(|e|format!("Claim interpretation rejected: {e:?}"))?;
        analysis.human_review(source_id,claim,quote)
            .map_err(|_|"User interpretation review could not be recorded")?;
        analysis.summarize_claims(&ledger)
            .map_err(|_|"Research source validation failed".to_owned())
            .map(|v|v.into_iter().map(claim_view).collect())
    }
    /// Read-only view. Every claim is rechecked against the current document
    /// SHA so corrupted or superseded evidence fails closed.
    pub(crate) fn claims(
        &self,chat_id:&str
    )->Result<Vec<ResearchClaimView>,String>{
        let _=self.browser_evidence(chat_id)?;
        let path=self.root.join(format!("{chat_id}.analysis.sqlite"));
        match fs::symlink_metadata(&path){
            Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Ok(Vec::new()),
            Ok(m) if !m.file_type().is_file()||m.file_type().is_symlink()=>
                return Err("Research analysis file untrusted".into()),
            Err(_)=>return Err("Research analysis file unavailable".into()),
            _=>{}
        };
        let ledger=EvidenceLedger::open(&self.root.join(format!("{chat_id}.sqlite")))
            .map_err(|_|"Research evidence ledger unavailable")?;
        let analysis=AnalysisStore::open(&path)
            .map_err(|_|"Research analysis database unavailable")?;
        analysis.summarize_claims(&ledger)
            .map_err(|_|"Research claim provenance invalid".to_owned())
            .map(|v|v.into_iter().map(claim_view).collect())
    }
    /// No model-generated bibliography is accepted: this is an entirely
    /// deterministic report over rehashed, verified full-document passages.
    pub(crate) fn verified_report(
        &self,chat_id:&str,title:&str
    )->Result<VerifiedResearchReport,String>{
        if !safe_id(chat_id){return Err("Invalid Research conversation".into());}
        let _=self.browser_evidence(chat_id)?;
        let analysis_path=self.root.join(format!("{chat_id}.analysis.sqlite"));
        if let Ok(meta)=fs::symlink_metadata(&analysis_path){
            if !meta.file_type().is_file()||meta.file_type().is_symlink(){
                return Err("Research analysis database untrusted".into());
            }
        }
        let ledger=EvidenceLedger::open(&self.root.join(format!("{chat_id}.sqlite")))
            .map_err(|_|"Research evidence ledger unavailable")?;
        let analysis=AnalysisStore::open(&analysis_path)
            .map_err(|_|"Research analysis database unavailable")?;
        render_verified_report(title,&ledger,&analysis)
            .map_err(|e|format!("Report provenance gate rejected export: {e:?}"))
    }

    /// User-approved metadata corroboration of one already-persisted
    /// OpenAlex lead. No arbitrary URL, DOI or model-selected authority.
    /// A successful result is ONLY bibliographic corroboration.
    /// Human-approved citation preview against the *same* independently
    /// Crossref-corroborated source. Re-fetches exact catalog DOI with the
    /// native pinned HTTPS client; renderer/model cannot supply metadata,
    /// a remote URL, author list, date, journal or publication place.
    /// No new EvidenceLedger promotion or internet/AI tool grant is created.
    pub(crate) fn citation_preview(
        &self,chat_id:&str,source_id:&str,style:Style
    )->Result<ResearchCitationPreviewView,String>{
        if !safe_id(chat_id)||!source_id.starts_with("https://openalex.org/")
            ||source_id.len()>128||source_id.chars().any(char::is_control){
            return Err("Select a known OpenAlex source in this conversation".into());
        }
        if !self.available(){
            return Err("Pinned native Crossref HTTPS client unavailable".into());
        }
        let root=fs::symlink_metadata(&self.root)
            .map_err(|_|"Research evidence directory unavailable")?;
        if !root.file_type().is_dir()||root.file_type().is_symlink(){
            return Err("Research evidence directory untrusted".into());
        }
        let db=self.root.join(format!("{chat_id}.sqlite"));
        let file=fs::symlink_metadata(&db)
            .map_err(|_|"Research evidence ledger unavailable")?;
        if !file.file_type().is_file()||file.file_type().is_symlink(){
            return Err("Research evidence ledger untrusted".into());
        }
        let mut ledger=EvidenceLedger::open(&db)
            .map_err(|_|"Research evidence ledger unavailable")?;
        if !matches!(ledger.status(source_id).map_err(|_|"Unknown research source")?,
            SourceStatus::MetadataCorroborated|
            SourceStatus::DocumentAccessed|SourceStatus::PassageVerified){
            return Err("Independently corroborate this source before citing".into());
        }
        let original=ledger.discovered_candidate(source_id)
            .map_err(|_|"Original source identity unavailable")?;
        let doi=canonical_doi(&original.identifier)
            .ok_or("Original source DOI unavailable")?;
        let url=format!("https://api.crossref.org/works/{}",encode_doi_component(&doi));
        let fetcher=SecureHttpFetcher{curl_binary:&self.curl,ca_bundle:None};
        let page=fetcher.fetch_bounded(&url,1024*1024)
            .map_err(|_|"Crossref citation metadata retrieval unavailable")?;
        if page.http_status!=200||page.redirect_target.is_some()
            ||page.canonical_url!=url{
            return Err("Crossref DOI endpoint redirected or returned an unverified response".into());
        }
        let catalog_sha=format!("{:x}",Sha256::digest(&page.bytes));
        let record=catalog_work_from_crossref(&ledger,source_id,&page.bytes)
            .map_err(|e|format!("Citation metadata failed identity/quality check: {e:?}"))?;
        let rendered=render_correlated(&ledger,&record,style)
            .map_err(|e|format!("Citation style cannot safely render this record: {e:?}"))?;
        let csl_json=export_csl_json(&ledger,&record)
            .map_err(|e|format!("CSL-compatible evidence export refused: {e:?}"))?;
        // Store exactly these already reviewed, DOI-correlated response bytes.
        // A changed Crossref payload cannot silently rewrite the sealed
        // citation edition/author/date used by an earlier local bibliography.
        ledger.pin_citation_catalog(source_id,&page.bytes)
            .map_err(|_|"Citation catalog changed, cannot overwrite reviewed snapshot")?;
        Ok(ResearchCitationPreviewView{
            source_id:rendered.source_id,
            style:rendered.style.into(),
            bibliography:rendered.bibliography,
            first_note:rendered.first_note,
            catalog_response_sha256:catalog_sha,
            csl_json,
            bibliography_identity_corroborated:rendered.bibliography_identity_corroborated,
            author_identity_corroborated:rendered.author_identity_corroborated,
            publication_details_verified:rendered.publication_details_verified,
            quotation_or_page_verified:rendered.quotation_or_page_verified,
            factual_claim_verified:rendered.factual_claim_verified,
            coverage:rendered.coverage.into(),
            message:"Crossref DOI/title/author metadata matched the existing corroborated Research source. Publication particulars, page locators, quotations and historical/factual interpretations still require manual review.",
        })
    }

    /// Build a complete local bibliography only from the immutable,
    /// per-conversation reviewed DOI catalog snapshots. No network access,
    /// model-authored metadata or guessed missing bibliography fields.
    pub(crate) fn bibliography_preview(
        &self,chat_id:&str,style:Style
    )->Result<ResearchBibliographyView,String>{
        if !safe_id(chat_id){return Err("Select a trusted Research conversation".into());}
        let root=fs::symlink_metadata(&self.root)
            .map_err(|_|"Research evidence directory unavailable")?;
        if !root.file_type().is_dir()||root.file_type().is_symlink(){
            return Err("Research evidence directory untrusted".into());
        }
        let db=self.root.join(format!("{chat_id}.sqlite"));
        let file=fs::symlink_metadata(&db)
            .map_err(|_|"Research evidence ledger unavailable")?;
        if !file.file_type().is_file()||file.file_type().is_symlink(){
            return Err("Research evidence ledger untrusted".into());
        }
        let ledger=EvidenceLedger::open(&db)
            .map_err(|_|"Research evidence ledger unavailable")?;
        let ids=ledger.citation_catalog_source_ids(121)
            .map_err(|_|"Reviewed citation inventory unavailable")?;
        if ids.is_empty(){return Err("No human-reviewed citation snapshots in this conversation".into());}
        if ids.len()>120{return Err("Bibliography exceeds 120 reviewed catalog sources".into());}
        let mut works=Vec::with_capacity(ids.len());
        let mut provenance=HashMap::new();
        for id in ids{
            let pinned=ledger.citation_catalog(&id)
                .map_err(|_|"Citation snapshot digest or source status changed")?
                .ok_or("Citation snapshot no longer exists")?;
            let work=catalog_work_from_crossref(&ledger,&id,&pinned.response_json)
                .map_err(|e|format!("Stored citation no longer passes DOI checks: {e:?}"))?;
            provenance.insert(id,pinned.response_sha256);
            works.push(work);
        }
        let rendered=render_bibliography(&ledger,&works,style)
            .map_err(|e|format!("Citation bibliography refused an unsupported entry: {e:?}"))?;
        let by_id=works.iter().map(|work|(work.source_id.clone(),work))
            .collect::<HashMap<String,_>>();
        let mut csl=Vec::with_capacity(rendered.len());
        let mut body=format!(
            "# Research bibliography — {}\n\n**Human review required.**              Corroborated source identity only; publication details, direct              quotations, pages and factual claims are not independently              verified. No ancient texts or guessed SBL abbreviations.\n\n",
             style.label()
        );
        let mut manifest=Sha256::new();
        for item in &rendered{
            let work=by_id.get(&item.source_id)
                .ok_or("Reviewed source identity lost during bibliography assembly")?;
            let sha=provenance.get(&item.source_id)
                .ok_or("Citation source response hash unavailable")?;
            let mut record=export_csl_json(&ledger,*work)
                .map_err(|_|"Citation structured export rejected corroborated source")?;
            record["_lu_evidence"]["crossref_response_sha256"]=
                serde_json::json!(sha);
            csl.push(record);
            body.push_str("- ");
            body.push_str(&item.bibliography);
            body.push_str("\n");
            manifest.update(item.source_id.as_bytes());
            manifest.update([0]);
            manifest.update(sha.as_bytes());
            manifest.update([0]);
        }
        body.push_str("\n## Crossref catalog response SHA-256 inventory\n\n");
        for item in &rendered{
            let sha=provenance.get(&item.source_id)
                .ok_or("Citation inventory digest unavailable")?;
            body.push_str(&format!("- {}: {}\n",item.source_id,sha));
        }
        if body.len()>2*1024*1024{return Err("Bibliography exceeds native export limit".into());}
        Ok(ResearchBibliographyView{
            style:style.label().into(),source_count:rendered.len(),
            markdown:body,csl_json:serde_json::Value::Array(csl),
            catalog_manifest_sha256:format!("{:x}",manifest.finalize()),
            publication_details_verified:false,
            quotation_or_page_verified:false,factual_claim_verified:false,
            message:"All entries came from sealed chat-owned DOI/author-attested Crossref snapshots. No source, translation, page, quote or factual interpretation is automatically verified.",
        })
    }

    /// Separate supervised Crossref re-review. Existing source, DOI and
    /// original author/title identities are looked up from the chat-owned
    /// database, never from the model or user-provided network parameters.
    /// A revision is proposed, NOT applied, until the native host displays
    /// old and new bibliography text and receives explicit human approval.
    pub(crate) fn prepare_citation_revision(
        &self,chat_id:&str,source_id:&str
    )->Result<ResearchCitationRevisionCandidate,String>{
        if !safe_id(chat_id)||!source_id.starts_with("https://openalex.org/")
            ||source_id.len()>128||source_id.chars().any(char::is_control){
            return Err("Select a known DOI-correlated Research source".into());
        }
        if !self.available(){return Err("Pinned native Crossref client unavailable".into());}
        let root=fs::symlink_metadata(&self.root)
            .map_err(|_|"Research evidence directory unavailable")?;
        if !root.file_type().is_dir()||root.file_type().is_symlink(){
            return Err("Research evidence directory untrusted".into());
        }
        let db=self.root.join(format!("{chat_id}.sqlite"));
        let file=fs::symlink_metadata(&db)
            .map_err(|_|"Research evidence ledger unavailable")?;
        if !file.file_type().is_file()||file.file_type().is_symlink(){
            return Err("Research evidence ledger untrusted".into());
        }
        let ledger=EvidenceLedger::open(&db)
            .map_err(|_|"Research evidence ledger unavailable")?;
        let pinned=ledger.citation_catalog(source_id)
            .map_err(|_|"Prior citation snapshot failed its digest check")?
            .ok_or("No previously reviewed citation to revise")?;
        let original=ledger.discovered_candidate(source_id)
            .map_err(|_|"Original catalog identity unavailable")?;
        let doi=canonical_doi(&original.identifier)
            .ok_or("Original DOI invalid")?;
        let url=format!("https://api.crossref.org/works/{}",encode_doi_component(&doi));
        let fetcher=SecureHttpFetcher{curl_binary:&self.curl,ca_bundle:None};
        let response=fetcher.fetch_bounded(&url,1024*1024)
            .map_err(|_|"Crossref catalog re-review fetch unavailable")?;
        if response.http_status!=200||response.redirect_target.is_some()
            ||response.canonical_url!=url{
            return Err("Crossref catalog revision endpoint is not exact/200".into());
        }
        let old=catalog_work_from_crossref(&ledger,source_id,&pinned.response_json)
            .map_err(|_|"Prior citation is no longer a valid catalog record")?;
        let next=catalog_work_from_crossref(&ledger,source_id,&response.bytes)
            .map_err(|_|"Changed catalog metadata does not match original source")?;
        let previous_bibliography=render_correlated(&ledger,&old,Style::Chicago18)
            .map_err(|_|"Prior citation could not be reviewed")?.bibliography;
        let proposed_bibliography=render_correlated(&ledger,&next,Style::Chicago18)
            .map_err(|_|"Proposed revision could not be reviewed")?.bibliography;
        let new_response_sha256=format!("{:x}",Sha256::digest(&response.bytes));
        if new_response_sha256==pinned.response_sha256{
            return Err("Crossref metadata is unchanged; no revision needed".into());
        }
        Ok(ResearchCitationRevisionCandidate{
            source_id:source_id.to_owned(),
            expected_previous_sha256:pinned.response_sha256,
            new_response_sha256,previous_bibliography,proposed_bibliography,
            proposed_response:response.bytes
        })
    }
    /// Called only after native user comparison and approval. Re-validates
    /// the exact bytes and current ledger source before the atomic CAS.
    pub(crate) fn apply_citation_revision(
        &self,chat_id:&str,candidate:ResearchCitationRevisionCandidate
    )->Result<ResearchCitationRevisionReceipt,String>{
        if !safe_id(chat_id)||candidate.source_id.len()>128
            ||!candidate.source_id.starts_with("https://openalex.org/"){
            return Err("Invalid citation revision conversation or source".into());
        }
        let db=self.root.join(format!("{chat_id}.sqlite"));
        let metadata=fs::symlink_metadata(&db)
            .map_err(|_|"Reviewed catalog database unavailable")?;
        if !metadata.file_type().is_file()||metadata.file_type().is_symlink(){
            return Err("Reviewed catalog database is not trusted".into());
        }
        let mut ledger=EvidenceLedger::open(&db)
            .map_err(|_|"Reviewed catalog database could not be opened")?;
        let _=catalog_work_from_crossref(
            &ledger,&candidate.source_id,&candidate.proposed_response
        ).map_err(|_|"Revision no longer passes corroboration checks")?;
        if format!("{:x}",Sha256::digest(&candidate.proposed_response))
            !=candidate.new_response_sha256{
            return Err("Proposed catalog bytes changed during human approval".into());
        }
        let pinned=ledger.replace_citation_catalog_after_review(
            &candidate.source_id,&candidate.expected_previous_sha256,
            &candidate.proposed_response
        ).map_err(|_|"Stale consent or conflicting DOI citation revision refused")?;
        let revisions=ledger.citation_catalog_revision_receipts(&candidate.source_id)
            .map_err(|_|"Citation revision audit receipt could not be rehashed")?;
        Ok(ResearchCitationRevisionReceipt{
            source_id:candidate.source_id,
            previous_sha256:candidate.expected_previous_sha256,
            current_sha256:pinned.response_sha256,
            revision_count:revisions.len(),
            publication_details_verified:false,
            message:"Native user-reviewed catalog change recorded with prior immutable bytes and SHA-256 audit. Publication details and quotations remain independently unverified.",
        })
    }

    pub(crate) fn corroborate_crossref(
        &self,chat_id:&str,source_id:&str
    )->Result<ResearchCorroborationView,String>{
        self.corroborate_crossref_admitted(chat_id,source_id,None)
    }
    fn corroborate_crossref_admitted(
        &self,chat_id:&str,source_id:&str,cancel:Option<&AtomicBool>
    )->Result<ResearchCorroborationView,String>{
        if !safe_id(chat_id)||source_id.len()>128
          ||!source_id.starts_with("https://openalex.org/")
          ||source_id.chars().any(char::is_control){
            return Err("Invalid discovered research source".into());
        }
        if !self.available(){
            return Err("Reviewed Lu research HTTPS runtime not installed".into());
        }
        let dir=fs::symlink_metadata(&self.root)
            .map_err(|_|"Research ledger directory unavailable")?;
        if !dir.file_type().is_dir()||dir.file_type().is_symlink(){
            return Err("Research ledger directory untrusted".into());
        }
        let db=self.root.join(format!("{chat_id}.sqlite"));
        let metadata=fs::symlink_metadata(&db)
            .map_err(|_|"Existing research evidence ledger unavailable")?;
        if !metadata.file_type().is_file()||metadata.file_type().is_symlink(){
            return Err("Research evidence ledger untrusted".into());
        }
        let mut ledger=EvidenceLedger::open(&db)
            .map_err(|_|"Research evidence ledger unavailable")?;
        let discovered=ledger.discovered_candidate(source_id)
            .map_err(|_|"This source was not discovered in this chat")?;
        if ledger.status(source_id).map_err(|_|"Evidence status unavailable")?
           !=SourceStatus::Discovered {
            return Err("This source is no longer an uncorroborated lead".into());
        }
        let doi=canonical_doi(&discovered.identifier)
            .ok_or("Stored source DOI is invalid")?;
        let url=format!("https://api.crossref.org/works/{}",encode_doi_component(&doi));
        let fetcher=SecureHttpFetcher{
            curl_binary:&self.curl,ca_bundle:None,
        };
        let response=match cancel{
            Some(flag)=>fetcher.fetch_cancellable(&url,flag),
            None=>fetcher.fetch(&url),
        }.map_err(|_|"Crossref metadata retrieval unavailable")?;
        if response.http_status!=200{
            return Err("Crossref did not return a verified work record".into());
        }
        let record=parse_crossref_metadata(&response.bytes,&doi,&response.canonical_url)
            .map_err(str::to_owned)?;
        let publisher_url=crossref_publisher_url(&response.bytes,&doi);
        let catalog_sha256=format!("{:x}",Sha256::digest(&response.bytes));
        match ledger.corroborate(source_id,&record){
            Ok(())=>{
                if let Some(url)=publisher_url.as_deref(){
                    ledger.bind_crossref_publisher_url(
                        source_id,url,&catalog_sha256
                    ).map_err(|_|"Crossref publisher-link receipt was rejected")?;
                }
                Ok(ResearchCorroborationView{
                source_id:source_id.to_owned(),
                verification_status:"metadata_corroborated",
                record_url:record.record_url,
                citation_verified:false,
                message:"Independent metadata matches the discovered lead. Publisher URL is recorded only when independently supplied by Crossref; no passage or citation is yet verified.",
                })
            },
            Err(EvidenceError::MetadataMismatch)=>Ok(ResearchCorroborationView{
                source_id:source_id.to_owned(),
                verification_status:"metadata_mismatch",
                record_url:record.record_url,
                citation_verified:false,
                message:"Independent metadata differs from discovery. This lead cannot be cited.",
            }),
            Err(_)=>Err("Research corroboration could not be persisted".into()),
        }
    }
}
#[cfg(test)]
mod tests{
    #[test]
    fn p07_bibliography_reopens_sealed_crossref_catalog_offline_after_restart(){
        let root=std::env::temp_dir().join(format!(
            "lu-p07-offline-{}-{}",std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        let db_root=root.join("lu-research-evidence");
        fs::create_dir_all(&db_root).unwrap();
        let db=db_root.join("c-research.sqlite");
        let mut ledger=EvidenceLedger::open(&db).unwrap();
        ledger.discover(&Candidate{
            id:"https://openalex.org/W012345".into(),title:"Study".into(),
            author:"Jane Doe".into(),identifier:"doi:10.1000/example".into()
        }).unwrap();
        ledger.corroborate("https://openalex.org/W012345",&TrustedBibliography{
            record_url:"https://api.crossref.org/works/10.1000/example".into(),
            title:"Study".into(),author:"Jane Doe".into(),
            identifier:"doi:10.1000/example".into()
        }).unwrap();
        let bytes=serde_json::json!({"message":{
            "DOI":"10.1000/example","type":"book","title":["Study"],
            "author":[{"given":"Jane","family":"Doe"}],
            "issued":{"date-parts":[[2024]]},"publisher":"Academic Press"
        }}).to_string();
        let checked=catalog_work_from_crossref(
            &ledger,"https://openalex.org/W012345",bytes.as_bytes()
        ).unwrap();
        assert_eq!(checked.year,Some(2024));
        ledger.pin_citation_catalog(
            "https://openalex.org/W012345",bytes.as_bytes()
        ).unwrap();
        drop(ledger);
        // A brand-new host with no external HTTP client can rebuild the
        // complete bibliography from only the sealed per-chat SQLite.
        let research=ResearchDesktop::new(root.join("unused"),root.clone());
        let view=research.bibliography_preview("c-research",Style::Chicago18).unwrap();
        assert_eq!(view.source_count,1);
        assert!(view.markdown.contains("Doe, Jane"));
        assert!(view.markdown.contains("SHA-256 inventory"));
        assert!(view.csl_json[0].get("_lu_evidence").is_some());
        assert!(!view.publication_details_verified);
        assert!(view.catalog_manifest_sha256.len()==64);
        assert!(research.bibliography_preview("c-other",Style::Apa7).is_err());
        drop(research);
        fs::remove_dir_all(root).unwrap();
    }

    use super::*;
    #[test]
    fn native_browser_read_is_kept_in_research_ledger_not_promoted_to_citation(){
        let dir=std::env::temp_dir().join(format!("lu-research-browser-{}-{}",
            std::process::id(),std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let host=ResearchDesktop::new(dir.join("unused-resources"),dir.clone());
        let view=host.record_browser_read("c-first","g-1","browser-call-1",
            "https://archive.example/article","READ",
            "The observed web content",false).unwrap();
        assert_eq!(view.verification_status,"browser_observed_untrusted");
        assert!(!view.citation_verified);
        let first=EvidenceLedger::open(
            &dir.join("lu-research-evidence/c-first.sqlite")).unwrap();
        assert!(first.has_browser_excerpt("g-1","browser-call-1","observed web").unwrap());
        let listed=host.browser_evidence("c-first").unwrap();
        assert_eq!(listed.len(),1);
        assert_eq!(listed[0].excerpt,"The observed web content");
        assert!(host.browser_evidence("c-second").unwrap().is_empty());
        assert!(matches!(EvidenceLedger::open(
            &dir.join("lu-research-evidence/c-second.sqlite")).unwrap()
            .browser_observation("g-1","browser-call-1"),Err(EvidenceError::Missing)));
        assert!(host.record_browser_read("c-first","g-1","browser-call-1",
            "https://archive.example/other","READ","The observed web content",false).is_err());
        assert!(host.record_browser_read("c-../escape","g-1","browser-call-2",
            "https://archive.example/article","READ","test",false).is_err());
        drop(first);
        let _=std::fs::remove_dir_all(dir);
    }
    #[test]
    fn exact_browser_quote_is_not_an_automatically_verified_scholarly_citation(){
        let dir=std::env::temp_dir().join(format!("lu-quote-{}-{}",
            std::process::id(),std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let host=ResearchDesktop::new(dir.join("unused-resources"),dir.clone());
        host.record_browser_read("c-first","g-quote","browser-call-quote",
            "https://archive.example/article","READ",
            "Observed actual quote in this source.",false).unwrap();
        let proof=host.verify_browser_quote("c-first","g-quote","browser-call-quote",
            "actual quote",None).unwrap();
        assert_eq!(proof.source_url,"https://archive.example/article");
        assert!(!proof.publisher_link_matched);
        assert!(!proof.document_accessed);
        assert!(!proof.citation_verified);
        assert!(host.verify_browser_quote("c-first","g-quote","browser-call-quote",
            "invented quote",None).is_err());
        assert!(host.verify_browser_quote("c-other","g-quote","browser-call-quote",
            "actual quote",None).is_err());
        assert!(host.verify_browser_quote("c-first","g-quote","browser-call-quote",
            "actual quote",Some("invented-study")).is_err());
        let _=std::fs::remove_dir_all(dir);
    }
    #[test]
    fn staged_builder_signed_windows_curl_matches_exact_compiled_digest(){
        #[cfg(windows)]
        if let Some(expected)=option_env!("LU_RESEARCH_CURL_SHA256"){
            let candidate=Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("resources/lu-research/curl.exe");
            assert!(verified_file(&candidate,expected),
                "Windows Research client or compiled digest differs");
        }
    }

    #[test]
    fn independent_crossref_work_metadata_must_match_exact_stored_doi(){
        let body=serde_json::json!({"message":{
            "DOI":"10.1234/study1",
            "title":["A Tested Study"],
            "author":[{"given":"Jane","family":"Doe"},{"given":"John","family":"Smith"}]
        }}).to_string();
        let record=parse_crossref_metadata(body.as_bytes(),"10.1234/study1",
            "https://api.crossref.org/works/10.1234%2Fstudy1").unwrap();
        assert_eq!(record.title,"A Tested Study");
        assert_eq!(record.author,"Jane Doe; John Smith");
        assert_eq!(record.identifier,"10.1234/study1");
        assert!(parse_crossref_metadata(body.as_bytes(),"10.1234/other",
            "https://api.crossref.org/works/10.1234%2Fother").is_err());
        let malformed=serde_json::json!({"message":{
            "DOI":"10.1234/study1","title":["A Tested Study"],"author":[]
        }}).to_string();
        assert!(parse_crossref_metadata(malformed.as_bytes(),"10.1234/study1",
            "https://api.crossref.org/works/10.1234%2Fstudy1").is_err());
        assert_eq!(encode_doi_component("10.1234/study1"),"10.1234%2Fstudy1");
    }
    #[test]
    fn crossref_prefers_unambiguous_doi_bound_pdf_and_rejects_ambiguous_catalog_links(){
        let json=serde_json::json!({"message":{
            "DOI":"10.1234/verified",
            "URL":"https://doi.org/10.1234/verified",
            "link":[{"URL":"https://publisher.example/full.pdf",
                "content-type":"application/pdf"}]
        }}).to_string();
        assert_eq!(crossref_publisher_url(json.as_bytes(),"10.1234/verified"),
            Some("https://publisher.example/full.pdf".into()));
        assert!(crossref_publisher_url(json.as_bytes(),"10.1234/other").is_none());
        let ambiguous=serde_json::json!({"message":{
            "DOI":"10.1234/verified",
            "URL":"https://doi.org/10.1234/verified",
            "link":[
                {"URL":"https://publisher.example/a.pdf","content-type":"application/pdf"},
                {"URL":"https://publisher.example/b.pdf","content-type":"application/pdf"}
            ]
        }}).to_string();
        assert!(crossref_publisher_url(ambiguous.as_bytes(),"10.1234/verified").is_none());
    }
    #[test]
    fn crossref_publisher_link_is_independent_and_doi_checked(){
        let matched=serde_json::json!({"message":{
            "DOI":"10.1234/study",
            "URL":"https://publisher.example/articles/study",
            "title":["Study"],"author":[{"given":"J","family":"Doe"}]
        }}).to_string();
        assert_eq!(crossref_publisher_url(matched.as_bytes(),"10.1234/study"),
            Some("https://publisher.example/articles/study".into()));
        assert!(crossref_publisher_url(matched.as_bytes(),"10.1234/other").is_none());
        let bad=matched.replace("https://publisher.example/articles/study",
            "https://alice@publisher.example/articles/study");
        assert!(crossref_publisher_url(bad.as_bytes(),"10.1234/study").is_none());
        let missing=matched.replace("https://publisher.example/articles/study",
            "http://publisher.example/articles/study");
        assert!(crossref_publisher_url(missing.as_bytes(),"10.1234/study").is_none());
    }
    #[test]
    fn no_unbounded_research_pagination_or_hidden_runtime(){
        let host=ResearchDesktop::new(PathBuf::from("/missing"),
            PathBuf::from("/missing"));
        assert!(host.search_page("c-owner","research",0).is_err());
        assert!(host.search_page("c-owner","research",6).is_err());
        assert!(host.search_page("c-owner","research",1).is_err());
    }
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
