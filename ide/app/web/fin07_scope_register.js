// FIN-07 source scope ledger; not a release manifest or acceptance receipt.
// Items are deliberately conservative until independent original native
// Windows/Linux compiler, worker, install and physical owner evidence exists.
const groups=Object.freeze([
  {id:'FIN-01',priority:'P0',state:'source_slice_implemented',
   title:'Global custody and project readiness',
   remaining:'Build/package, debugger and protected LSP share original-journal custody; default-deny global stale-marker clearance now queries genuine Windows named Job Objects or delegated Linux cgroup-v2 kernel state, with owner-exit proof and a one-use native dialog. Source wired; actual installed native kernel acceptance, interruption races and release security review remain open. No destructive Git or package execution is promoted.'},
  {id:'FIN-02',priority:'P0',state:'implementation_remaining',
   title:'Protected runtime packaging and admission',
   remaining:'Build-stage pin/layout and read-only actual Linux cgroup-host readiness implemented. Real trusted resident host/IPC, approved binary distribution, Windows service/ACL and actual native installed evidence still missing.'},
  {id:'FIN-03',priority:'P1',state:'implementation_remaining',
   title:'GitHub and remaining local Git operations',
   remaining:'Native reviewed unstage plus default-disabled single-file restore/stash-save candidates remain unaccepted. FIN-03G now adds read-only exact-object impact previews for revert and stash apply/drop with path-overlap warnings and no execution route. Actual protected revert, stash apply/drop, worktree management, authenticated publication, OS custody and native acceptance remain open.'},
  {id:'FIN-04',priority:'P1',state:'implementation_remaining',
   title:'Production debugging and complete test execution',
   remaining:'Protected exact Rust libtest remains; FIN-04H supplies guarded Rust/Python/Node exact-case source (CTest built-test enumeration and native acceptance open). FIN-04R adds a separately feature-gated first-party Windows LLDB owner-event RELEASE OBSERVER, measured source/binary provenance gate, original Job/journal-bound live read-only event receipt, and native-dialog OS-only interrupted-action recovery witness with no replay and no global-marker clearance. Original one-use action-journal review and post-command verifier can compile in release, but non-test release debugger dispatch admission is HARD DENIED. The external CI-only PE/PDB symbolizer is not an accepted protected production worker; independent post-command PDB, real one-use production action consent, real Windows installed acceptance and CTest remain open.'},
  {id:'FIN-05',priority:'P1',state:'implementation_remaining',
   title:'Package execution and third-party provenance',
   remaining:'Existing protected Linux uv Sync remains. Opt-in Linux vcpkg source now requires pinned binary cache and binary-only offline installation, but lacks desktop execution authorization, completed output provenance and accepted runtime payload. Conan, Windows and supply-chain closure remain open.'},
  {id:'FIN-06',priority:'P1',state:'source_slice_implemented',
   title:'Human development workflow',
   remaining:'Native/installed user interaction acceptance. Conventional PTY terminal separately deferred; typed safe terminal remains.'},
  {id:'FIN-07',priority:'P0',state:'scope_register_open',
   title:'Implementation scope decision',
   remaining:'Owner feature-scope approval only after P0 required capabilities and original native/installed evidence reconcile.'},
]);
const PROJECT=/^ide-[a-f0-9]{32}$/;
export function fin07ScopeRegister(projectId){
 if(!PROJECT.test(projectId||''))return null;
 return Object.freeze({
   projectId,kind:'advisory_source_scope_register',
   rows:groups,unresolvedImplementation:groups.filter(row=>
      row.state==='implementation_remaining').length,
   frozen:false,releaseAuthorized:false,
   sourceCompilerAccepted:false,
   nativeWindowsAccepted:false,nativeLinuxAccepted:false,
   installedPackageAccepted:false,physicalHumanAccepted:false,
   taskAutomaticallyAccepted:false,
 });
}

/** FIN-07 explicit evidence reconciliation. Source implementations do not
 * promote themselves to operating-system or release acceptance. Evidence
 * must remain per-gate, exact-source, and independently reviewed. */
const FIN07_GATES=Object.freeze([
 'sourceCompiler','nativeWindows','nativeLinux',
 'installedWindows','installedLinux','physicalHuman',
 'security','dependencyLicenses'
]);
export function fin07ReconcileEvidence(projectId,records) {
 const register=fin07ScopeRegister(projectId);
 if(!register || !Array.isArray(records) || records.length>FIN07_GATES.length ||
    records.some(row=>!FIN07_GATES.includes(row?.gate)))
   return null;
 const seen=new Set();
 const gates=[];
 for(const gate of FIN07_GATES) {
   const evidence=records.filter(row=>row?.gate===gate);
   if(evidence.length>1)return null;
   const row=evidence[0];
   if(row && (seen.has(gate) || typeof row!=='object' ||
      row.projectId!==projectId ||
      !['missing','stale','pending_owner','review_required','independently_verified'].includes(row.status) ||
      (row.status==='independently_verified' &&
        (!/^[a-f0-9]{40}$/.test(row.sourceHead||'') ||
         !/^[a-f0-9]{64}$/.test(row.receiptSha256||'') ||
         row.independentReview!==true)) ||
      row.executionAuthorized!==false ||
      row.releaseAuthorized!==false))return null;
   seen.add(gate);
   gates.push(Object.freeze({gate,status:row?.status||'missing',
     exactSourceVerified:row?.status==='independently_verified',
     releaseAuthorized:false}));
 }
 return Object.freeze({projectId,kind:'advisory_evidence_reconciliation',
   gates:Object.freeze(gates),frozen:false,releaseAuthorized:false,
   ownerFeatureFreezeRequired:true,
   unresolvedImplementation:register.unresolvedImplementation,
   allEvidenceVerified:gates.every(row=>row.exactSourceVerified),
   taskAutomaticallyAccepted:false});
}

/** FIN-07B: reconcile the implementation ledger against FIN-06 observations.
 * A live observation may describe a slice as inspectable. It never makes
 * source-completeness, production safety or installed acceptance true. */
export function fin07ImplementationReconciliation(projectId,workflow) {
 const register=fin07ScopeRegister(projectId);
 if(!register || workflow?.kind!=='advisory_workflow_readiness' ||
    workflow.projectId!==projectId || workflow.executionAuthorized!==false ||
    workflow.installedAccepted!==false || workflow.taskAccepted!==false ||
    !Array.isArray(workflow.rows) || workflow.rows.length!==6 ||
    typeof workflow.blockedByRecovery!=='boolean')return null;
 const projections=register.rows.map(group=>{
   const boundary=group.id==='FIN-01' ? 'cross_supervisor_custody_unverified'
     : group.id==='FIN-02' ? 'deferred_native_runtime_admission'
     : group.id==='FIN-03' ? (workflow.gitObserved
        ? 'git_observed_mutation_gates_open':'git_native_evidence_missing')
     : group.id==='FIN-04' ? (workflow.debuggerLaunchObserved
        ? 'debug_launch_observed_production_controls_open':'protected_debugger_unavailable')
     : group.id==='FIN-05' ? (workflow.packageReviewObserved
        ? 'manifest_review_observed_executor_open':'package_executor_unavailable')
     : group.id==='FIN-06' ? (workflow.blockedByRecovery
        ? 'workflow_recovery_blocked':'guided_navigation_source_only')
     : 'owner_feature_scope_and_native_evidence_required';
   return Object.freeze({id:group.id,priority:group.priority,
     sourceState:group.state,boundary,
     implementationClosed:false,verifiedInstalled:false,
     ownerAccepted:false,releaseAuthorized:false});
 });
 return Object.freeze({projectId,kind:'conservative_fin_group_reconciliation',
   groups:Object.freeze(projections),
   unresolvedGroups:projections.filter(row=>!row.implementationClosed).length,
   sourceImplementationObserved:true,independentNativeAcceptance:false,
   productionReleaseEligible:false,featureFrozen:false,
   ownerAcceptanceRequired:true,automaticTaskCompletion:false});
}
