/**
 * IDE-INT03: bounded and strictly nonauthoritative Build/Test log hints.
 *
 * Compiler/test output is attacker-controlled project text, NOT a diagnostic,
 * a source revision, a pass/fail receipt, a filesystem capability, or an
 * excuse to run an executable. These hints are rendered as read-only text.
 */
const PROJECT=/^ide-[a-f0-9]{32}$/;
const JOB=/^job-[a-f0-9]{32}$/;
const MAX_HINTS=24;
const MAX_LOG_CHARS=65536;

function safeRelativePath(value) {
  if (typeof value!=='string' || value.length===0 || value.length>512 ||
      value.startsWith('/') || value.includes('\\') ||
      /[^\x20-\x7e]/.test(value) ||
      value.includes(':') || value.includes('://') ||
      value.split('/').some(x=>!x || x==='.' || x==='..') ||
      value.startsWith('-')) return null;
  return value;
}

function parseLine(line) {
  const clean=line.replace(/[\x00-\x1f\x7f]/g,' ').slice(0,280).trim();
  if (!clean) return null;
  // Rust points at a source position after a compiler error. The path is
  // only a string hint, never auto-opened or sent as an IPC filesystem path.
  const rust=clean.match(/^-->\s+([A-Za-z0-9_./+\-]+):(\d{1,6}):(\d{1,6})/);
  if (rust) {
    const path=safeRelativePath(rust[1]);
    const lineNumber=Number(rust[2]);
    if (path && lineNumber>0 && lineNumber<=250000)
      return {category:'compiler_location_hint',relativePath:path,
        line:lineNumber,message:clean};
  }
  const pytest=clean.match(/^([A-Za-z0-9_./+\-]+\.py):(\d{1,6}):\s*(.+)$/);
  if (pytest) {
    const path=safeRelativePath(pytest[1]);
    const number=Number(pytest[2]);
    if (path && number>0 && number<=250000)
      return {category:'python_test_location_hint',relativePath:path,
        line:number,message:clean};
  }
  if (/^(error(\[[A-Z]\d+\])?:|error:|FAILED\s+|FAIL\s+|FAILED$|AssertionError:|fatal error:|CMake Error)/i.test(clean))
    return {category:'untrusted_error_text',relativePath:null,line:null,message:clean};
  if (/^(warning:|test result:|=+ FAILURES =+|=+ ERROR)/i.test(clean))
    return {category:'untrusted_summary_text',relativePath:null,line:null,message:clean};
  return null;
}

export function boundedBuildTestHints(output, projectId, jobId) {
  if (!PROJECT.test(projectId||'') || !JOB.test(jobId||'') ||
      output?.projectId!==projectId || output?.jobId!==jobId ||
      output.liveUnverified!==true || !Array.isArray(output.chunks) ||
      output.chunks.length>256) return Object.freeze([]);
  let used=0;
  const out=[];
  const seen=new Set();
  for (const chunk of output.chunks) {
    if (!chunk || !['stdout','stderr','combined'].includes(chunk.stream) ||
        typeof chunk.text!=='string' || chunk.text.length>8192) return Object.freeze([]);
    used+=chunk.text.length;
    if (used>MAX_LOG_CHARS) return Object.freeze([]);
    for (const line of chunk.text.split(/\r?\n/).slice(0,200)) {
      const parsed=parseLine(line);
      if (!parsed) continue;
      const key=parsed.category+':'+parsed.message;
      if (seen.has(key)) continue;
      seen.add(key);
      out.push(Object.freeze({...parsed,
        verified:false,navigable:false,assertionProved:false}));
      if (out.length===MAX_HINTS) return Object.freeze(out);
    }
  }
  return Object.freeze(out);
}
