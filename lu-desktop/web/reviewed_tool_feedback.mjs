// E1-P14: Human-readable interpretation of host-recorded tool activity.
// Presentation only: never supplies permissions, execution, retries, or proof.
const FALLBACK=Object.freeze({
  title:'Execution status unavailable',
  meaning:'This activity entry does not establish whether an external operation succeeded.',
  nextStep:'Inspect the original source and ask for a new, explicitly authorized check if needed.'
});

const REVIEWED=Object.freeze({
  EFFECT_UNCERTAIN_OR_IN_FLIGHT:{
    title:'Operation needs review',
    meaning:'The journal contains an unfinished or uncertain effect. It does not prove the worker stopped or that no change occurred.',
    nextStep:'Inspect the saved effect and current source before deciding on a separately approved recovery. Do not retry automatically.'
  },
  MODEL_FAILED:{
    title:'Model response failed',
    meaning:'The model failed, but a native operation could already have occurred. A failed answer is not evidence of a rolled-back effect.',
    nextStep:'Inspect the effect journal and current source before initiating a new request.'
  },
  EFFECT_FAILED:{
    title:'External operation failed',
    meaning:'The native effect journal records failure. Partial external changes cannot be ruled out solely from this status.',
    nextStep:'Review the original target and recorded evidence before any new authorized operation.'
  },
  OUTCOME_CONFLICT:{
    title:'External result contradicted',
    meaning:'An independent host observation disagreed with the external application result. Lu cannot treat those facts as verified.',
    nextStep:'Review the source and request a fresh check with current authorization if appropriate.'
  },
  REJECTED:{
    title:'Request rejected',
    meaning:'A tool or model event was rejected. This record does not confer permission or prove the requested operation ran.',
    nextStep:'Review the requested scope and make a new explicit choice if you still want the operation.'
  },
  APPLIED_RECEIPT_MISSING:{
    title:'Execution receipt missing',
    meaning:'An effect is marked APPLIED without its retained receipt. Neither the answer nor the state alone verifies the outcome.',
    nextStep:'Inspect the source and journal discrepancy; do not automatically replay the effect.'
  },
  INDEPENDENTLY_VERIFIED_AT_OBSERVATION:{
    title:'Source matched at observation',
    meaning:'An independent host check matched the recorded result at that time. It does not certify current file state or ongoing process termination.',
    nextStep:'Recheck the source before relying on this result as current.'
  },
  ANSWER_RECORDED_WITH_APPLIED_JOURNAL:{
    title:'Execution recorded; facts not fully verified',
    meaning:'The host retained an APPLIED effect record, but independent verification is missing for at least one result.',
    nextStep:'Examine the saved source and evidence before treating the answer as a confirmed fact.'
  },
  MODEL_ANSWER_ONLY:{
    title:'Model answer only',
    meaning:'Lu recorded model text without a verified native execution effect. The answer is not an OS operation receipt.',
    nextStep:'If you need an external fact, explicitly authorize a new tool check.'
  },
  INCOMPLETE_OR_INTERRUPTED:{
    title:'No completed result',
    meaning:'No final answer or independently confirmed native outcome is established for this activity.',
    nextStep:'Review the task and saved effects before any new attempt.'
  }
});

const OTHER=Object.freeze({
  COMPLETED:{
    title:'Model turn completed',
    meaning:'A model response was recorded. This activity summary alone does not certify an operating-system effect or independently verified fact.',
    nextStep:'Consult the source or a separate verified native receipt if external correctness matters.'
  },
  FAILED:{
    title:'Model turn failed',
    meaning:'The model turn failed. This summary does not establish that every associated native effect was rolled back.',
    nextStep:'Review recorded effects before another authorized attempt.'
  },
  REJECTED:REVIEWED.REJECTED,
  INCOMPLETE_OR_INTERRUPTED:REVIEWED.INCOMPLETE_OR_INTERRUPTED
});

// Do not echo unrecognized status strings as authoritative UI labels.
// Both trusted-host data and future schema revisions fail closed here.
export function explainToolOutcome(source,state){
  if(source==='Reviewed app'){
    return Object.hasOwn(REVIEWED,state)?REVIEWED[state]:FALLBACK;
  }
  return Object.hasOwn(OTHER,state)?OTHER[state]:FALLBACK;
}
