/** FIN-04: test filename and original-job reporter evidence belong to
 * different trust domains; this is an explicit read-only operator catalog. */
const PROJECT=/^ide-[a-f0-9]{32}$/;
const LANG=new Set(['rust','python','javascript','cpp']);
const STATUS=new Set(['reported_pass','reported_fail','reported_skip']);
const SAFE=/^[^\\:\x00-\x1f\x7f]{1,1024}$/u;
const JOB=/^job-[a-f0-9]{32}$/;
function safeRelativeSource(path) {
  return typeof path==='string' && SAFE.test(path) &&
    !path.startsWith('/') && !path.split('/').some(s=>!s||s==='.'||s==='..');
}
export function checkedTestExplorer(discovery,report,projectId,filter='all') {
  if(!PROJECT.test(projectId||'') ||
     !['all',...LANG].includes(filter))return null;
  const valid=discovery?.projectId===projectId &&
    discovery.incomplete===true && Array.isArray(discovery.files) &&
    discovery.files.length<=96;
  const files=[];
  if(valid){
    const seen=new Set();
    for(const row of discovery.files){
      if(!row || !safeRelativeSource(row.path) ||
        seen.has(row.path) || !LANG.has(row.language) ||
        row.runnable!==false ||
        !(row.recipe===null || typeof row.recipe==='string'))
        return null;
      seen.add(row.path);
      if(filter==='all' || filter===row.language)
        files.push(Object.freeze({path:row.path,language:row.language,
          recipe:row.recipe,executionAuthorized:false}));
    }
  }
  const cases=[];
  const seenCaseIds=new Set();
  if(report?.projectId===projectId && JOB.test(report.jobId||'') &&
     report.taskAcceptanceAuthorized===false &&
     report.independentlyVerified===false &&
     Array.isArray(report.cases) && report.cases.length<=128){
    for(const row of report.cases){
      if(!row || !STATUS.has(row.status) ||
         typeof row.id!=='string' ||
         !/^case_[a-f0-9]{64}$/.test(row.id) ||
         typeof row.name!=='string' || row.name.length>240 ||
         row.independentlyVerified!==false || seenCaseIds.has(row.id))return null;
      seenCaseIds.add(row.id);
      const source=row.source;
      const sourceValid=source && typeof source==='object' &&
        safeRelativeSource(source.relativePath) &&
        /^[a-f0-9]{64}$/.test(source.diskSha256||'') &&
        Number.isSafeInteger(source.line) &&
        source.line>=1 && source.line<=250000 &&
        source.characterUtf16===0;
      // Reporter hints are untrusted source-navigation metadata.
      if(source && !sourceValid)return null;
      cases.push(Object.freeze({id:row.id,name:row.name,status:row.status,
        canReviewSource:row.status==='reported_fail' && !!sourceValid,
        executionAuthorized:false,assertionVerified:false}));
    }
  }
  return Object.freeze({projectId,kind:'advisory_test_explorer',
    files:Object.freeze(files),cases:Object.freeze(cases),
    discoveryIncomplete:true,sourceMetadataOnly:true,
    currentNativeJobId:cases.length?report.jobId:null,
    singleCaseExecutionSupported:false,
    executionAuthorized:false,installedAccepted:false,
    independentlyVerified:false});
}
