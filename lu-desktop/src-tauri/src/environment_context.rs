//! WAVE4-ENV-01: fresh, host-measured diagnostic observations for Lu chat.
//! Not a model-selected shell, installer, environment variable dump or
//! claim that an inferred GPU backend is actually operational.
use lu_core::{
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
#[cfg(test)]
mod tests{
    use super::*;
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
    fn snapshot_is_labeled_and_no_backend_readiness_is_inferred(){
        let s=fresh_environment_snapshot();
        assert!(s.contains("OS family:"));
        assert!(s.contains("Available logical CPU threads:"));
        assert!(s.contains("NOT established"));
        assert!(!s.contains("password="));
    }
}
