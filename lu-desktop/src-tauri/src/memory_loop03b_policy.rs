//! MEMORY-LOOP-03B: pure, deterministic disposition of source-checked
//! procedural, belief-revision and context-change facts. Model text cannot
//! select a verifier, mutate the disposition or authorize external work.
#![allow(dead_code)]
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(crate) enum MemoryKind{Procedure,Belief,ContextChange}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(crate) enum SourceRead{
    Unavailable,
    Procedure{verified_records:usize,details_truncated:bool},
    Belief{verified:usize,reported:usize,contradicted:usize,
        stale:usize,retracted_or_superseded:usize},
    ContextChange{comparison_present:bool,needs_review:bool,
        source_changed:bool,has_blockers:bool},
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(crate) struct MemoryDisposition{
    pub status:&'static str,
    pub human_review_required:bool,
    pub historically_verified_data_available:bool,
    pub no_current_source_freshness_guarantee:bool,
    pub execution_authority:&'static str,
    pub prior_grant_reusable:bool,
    pub automatic_retry_allowed:bool,
    pub model_may_override:bool,
}
pub(crate) fn classify(kind:MemoryKind,facts:SourceRead)->MemoryDisposition{
    let (status,review,verified)=match(kind,facts){
        (_,SourceRead::Unavailable)=>
            ("ORIGINAL_MEMORY_SOURCE_UNAVAILABLE",true,false),
        (MemoryKind::Procedure,SourceRead::Procedure{
            verified_records,details_truncated
        })=>{
            if verified_records==0{
                ("NO_CURRENTLY_REVALIDATED_VERIFIED_PROCEDURE",true,false)
            }else if verified_records>8||details_truncated{
                ("HISTORICAL_PROCEDURE_PARTIALLY_DISPLAYED",true,true)
            }else{
                ("HISTORICAL_SOURCE_ORACLE_VERIFIED_PROCEDURE",false,true)
            }
        },
        (MemoryKind::Belief,SourceRead::Belief{
            verified,reported,contradicted,stale,retracted_or_superseded
        })=>{
            if contradicted>0{
                ("CONTRADICTED_BELIEFS_HUMAN_REVIEW",true,false)
            }else if stale>0{
                ("STALE_BELIEF_REQUIRES_REVALIDATION",true,false)
            }else if reported>0{
                ("REPORTED_UNVERIFIED_BELIEF",true,false)
            }else if retracted_or_superseded>0{
                ("RETRACTED_OR_SUPERSEDED_BELIEF",true,false)
            }else if verified>0{
                ("HISTORICAL_SOURCE_ORACLE_VERIFIED_BELIEF",false,true)
            }else{
                ("NO_SOURCE_BACKED_BELIEF_RECORDS",true,false)
            }
        },
        (MemoryKind::ContextChange,SourceRead::ContextChange{
            comparison_present,needs_review,source_changed,has_blockers
        })=>{
            if !comparison_present{
                ("NO_COMPARABLE_ORIGINAL_CHECKPOINTS",true,false)
            }else if source_changed||has_blockers||needs_review{
                ("ORIGINAL_CONTEXT_CHANGED_HUMAN_REVIEW",true,false)
            }else{
                ("HISTORICAL_CONTEXT_COMPARISON_NO_CHANGE",false,false)
            }
        },
        _=>("MISMATCHED_MEMORY_EVIDENCE",true,false),
    };
    MemoryDisposition{
        status,human_review_required:review,
        historically_verified_data_available:verified,
        no_current_source_freshness_guarantee:true,
        execution_authority:"NOT_AUTHORIZED_FOR_EXECUTION",
        prior_grant_reusable:false,
        automatic_retry_allowed:false,
        model_may_override:false,
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    fn no_grant(x:MemoryDisposition){
        assert_eq!(x.execution_authority,"NOT_AUTHORIZED_FOR_EXECUTION");
        assert!(!x.prior_grant_reusable);
        assert!(!x.automatic_retry_allowed);
        assert!(!x.model_may_override);
        assert!(x.no_current_source_freshness_guarantee);
    }
    #[test]
    fn verified_historical_procedure_is_only_advice_not_replay(){
        let x=classify(MemoryKind::Procedure,SourceRead::Procedure{
            verified_records:1,details_truncated:false});
        assert_eq!(x.status,"HISTORICAL_SOURCE_ORACLE_VERIFIED_PROCEDURE");
        assert!(x.historically_verified_data_available);
        no_grant(x);
    }
    #[test]
    fn missing_or_partial_procedure_requires_review(){
        for x in [
            SourceRead::Unavailable,
            SourceRead::Procedure{verified_records:0,details_truncated:false},
            SourceRead::Procedure{verified_records:1,details_truncated:true},
            SourceRead::Procedure{verified_records:9,details_truncated:false},
        ]{
            let a=classify(MemoryKind::Procedure,x);
            assert!(a.human_review_required);no_grant(a);
        }
    }
    #[test]
    fn contradicted_claim_wins_over_verified_historical_receipt(){
        let a=classify(MemoryKind::Belief,SourceRead::Belief{
            verified:1,reported:0,contradicted:1,stale:0,
            retracted_or_superseded:0});
        assert_eq!(a.status,"CONTRADICTED_BELIEFS_HUMAN_REVIEW");
        assert!(!a.historically_verified_data_available);no_grant(a);
    }
    #[test]
    fn reported_stale_superseded_and_empty_are_not_verified(){
        for evidence in [
            SourceRead::Belief{verified:0,reported:1,contradicted:0,stale:0,
                retracted_or_superseded:0},
            SourceRead::Belief{verified:1,reported:0,contradicted:0,stale:1,
                retracted_or_superseded:0},
            SourceRead::Belief{verified:0,reported:0,contradicted:0,stale:0,
                retracted_or_superseded:1},
            SourceRead::Belief{verified:0,reported:0,contradicted:0,stale:0,
                retracted_or_superseded:0},
        ]{
            let a=classify(MemoryKind::Belief,evidence);
            assert!(a.human_review_required);assert!(!a.historically_verified_data_available);
            no_grant(a);
        }
    }
    #[test]
    fn one_historical_verified_belief_still_needs_current_source_check(){
        let x=classify(MemoryKind::Belief,SourceRead::Belief{
            verified:1,reported:0,contradicted:0,stale:0,
            retracted_or_superseded:0});
        assert_eq!(x.status,"HISTORICAL_SOURCE_ORACLE_VERIFIED_BELIEF");
        no_grant(x);
    }
    #[test]
    fn context_change_or_missing_comparison_cannot_resume_a_worker(){
        for (comparison_present,needs_review,source_changed,has_blockers) in [
            (false,false,false,false),(true,true,false,false),
            (true,false,true,false),(true,false,false,true)
        ]{
            let a=classify(MemoryKind::ContextChange,
                SourceRead::ContextChange{
                    comparison_present,needs_review,source_changed,has_blockers});
            assert!(a.human_review_required);
            assert!(!a.historically_verified_data_available);no_grant(a);
        }
        let no_change=classify(MemoryKind::ContextChange,
            SourceRead::ContextChange{
                comparison_present:true,needs_review:false,
                source_changed:false,has_blockers:false});
        assert_eq!(no_change.status,"HISTORICAL_CONTEXT_COMPARISON_NO_CHANGE");
        no_grant(no_change);
    }
    #[test]
    fn mismatched_evidence_kind_and_unreadable_history_fail_closed(){
        let p=classify(MemoryKind::Procedure,
            SourceRead::Belief{verified:1,reported:0,contradicted:0,stale:0,
                retracted_or_superseded:0});
        assert_eq!(p.status,"MISMATCHED_MEMORY_EVIDENCE");
        assert!(p.human_review_required);
        let c=classify(MemoryKind::ContextChange,SourceRead::Unavailable);
        assert_eq!(c.status,"ORIGINAL_MEMORY_SOURCE_UNAVAILABLE");
        no_grant(p);no_grant(c);
    }
}
