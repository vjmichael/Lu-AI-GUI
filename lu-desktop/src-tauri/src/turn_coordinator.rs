//! CP-05A: one host-side decision for ordinary human Send.
//! A route is only a candidate for an existing native admission handler.
//! It contains no capability, worker handle, approval or execution receipt.
//! Scoped handlers still persist/verify the human turn and obtain consent.
use crate::{browser_model_desktop, core_utility_desktop, host_clock, perf01_router,
    reader_desktop, reviewed_app_desktop, workspace_desktop};
use lu_core::native_core_utility_turn::CoreUtilityRequest;

#[derive(Debug)]
pub(crate) enum TurnRoute {
    Model,
    Plan,
    CpuWorkspace(String),
    CpuChatCounts,
    CpuModelStatus,
    CpuChatLiteral(String),
    CpuClock,
    Browser(String),
    Git,
    Reader { exact: bool },
    Workspace,
    Specialist(&'static str),
    Utility(CoreUtilityRequest),
}

/// Ordinary Send is the only composer ingress. Legacy RPC callers can still
/// explicitly request a reviewed workspace or inert plan; these are exclusive
/// compatibility modes, and neither is a capability grant.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(crate) enum SendMode { Ordinary, ReviewedWorkspace, DraftPlan }
impl SendMode {
    pub(crate) fn from_legacy(workspace:Option<bool>,plan:Option<bool>)->Result<Self,String>{
        match (workspace.unwrap_or(false),plan.unwrap_or(false)){
            (false,false)=>Ok(Self::Ordinary),
            (true,false)=>Ok(Self::ReviewedWorkspace),
            (false,true)=>Ok(Self::DraftPlan),
            (true,true)=>Err("Choose only one explicit reviewed mode".into()),
        }
    }
}

/// The inputs are the original native Send arguments, never augmented model
/// prompts, recalled memory or worker output. Token limits are validated,
/// not used as a tool-mode sentinel; native step limits remain host-owned.
/// A caller cannot acquire resource authority by picking a route.
pub(crate) fn select(prompt:&str, image_refs:&[String], max_new_tokens:u32,
    mode:SendMode)->Result<TurnRoute,String>{
    let workspace=mode==SendMode::ReviewedWorkspace;
    let plan=mode==SendMode::DraftPlan;
    if !(1..=4096).contains(&max_new_tokens){return Err("Invalid bounded Send token request".into())}
    if crate::contains_document_context(prompt){
        if workspace||plan{
            return Err("Send attached documents with an ordinary chat message.".into())
        }
        return Ok(TurnRoute::Model)
    }
    let images=!image_refs.is_empty();
    if plan{
        if images||max_new_tokens>2048{
            return Err("Draft planning requires text only and bounded generation".into())
        }
        return Ok(TurnRoute::Plan)
    }
    // An explicit no-tools preference is a native veto, even for a
    // formerly deterministic scoped route. Never negotiate around it.
    if lu_core::capability_router::user_declines_tools(prompt){
        if workspace{return Err("Workspace mode conflicts with your no-tools request".into())}
        return Ok(TurnRoute::Model)
    }
    match perf01_router::classify(prompt,images,workspace){
        perf01_router::Route::ApprovedWorkspaceLiteral(needle)=>{
            return Ok(TurnRoute::CpuWorkspace(needle))
        }
        perf01_router::Route::CurrentChatCounts=>return Ok(TurnRoute::CpuChatCounts),
        perf01_router::Route::LocalModelStatus=>return Ok(TurnRoute::CpuModelStatus),
        perf01_router::Route::CurrentChatLiteral(needle)=>return Ok(TurnRoute::CpuChatLiteral(needle)),
        perf01_router::Route::ModelRequired(_)=>{}
    }
    if !workspace&&!images&&host_clock::explicit_current_time_request(prompt){
        return Ok(TurnRoute::CpuClock)
    }
    let reviewed=!workspace&&!images;
    if reviewed{
        if let Some(search)=browser_model_desktop::public_search_intent(prompt)?{
            return Ok(TurnRoute::Browser(search.origin.into()))
        }
        if let Some(origin)=browser_model_desktop::explicit_browse_origin(prompt){
            return Ok(TurnRoute::Browser(origin))
        }
        if workspace_desktop::explicit_read_git_request(prompt){
            workspace_desktop::validate_workspace_send_request(prompt,image_refs,max_new_tokens).map_err(str::to_owned)?;
            return Ok(TurnRoute::Git)
        }
        // Exact file outranks broad directory classification.
        if reader_desktop::explicit_reader_file_path(prompt).is_some(){
            return Ok(TurnRoute::Reader { exact: true })
        }
    }
    if workspace||(!images&&workspace_desktop::explicit_read_workspace_request(prompt)){
        workspace_desktop::validate_workspace_send_request(prompt,image_refs,max_new_tokens).map_err(str::to_owned)?;
        return Ok(TurnRoute::Workspace)
    }
    if reviewed{
        if reader_desktop::explicit_reader_document_request(prompt){return Ok(TurnRoute::Reader { exact: false })}
        if let Some(kind)=reviewed_app_desktop::explicit_specialist_request(prompt){return Ok(TurnRoute::Specialist(kind))}
        if reviewed_app_desktop::explicit_media_metadata_request(prompt){return Ok(TurnRoute::Specialist("media"))}
        if let Some(request)=core_utility_desktop::explicit_core_utility_request(prompt){return Ok(TurnRoute::Utility(request))}
    }
    Ok(TurnRoute::Model)
}

#[cfg(test)]
mod tests{
    use super::*;
    fn route(s:&str)->TurnRoute{select(s,&[],512,SendMode::Ordinary).unwrap()}
    // Shared native/renderer fixture. Run this against the exact old ordinary
    // (both legacy flags false) route and again after simplifying ingress.
    fn parity_signature(result:Result<TurnRoute,String>)->String{
        match result{
            Err(_)=>"error".into(),
            Ok(TurnRoute::Model)=>"model".into(),
            Ok(TurnRoute::Plan)=>"plan".into(),
            Ok(TurnRoute::CpuWorkspace(q))=>format!("cpu-workspace:{q}"),
            Ok(TurnRoute::CpuChatCounts)=>"cpu-counts".into(),
            Ok(TurnRoute::CpuModelStatus)=>"cpu-status".into(),
            Ok(TurnRoute::CpuChatLiteral(q))=>format!("cpu-chat:{q}"),
            Ok(TurnRoute::CpuClock)=>"cpu-clock".into(),
            Ok(TurnRoute::Browser(o))=>format!("browser:{o}"),
            Ok(TurnRoute::Git)=>"git".into(),
            Ok(TurnRoute::Reader{exact:true})=>"reader-exact".into(),
            Ok(TurnRoute::Reader{exact:false})=>"reader-picker".into(),
            Ok(TurnRoute::Workspace)=>"workspace".into(),
            Ok(TurnRoute::Specialist(kind))=>format!("specialist:{kind}"),
            Ok(TurnRoute::Utility(r))=>format!("utility:{}",core_utility_desktop::utility_native_id(&r)),
        }
    }
    #[test]
    fn cp05d4_ordinary_route_parity_matrix(){
        let fixtures:Vec<serde_json::Value>=serde_json::from_str(include_str!("../../tests/cp05-routing-parity.json")).unwrap();
        for case in fixtures{
            let prompt=case["prompt"].as_str().unwrap();
            let expected=case["route"].as_str().unwrap();
            for budget in [1,256,512,513,4096]{
                assert_eq!(parity_signature(select(prompt,&[],budget,SendMode::Ordinary)),expected,"{prompt} / {budget}");
            }
            assert_eq!(parity_signature(select(prompt,&["img-1".into()],512,SendMode::Ordinary)),"model","{prompt}");
            assert_eq!(parity_signature(select(&format!("{prompt}\n[Attached text file: source]"),&[],512,SendMode::Ordinary)),"model","{prompt}");
        }
    }
    #[test]
    fn legacy_rpc_mode_adapter_preserves_only_exclusive_explicit_modes(){
        for ordinary in [(None,None),(Some(false),None),(None,Some(false)),(Some(false),Some(false))]{
            assert_eq!(SendMode::from_legacy(ordinary.0,ordinary.1).unwrap(),SendMode::Ordinary);
        }
        assert_eq!(SendMode::from_legacy(Some(true),None).unwrap(),SendMode::ReviewedWorkspace);
        assert_eq!(SendMode::from_legacy(None,Some(true)).unwrap(),SendMode::DraftPlan);
        assert!(SendMode::from_legacy(Some(true),Some(true)).is_err());
        assert!(matches!(select("Read my project",&[],512,SendMode::ReviewedWorkspace).unwrap(),TurnRoute::Workspace));
        assert!(matches!(select("Plan my saved project",&[],512,SendMode::DraftPlan).unwrap(),TurnRoute::Plan));
        assert!(select("Plan my saved project",&[],2049,SendMode::DraftPlan).is_err());
        assert!(select("List files in this folder",&["img-1".into()],512,SendMode::ReviewedWorkspace).is_err());
    }
    #[test]
    fn exact_cpu_and_reviewed_precedence(){
        assert!(matches!(route("Search this chat for: needle"),TurnRoute::CpuChatLiteral(q) if q=="needle"));
        assert!(matches!(route("Search this folder for: needle"),TurnRoute::CpuWorkspace(q) if q=="needle"));
        assert!(matches!(route("Show current chat counts"),TurnRoute::CpuChatCounts));
        assert!(matches!(route("Show local model status"),TurnRoute::CpuModelStatus));
        assert!(matches!(route("What time is it?"),TurnRoute::CpuClock));
        assert!(matches!(route("Read /home/operator/report.pdf"),TurnRoute::Reader { exact: true }));
        assert!(matches!(route("List files in this folder"),TurnRoute::Workspace));
        assert!(matches!(route("Check git status in this repository"),TurnRoute::Git));
        assert!(matches!(route("Summarize this PDF document"),TurnRoute::Reader { exact: false }));
        assert!(matches!(route("Inspect the codec and duration of my local WAV file"),TurnRoute::Specialist("media")));
        assert!(matches!(route("Check if python is installed on this computer"),TurnRoute::Utility(_)));
    }
    #[test]
    fn provider_selection_is_exact_and_not_a_grant(){
        assert!(matches!(route("Search the web for rust docs"),TurnRoute::Browser(o) if o==browser_model_desktop::PUBLIC_SEARCH_ORIGIN));
        assert!(matches!(route("Search DuckDuckGo for rust docs"),TurnRoute::Browser(o) if o==browser_model_desktop::HTML_SEARCH_ORIGIN));
        assert!(matches!(route("Browse https://example.org"),TurnRoute::Browser(o) if o=="https://example.org"));
        assert!(select("Search the web for /home/key",&[],512,SendMode::Ordinary).is_err());
        assert!(matches!(route("Do not browse https://example.org"),TurnRoute::Model));
    }
    #[test]
    fn attachment_modes_and_legacy_budget_do_not_mint_scopes(){
        for prompt in ["Search DuckDuckGo for docs","Search this folder for: needle","What time is it?","Read /home/operator/report.pdf"]{
            assert!(matches!(select(prompt,&["img-1".into()],512,SendMode::Ordinary).unwrap(),TurnRoute::Model));
            assert!(matches!(route(&format!("{prompt}\n[Attached text file: doc]")),TurnRoute::Model));
        }
        assert!(select("Read [Attached text file: doc]",&[],512,SendMode::ReviewedWorkspace).is_err());
        assert!(select("Read [Attached text file: doc]",&[],512,SendMode::DraftPlan).is_err());
        assert!(select("Read /home/a.pdf",&["img-1".into()],512,SendMode::DraftPlan).is_err());
        assert!(SendMode::from_legacy(Some(true),Some(true)).is_err());
        assert!(matches!(select("Plan my project",&[],512,SendMode::DraftPlan).unwrap(),TurnRoute::Plan));
        for budget in [1,256,512,513,4096]{
            assert!(matches!(select("Search DuckDuckGo for docs",&[],budget,SendMode::Ordinary).unwrap(),TurnRoute::Browser(o) if o==browser_model_desktop::HTML_SEARCH_ORIGIN));
            assert!(matches!(select("Search this folder for: needle",&[],budget,SendMode::Ordinary).unwrap(),TurnRoute::CpuWorkspace(q) if q=="needle"));
            assert!(matches!(select("Read /home/operator/report.pdf",&[],budget,SendMode::Ordinary).unwrap(),TurnRoute::Reader{exact:true}));
            assert!(matches!(select("List files in this folder",&[],budget,SendMode::Ordinary).unwrap(),TurnRoute::Workspace));
        }
        for invalid in [0,4097]{assert!(select("Search this folder for: needle",&[],invalid,SendMode::Ordinary).is_err());
            assert!(select("Search DuckDuckGo for docs",&[],invalid,SendMode::Ordinary).is_err());}
        assert!(matches!(select("What time is it?",&[],512,SendMode::ReviewedWorkspace).unwrap(),TurnRoute::Workspace));
    }
    #[test]
    fn original_no_tools_preference_vetoes_native_selection(){
        for p in ["List files in this folder without tools","Search the web for Rust without using tools","Search this folder for: invoices without tools","Do not use tools to check Git status"]{
            assert!(matches!(route(p),TurnRoute::Model),"{p}");
        }
        assert!(select("Read my project without tools",&[],512,SendMode::ReviewedWorkspace).is_err());
    }

}
