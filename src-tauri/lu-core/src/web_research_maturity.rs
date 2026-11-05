//! TOOLS-10 frontier recognition: sparse evidence is not negative evidence.
//! Maturity is a provisional research-landscape assessment, not scientific fact.
//! No classification is permitted solely because an initial search found few hits.
use crate::web_research_sufficiency::{
    NextAction, ResearchMode, SearchAssessment, SufficiencyReport,
};
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum EvidenceMaturity {
    Undetermined,
    Emerging,
    Underdeveloped,
    DevelopedButInconclusive,
    Developed,
    AccessConstrained,
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum RecommendedOutcome {
    ContinueResearch,
    PreliminaryFrontierMap,
    ReportWithExplicitUncertainty,
    AssessExistingLiterature,
}
#[derive(Debug,Clone)]
pub struct ResearchLandscape {
    /// Distinct lawful avenues, e.g. primary catalogs, academic indices,
    /// alternate terminology, adjacent disciplines, and reference tracing.
    pub diverse_search_avenues_checked: u32,
    /// Distinct works, not raw hit counts or repeated citations.
    pub unique_relevant_works_discovered: u32,
    pub primary_works_accessed: u32,
    pub independently_replicated_findings: u32,
    pub distinct_research_programs: u32,
    pub substantive_methodological_gaps: bool,
    pub emerging_recent_research: bool,
    pub historic_research_identified: bool,
    pub major_access_barriers: bool,
    /// Separate independent survey of the research landscape; false means no
    /// provisional maturity conclusion, even if the primary query was sparse.
    pub landscape_reviewed: bool,
}
#[derive(Debug,Clone)]
pub struct MaturityDecision {
    pub maturity: EvidenceMaturity,
    pub outcome: RecommendedOutcome,
    pub stop_research: bool,
    pub evidence_is_insufficient: bool,
    pub justification: Vec<&'static str>,
    pub suggested_next_steps: Vec<&'static str>,
}
pub fn evaluate_maturity(
    landscape:&ResearchLandscape,
    search:&SearchAssessment,
    base:&SufficiencyReport,
)->MaturityDecision {
    let mut justification=Vec::new();
    let mut next=Vec::new();
    let sufficient_survey=landscape.landscape_reviewed
        && landscape.diverse_search_avenues_checked>=3;
    let few_independent_works=landscape.unique_relevant_works_discovered<5
        && landscape.independently_replicated_findings==0;
    let access_constrained=landscape.major_access_barriers
        && landscape.unique_relevant_works_discovered>landscape.primary_works_accessed.saturating_mul(2);
    let maturity=if !sufficient_survey {
        justification.push("FIELD_MATURITY_NOT_ESTABLISHED");
        EvidenceMaturity::Undetermined
    } else if access_constrained {
        justification.push("ACCESS_BARRIERS_PREVENT_MATURITY_JUDGMENT");
        EvidenceMaturity::AccessConstrained
    } else if few_independent_works {
        if landscape.emerging_recent_research && !landscape.historic_research_identified {
            justification.push("PROVISIONALLY_EMERGING_FIELD");
            EvidenceMaturity::Emerging
        } else {
            justification.push("PROVISIONALLY_UNDERDEVELOPED_FIELD");
            EvidenceMaturity::Underdeveloped
        }
    } else if landscape.independently_replicated_findings<2
        || landscape.substantive_methodological_gaps {
        justification.push("EVIDENCE_PRESENT_BUT_NOT_YET_CONCLUSIVE");
        EvidenceMaturity::DevelopedButInconclusive
    } else {
        EvidenceMaturity::Developed
    };
    let uncertainty=matches!(maturity,
        EvidenceMaturity::Undetermined|EvidenceMaturity::Emerging|
        EvidenceMaturity::Underdeveloped|EvidenceMaturity::DevelopedButInconclusive|
        EvidenceMaturity::AccessConstrained);
    if uncertainty {
        next.push("SEPARATE_UNTESTED_HYPOTHESES_FROM_SUPPORTED_FINDINGS");
        next.push("IDENTIFY_TESTABLE_PREDICTIONS_AND_REQUIRED_DATA");
        next.push("CONSIDER_PRIMARY_EVIDENCE_AND_ADJACENT_DISCIPLINES");
        next.push("DISCLOSE_UNRESOLVED_QUESTIONS_AND_ACCESS_GAPS");
    }
    let exhausted=matches!(base.action,NextAction::ExhaustedWithDisclosedGaps|
        NextAction::ExhaustiveCandidateCeilingReached);
    let stop=if search.user_requested_more { false }
        else if matches!(search.mode,ResearchMode::Exhaustive{..}) {
            exhausted
        } else {
            exhausted||matches!(base.action,NextAction::SufficientForReview)
        };
    let outcome=if !stop {RecommendedOutcome::ContinueResearch}
        else if matches!(maturity,EvidenceMaturity::Emerging|EvidenceMaturity::Underdeveloped) {
            RecommendedOutcome::PreliminaryFrontierMap
        } else if uncertainty {RecommendedOutcome::ReportWithExplicitUncertainty}
        else {RecommendedOutcome::AssessExistingLiterature};
    if matches!(outcome,RecommendedOutcome::PreliminaryFrontierMap) {
        justification.push("NO_PREMATURE_THESIS_REJECTION_OR_REPLACEMENT");
    }
    MaturityDecision {
        maturity,outcome,stop_research:stop,
        evidence_is_insufficient:uncertainty,
        justification,suggested_next_steps:next
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::web_research_sufficiency::SufficiencyReport;
    use std::collections::BTreeMap;
    fn report(action:NextAction)->SufficiencyReport {
        SufficiencyReport {action,supports_independent_lineages:0,
            opposes_independent_lineages:0,alternative_lineages:0,distinct_voices:0,
            major_voices:0,other_voices:0,reason_codes:vec![],
            voice_map:BTreeMap::new()}
    }
    fn search()->SearchAssessment {
        SearchAssessment {mode:ResearchMode::Normal,
            distinct_candidates_examined:3,goal_addressed:false,
            open_critical_questions:2,outstanding_material_objections:0,
            recent_unproductive_strategies:3,meaningful_avenues_remaining:false,
            user_requested_more:false}
    }
    fn landscape()->ResearchLandscape {
        ResearchLandscape {diverse_search_avenues_checked:4,
            unique_relevant_works_discovered:3,primary_works_accessed:2,
            independently_replicated_findings:0,distinct_research_programs:1,
            substantive_methodological_gaps:true,emerging_recent_research:true,
            historic_research_identified:false,major_access_barriers:false,
            landscape_reviewed:true}
    }
    #[test]
    fn emerging_field_yields_research_agenda_not_false_thesis_rejection() {
        let d=evaluate_maturity(&landscape(),&search(),
            &report(NextAction::ExhaustedWithDisclosedGaps));
        assert_eq!(d.maturity,EvidenceMaturity::Emerging);
        assert_eq!(d.outcome,RecommendedOutcome::PreliminaryFrontierMap);
        assert!(d.evidence_is_insufficient);
        assert!(d.justification.contains(&"NO_PREMATURE_THESIS_REJECTION_OR_REPLACEMENT"));
    }
    #[test]
    fn one_search_failure_does_not_prove_a_new_field() {
        let mut l=landscape();l.diverse_search_avenues_checked=1;
        let d=evaluate_maturity(&l,&search(),
            &report(NextAction::ExhaustedWithDisclosedGaps));
        assert_eq!(d.maturity,EvidenceMaturity::Undetermined);
        assert_eq!(d.outcome,RecommendedOutcome::ReportWithExplicitUncertainty);
    }
    #[test]
    fn paywalled_mature_field_must_not_be_called_underdeveloped() {
        let mut l=landscape();l.unique_relevant_works_discovered=50;
        l.primary_works_accessed=2;l.major_access_barriers=true;
        let d=evaluate_maturity(&l,&search(),
            &report(NextAction::ExhaustedWithDisclosedGaps));
        assert_eq!(d.maturity,EvidenceMaturity::AccessConstrained);
    }
    #[test]
    fn user_can_insist_on_additional_exploration() {
        let mut s=search();s.user_requested_more=true;
        let d=evaluate_maturity(&landscape(),&s,
            &report(NextAction::ExhaustedWithDisclosedGaps));
        assert!(!d.stop_research);
        assert_eq!(d.outcome,RecommendedOutcome::ContinueResearch);
    }
}
