// Pure CP-01C composer decisions. The native receipt, not model output,
// text equality, or an event alone, establishes durable image admission.
export function submissionNonce(cryptoSource=globalThis.crypto){
  const bytes=new Uint8Array(16);
  cryptoSource.getRandomValues(bytes);
  return 's-'+Array.from(bytes,b=>b.toString(16).padStart(2,'0')).join('');
}
export function snapshotDraft(draft){
  return {prompt:draft.prompt,images:[...draft.images],files:[...draft.files]};
}
export function sameSubmittedDraft(current,original){
  return current.prompt===original.prompt&&
    current.images.length===original.images.length&&
    current.images.every((im,i)=>im.imageRef===original.images[i].imageRef)&&
    current.files.length===original.files.length&&
    current.files.every((f,i)=>f.name===original.files[i].name&&
      f.size===original.files[i].size&&f.text===original.files[i].text);
}
export function receiptDecision(attempt,receipt){
  if(!attempt?.generationId||receipt?.generationId!==attempt.generationId)
    return 'uncertain';
  const saved=Number.isSafeInteger(receipt.visibleSeq)&&receipt.visibleSeq>0;
  if(saved&&receipt.rejected===false)return 'saved';
  if(!saved&&receipt.visibleSeq===null&&receipt.rejected===true)return 'rejected';
  return 'uncertain';
}
export function clearSubmittedDraft(current,original){
  // Do not clear newly edited composer text/files even if a late receipt
  // arrives. A subsequent native manifest read determines image membership.
  if(!sameSubmittedDraft(current,original))return current;
  return {prompt:'',images:[],files:[]};
}
export function emptyDraft(draft){
  return !draft.prompt.trim()&&!draft.images.length&&!draft.files.length;
}
export function removeOwnedJob(jobs,chatId,attempt){
  const job=jobs[chatId];
  if(!job||!(attempt.generationId?job.generationId===attempt.generationId:
    job.submissionMarker===attempt.marker))return jobs;
  const next={...jobs};delete next[chatId];return next;
}
