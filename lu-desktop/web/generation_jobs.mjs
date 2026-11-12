// Pure, bounded, chat-scoped stream state. Native host is the authority for
// reconnect snapshots; UI events carry per-generation contiguous chunk indices.
// No model I/O, file effects, or tool authority is supplied by these reducers.
export const MAX_LIVE_CHARS = 1024 * 1024;

function valid(payload) {
  return typeof payload?.chatId === 'string' && payload.chatId.length > 0 &&
    typeof payload?.generationId === 'string' && payload.generationId.length > 0;
}
function validIndex(value) {
  return Number.isSafeInteger(value) && value >= 0 && value <= MAX_LIVE_CHARS;
}
export function queuedJob(jobs, payload) {
  if (!valid(payload)) return jobs;
  const prior = jobs[payload.chatId];
  if (prior?.generationId && prior.generationId !== payload.generationId) return jobs;
  return {...jobs,[payload.chatId]:{
    ...(prior||{}),generationId:payload.generationId,phase:'queued',
    position:payload.position,
    text:prior?.text||'',nextIndex:prior?.nextIndex||0,
    needsSync:Boolean(prior?.needsSync)
  }};
}
export function startedJob(jobs,payload) {
  if (!valid(payload)) return jobs;
  const prior=jobs[payload.chatId];
  if (prior?.generationId && prior.generationId!==payload.generationId) return jobs;
  // A reloaded WebView may miss 'queued'; the native snapshot repairs the
  // otherwise unknown earlier chunk cursor before any new chunks are appended.
  return {...jobs,[payload.chatId]:{
    ...(prior||{}),generationId:payload.generationId,
    phase:'running',position:0,text:prior?.text||'',
    nextIndex:prior?.nextIndex||0,needsSync:!prior||Boolean(prior.needsSync)
  }};
}
export function tokenForJob(jobs,payload) {
  if (!valid(payload)||typeof payload.text!=='string'||payload.text.length===0)
    return jobs;
  const prior=jobs[payload.chatId];
  if (prior?.generationId && prior.generationId!==payload.generationId)
    return jobs; // A late event may not replace the next generation.
  if (!prior) {
    // Never append a token at an unknown offset. Reconnect by authenticated
    // host session+chat, rather than guessing the missing prefix.
    return {...jobs,[payload.chatId]:{
      generationId:payload.generationId,phase:'running',position:0,
      text:'',nextIndex:0,needsSync:true
    }};
  }
  const expected=prior.nextIndex||0;
  const index=payload.index;
  if (index!==undefined && (!validIndex(index) || index>expected)) {
    return {...jobs,[payload.chatId]:{...prior,needsSync:true}};
  }
  if (index!==undefined && index<expected) return jobs; // Duplicate/late.
  if ((prior.text||'').length+payload.text.length>MAX_LIVE_CHARS) {
    return {...jobs,[payload.chatId]:{...prior,needsSync:true}};
  }
  return {...jobs,[payload.chatId]:{
    ...prior,generationId:payload.generationId,phase:'running',
    text:(prior.text||'')+payload.text,nextIndex:expected+1
  }};
}
export function resumedJob(jobs,chatId,snapshot) {
  if (!chatId) return jobs;
  if (snapshot==null) return removeJob(jobs,chatId);
  if (!valid(snapshot) || snapshot.chatId!==chatId ||
      !['queued','running'].includes(snapshot.phase) ||
      typeof snapshot.text!=='string' ||
      snapshot.text.length>MAX_LIVE_CHARS ||
      !validIndex(snapshot.nextIndex)) return jobs;
  const old=jobs[chatId];
  if (old?.generationId===snapshot.generationId &&
      validIndex(old.nextIndex) && old.nextIndex>snapshot.nextIndex) {
    return jobs; // A later UI chunk must win over an older RPC snapshot.
  }
  return {...jobs,[chatId]:{
    generationId:snapshot.generationId,phase:snapshot.phase,
    position:snapshot.phase==='queued'?1:0,text:snapshot.text,
    nextIndex:snapshot.nextIndex,
    cancelling:Boolean(snapshot.cancelling),needsSync:false
  }};
}
export function removeJob(jobs,chatId) {
  if (!jobs[chatId]) return jobs;
  const next={...jobs};delete next[chatId];return next;
}
