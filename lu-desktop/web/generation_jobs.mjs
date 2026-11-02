// Pure state reducer: each in-flight generation is owned by exactly one chat.
// No model I/O, URLs, or privileged access is performed in this module.
export function queuedJob(jobs, payload) {
  if (!payload?.chatId || !payload?.generationId) return jobs;
  const prior = jobs[payload.chatId];
  if (prior?.generationId && prior.generationId !== payload.generationId) return jobs;
  return {...jobs, [payload.chatId]: {...(prior || {}),
    generationId: payload.generationId, phase:'queued', position:payload.position,
    text:prior?.text || ''}};
}
export function startedJob(jobs, payload) {
  if (!payload?.chatId || !payload?.generationId) return jobs;
  const prior = jobs[payload.chatId];
  if (!prior || (prior.generationId && prior.generationId !== payload.generationId)) return jobs;
  return {...jobs,[payload.chatId]:{...prior,generationId:payload.generationId,
    phase:'running',position:0}};
}
export function tokenForJob(jobs, payload) {
  if (!payload?.chatId || !payload?.generationId || typeof payload.text !== 'string') return jobs;
  const prior = jobs[payload.chatId];
  if (!prior || (prior.generationId && prior.generationId !== payload.generationId)) return jobs;
  return {...jobs,[payload.chatId]:{...prior,generationId:payload.generationId,
    phase:'running',text:(prior.text || '')+payload.text}};
}
export function removeJob(jobs, chatId) {
  if (!jobs[chatId]) return jobs;
  const next = {...jobs}; delete next[chatId]; return next;
}
