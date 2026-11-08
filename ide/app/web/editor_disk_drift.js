/**
 * B10: explicit, read-only external source drift inspection.
 *
 * Nothing in this module authorizes filesystem I/O or mutates an editor.
 * Native reads must still use the registered cap-std source capability.
 * Results are observations at a moment in time, not save/reload approval.
 */
import { verifiedRelativeSourcePath } from './editor_working_set.js';

const sha = value => typeof value === 'string' && /^[a-f0-9]{64}$/.test(value);
const validProject = value => typeof value === 'string' && /^ide-[a-f0-9]{32}$/.test(value);
export const MAX_DISK_AUDIT_TABS = 48;
export const MAX_COMPARISON_CHARS = 524288;
export const COMPARISON_VISIBLE_LINES = 6;
export const COMPARISON_LINE_CHARS = 240;

export function diskAuditTargets(tabs, projectId) {
  if (!validProject(projectId) || !Array.isArray(tabs)) throw Error('B10_INVALID_AUDIT');
  const selected = tabs.filter(tab=>tab?.projectId===projectId);
  if (selected.length>MAX_DISK_AUDIT_TABS) throw Error('B10_AUDIT_BUDGET');
  const seen=new Set();
  return selected.map(tab=>{
    if (typeof tab.key!=='string' || tab.key!==projectId+':'+tab.relativePath ||
        verifiedRelativeSourcePath(tab.relativePath)!==tab.relativePath ||
        !sha(tab.revision) ||
        !Number.isSafeInteger(tab.generation) ||
        !Number.isSafeInteger(tab.bufferRevision) ||
        seen.has(tab.key)) throw Error('B10_INVALID_SOURCE_IDENTITY');
    seen.add(tab.key);
    // Keep this snapshot free of source code and draft text.
    return Object.freeze({
      projectId,key:tab.key,relativePath:tab.relativePath,
      savedRevision:tab.revision,generation:tab.generation,
      bufferRevision:tab.bufferRevision,
    });
  });
}

export function classifyDiskAudit(target, current, preview, projectId, sameState) {
  if (!target || !current || !validProject(projectId) ||
      target.projectId!==projectId ||
      current.projectId!==projectId || target.key!==current.key ||
      target.relativePath!==current.relativePath ||
      target.savedRevision!==current.revision ||
      target.generation!==current.generation ||
      target.bufferRevision!==current.bufferRevision || !sameState)
    return {status:'stale',diskRevision:null};
  if (!preview || preview.projectId!==projectId ||
      preview.relativePath!==target.relativePath ||
      !sha(preview.revision) || typeof preview.text!=='string')
    return {status:'invalid_native_result',diskRevision:null};
  return {
    status:preview.revision===target.savedRevision?'current':'changed',
    diskRevision:preview.revision,
  };
}

/** Inspect at most the first differing bounded region; NOT a full merge diff.
 * Normalize CRLF, LF and CR only for line-level comparison and previews.
 */
function boundedRegion(left,right) {
  const a=left.split(/\r\n|\n|\r/);
  const b=right.split(/\r\n|\n|\r/);
  let prefix=0;
  while(prefix<a.length && prefix<b.length && a[prefix]===b[prefix]) prefix++;
  let suffix=0;
  while(suffix<a.length-prefix && suffix<b.length-prefix &&
    a[a.length-1-suffix]===b[b.length-1-suffix]) suffix++;
  const excerpt=(lines, end)=>{
    const sampled=lines.slice(prefix,Math.min(end,prefix+COMPARISON_VISIBLE_LINES))
      .map(line=>line.slice(0,COMPARISON_LINE_CHARS));
    return Object.freeze({
      startLine:prefix+1,lines:sampled,
      moreLines:Math.max(0,end-prefix-COMPARISON_VISIBLE_LINES),
      clippedLines:lines.slice(prefix,Math.min(end,prefix+COMPARISON_VISIBLE_LINES))
        .filter(line=>line.length>COMPARISON_LINE_CHARS).length,
    });
  };
  const leftEnd=a.length-suffix,rightEnd=b.length-suffix;
  return Object.freeze({
    identical:left===right,
    // A bytewise newline change can yield no differing logical lines. Keep
    // it visible instead of wrongly claiming that two byte streams match.
    lineEndingOnlyDifference:left!==right && prefix===a.length && prefix===b.length,
    firstDifferentLine:prefix+1,
    leftChangedLines:Math.max(0,leftEnd-prefix),
    rightChangedLines:Math.max(0,rightEnd-prefix),
    left:excerpt(a,leftEnd),right:excerpt(b,rightEnd),
    // A prefix/suffix excerpt must not be misrepresented as a complete diff.
    method:'bounded-first-differing-region-not-merge',
  });
}

export function summarizeExternalDifference(saved,draft,disk) {
  if ([saved,draft,disk].some(s=>typeof s!=='string' ||
      s.length>MAX_COMPARISON_CHARS)) throw Error('B10_COMPARE_TOO_LARGE');
  return Object.freeze({
    savedVersusDisk:boundedRegion(saved,disk),
    draftVersusDisk:boundedRegion(draft,disk),
    localUnsaved:draft!==saved,
    // Byte-wise verification remains authoritative in the native host;
    // these are for bounded visual comparison only.
    savedChars:saved.length,draftChars:draft.length,diskChars:disk.length,
  });
}
