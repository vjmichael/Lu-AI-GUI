//! Research sufficiency: evidence quality and intellectual diversity, not link counts.
//! This gate evaluates only passages verified by the host-owned EvidenceLedger.
//! Classifications, source independence, prominence and goal coverage are *assessments*,
//! never facts magically established by the model.
use crate::web_research_evidence::{EvidenceError, EvidenceLedger};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResearchMode {
    Normal,
    /// Explicit user opt-in. Cap is inclusive, never a required source tally.
    Exhaustive { candidate_ceiling: u32 },
}
impl ResearchMode {
    pub fn exhaustive() -> Self { Self::Exhaustive { candidate_ceiling: 120 } }
    fn valid(self) -> bool {
        match self {
            Self::Normal => true,
            Self::Exhaustive { candidate_ceiling } => (1..=120).contains(&candidate_ceiling),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Position {
    Supports,
    Opposes,
    Alternative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceProminence { Major, Emerging, Minor, Unknown }

/// Evidence-backed attribution; position, independent lineage, and prominence
/// are *model evaluations* requiring audit, not publication metadata.
#[derive(Debug, Clone)]
pub struct AttributedEvidence {
    pub source_id: String,
    pub verified_quote: String,
    pub position: Position,
    /// Sources relying on one underlying data set/edition/corpus share a lineage.
    pub independent_lineage: String,
    pub voice: String,
    pub prominence: VoiceProminence,
}

#[derive(Debug, Clone)]
pub struct SearchAssessment {
    pub mode: ResearchMode,
    pub distinct_candidates_examined: u32,
    pub goal_addressed: bool,
    pub open_critical_questions: u32,
    pub outstanding_material_objections: u32,
    /// Consecutive substantially different strategies with no relevant new evidence.
    pub recent_unproductive_strategies: u32,
    pub meaningful_avenues_remaining: bool,
    pub user_requested_more: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NextAction {
    Continue,
    SufficientForReview,
    ExhaustedWithDisclosedGaps,
    ExhaustiveCandidateCeilingReached,
}

#[derive(Debug)]
pub enum AssessmentError {
    InvalidInput,
    UnverifiedEvidence(EvidenceError),
    UnverifiedPassage,
}
impl From<EvidenceError> for AssessmentError {
    fn from(value: EvidenceError) -> Self {Self::UnverifiedEvidence(value)}
}
#[derive(Debug)]
pub struct SufficiencyReport {
    pub action: NextAction,
    pub supports_independent_lineages: usize,
    pub opposes_independent_lineages: usize,
    pub alternative_lineages: usize,
    pub distinct_voices: usize,
    pub major_voices: usize,
    pub other_voices: usize,
    pub reason_codes: Vec<&'static str>,
    /// This indicates *model-assessed* intellectual prominence, not verification
    /// of bibliometric influence. It must be presented as an assessment.
    pub voice_map: BTreeMap<String, (BTreeSet<Position>, VoiceProminence)>,
}

pub fn assess_sufficiency(
    ledger: &EvidenceLedger,
    evidence: &[AttributedEvidence],
    assessment: &SearchAssessment,
) -> Result<SufficiencyReport, AssessmentError> {
    if !assessment.mode.valid() || evidence.len() > 4096 {
        return Err(AssessmentError::InvalidInput);
    }
    let mut families: BTreeMap<Position, BTreeSet<&str>> = BTreeMap::new();
    let mut voices: BTreeMap<String, (BTreeSet<Position>, VoiceProminence)> = BTreeMap::new();
    for item in evidence {
        if item.source_id.trim().is_empty() || item.source_id.len()>128
            || item.verified_quote.trim().is_empty() || item.verified_quote.len()>4096
            || item.independent_lineage.trim().is_empty() || item.independent_lineage.len()>256
            || item.voice.trim().is_empty() || item.voice.len()>256 {
            return Err(AssessmentError::InvalidInput);
        }
        if !ledger.has_verified_passage(&item.source_id,&item.verified_quote)?
            || !ledger.eligible_for_reviewed_bibliography(&item.source_id)? {
            return Err(AssessmentError::UnverifiedPassage);
        }
        families.entry(item.position).or_default().insert(&item.independent_lineage);
        let entry=voices.entry(item.voice.clone())
            .or_insert_with(||(BTreeSet::new(),item.prominence));
        entry.0.insert(item.position);
        // Conflicting prominence assessments are not silently upgraded to major.
        if entry.1 != item.prominence {entry.1=VoiceProminence::Unknown;}
    }
    let support=families.get(&Position::Supports).map_or(0,BTreeSet::len);
    let opposition=families.get(&Position::Opposes).map_or(0,BTreeSet::len);
    let alternative=families.get(&Position::Alternative).map_or(0,BTreeSet::len);
    let major=voices.values().filter(|v| v.1==VoiceProminence::Major).count();
    let others=voices.values().filter(|v| v.1!=VoiceProminence::Major).count();

    let mut reasons=Vec::new();
    if support<2 {reasons.push("NEED_INDEPENDENT_SUPPORT");}
    if opposition<2 {reasons.push("NEED_INDEPENDENT_COUNTEREVIDENCE");}
    if alternative==0 {reasons.push("NEED_ALTERNATIVE_PERSPECTIVE");}
    if voices.len()<2 {reasons.push("NEED_DIVERSE_VOICES");}
    if !assessment.goal_addressed {reasons.push("USER_GOAL_NOT_YET_ADDRESSED");}
    if assessment.open_critical_questions>0 {reasons.push("OPEN_CRITICAL_QUESTIONS");}
    if assessment.outstanding_material_objections>0 {
        reasons.push("UNRESOLVED_MATERIAL_OBJECTIONS");
    }
    // The model's self-assessment is necessary but never sufficient by itself.
    let adequately_balanced=reasons.is_empty();
    let stagnating=assessment.recent_unproductive_strategies>=3;
    let exhausted=!assessment.meaningful_avenues_remaining || stagnating;
    let action=match assessment.mode {
        ResearchMode::Normal => {
            if assessment.user_requested_more {NextAction::Continue}
            else if adequately_balanced {NextAction::SufficientForReview}
            else if exhausted {NextAction::ExhaustedWithDisclosedGaps}
            else {NextAction::Continue}
        }
        ResearchMode::Exhaustive {candidate_ceiling} => {
            if assessment.distinct_candidates_examined>=candidate_ceiling {
                NextAction::ExhaustiveCandidateCeilingReached
            } else if exhausted {
                // Never label a bounded survey genuinely exhaustive.
                NextAction::ExhaustedWithDisclosedGaps
            } else {NextAction::Continue}
        }
    };
    if stagnating {reasons.push("SEARCH_SATURATION_OBSERVED");}
    if !assessment.meaningful_avenues_remaining {reasons.push("NO_AUTHORIZED_AVENUES_REMAIN");}
    if matches!(action,NextAction::ExhaustiveCandidateCeilingReached) {
        reasons.push("USER_CHOSEN_CANDIDATE_CEILING");
    }
    Ok(SufficiencyReport{
        action,
        supports_independent_lineages:support,
        opposes_independent_lineages:opposition,
        alternative_lineages:alternative,
        distinct_voices:voices.len(),
        major_voices:major,
        other_voices:others,
        reason_codes:reasons,
        voice_map:voices,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web_research_evidence::{Candidate,TrustedBibliography};
    use std::path::Path;
    fn seed(ledger:&mut EvidenceLedger, id:&str, text:&str) {
        let doi=format!("doi:10.1234/{id}");
        ledger.discover(&Candidate{id:id.into(),title:format!("Work {id}"),
            author:format!("Scholar {id}"),identifier:doi.clone()}).unwrap();
        ledger.corroborate(id,&TrustedBibliography{
            record_url:format!("https://catalog.example/{id}"),title:format!("Work {id}"),
            author:format!("Scholar {id}"),identifier:doi,
        }).unwrap();
        ledger.store_accessed_text(id,text).unwrap();
        ledger.verify_passage(id,"full-text",text).unwrap();
    }
    fn item(id:&str, position:Position, lineage:&str, voice:&str)->AttributedEvidence {
        AttributedEvidence{
            source_id:id.into(),verified_quote:format!("Text for {id}"),
            position,independent_lineage:lineage.into(),voice:voice.into(),
            prominence:if voice=="Established" {VoiceProminence::Major} else {VoiceProminence::Minor},
        }
    }
    fn input(mode:ResearchMode)->SearchAssessment {
        SearchAssessment {
            mode,distinct_candidates_examined:7,goal_addressed:true,
            open_critical_questions:0,outstanding_material_objections:0,
            recent_unproductive_strategies:0,meaningful_avenues_remaining:true,
            user_requested_more:false,
        }
    }
    #[test]
    fn different_papers_reusing_one_original_do_not_count_as_independent() {
        let mut l=EvidenceLedger::open(Path::new(":memory:")).unwrap();
        for id in ["a","b","c"] {seed(&mut l,id,&format!("Text for {id}"));}
        let findings=[
            item("a",Position::Supports,"shared-dataset","Established"),
            item("b",Position::Supports,"shared-dataset","Established"),
            item("c",Position::Opposes,"counter-1","Independent"),
        ];
        let r=assess_sufficiency(&l,&findings,&input(ResearchMode::Normal)).unwrap();
        assert_eq!(r.action,NextAction::Continue);
        assert_eq!(r.supports_independent_lineages,1);
        assert!(r.reason_codes.contains(&"NEED_INDEPENDENT_SUPPORT"));
    }
    #[test]
    fn balanced_evidence_can_stop_with_no_source_quota() {
        let mut l=EvidenceLedger::open(Path::new(":memory:")).unwrap();
        let ids=["a","b","c","d","e"];
        for id in ids {seed(&mut l,id,&format!("Text for {id}"));}
        let findings=[
            item("a",Position::Supports,"a","Established"),
            item("b",Position::Supports,"b","Emerging"),
            item("c",Position::Opposes,"c","Established"),
            item("d",Position::Opposes,"d","Emerging"),
            item("e",Position::Alternative,"e","Minority"),
        ];
        let r=assess_sufficiency(&l,&findings,&input(ResearchMode::Normal)).unwrap();
        assert_eq!(r.action,NextAction::SufficientForReview);
        assert_eq!(r.distinct_voices,3);
        assert_eq!(r.alternative_lineages,1);
        let mut still_search=input(ResearchMode::Normal);
        still_search.outstanding_material_objections=1;
        assert_eq!(assess_sufficiency(&l,&findings,&still_search).unwrap().action,NextAction::Continue);
    }
    #[test]
    fn exhaustive_mode_respects_user_requested_ceiling_not_a_minimum() {
        let l=EvidenceLedger::open(Path::new(":memory:")).unwrap();
        let mut settings=input(ResearchMode::exhaustive());
        assert_eq!(assess_sufficiency(&l,&[],&settings).unwrap().action,NextAction::Continue);
        settings.distinct_candidates_examined=120;
        let report=assess_sufficiency(&l,&[],&settings).unwrap();
        assert_eq!(report.action,NextAction::ExhaustiveCandidateCeilingReached);
        assert!(report.reason_codes.contains(&"NEED_INDEPENDENT_COUNTEREVIDENCE"));
        settings.mode=ResearchMode::Exhaustive{candidate_ceiling:121};
        assert!(matches!(assess_sufficiency(&l,&[],&settings),Err(AssessmentError::InvalidInput)));
    }
    #[test]
    fn stale_quote_cannot_manufacture_research_sufficiency() {
        let mut l=EvidenceLedger::open(Path::new(":memory:")).unwrap();
        seed(&mut l,"a","Text for a");
        let mut wrong=item("a",Position::Supports,"a","Established");
        wrong.verified_quote="Invented quote".into();
        assert!(matches!(assess_sufficiency(&l,&[wrong],&input(ResearchMode::Normal)),
            Err(AssessmentError::UnverifiedPassage)));
    }
    #[test]
    fn search_saturation_finishes_honestly_with_gaps() {
        let l=EvidenceLedger::open(Path::new(":memory:")).unwrap();
        let mut settings=input(ResearchMode::Normal);
        settings.recent_unproductive_strategies=3;
        assert_eq!(assess_sufficiency(&l,&[],&settings).unwrap().action,
            NextAction::ExhaustedWithDisclosedGaps);
    }
}
