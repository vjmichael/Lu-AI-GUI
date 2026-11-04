//! TOOLS-10: Lu-owned, explicit NetworkRead native tool facade.
//! Search hits remain unverified bibliographic leads, never citation receipts.
//! The trusted host selects curl, certificate configuration and grants.
//! No MCP, browser credential exposure, generic shell or model-created rights.
use crate::{
    contracts::{ModuleKind,ModuleManifest,NativeTool,ProtoPayload,ToolDescriptor,ToolEffect,
      ToolFailure,ToolFuture,ToolInvocation,ToolOutput,CORE_CONTRACT_VERSION},
    model::{expect_fields,ModelToolCatalog,ModelToolCodec,ModelToolDefinition,ModelToolError},
    registry::NativeToolRegistry,
    web_http_fetch::SecureHttpFetcher,
    web_search_openalex::search_openalex,
    wire::proto,
};
use prost::Message;
use serde_json::{json,Map,Value};
use std::{path::{Path,PathBuf},sync::Arc};

const MODULE:&str="lu.web";
const FETCH:&str="lu.web.fetch";
const SEARCH:&str="lu.web.academic_search";
const FETCH_IN:&str="type.googleapis.com/lu.v1.WebFetchRequest";
const FETCH_OUT:&str="type.googleapis.com/lu.v1.WebFetchResult";
const SEARCH_IN:&str="type.googleapis.com/lu.v1.WebAcademicSearchRequest";
const SEARCH_OUT:&str="type.googleapis.com/lu.v1.WebAcademicSearchResult";
fn fail(code:&str,message:&str)->ToolFailure {
    ToolFailure::new(code,message,false)
}
fn valid_https_candidate(url:&str)->bool {
    url.starts_with("https://") && url.len()<=2048 &&
    !url.chars().any(char::is_control) && !url[8..].split('/').next()
        .unwrap_or("").contains('@')
}
#[derive(Clone)]
struct ApprovedNetwork {
    binary:PathBuf,
    ca:Option<PathBuf>,
}
#[derive(Clone,Copy)]
enum Operation {Fetch,Search}
struct WebTool {
    descriptor:ToolDescriptor,
    trusted:Arc<ApprovedNetwork>,
    operation:Operation,
}
impl NativeTool for WebTool {
    fn descriptor(&self)->&ToolDescriptor {&self.descriptor}
    fn invoke(&self,call:ToolInvocation)->ToolFuture<'_> {
        let trusted=Arc::clone(&self.trusted);
        let operation=self.operation;
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                let fetcher=SecureHttpFetcher{
                    curl_binary:&trusted.binary,
                    ca_bundle:trusted.ca.as_deref(),
                };
                match operation {
                    Operation::Fetch => {
                        let request=proto::WebFetchRequest::decode(call.input.bytes.as_slice())
                            .map_err(|_|fail("BAD_PROTOBUF","invalid fetch request"))?;
                        if request.encode_to_vec()!=call.input.bytes ||
                           !valid_https_candidate(&request.url) {
                            return Err(fail("BAD_INPUT","invalid HTTPS fetch request"));
                        }
                        let page=fetcher.fetch(&request.url)
                            .map_err(|_|fail("FETCH_DENIED","HTTPS fetch failed or was refused"))?;
                        let text=String::from_utf8(page.bytes)
                            .map_err(|_|fail("CONTENT_NOT_TEXT","response is not UTF-8 text"))?;
                        let result=proto::WebFetchResult {
                            source_url:page.canonical_url,http_status:page.http_status,
                            content:text,content_sha256:page.sha256,truncated:false,
                        };
                        Ok(ToolOutput{payload:ProtoPayload{
                            type_url:FETCH_OUT.into(),bytes:result.encode_to_vec()
                        }})
                    }
                    Operation::Search => {
                        let request=proto::WebAcademicSearchRequest::decode(call.input.bytes.as_slice())
                            .map_err(|_|fail("BAD_PROTOBUF","invalid search request"))?;
                        if request.encode_to_vec()!=call.input.bytes ||
                           request.query.trim().is_empty() || request.query.len()>512 ||
                           request.query.chars().any(char::is_control) ||
                           !(1..=100).contains(&request.page) {
                            return Err(fail("BAD_INPUT","invalid academic search request"));
                        }
                        let page=search_openalex(&fetcher,&request.query,request.page)
                            .map_err(|_|fail("SEARCH_UNAVAILABLE","academic provider unavailable"))?;
                        let result=proto::WebAcademicSearchResult {
                            leads:page.leads.into_iter().map(|l|proto::WebAcademicSearchLead{
                                provider:l.provider,provider_id:l.provider_id,
                                doi:l.doi,title:l.title,authors:l.authors,catalog_url:l.catalog_url,
                            }).collect(),
                            page:page.page,next_page:page.next_page,
                            observed_sha256:page.observed_sha256,leads_verified:false,
                        };
                        Ok(ToolOutput{payload:ProtoPayload{
                            type_url:SEARCH_OUT.into(),bytes:result.encode_to_vec()
                        }})
                    }
                }
            }).await.map_err(|_|fail("WORKER_FAILED","web worker stopped"))?
        })
    }
}
struct FetchCodec;
impl ModelToolCodec for FetchCodec {
    fn input_type_url(&self)->&'static str {FETCH_IN}
    fn output_type_url(&self)->&'static str {FETCH_OUT}
    fn encode(&self,args:&Map<String,Value>)->Result<ProtoPayload,ModelToolError>{
        expect_fields(args,&["url"],&["url"])?;
        let url=args.get("url").and_then(Value::as_str)
            .ok_or_else(||ModelToolError::InvalidArguments("url must be text".into()))?;
        if !valid_https_candidate(url) {return Err(ModelToolError::InvalidArguments(
            "expected bounded HTTPS URL".into()));}
        Ok(ProtoPayload{type_url:FETCH_IN.into(),
            bytes:proto::WebFetchRequest{url:url.into()}.encode_to_vec()})
    }
    fn decode(&self,result:&ProtoPayload)->Result<Value,ModelToolError>{
        let data=proto::WebFetchResult::decode(result.bytes.as_slice())
            .map_err(|_|ModelToolError::InvalidResult("bad fetch result".into()))?;
        Ok(json!({"source_url":data.source_url,"http_status":data.http_status,
            "content":data.content,"sha256":data.content_sha256,
            "truncated":data.truncated,"source_is_untrusted":true}))
    }
}
struct SearchCodec;
impl ModelToolCodec for SearchCodec {
    fn input_type_url(&self)->&'static str {SEARCH_IN}
    fn output_type_url(&self)->&'static str {SEARCH_OUT}
    fn encode(&self,args:&Map<String,Value>)->Result<ProtoPayload,ModelToolError>{
        expect_fields(args,&["query","page"],&["query"])?;
        let query=args.get("query").and_then(Value::as_str)
            .ok_or_else(||ModelToolError::InvalidArguments("query must be text".into()))?;
        if query.trim().is_empty() || query.len()>512 || query.chars().any(char::is_control) {
            return Err(ModelToolError::InvalidArguments("invalid research query".into()));
        }
        let page=match args.get("page") {
            None=>1,
            Some(Value::Number(n))=>n.as_u64().filter(|n|(1..=100).contains(n))
                .ok_or_else(||ModelToolError::InvalidArguments("invalid page".into()))? as u32,
            _=>return Err(ModelToolError::InvalidArguments("invalid page".into())),
        };
        Ok(ProtoPayload{type_url:SEARCH_IN.into(),
            bytes:proto::WebAcademicSearchRequest{query:query.into(),page}.encode_to_vec()})
    }
    fn decode(&self,result:&ProtoPayload)->Result<Value,ModelToolError>{
        let data=proto::WebAcademicSearchResult::decode(result.bytes.as_slice())
            .map_err(|_|ModelToolError::InvalidResult("bad search result".into()))?;
        if data.leads_verified {return Err(ModelToolError::InvalidResult(
            "search result cannot claim independent source verification".into()));}
        Ok(json!({"leads":data.leads.into_iter().map(|l|json!({
            "provider":l.provider,"provider_id":l.provider_id,
            "doi":l.doi,"title":l.title,"authors":l.authors,
            "catalog_url":l.catalog_url,"verified":false
        })).collect::<Vec<_>>(),"page":data.page,"next_page":data.next_page,
        "provider_response_sha256":data.observed_sha256,
        "source_is_untrusted":true,"leads_verified":false}))
    }
}
pub fn register_web_research_tools(
    registry:&NativeToolRegistry,
    catalog:&mut ModelToolCatalog,
    trusted_curl:&Path,
    host_ca_bundle:Option<&Path>,
)->Result<(),String>{
    let binary=trusted_curl.canonicalize().map_err(|_|"curl executable unavailable")?;
    if !binary.is_file() {return Err("curl executable is not a file".into());}
    let ca=host_ca_bundle.map(|path|path.canonicalize()
        .map_err(|_|"host CA bundle unavailable")).transpose()?;
    if ca.as_ref().is_some_and(|path|!path.is_file()) {
        return Err("CA bundle is not a file".into());
    }
    let trusted=Arc::new(ApprovedNetwork{binary,ca});
    let descriptor=|id:&str,title:&str,input:&str,output:&str|ToolDescriptor{
        id:id.into(),display_name:title.into(),effect:ToolEffect::NetworkRead,
        input_type_url:input.into(),output_type_url:output.into(),
    };
    let fetch=descriptor(FETCH,"Fetch bounded HTTPS text",FETCH_IN,FETCH_OUT);
    let search=descriptor(SEARCH,"Discover academic publications",SEARCH_IN,SEARCH_OUT);
    registry.register(ModuleManifest{
        id:MODULE.into(),display_name:"Lu native web research".into(),
        contract_version:CORE_CONTRACT_VERSION,kind:ModuleKind::Core,
        tools:vec![fetch.clone(),search.clone()],
        panels:vec![],workflows:vec![],
    },vec![
        Arc::new(WebTool{descriptor:fetch,trusted:Arc::clone(&trusted),operation:Operation::Fetch}),
        Arc::new(WebTool{descriptor:search,trusted,operation:Operation::Search}),
    ]).map_err(|e|format!("web registry refused: {e:?}"))?;
    for (name,id,description,parameters,codec) in [
      ("lu_web_fetch",FETCH,"Fetch read-only HTTPS text from an authorized public origin.",
        json!({"type":"object","properties":{"url":{"type":"string","maxLength":2048}},
              "required":["url"],"additionalProperties":false}),Arc::new(FetchCodec) as Arc<dyn ModelToolCodec>),
      ("lu_web_academic_search",SEARCH,"Find unverified DOI leads in academic catalogs.",
        json!({"type":"object","properties":{
            "query":{"type":"string","maxLength":512},
            "page":{"type":"integer","minimum":1,"maximum":100}},
            "required":["query"],"additionalProperties":false}),Arc::new(SearchCodec) as Arc<dyn ModelToolCodec>),
    ] {
        catalog.insert(registry,ModelToolDefinition{
            name:name.into(),native_tool_id:id.into(),description:description.into(),
            parameters,codec,
        }).map_err(|e|format!("web catalog refused: {e:?}"))?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn candidate_urls_do_not_accept_model_credentials_or_local_schemes() {
        for input in ["http://example.org","file:///etc/passwd",
                      "https://username:password@example.org","https://example.org\nunsafe"] {
            assert!(!valid_https_candidate(input));
        }
        assert!(valid_https_candidate("https://api.openalex.org/works"));
    }
    #[test]
    fn a_search_result_cannot_smuggle_verified_true() {
        let codec=SearchCodec;
        let spoof=proto::WebAcademicSearchResult{
            leads:vec![],page:1,next_page:None,observed_sha256:"test".into(),
            leads_verified:true,
        };
        assert!(matches!(codec.decode(&ProtoPayload{
            type_url:SEARCH_OUT.into(),bytes:spoof.encode_to_vec(),
        }),Err(ModelToolError::InvalidResult(_))));
    }
}
