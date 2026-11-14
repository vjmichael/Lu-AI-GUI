// DEV-03.3B: native untrusted output parser, never an execution grant.
export const MAX_OUTPUT_LINES = 256;
export const MAX_OUTPUT_TEXT = 65536;
const STREAMS = new Set(['stdout','stderr','combined']);
const ID = /^[a-zA-Z0-9_-]{1,96}$/;

export function acceptNativeOutput(previous, projectId, jobId, page) {
  if (typeof projectId !== 'string' || !/^ide-[a-fA-F0-9]{32}$/.test(projectId) ||
      typeof jobId !== 'string' || !ID.test(jobId) ||
      !page || page.liveUnverified !== true ||
      !Array.isArray(page.chunks) || page.chunks.length > 64 ||
      !Number.isSafeInteger(page.lastSequence) || page.lastSequence < 0) return null;
  const matching = previous?.projectId === projectId && previous?.jobId === jobId;
  const prior = matching ? previous : {projectId,jobId,chunks:[],lastSequence:0,truncated:false};
  let sequence = prior.lastSequence;
  let next = prior.chunks;
  for (const item of page.chunks) {
    if (!item || !Number.isSafeInteger(item.sequence) || item.sequence <= sequence ||
        item.sequence > page.lastSequence || !STREAMS.has(item.stream) ||
        typeof item.text !== 'string' || item.text.length > 8192) return null;
    sequence = item.sequence;
    next = [...next,{sequence,stream:item.stream,text:item.text}];
  }
  if (page.lastSequence < sequence) return null;
  // A cursor can move beyond chunks returned due to pagination; only advance
  // to the last *received* chunk, not a speculative server upper bound.
  let byteBudget = next.reduce((sum,item)=>sum+item.text.length,0);
  let truncated = prior.truncated || page.truncated === true;
  while (next.length > MAX_OUTPUT_LINES || byteBudget > MAX_OUTPUT_TEXT) {
    byteBudget -= next[0].text.length;
    next = next.slice(1);
    truncated = true;
  }
  return {projectId,jobId,chunks:next,lastSequence:sequence,truncated,
    liveUnverified:true};
}
