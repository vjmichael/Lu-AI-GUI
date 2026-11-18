//! OBSERVE-GUI-VERIFY-04: compare a pinned intended build/test contract
//! to independently verified running-application evidence. The model or
//! application-under-test cannot declare its own GUI tests passed.
//! This does not build, run, click, screenshot or control any application.
use std::collections::{BTreeSet,BTreeMap};

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum ProofKind {
    /// Host-attested OS accessibility tree or app-independent UIA probe.
    Accessibility,
    /// User-authorized window capture processed by a trusted visual checker.
    Visual,
    /// Native effect journal / test harness receipt from outside the app.
    NativeOutcome,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct ExpectedCheck {
    pub check_id:String,
    pub required_proof:ProofKind,
    /// Pre-agreed structured value/digest (never unconstrained model prose).
    pub expected_value:String,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct BuildTestPlan {
    pub source_commit:String,
    pub artifact_sha256:String,
    pub intended_app_id:String,
    pub checks:Vec<ExpectedCheck>,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct WitnessedCheck {
    pub check_id:String,
    pub proof:ProofKind,
    pub observed_value:String,
    /// Must be set only by Lu's independent native verification broker after
    /// checking OS identity, source and effect receipt. Renderer claims,
    /// model predictions and screenshot commentary must not set this bit.
    pub independently_attested:bool,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct RunningBuildEvidence {
    pub source_commit:String,
    pub loaded_artifact_sha256:String,
    pub app_id:String,
    pub os_app_identity_attested:bool,
    pub checks:Vec<WitnessedCheck>,
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum Verdict {Pass,Fail,Inconclusive}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct VerificationReport {
    pub verdict:Verdict,
    pub matched:usize,
    pub mismatched:Vec<String>,
    pub unverified:Vec<String>,
    pub reason:&'static str,
}
fn token(s:&str)->bool{
    !s.is_empty()&&s.len()<=128&&
    s.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-'||b==b'_')
}
fn is_sha256(s:&str)->bool{s.len()==64&&s.bytes().all(|b|b.is_ascii_hexdigit())}
/// No diagnostics inferred from model statements. A *matching-looking* UI
/// is not enough if the application actually running is an older artifact.
pub fn compare_intended_to_running(
    plan:&BuildTestPlan,actual:&RunningBuildEvidence
)->VerificationReport{
    let inconclusive=|reason|VerificationReport{
        verdict:Verdict::Inconclusive,matched:0,
        mismatched:vec![],unverified:vec![],reason,
    };
    if !token(&plan.source_commit)||!token(&actual.source_commit)
        || !is_sha256(&plan.artifact_sha256)
        || !is_sha256(&actual.loaded_artifact_sha256)
        || !token(&plan.intended_app_id)||!token(&actual.app_id)
        || plan.checks.is_empty()||plan.checks.len()>64
        ||plan.checks.iter().any(|c|
            !token(&c.check_id)||c.expected_value.is_empty()
            ||c.expected_value.len()>1024){
        return inconclusive("Invalid or unbounded build/test evidence");
    }
    let ids:BTreeSet<_>=plan.checks.iter().map(|c|&c.check_id).collect();
    if ids.len()!=plan.checks.len()||
       actual.checks.len()>128 ||
       actual.checks.iter().any(|c|
            !token(&c.check_id)||c.observed_value.len()>1024){
        return inconclusive("Ambiguous or invalid witness identity");
    }
    // Ignore no extra "observations" just because every expected check
    // happens to match. Unexpected IDs can hide a contradictory app-provided
    // result and are never part of the user-approved test contract.
    if actual.checks.iter().any(|c|!ids.contains(&c.check_id)){
        return inconclusive("Evidence contains checks not in intended plan");
    }
    if !actual.os_app_identity_attested{
        return inconclusive("Running application identity not OS-attested");
    }
    if plan.source_commit!=actual.source_commit
        ||!plan.artifact_sha256.eq_ignore_ascii_case(&actual.loaded_artifact_sha256)
        ||plan.intended_app_id!=actual.app_id{
        return inconclusive("Running build does not match pinned intended artifact");
    }
    let mut grouped:BTreeMap<&str,Vec<&WitnessedCheck>>=BTreeMap::new();
    for c in &actual.checks{
        grouped.entry(c.check_id.as_str()).or_default().push(c);
    }
    let mut result=VerificationReport{
        verdict:Verdict::Inconclusive,matched:0,
        mismatched:vec![],unverified:vec![],
        reason:"Incomplete independent GUI and effect evidence",
    };
    for expected in &plan.checks{
        let proof=grouped.get(expected.check_id.as_str());
        // More than one independently asserted value is contradictory,
        // never select whichever makes the AI's claim appear true.
        let unique=proof.and_then(|records|
            if records.len()==1{Some(records[0])}else{None});
        let Some(actual)=unique else{
            result.unverified.push(expected.check_id.clone());
            continue;
        };
        if !actual.independently_attested
            ||actual.proof!=expected.required_proof{
            result.unverified.push(expected.check_id.clone());
        }else if actual.observed_value==expected.expected_value{
            result.matched+=1;
        }else{
            result.mismatched.push(expected.check_id.clone());
        }
    }
    if !result.mismatched.is_empty(){
        result.verdict=Verdict::Fail;
        result.reason="One or more independently verified assertions failed";
    }else if result.unverified.is_empty(){
        result.verdict=Verdict::Pass;
        result.reason="All pinned GUI and effect assertions independently verified";
    }
    result
}
#[cfg(test)]
mod tests{
    use super::*;
    fn build()->BuildTestPlan{
        BuildTestPlan{
            source_commit:"commit-123".into(),
            artifact_sha256:"a".repeat(64),
            intended_app_id:"lu-desktop".into(),
            checks:vec![
                ExpectedCheck{check_id:"model-picker".into(),
                    required_proof:ProofKind::Accessibility,expected_value:"visible".into()},
                ExpectedCheck{check_id:"layout".into(),
                    required_proof:ProofKind::Visual,expected_value:"layout-sha".into()},
                ExpectedCheck{check_id:"model-load".into(),
                    required_proof:ProofKind::NativeOutcome,expected_value:"ENGINE_READY".into()},
            ],
        }
    }
    fn witness()->RunningBuildEvidence{
        RunningBuildEvidence{
            source_commit:"commit-123".into(),
            loaded_artifact_sha256:"a".repeat(64),
            app_id:"lu-desktop".into(),os_app_identity_attested:true,
            checks:vec![
                WitnessedCheck{check_id:"model-picker".into(),proof:ProofKind::Accessibility,
                    observed_value:"visible".into(),independently_attested:true},
                WitnessedCheck{check_id:"layout".into(),proof:ProofKind::Visual,
                    observed_value:"layout-sha".into(),independently_attested:true},
                WitnessedCheck{check_id:"model-load".into(),proof:ProofKind::NativeOutcome,
                    observed_value:"ENGINE_READY".into(),independently_attested:true},
            ],
        }
    }
    #[test]
    fn fully_witnessed_matching_build_passes(){
        let r=compare_intended_to_running(&build(),&witness());
        assert_eq!(r.verdict,Verdict::Pass);assert_eq!(r.matched,3);
    }
    #[test]
    fn a_visible_button_does_not_establish_real_model_loading(){
        let mut w=witness();
        w.checks[2].independently_attested=false;
        let r=compare_intended_to_running(&build(),&w);
        assert_eq!(r.verdict,Verdict::Inconclusive);
        assert_eq!(r.unverified,vec!["model-load"]);
    }
    #[test]
    fn independent_wrong_outcome_fails_even_if_layout_matches(){
        let mut w=witness();
        w.checks[2].observed_value="ENGINE_FAILED".into();
        let r=compare_intended_to_running(&build(),&w);
        assert_eq!(r.verdict,Verdict::Fail);
        assert_eq!(r.mismatched,vec!["model-load"]);
    }
    #[test]
    fn wrong_binary_or_unproven_os_identity_never_passes(){
        let mut w=witness();w.loaded_artifact_sha256="b".repeat(64);
        assert_eq!(compare_intended_to_running(&build(),&w).verdict,Verdict::Inconclusive);
        w=witness();w.os_app_identity_attested=false;
        assert_eq!(compare_intended_to_running(&build(),&w).verdict,Verdict::Inconclusive);
    }
    #[test]
    fn extra_unplanned_or_model_supplied_evidence_cannot_pass(){
        let mut evidence=witness();
        evidence.checks.push(WitnessedCheck{
            check_id:"unplanned-model-pass".into(),
            proof:ProofKind::NativeOutcome,
            observed_value:"PASS".into(),
            independently_attested:true,
        });
        let report=compare_intended_to_running(&build(),&evidence);
        assert_eq!(report.verdict,Verdict::Inconclusive);
        assert_eq!(report.reason,"Evidence contains checks not in intended plan");
    }
    #[test]
    fn duplicate_or_model_claimed_evidence_never_passes(){
        let mut w=witness();w.checks.push(w.checks[2].clone());
        assert_eq!(compare_intended_to_running(&build(),&w).verdict,Verdict::Inconclusive);
        w=witness();w.checks[1].proof=ProofKind::Accessibility;
        assert_eq!(compare_intended_to_running(&build(),&w).verdict,Verdict::Inconclusive);
    }
}
