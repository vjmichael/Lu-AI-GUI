// DEV-03.3: read-only progress projection from native, project-scoped job status.
// No subprocess output, grants, paths, or source bytes may enter this timeline.
export const MAX_JOB_PROGRESS_EVENTS = 96;
const TOKEN = /^[a-zA-Z0-9_-]{1,96}$/;
const RECIPE = /^(rust_check|rust_build|rust_tests|lu_ide_host_tests)$/;
const PHASES = new Set(['preparing','prepared','authorized','admission_pending',
  'running','release_pending','cancel_requested','cancelled','completed','failed',
  'denied','recovery_required','uncertain']);
const MAX_JOBS = 8;

function valid(value, limit = 96) {
  return typeof value === 'string' && value.length <= limit && TOKEN.test(value);
}
function safeJob(source, expectedProjectId) {
  if (!source || typeof source !== 'object' ||
      source.projectId !== expectedProjectId ||
      !valid(source.jobId) || !RECIPE.test(source.recipeId || '')) return null;
  const phase = typeof source.phase === 'string' ? source.phase.toLowerCase() :
    (source.cancellationRequested === true ? 'cancel_requested' : 'running');
  if (!PHASES.has(phase)) return null;
  return {jobId:source.jobId, projectId:expectedProjectId,
    recipeId:source.recipeId, phase,
    cancellationRequested:source.cancellationRequested === true};
}
export function ingestJobProgress(previous, projectId, status) {
  if (!valid(projectId) || !status || typeof status !== 'object' ||
      !Array.isArray(status.recent) || !Array.isArray(status.quarantined)) return [];
  const old = Array.isArray(previous) ? previous.filter(event =>
    event?.projectId === projectId && valid(event.jobId) &&
    PHASES.has(event.phase)).slice(-MAX_JOB_PROGRESS_EVENTS) : [];
  // Native status is the authority for reported phases; never use WebView
  // supplied messages or arbitrary log text as progression evidence.
  const sources = [status.current, ...status.recent.slice(0, MAX_JOBS)];
  // Recovery views intentionally omit recipe identity. Do not infer it from
  // untrusted historical records or mislabel a quarantine as an active run.
  const observations = [];
  const seen = new Set();
  for (const source of sources) {
    const job = safeJob(source, projectId);
    if (!job || seen.has(job.jobId)) continue;
    seen.add(job.jobId);
    observations.push(job);
  }
  let result = old;
  for (const observation of observations) {
    const last = [...result].reverse().find(e => e.jobId === observation.jobId);
    const changed = !last || last.phase !== observation.phase ||
      last.cancellationRequested !== observation.cancellationRequested;
    if (changed) result = [...result, {...observation,
      ordinal: (last?.ordinal || 0) + 1}].slice(-MAX_JOB_PROGRESS_EVENTS);
  }
  return result;
}
export function visibleJobProgress(events, projectId) {
  if (!valid(projectId) || !Array.isArray(events)) return [];
  return events.filter(item => item?.projectId === projectId &&
    valid(item.jobId) && PHASES.has(item.phase)).slice(-MAX_JOB_PROGRESS_EVENTS);
}
