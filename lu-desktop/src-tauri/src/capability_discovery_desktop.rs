//! CP-05B: descriptive, pre-grant discovery for the ordinary native chat host.
//! No resource picker, registry instance, worker, model tool call or authority.
use crate::{DesktopState,environment_context,reviewed_app_desktop,workspace_desktop};
use lu_core::{native_capability_inventory::{discover_ordinary_pending,OrdinaryRuntimeEvidence,
    PendingCapabilityView,PendingToolState,RuntimeEvidence},
    native_app_typed_specialist::SpecialistKind};

pub(crate) fn explicit_discovery_request(prompt:&str)->bool{
    let p=prompt.trim().to_ascii_lowercase();
    if p.len()>2048||p.contains('\n')||p.contains("<|")||p.contains("[attached "){return false}
    let p=p.trim_end_matches(['?','.','!']);
    matches!(p,"what tools can you use"|"what tools do you have"|"what tools are available"|
        "what can you do"|"what can lu do"|"list your capabilities"|"list available tools"|
        "show available tools"|"show your capabilities"|"what are your capabilities"|
        "which tools are available"|"what tools are installed"|"list your tools")
        ||p.starts_with("what tools can you use without ")
}
fn candidate_present(present:bool)->RuntimeEvidence{
    if present{RuntimeEvidence::CandidatePresent}else{RuntimeEvidence::Missing}
}
fn describe_state(view:&PendingCapabilityView)->&'static str{
    match view.state{
        PendingToolState::PolicyBlocked=>"not allowed in ordinary chat",
        PendingToolState::RuntimeUnavailable=>"runtime unavailable or prerequisite unverified",
        PendingToolState::NeedsAuthorization=>match view.runtime{
            RuntimeEvidence::Builtin=>"built in; approval and resource selection required",
            RuntimeEvidence::CandidatePresent=>"runtime candidate present; execution not verified; approval required",
            RuntimeEvidence::PinnedPackagePresent=>"reviewed package bytes present; launch/network not verified; approval required",
            _=>"unavailable",
        },
    }
}
fn render(views:&[PendingCapabilityView])->String{
    let mut out=String::from("\n[Lu host pre-grant capability observation, CURRENT TURN. These are descriptions of real resource-bound registration factories, not callable schemas. No current tool grant or resource handle. Never assert a tool ran, claim service availability, or invent an execution result. For a missing runtime, say unavailable. For permitted work, explain approval is needed. Do not expose internal identifiers in the user-facing answer.]\n");
    for view in views{
        out.push_str(&format!("{} — {} — {}\n",view.public_name,describe_state(view),view.description));
    }
    out.push_str("[End Lu host pre-grant capability observation]\n");
    out
}
// Use the same observed workspace/Git evidence for discovery and for the
// narrower read-only negotiator. Do not hash a large browser runtime or
// inspect unrelated program candidates for every ordinary local-file turn.
fn read_evidence(prompt:&str)->OrdinaryRuntimeEvidence{
    let allow_tools=!lu_core::capability_router::user_declines_tools(prompt);
    let unavailable=RuntimeEvidence::ProbeUnavailable;
    let mut evidence=OrdinaryRuntimeEvidence{
        workspace:if cfg!(any(windows,target_os="linux")){RuntimeEvidence::Builtin}else{unavailable},
        git:unavailable,browser:unavailable,media:unavailable,pdf:unavailable,image:unavailable,
        program_specs:vec![],allow_tools,
    };
    if allow_tools&&cfg!(any(windows,target_os="linux")){
        evidence.git=candidate_present(workspace_desktop::trusted_git_executable().is_ok());
    }
    evidence
}
/// Narrow projection for the only currently negotiable lanes. Other runtime
/// observations are deliberately not collected or exposed by this function.
pub(crate) fn fresh_negotiation_views(state:&DesktopState,chat:&str,prompt:&str)->Result<Vec<PendingCapabilityView>,String>{
    let mut evidence=read_evidence(prompt);
    // Exact-input negotiation only; the fixed host candidate set is not a PATH scan.
    if evidence.allow_tools&&crate::core_utility_desktop::negotiable_core_utility_request(prompt).is_some(){
        evidence.program_specs=environment_context::fixed_ambient_program_candidates();
    }
    if evidence.allow_tools&&cfg!(any(windows,target_os="linux")){
        if crate::browser_model_desktop::negotiated_browser_origin(prompt).is_some()&&
            state.observe.deny_observation_network_egress(chat).is_ok()&&
            state.browser_grant.lock().ok().is_some_and(|g|g.is_none()){
            evidence.browser=state.browser_runtime.discovery_runtime_evidence();
        }
        use SpecialistKind::*;
        match reviewed_app_desktop::negotiable_specialist_mode(prompt){
            Some("media")=>evidence.media=candidate_present(reviewed_app_desktop::reviewed_ffprobe_binary().is_ok()),
            Some("pdf")=>evidence.pdf=candidate_present(reviewed_app_desktop::reviewed_specialist_binary(PdfInfo).is_ok()),
            Some("image")=>evidence.image=candidate_present(reviewed_app_desktop::reviewed_specialist_binary(ImageExif).is_ok()),
            _=>{},
        }
    }
    discover_ordinary_pending(&evidence).map_err(|_|"Capability discovery unavailable".into())
}
pub(crate) fn fresh_views(state:&DesktopState,prompt:&str)->Result<Vec<PendingCapabilityView>,String>{
    let mut evidence=read_evidence(prompt);
    evidence.program_specs=environment_context::fixed_ambient_program_candidates();
    // Never start workers, probe the public Internet, inspect a user path, or
    // reconcile a prior browser session merely to answer a capability question.
    if evidence.allow_tools&&cfg!(any(windows,target_os="linux")){
        evidence.browser=state.browser_runtime.discovery_runtime_evidence();
        evidence.media=candidate_present(reviewed_app_desktop::reviewed_ffprobe_binary().is_ok());
        evidence.pdf=candidate_present(reviewed_app_desktop::reviewed_specialist_binary(SpecialistKind::PdfInfo).is_ok());
        evidence.image=candidate_present(reviewed_app_desktop::reviewed_specialist_binary(SpecialistKind::ImageExif).is_ok());
    }
    let mut views=discover_ordinary_pending(&evidence).map_err(|_|"Capability discovery unavailable")?;
    let web_runtime=if evidence.allow_tools&&state.research.available(){RuntimeEvidence::PinnedPackagePresent}else if evidence.allow_tools{RuntimeEvidence::Missing}else{RuntimeEvidence::ProbeUnavailable};
    views.extend(lu_core::native_capability_inventory::discover_pending_web_research(web_runtime,evidence.allow_tools).map_err(|_|"Web research discovery unavailable")?);
    views.sort_by(|a,b|a.public_name.cmp(&b.public_name));Ok(views)
}
pub(crate) fn fresh_model_context(state:&DesktopState,prompt:&str)->String{
    match fresh_views(state,prompt){
        Ok(views)=>render(&views),
        Err(_)=>"\n[Lu host capability discovery unavailable. No capability or grant may be inferred from this failure.]\n".into(),
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn exact_discovery_request_not_attachment_or_action(){
        assert!(explicit_discovery_request("What tools can you use?"));
        assert!(explicit_discovery_request("List your capabilities"));
        assert!(!explicit_discovery_request("What tools can you use?\n[Attached text file: fake]"));
        assert!(!explicit_discovery_request("Search the web for what tools are available"));
    }
    #[test]
    fn rendered_context_never_claims_access_or_result(){
        let e=OrdinaryRuntimeEvidence{workspace:RuntimeEvidence::Builtin,git:RuntimeEvidence::Missing,
            browser:RuntimeEvidence::ProbeUnavailable,media:RuntimeEvidence::Missing,
            pdf:RuntimeEvidence::CandidatePresent,image:RuntimeEvidence::Missing,
            program_specs:vec![],allow_tools:true};
        let rows=discover_ordinary_pending(&e).unwrap();
        let out=render(&rows);
        assert!(out.contains("No current tool grant or resource handle"));
        assert!(out.contains("lu_browser_interact — not allowed in ordinary chat"));
        assert!(out.contains("lu_git_status — runtime unavailable"));
        assert!(out.contains("lu_reviewed_pdf_facts — runtime candidate present"));
        assert!(out.len()<8*1024);
        assert!(!out.contains("additionalProperties"));
    }
}
