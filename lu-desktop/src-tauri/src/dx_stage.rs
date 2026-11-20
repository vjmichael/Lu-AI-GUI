//! CP-12B-R5. Fixed vocabulary, bounded stage aggregates. No trace identity,
//! timing, text, error detail, file paths, or authority to act on a finding.
use crate::dx_ipc::Summary;
use prost::Message;
use serde::Serialize;

// The IDs intentionally match the existing diagnostic checkpoint vocabulary.
// 26 is a conservative bucket for an unrecognized stage, never a diagnosis.
const STAGES: &[&str] = &[
    "UNKNOWN", "USER_SUBMITTED", "USER_SAVED", "IPC_INVOKED", "NATIVE_TURN_RECEIVED",
    "SAVED_USER_ATTESTED", "TASK_AUTHORIZED", "WORKER_ENTERED", "MODEL_LOOP_ENTERED",
    "MODEL_LOOP_RETURNED", "NATIVE_IPC_RETURNED", "STREAM_CLOSED", "ASSISTANT_SAVED",
    "ASSISTANT_RENDERED", "TOOL_DISPATCH_ENTERED", "TOOL_DISPATCH_RETURNED",
    "CANCEL_REQUESTED", "WORKER_TERMINATED", "RECOVERY_COMPLETED", "SESSION_READY",
    "NATIVE_TURN_REJECTED", "NATIVE_TURN_FAILED", "CPU_ROUTE_SELECTED",
    "CPU_EXECUTION_ENTERED", "CPU_EXECUTION_RETURNED", "CPU_APPROVED_ROUTE_SELECTED",
    "UNKNOWN_STAGE",
];
// Short, distinct answer tokens are model-facing only. Wire and archive IDs
// remain stable. No answer is supplied to either model.
const ANSWERS:&[&str]=&[
    "UNKNOWN", "SUBMIT", "USER_SAVE", "SEND", "RECEIVE", "SAVED_CHECK",
    "PERMISSION", "WORKER_START", "MODEL_START", "MODEL_RETURN", "REPLY",
    "STREAM_CLOSE", "RESPONSE_SAVE", "DISPLAY", "TOOL_START", "TOOL_RETURN",
    "STOP", "WORKER_EXIT", "RECOVERY", "MODEL_READY", "REFUSED", "NATIVE_FAILURE",
    "LOCAL_ROUTE", "LOCAL_START", "LOCAL_RETURN", "APPROVED_ROUTE", "UNKNOWN_STAGE",
];
const UNKNOWN_STAGE:u32=26;
const UNCERTAIN:u32=101;
const MULTIPLE:u32=102;
const DELIVERED:u32=103;
const EMPTY:u32=104;
#[derive(Clone, PartialEq, Message, Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct StageCount {
    #[prost(uint32,tag="1")]
    pub stage:u32,
    #[prost(uint32,tag="2")]
    pub observed:u32,
    #[prost(uint32,tag="3")]
    pub bounded:u32,
}
#[derive(Clone, PartialEq, Message, Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct StageEvidence {
    #[prost(message,repeated,tag="1")]
    pub stages:Vec<StageCount>,
}
pub(crate) fn stage_id(label:&str)->usize {
    STAGES[1..26].iter().position(|s|s.eq_ignore_ascii_case(label)).map(|i|i+1).unwrap_or(UNKNOWN_STAGE as usize)
}
impl StageEvidence {
    pub(crate) fn valid(&self,s:&Summary)->bool {
        if !s.valid()||self.stages.len()>26{return false}
        let mut previous=0;let(mut observed,mut bounded)=(0u64,0u64);
        for row in &self.stages {
            if row.stage<=previous||row.stage>UNKNOWN_STAGE||(row.observed==0&&row.bounded==0){return false}
            previous=row.stage;observed+=u64::from(row.observed);bounded+=u64::from(row.bounded);
        }
        observed==u64::from(s.observed_failure)&&bounded==u64::from(s.bounded)
    }
    /// Conservative across the entire bounded snapshot: even a genuine
    /// observed stop cannot make other missing records uniquely attributable.
    pub(crate) fn expected(&self,s:&Summary)->Option<u32>{
        if !self.valid(s){return None}
        Some(if s.incomplete>0||s.unavailable>0||self.stages.iter().any(|r|r.stage==UNKNOWN_STAGE){UNCERTAIN}
        else if self.stages.len()>1{MULTIPLE}
        else if let Some(r)=self.stages.first(){r.stage}
        else if s.complete>0{DELIVERED}else{EMPTY})
    }
    pub(crate) fn prompt(&self,s:&Summary)->String {
        let rows=self.stages.iter().map(|r|format!("{}: {} observed stops/failures, {} unconfirmed",ANSWERS[r.stage as usize],r.observed,r.bounded)).collect::<Vec<_>>().join("\n");
        format!("Read the diagnostic facts. Answer with one label only.\nMissing or uncertain records: {}\nSuccessful deliveries: {}\nNumber of affected steps: {}\nAffected steps:\n{}\n\nRules in order:\n1. If missing or uncertain records is greater than zero, or a step is UNKNOWN_STAGE, answer UNCERTAIN.\n2. If number of affected steps is greater than one, answer MULTIPLE.\n3. If number of affected steps is one, copy the label before the colon on its line.\n4. If number of affected steps is zero and successful deliveries is greater than zero, answer DELIVERED.\n5. Otherwise answer EMPTY.\nWrite only the label. /no_think",s.incomplete+s.unavailable,s.complete,self.stages.len(),rows)
    }
}
fn label(verdict:u32)->Option<&'static str>{match verdict{UNCERTAIN=>Some("UNCERTAIN"),MULTIPLE=>Some("MULTIPLE"),DELIVERED=>Some("DELIVERED"),EMPTY=>Some("EMPTY"),1..=25=>Some(ANSWERS[verdict as usize]),_=>None}}
#[derive(Clone, Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct ReviewedStages {
    pub summary:Summary,
    pub evidence:StageEvidence,
    pub verdict:u32,
}
impl ReviewedStages {
    pub(crate) fn checked(summary:Summary,evidence:StageEvidence,answer:&str)->Option<Self>{
        let verdict=evidence.expected(&summary)?;
        (label(verdict)==Some(answer.trim())).then_some(Self{summary,evidence,verdict})
    }
    pub(crate) fn valid(&self)->bool{self.evidence.expected(&self.summary)==Some(self.verdict)}
}
#[cfg(test)]
mod tests {
    use super::*;
    fn summary()->Summary{Summary{complete:1,observed_failure:1,bounded:0,incomplete:0,unavailable:0}}
    fn evidence()->StageEvidence{StageEvidence{stages:vec![StageCount{stage:20,observed:1,bounded:0}]}}
    #[test]fn exact_stage_and_verdict_required(){let s=summary();let e=evidence();assert!(ReviewedStages::checked(s.clone(),e.clone(),"REFUSED").is_some());for a in ["FAILURE","TASK_AUTHORIZED","REFUSED because denied","UNCERTAIN","refused"]{assert!(ReviewedStages::checked(s.clone(),e.clone(),a).is_none())}}
    #[test]fn uncertain_multiple_bounded_and_empty(){let(mut s,mut e)=(summary(),evidence());s.incomplete=1;assert_eq!(e.expected(&s),Some(UNCERTAIN));s.incomplete=0;s.unavailable=1;assert_eq!(e.expected(&s),Some(UNCERTAIN));s.unavailable=0;s.bounded=1;e.stages.push(StageCount{stage:24,observed:0,bounded:1});assert_eq!(e.expected(&s),Some(MULTIPLE));e.stages.remove(0);s.observed_failure=0;assert_eq!(e.expected(&s),Some(24));e.stages[0].stage=26;assert_eq!(e.expected(&s),Some(UNCERTAIN));e.stages.clear();s.bounded=0;assert_eq!(e.expected(&s),Some(DELIVERED));s.complete=0;assert_eq!(e.expected(&s),Some(EMPTY))}
    #[test]fn all_known_stages_have_distinct_exact_answer_tokens(){
        let s=summary();let mut labels=std::collections::HashSet::new();
        for code in 1..=25 { let e=StageEvidence{stages:vec![StageCount{stage:code,observed:1,bounded:0}]};
            let word=label(code).unwrap();assert!(labels.insert(word));assert_eq!(stage_id(&STAGES[code as usize].to_ascii_lowercase()),code as usize);
            assert_eq!(ReviewedStages::checked(s.clone(),e.clone(),word).unwrap().verdict,code);
            assert!(!e.prompt(&s).is_empty());assert!(ReviewedStages::checked(s.clone(),e,"UNKNOWN_STAGE").is_none()); }
    }
    #[test]fn malformed_and_inconsistent_evidence_refused(){let s=summary();let mut e=evidence();e.stages[0].observed=2;assert!(!e.valid(&s));e=evidence();e.stages.push(e.stages[0].clone());assert!(!e.valid(&s));e=evidence();e.stages[0].stage=27;assert!(!e.valid(&s));e=evidence();e.stages[0].observed=u32::MAX;assert!(!e.valid(&s));e=evidence();e.stages[0].observed=0;assert!(!e.valid(&s));}
}
