// FIN-07 source scope ledger; not a release manifest or acceptance receipt.
// Items are deliberately conservative until independent original native
// Windows/Linux compiler, worker, install and physical owner evidence exists.
const groups=Object.freeze([
  {id:'FIN-01',priority:'P0',state:'source_slice_implemented',
   title:'Global custody and project readiness',
   remaining:'Prove shared cross-supervisor/OS-wide ownership and restart races in real native processes.'},
  {id:'FIN-02',priority:'P0',state:'implementation_remaining',
   title:'Protected runtime packaging and admission',
   remaining:'Build-stage pin/layout and read-only actual Linux cgroup-host readiness implemented. Real trusted resident host/IPC, approved binary distribution, Windows service/ACL and actual native installed evidence still missing.'},
  {id:'FIN-03',priority:'P1',state:'implementation_remaining',
   title:'GitHub and remaining local Git operations',
   remaining:'One-file native-reviewed Git unstage and no-replay journal reconciliation added. 6E-05 credential-backed publication, local revert/stash apply/drop, worktree mutation and physical consent remain missing.'},
  {id:'FIN-04',priority:'P1',state:'implementation_remaining',
   title:'Production debugging and complete test execution',
   remaining:'A narrowly scoped protected exact Rust libtest job added with original case-receipt correlation. Multi-framework per-case coverage, test-binary enumeration and production LLDB step/breakpoint/variables remain missing.'},
  {id:'FIN-05',priority:'P1',state:'implementation_remaining',
   title:'Package execution and third-party provenance',
   remaining:'Linux-only opt-in original protected uv Sync now has exact runtime-stage admission; vcpkg.json edit remains source-only. vcpkg/Conan execution, closed wheel supply chain and Windows uv source ACL/hostile OS tests still missing.'},
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
 if(!register || !Array.isArray(records) || records.length>FIN07_GATES.length)
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
