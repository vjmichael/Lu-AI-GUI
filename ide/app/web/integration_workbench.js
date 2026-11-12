/**
 * IDE-INT06: conservative workflow integration for a single OS-picked project.
 * Observations come only from existing native READ-ONLY commands.
 * A series of independent probes is NOT an atomic snapshot or execution grant.
 */
import {safeProjectRunProfileDiscovery} from './project_run_profiles.js';
import {checkedToolchainInventory} from './toolchain_terminal.js';
import {checkedLspWorkerStatus} from './lsp_runtime_ui.js';
import {debugAdmissionStatus} from './debugger_integration.js';

const PROJECT=/^ide-[a-f0-9]{32}$/;
const SHA=/^[a-f0-9]{64}$/;
const HEAD=/^[a-f0-9]{40}$/;
const REQUESTS=Object.freeze([
  ['profiles','ide_discover_project_run_profiles',true],
  ['git','ide_read_git_repository_inventory',true],
  ['packages','ide_inspect_package_managers',true],
  ['build','ide_build_status',true],
  ['debug','ide_debug_status',true],
  ['language','ide_lsp_worker_status',false],
]);
const ROUTES=new Set(['explorer','languages','tasks','source',
  'packages','terminal']);
const STATUSES=new Set(['observed','review_required','blocked',
  'reconciliation_required','unverified']);
const entry=(id,label,target,state,detail)=>Object.freeze({
  id,label,target,state,detail,
  // Navigating can never admit a process, install a package or accept a task.
  executionAuthorized:false,taskAccepted:false,installedAccepted:false,
});
const isObject=v=>v!==null && typeof v==='object' && !Array.isArray(v);
function checkedSource(v,projectId) {
  if(!isObject(v) || v.projectId!==projectId ||
      v.schemaVersion!==1 || v.remoteQueried!==false ||
      v.gitProcessStarted!==false || v.refsOrFilesModified!==false ||
      v.linkedWorktreeAccessGranted!==false ||
      typeof v.state!=='string' || v.state.length>96 ||
      typeof v.headState!=='string' || v.headState.length>40 ||
      (v.currentHeadSha!==null && !HEAD.test(v.currentHeadSha||'')) ||
      !Array.isArray(v.branches) || !Array.isArray(v.worktrees) ||
      v.branches.length>256 || v.worktrees.length>128) return null;
  return Object.freeze({
    inspected:v.state==='local_inventory_inspected',
    headState:v.headState,
    headSha:v.currentHeadSha,
  });
}
function checkedPackages(v,projectId) {
  if(!isObject(v) || v.projectId!==projectId ||
      v.schemaVersion!==1 || !SHA.test(v.fingerprint||'') ||
      v.executionAuthorized!==false || v.releaseAccepted!==false ||
      v.processStarted!==false || v.networkAccessed!==false ||
      v.filesModified!==false || v.resolutionSupported!==false ||
      v.applySupported!==false || !Array.isArray(v.candidates) ||
      !Array.isArray(v.blockers) || v.candidates.length>3 ||
      !v.blockers.every(s=>typeof s==='string' && s.length<100)) return null;
  const ids=new Set();
  let reviewable=0;
  for(const row of v.candidates) {
    if(!row || !['uv','vcpkg','conan'].includes(row.managerId) ||
        ids.has(row.managerId) || typeof row.state!=='string' ||
        row.executionAuthorized!==false || row.applySupported!==false ||
        row.resolutionSupported!==false) return null;
    ids.add(row.managerId);
    if(row.state==='candidate_needs_review')reviewable++;
  }
  return Object.freeze({reviewable,blockerCount:v.blockers.length});
}
function checkedProfiles(v,projectId) {
  try{return safeProjectRunProfileDiscovery(v,projectId);}
  catch{return null;}
}

/** The workflow is a review/navigation surface, not a release gate. */
export function projectWorkbench(projectId,raw) {
  if(!PROJECT.test(projectId||'') || !isObject(raw))return null;
  const git=checkedSource(raw.git,projectId);
  const packages=checkedPackages(raw.packages,projectId);
  const profiles=checkedProfiles(raw.profiles,projectId);
  const inventory=checkedToolchainInventory(raw.build,projectId);
  const nativeBuild=inventory.kind==='native_pin_observation';
  const lsp=checkedLspWorkerStatus(raw.language);
  const debug=debugAdmissionStatus(raw.debug,projectId);
  const buildRecovery=Array.isArray(raw.build?.quarantined) &&
      raw.build.quarantined.some(x=>x?.requiresReconciliation===true) ||
    Array.isArray(raw.build?.journal) &&
      raw.build.journal.some(x=>x?.requiresReconciliation===true);
  const debugRecovery=raw.debug?.globalRecoveryRequired===true ||
    raw.debug?.orphanedDebugActionRequired===true ||
    Array.isArray(raw.debug?.quarantined) &&
      raw.debug.quarantined.some(x=>x?.requiresReconciliation===true);
  const lspRecovery=lsp.sessionState==='reconciliation_required';
  const recovery=!!(buildRecovery || debugRecovery || lspRecovery);
  const matchedProfiles=profiles?.profiles.filter(p=>
    p.manifestState==='saved_file_sha_observed_not_validated' &&
    SHA.test(p.sha||''))||[];
  const nativeReady=inventory.rows.some(x=>x.ready);
  const rows=[
    entry('editor','Editor & saved sources','explorer',
      'review_required','Open or review saved project files. Unsaved CodeMirror buffers are not certified by this workbench.'),
    entry('source','Local Git inventory','source',
      !git?'unverified':git.inspected?'observed':'review_required',
      git?.inspected && git.headSha ?
        'Local HEAD observed: '+git.headSha.slice(0,12)+'…; remote, dirty buffers and merge safety not implied.' :
        'Local branch/HEAD not verified. Inspect the repository before Git operations.'),
    entry('language','Language services','languages',
      lspRecovery?'reconciliation_required':lsp.ready?'review_required':'unverified',
      lspRecovery?'An interrupted native language worker requires OS reconciliation.' :
      lsp.ready?'Pinned provider observed; starting a session still needs native approval.' :
      'Native provider availability is not verified.'),
    entry('build','Build & test','tasks',
      buildRecovery?'reconciliation_required':
      nativeBuild && nativeReady && matchedProfiles.length>0?
        'review_required':'unverified',
      buildRecovery?'Previous protected build/test job requires independent OS cleanup.' :
      nativeBuild && nativeReady && matchedProfiles.length>0?
        matchedProfiles.length+' saved manifest candidates; select a reviewed profile and obtain new OS approval.' :
        'No matching saved profile and installed pinned runtime have been jointly observed.'),
    entry('debug','Debugger','tasks',
      debugRecovery?'reconciliation_required':
      debug.ready && debug.projectDebugSupported?'review_required':'unverified',
      debugRecovery?'Debugger/action native cleanup is required.' :
      debug.ready && debug.projectDebugSupported?
        'Available for reviewed protected project target; no attach, arbitrary launch or automatic step.' :
        'Debugger target or native availability has not been verified.'),
    entry('packages','Packages & toolchains','packages',
      packages?.blockerCount?'blocked':
      packages?.reviewable && nativeBuild?'review_required':'unverified',
      packages?.blockerCount?'Manifest/lockfile evidence contains blockers. No resolver or install authorization.' :
      packages?.reviewable && nativeBuild?'Review detected managers against independently pinned runtimes; no installation rights.' :
      'Manifest/runtime evidence missing; no package operation is authorized.'),
    entry('terminal','Protected terminal','terminal',
      'review_required','Only fixed typed commands. Native Build/Test review and per-job OS consent remain required.'),
    entry('acceptance','Release acceptance','tasks',
      'unverified','Exact-head compiler, Windows/Linux native custody, installed-app and physical-human acceptance remain deferred.'),
  ];
  if(!rows.every(r=>STATUSES.has(r.state) && ROUTES.has(r.target)))
    return null;
  const unresolved=rows.filter(r=>r.state==='reconciliation_required');
  const priority=unresolved.length?unresolved[0].id:
    rows.find(r=>r.state==='blocked')?.id||
    rows.find(r=>r.state==='unverified' && r.id!=='acceptance')?.id||
    'editor';
  return Object.freeze({
    projectId,kind:'non_atomic_read_only_observations',
    rows:Object.freeze(rows),priority,recoveryRequired:recovery,
    sourcesObserved:Object.freeze({
      git:!!git,packages:!!packages,profiles:!!profiles,
      runtime:nativeBuild,
      language:!!raw.language && lsp.sessionState!=='unavailable',
      debugger:!!raw.debug && Array.isArray(raw.debug.journal),
    }),
    executionAuthorized:false,
    taskAccepted:false,
    acceptanceVerified:false,
    installedAppAccepted:false,
  });
}

/**
 * Fixed allowlist: no URLs, paths, executables, mutating Git actions, or
 * untrusted shell arguments can be constructed from a workbench row.
 * Every result is separately scoped and checked only after ALL reads settle.
 */
export async function inspectProjectWorkbench(projectId,invokeRead,stillCurrent) {
  if(!PROJECT.test(projectId||'') ||
    typeof invokeRead!=='function' || typeof stillCurrent!=='function')
    return null;
  const values=await Promise.allSettled(REQUESTS.map(async([field,command,scoped])=>{
    const args=scoped?{projectId}:undefined;
    const result=await invokeRead(command,args);
    return [field,result];
  }));
  if(!stillCurrent())return null;
  const raw={};
  for(const result of values){
    if(result.status==='fulfilled' &&
        Array.isArray(result.value) && result.value.length===2)
      raw[result.value[0]]=result.value[1];
  }
  return projectWorkbench(projectId,raw);
}
export function checkedWorkbenchNavigation(workbench,id,projectId) {
  if(!workbench || workbench.projectId!==projectId ||
     workbench.kind!=='non_atomic_read_only_observations' ||
     workbench.executionAuthorized!==false ||
     !Array.isArray(workbench.rows) ||
     workbench.rows.length!==8) return null;
  const row=workbench.rows.find(r=>r.id===id);
  if(!row || !ROUTES.has(row.target) || !STATUSES.has(row.state) ||
     row.executionAuthorized!==false || row.taskAccepted!==false)
    return null;
  return Object.freeze({section:row.target,projectId,executionAuthorized:false});
}
