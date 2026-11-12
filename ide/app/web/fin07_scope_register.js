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
