/**
 * B11: bounded, deterministic, host-agnostic three-way line merge.
 *
 * This is an EXPLICIT user-reviewed editor-buffer operation, never a Save,
 * disk write, watcher, command, worker or grant of filesystem authority.
 * Conflicts stay unresolved until the user chooses base/draft/disk for each.
 * Line tokens preserve exact LF/CRLF/CR bytes, including final newlines.
 */
export const MAX_MERGE_LINES = 600;
export const MAX_MERGE_CHARS = 128 * 1024;
export const MAX_MERGE_CONFLICTS = 80;
export const MAX_MERGE_PREVIEW_LINES = 5;
export const MAX_MERGE_PREVIEW_CHARS = 180;

const linesOf = text => text.match(/[^\r\n]*(?:\r\n|\n|\r|$)/g)?.filter(Boolean) ?? [];
const joined = lines => lines.join('');

function ensureBounded(text) {
  if (typeof text !== 'string' || text.length > MAX_MERGE_CHARS)
    throw Error('B11_INPUT_TOO_LARGE');
  const lines = linesOf(text);
  if (lines.length > MAX_MERGE_LINES) throw Error('B11_LINE_BUDGET');
  return lines;
}

/** LCS-based minimal line edit hunks. O(600^2) bounded in both dimensions. */
function lineHunks(base, target) {
  const n = base.length, m = target.length;
  const dp = Array.from({length:n + 1}, () => new Uint16Array(m + 1));
  for (let i = n - 1; i >= 0; --i) {
    for (let j = m - 1; j >= 0; --j) {
      dp[i][j] = base[i] === target[j]
        ? dp[i + 1][j + 1] + 1
        : Math.max(dp[i + 1][j], dp[i][j + 1]);
    }
  }
  let i=0,j=0,work=null;
  const out=[];
  const flush=()=>{if(work){out.push(work);work=null;}};
  const begin=()=>{if(!work)work={start:i,end:i,lines:[]};};
  while(i<n || j<m) {
    if(i<n && j<m && base[i]===target[j]) {
      flush();i++;j++;
    } else if(i<n && (j===m || dp[i+1][j]>=dp[i][j+1])) {
      begin();i++;work.end=i;
    } else {
      begin();work.lines.push(target[j]);j++;
    }
  }
  flush();
  return out;
}

function sameBaseRegion(a,b) {
  // Two zero-width inserts at the same boundary conflict. An insert at
  // an edit boundary is ambiguous too; conservatively require review.
  if(a.start===a.end) return b.start<=a.start && a.start<=b.end;
  if(b.start===b.end) return a.start<=b.start && b.start<=a.end;
  return a.start<b.end && b.start<a.end;
}

function patchedRegion(base,hunks,start,end) {
  if(!hunks.length) return joined(base.slice(start,end));
  const sorted=[...hunks].sort((a,b)=>a.start-b.start || a.end-b.end);
  const pieces=[];
  let at=start;
  for(const h of sorted) {
    if(h.start<at || h.end>end) throw Error('B11_INVALID_HUNK_RANGE');
    pieces.push(joined(base.slice(at,h.start)),joined(h.lines));
    at=h.end;
  }
  pieces.push(joined(base.slice(at,end)));
  return pieces.join('');
}

function addText(parts,text) {
  if(text) {
    const last=parts[parts.length-1];
    if(last?.kind==='text') last.text+=text;
    else parts.push({kind:'text',text});
  }
}

/**
 * Build a source-only merge preview. Different edits on disjoint base ranges
 * combine automatically; potentially touching/overlapping edits are conflicts
 * unless the resulting exact bytes match.
 */
export function planThreeWayMerge(baseText,draftText,diskText) {
  const base=ensureBounded(baseText), draft=ensureBounded(draftText);
  const disk=ensureBounded(diskText);
  const local=lineHunks(base,draft).map(h=>({...h,side:'draft'}));
  const external=lineHunks(base,disk).map(h=>({...h,side:'disk'}));
  const all=[...local,...external].sort(
    (a,b)=>a.start-b.start || a.end-b.end ||
      (a.side==='draft'?-1:1));
  const groups=[];
  for(const h of all) {
    let group=groups[groups.length-1];
    if(!group || !group.hunks.some(other=>sameBaseRegion(other,h))) {
      groups.push({start:h.start,end:h.end,hunks:[h]});
    } else {
      group.start=Math.min(group.start,h.start);
      group.end=Math.max(group.end,h.end);
      group.hunks.push(h);
    }
  }
  // Merge transitive ranges after a bridging edit grows a group.
  for(let g=0;g<groups.length-1;) {
    if(groups[g].hunks.some(a=>groups[g+1].hunks.some(b=>sameBaseRegion(a,b)))) {
      const next=groups.splice(g+1,1)[0];
      groups[g].start=Math.min(groups[g].start,next.start);
      groups[g].end=Math.max(groups[g].end,next.end);
      groups[g].hunks.push(...next.hunks);
    } else g++;
  }
  const parts=[],conflicts=[];
  let cursor=0;
  for(const group of groups) {
    if(group.start<cursor) throw Error('B11_OVERLAPPING_GROUPS');
    addText(parts,joined(base.slice(cursor,group.start)));
    const draftHunks=group.hunks.filter(h=>h.side==='draft');
    const diskHunks=group.hunks.filter(h=>h.side==='disk');
    const baseValue=joined(base.slice(group.start,group.end));
    const left=patchedRegion(base,draftHunks,group.start,group.end);
    const right=patchedRegion(base,diskHunks,group.start,group.end);
    if(!draftHunks.length) addText(parts,right);
    else if(!diskHunks.length || left===right) addText(parts,left);
    else {
      if(conflicts.length>=MAX_MERGE_CONFLICTS)
        throw Error('B11_CONFLICT_BUDGET');
      const id='conflict-'+(conflicts.length+1);
      const conflict={id,firstBaseLine:group.start+1,
        baseText:baseValue,draftText:left,diskText:right};
      conflicts.push(conflict);
      parts.push({kind:'conflict',id});
    }
    cursor=group.end;
  }
  addText(parts,joined(base.slice(cursor)));
  return {schemaVersion:1,parts,conflicts,baseChars:baseText.length,
    draftChars:draftText.length,diskChars:diskText.length,
    hasDiskDrift:baseText!==diskText};
}

/** All conflicts must receive an explicit human choice. No default. */
export function renderReviewedMerge(plan,choices) {
  if(plan?.schemaVersion!==1 || !Array.isArray(plan.parts) ||
      !Array.isArray(plan.conflicts) || !choices || typeof choices!=='object')
    throw Error('B11_INVALID_PLAN');
  const known=new Map(plan.conflicts.map(c=>[c.id,c]));
  if(known.size!==plan.conflicts.length) throw Error('B11_INVALID_PLAN');
  for(const conflict of plan.conflicts) {
    if(!Object.hasOwn(choices,conflict.id) ||
      !['draft','disk','base'].includes(choices[conflict.id]))
      return {ready:false,missing:conflict.id,text:null};
  }
  const text=plan.parts.map(part=>{
    if(part.kind==='text') return part.text;
    if(part.kind!=='conflict' || !known.has(part.id))
      throw Error('B11_INVALID_PLAN');
    return known.get(part.id)[choices[part.id]+'Text'];
  }).join('');
  if(text.length>MAX_MERGE_CHARS || linesOf(text).length>MAX_MERGE_LINES)
    throw Error('B11_MERGED_RESULT_TOO_LARGE');
  return {ready:true,missing:null,text};
}

/** Plain text React-only preview, never an HTML fragment or patch command. */
export function conflictExcerpt(text) {
  const lines=linesOf(text);
  const shown=lines.slice(0,MAX_MERGE_PREVIEW_LINES)
    .map(line=>line.slice(0,MAX_MERGE_PREVIEW_CHARS));
  return {
    lines:shown,
    extraLines:Math.max(0,lines.length-shown.length),
    clipped:shown.some((_,i)=>lines[i].length>MAX_MERGE_PREVIEW_CHARS),
  };
}

/**
 * The merge is applicable only to the exact tab, base disk snapshot, and
 * live CodeMirror text from which the user reviewed it. This does not
 * eliminate a later external filesystem write; native Save CAS still does.
 */
export function mergeCanApply(review,tab,state,projectId,preview) {
  return !!(review && tab && state &&
    projectId===review.projectId && tab.projectId===projectId &&
    tab.key===review.tabKey && tab.relativePath===review.relativePath &&
    tab.revision===review.savedRevision &&
    tab.generation===review.generation &&
    tab.bufferRevision===review.bufferRevision &&
    tab.savedText===review.savedText &&
    state===review.editorState &&
    state.sliceDoc()===review.draftText &&
    preview?.projectId===projectId &&
    preview?.relativePath===review.relativePath &&
    preview?.revision===review.diskRevision &&
    preview?.text===review.diskText);
}
