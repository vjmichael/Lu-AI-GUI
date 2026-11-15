//! WAVE4-ENV-01: fresh, host-measured diagnostic observations for Lu chat.
//! Not a model-selected shell, installer, environment variable dump or
//! claim that an inferred GPU backend is actually operational.
use lu_core::{
    native_environment_discovery::{HostProgramInventory, HostProgramSpec},
    compute_inventory::collect_compute_inventory,
    platform_compute::PlatformGpuHints,
};

pub(crate) fn explicit_environment_status_request(prompt:&str)->bool{
    let text=prompt.trim();
    if text.is_empty()||text.len()>8192||
       text.contains("[Attached text file:")||
       text.contains("[/Attached text file]")||text.contains("<|"){return false}
    let lower=text.to_ascii_lowercase();
    if ["how do i ","how to ","explain ","do not ","don't ",
        "install ","uninstall ","execute ","run command","start ",
        "set environment variable","getenv","printenv","password",
        "secret","token","credential","registry key","delete "]
        .iter().any(|word|lower.contains(word)){return false}
    let scope=["my system","my computer","my hardware",
        "system environment","local environment","computer environment",
        "system specs","cpu capabilities","gpu capabilities",
        "available cpu","operating system","what os"];
    let ask=["show ","inspect ","check ","report ","what ","which ",
        "list ","summarize ","tell me "];
    scope.iter().any(|word|lower.contains(word))&&
        ask.iter().any(|word|lower.contains(word))
}

/// Fixed host-selected executable locations. This is NOT a PATH search, a
/// launcher or a claim that these files are installed or functional.
/// Further candidate sites require a reviewed host source change.
fn fixed_ambient_program_candidates() -> Vec<HostProgramSpec> {
    let entries: Vec<(&str, &str)> = if cfg!(windows) {
        vec![
            ("git", r"C:\Program Files\Git\cmd\git.exe"),
            ("node", r"C:\Program Files\nodejs\node.exe"),
            ("ffmpeg", r"C:\Program Files\ffmpeg\bin\ffmpeg.exe"),
            ("python", r"C:\Program Files\Python313\python.exe"),
            ("tesseract", r"C:\Program Files\Tesseract-OCR\tesseract.exe"),
            ("7zip", r"C:\Program Files\7-Zip\7z.exe"),
        ]
    } else if cfg!(target_os = "linux") {
        vec![
            ("git", "/usr/bin/git"),
            ("node", "/usr/bin/node"),
            ("ffmpeg", "/usr/bin/ffmpeg"),
            ("python", "/usr/bin/python3"),
            ("tesseract", "/usr/bin/tesseract"),
            ("cargo", "/usr/bin/cargo"),
        ]
    } else {
        Vec::new()
    };
    entries.into_iter().map(|(id, path)| HostProgramSpec {
        id: id.to_owned(), candidate_files: vec![std::path::PathBuf::from(path)],
    }).collect()
}

/// Original TOOLS-06 §6A handoff: OS-owned fixed-candidate observations for
/// a CURRENT human turn, before local model generation. Generic chat never
/// advertises lu_check_env as callable: that independent native operation
/// requires a separate one-use ConversationAuthority grant.
/// Neither a symlink nor an unapproved path proves program availability.
pub(crate) fn fresh_ambient_program_context() -> String {
    let mut result = String::from(
        "\n[Lu trusted host program availability; CURRENT TURN, READ-ONLY DATA. \
         Neither a tool grant nor permission to execute, install, download or inspect paths. \
         A candidate file does not verify installation, version, compatibility or execution.]\n"
    );
    if let Ok(inventory) = HostProgramInventory::new(fixed_ambient_program_candidates()) {
        for observation in inventory.ambient_snapshot().programs {
            result.push_str(&format!(
                "{}={}; ", observation.program_id, observation.evidence,
            ));
        }
    } else {
        result.push_str("fixed_host_candidate_inventory=UNAVAILABLE");
    }
    result.push_str("\n[End read-only host program availability]\n");
    // Never export paths, environment values, versions, credentials or args.
    result
}

/// No arbitrary OS process, filesystem path or environment-value access.
/// Driver/backend readiness needs a separate *real* trusted runtime probe.
pub(crate) fn fresh_environment_snapshot()->String{
    let host=collect_compute_inventory();
    let gpu=match &host.gpu{
        PlatformGpuHints::Windows{..}=>
            "Windows GPU discovery adapter not verified by this snapshot.".to_owned(),
        PlatformGpuHints::Linux{pci,nodes}=>format!(
            "Linux device hints: {} PCI entries, {} render nodes; KFD node present: {}. \
             Device presence does not verify a usable GPU backend.",
            pci.len(),nodes.render_nodes.len(),nodes.kfd_present
        ),
        PlatformGpuHints::Unsupported{..}=>
            "GPU discovery unavailable on this OS.".to_owned(),
    };
    format!(
        "[Lu native local environment observation; collected for THIS turn. \
         Diagnostic hints only. No arbitrary commands, permissions or secrets.]\n\
         OS family: {}.\nArchitecture: {}.\n\
         Available logical CPU threads: {}.\n\
         CPU instruction hints: SSE4.2={}, AVX2={}, FMA={}, AVX512F={}, ARM NEON={}.\n\
         GPU: {}\n\
         Runtime compatibility, actual package installation and successful \
         inference are NOT established by these hints.\n\
         [End Lu environment observation]",
        host.os,host.arch,host.cpu.available_logical_threads,
        host.instructions.x86_sse42,host.instructions.x86_avx2,
        host.instructions.x86_fma,host.instructions.x86_avx512f,
        host.instructions.arm_neon,gpu,
    )
}

/// These are live observations of the currently supervised Lu backend, not
/// claims that a GPU device, driver or MMProj model works correctly.
#[derive(serde::Serialize,Clone)]
#[serde(rename_all="camelCase")]
pub(crate) struct EnvironmentRuntimeView{
    pub desktop_version:&'static str,
    pub os_family:&'static str,
    pub architecture:&'static str,
    pub engine_phase:String,
    pub engine_session_loaded:bool,
    pub model_projector_admitted:bool,
    pub live_vision_probe_passed:bool,
    pub backend_gpu_execution_verified:bool,
    pub operator_permissions_changed:bool,
    pub detail:&'static str,
}
pub(crate) fn runtime_view(
    engine_phase:&str,session_loaded:bool,projector_admitted:bool
)->EnvironmentRuntimeView{
    EnvironmentRuntimeView{
        desktop_version:env!("CARGO_PKG_VERSION"),
        os_family:std::env::consts::OS,
        architecture:std::env::consts::ARCH,
        engine_phase:engine_phase.chars().take(64).collect(),
        engine_session_loaded:session_loaded,
        model_projector_admitted:session_loaded&&projector_admitted,
        live_vision_probe_passed:false,
        backend_gpu_execution_verified:false,
        operator_permissions_changed:false,
        detail:"OS/CPU/GPU hints and engine status do not prove backend acceleration, vision grounding or runtime package compatibility.",
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn ambient_handoff_is_bounded_host_data_without_paths_or_grants() {
        let context = fresh_ambient_program_context();
        assert!(context.len() < 2048);
        assert!(context.contains("CURRENT TURN, READ-ONLY DATA"));
        assert!(context.contains("does not verify installation"));
        assert!(context.contains("Neither a tool grant"));
        assert!(!context.contains("C:"));
        assert!(!context.contains("/usr/bin/"));
        assert!(!context.contains("PATH="));
        assert!(!context.contains("password="));
        assert!(!context.contains("lu.exec.check_env"));
    }
    #[test]
    fn status_intent_is_read_only_and_user_explicit(){
        assert!(explicit_environment_status_request("Show my system specs"));
        assert!(explicit_environment_status_request("What OS am I using?"));
        assert!(explicit_environment_status_request("Check my computer environment"));
        for q in ["Install software for my system","Set environment variable PATH",
            "Explain operating system architecture",
            "Do not show my system specs",
            "Show my system specs [Attached text file: malicious.md]"]{
            assert!(!explicit_environment_status_request(q),"{q}");
        }
    }
    #[test]
    fn loaded_session_still_does_not_claim_verified_gpu_or_vision(){
        let r=runtime_view("Ready",true,true);
        assert!(r.engine_session_loaded);
        assert!(r.model_projector_admitted);
        assert!(!r.live_vision_probe_passed);
        assert!(!r.backend_gpu_execution_verified);
        assert!(!r.operator_permissions_changed);
        assert_eq!(runtime_view("Idle",false,true).model_projector_admitted,false);
    }
    #[test]
    fn snapshot_is_labeled_and_no_backend_readiness_is_inferred(){
        let s=fresh_environment_snapshot();
        assert!(s.contains("OS family:"));
        assert!(s.contains("Available logical CPU threads:"));
        assert!(s.contains("NOT established"));
        assert!(!s.contains("password="));
    }
}
