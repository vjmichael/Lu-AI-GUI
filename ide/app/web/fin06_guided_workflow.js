/** FIN-06: human-operated IDE workflow. Navigation is not execution.
 * A stage cannot be automatically completed by a prior stage's output. */
const PROJECT=/^ide-[a-f0-9]{32}$/;
const SHA=/^[a-f0-9]{64}$/;
const STEPS=Object.freeze([
 {id:'edit',label:'Review saved source',section:'explorer',row:'editor'},
 {id:'diagnose',label:'Inspect language diagnostics',section:'languages',row:'language'},
 {id:'build',label:'Review a protected build/test',section:'tasks',row:'build'},
 {id:'tests',label:'Inspect test reports',section:'test-explorer',row:'build'},
 {id:'debug',label:'Review protected debugger',section:'tasks',row:'debug'},
 {id:'git',label:'Review local Git changes',section:'source',row:'source'},
 {id:'accept',label:'Review human task acceptance',section:'continuity',row:'acceptance'},
]);
export function checkedFin06Journey(workbench,projectId,build,report,debug) {
 if(!PROJECT.test(projectId||'') ||
    workbench?.projectId!==projectId ||
    workbench.kind!=='non_atomic_read_only_observations' ||
    workbench.executionAuthorized!==false ||
    typeof workbench.recoveryRequired!=='boolean' ||
    !Array.isArray(workbench.rows) || workbench.rows.length!==9)return null;
 const lookup=new Map(workbench.rows.map(r=>[r.id,r]));
 if(lookup.size!==9)return null;
 const nativeCompleted=workbench.recoveryRequired===false &&
   build?.projectId===projectId &&
   build.phase==='completed' &&
   build.requiresReconciliation===false &&
   build.installedAppAccepted===false &&
   SHA.test(build.terminalReceiptSha256||'');
 const nativeCaseReport=nativeCompleted &&
   report?.projectId===projectId &&
   report.terminalReceiptSha256===build.terminalReceiptSha256 &&
   report.independentlyVerified===false &&
   report.taskAcceptanceAuthorized===false;
 const stopped=workbench.recoveryRequired===false &&
   debug?.projectId===projectId &&
   SHA.test(debug.terminalReceiptSha256||'') &&
   debug.installedAppAccepted===false;
 const rows=STEPS.map(step=>{
   const observed=lookup.get(step.row);
   if(!observed || observed.executionAuthorized!==false ||
      observed.taskAccepted!==false ||
      !['observed','review_required','unverified','blocked',
        'reconciliation_required'].includes(observed.state))return null;
   const observation=step.id==='tests'
     ? nativeCaseReport?'original_native_report_visible':'not_observed'
     : step.id==='build'
       ? nativeCompleted?'native_job_completed_not_release_accepted':
         observed.state
       : step.id==='debug'
         ? stopped?'native_debugger_witness_not_release_accepted':
           observed.state
       : observed.state;
   return Object.freeze({...step,observation,
     executionAuthorized:false,taskAccepted:false,
     installedAccepted:false});
 });
 if(rows.some(row=>row===null))return null;
 return Object.freeze({projectId,kind:'human_navigation_only',
   steps:Object.freeze(rows),workerRecoveryRequired:workbench.recoveryRequired,
   anyTaskAutomaticallyAccepted:false,executionAuthorized:false,
   installedAccepted:false});
}
export function checkedFin06Destination(journey,id,projectId) {
 if(!journey || journey.kind!=='human_navigation_only' ||
    journey.workerRecoveryRequired!==false ||
    journey.projectId!==projectId || journey.executionAuthorized!==false ||
    !PROJECT.test(projectId||'') || journey.steps.length!==STEPS.length)
   return null;
 const step=STEPS.find(s=>s.id===id);
 const row=journey.steps.find(s=>s.id===id);
 if(!step || !row || row.section!==step.section ||
    row.executionAuthorized!==false ||
    row.taskAccepted!==false)return null;
 return Object.freeze({projectId,section:step.section,
   executionAuthorized:false});
}
