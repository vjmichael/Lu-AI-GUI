// DEV-03.4: validate native read-only test-file discovery; no run admission.
const PROJECT=/^ide-[a-f0-9]{32}$/;
const FIXED=new Set(["tests","test","src","src/tests","src/test","__tests__","ide/host/tests"]);
const RECIPES=new Set(["rust_tests","lu_ide_host_tests"]);
const LANGUAGES=new Set(["rust","python","javascript","cpp"]);
const PATH=/^[^\\\x00-\x1f\x7f:]{1,1024}$/;
export function safeConventionalTestDiscovery(raw,projectId){
  if(!PROJECT.test(projectId)||raw?.projectId!==projectId||
      raw.schemaVersion!==1||raw.source!=="fixed_native_conventional_file_metadata"||
      raw.processStarted!==false||raw.workerGrantIssued!==false||
      raw.sourceContentsExported!==false||raw.discoveryIsExecutionAuthority!==false||
      raw.incomplete!==true||raw.directoriesChecked!==7||
      typeof raw.truncated!=="boolean"||!Array.isArray(raw.files)||
      raw.files.length>96||!Array.isArray(raw.scanNotes)||
      raw.scanNotes.length>7)throw Error("DEV034_DISCOVERY_DENIED");
  let previous="";
  const files=raw.files.map(entry=>{
    if(!entry||entry.runnable!==false||
       !LANGUAGES.has(entry.language)||
       !(entry.recipeId===null||RECIPES.has(entry.recipeId))||
       (entry.language==="rust")!==(entry.recipeId!==null)||
       typeof entry.relativePath!=="string"||!PATH.test(entry.relativePath)||
       entry.relativePath.split("/").some(x=>!x||x==="."||x==="..")||
       ![...FIXED].some(dir=>entry.relativePath.startsWith(dir+"/")&&
          entry.relativePath.slice(dir.length+1).indexOf("/")===-1)||
       entry.relativePath<=previous||
       !Number.isSafeInteger(entry.fileBytes)||entry.fileBytes<0)
       throw Error("DEV034_FILE_DENIED");
    previous=entry.relativePath;
    return Object.freeze({path:entry.relativePath,language:entry.language,
      recipe:entry.recipeId,bytes:entry.fileBytes,runnable:false});
  });
  if(!raw.scanNotes.every(note=>typeof note==="string"&&note.length<=120&&
      /^[a-z0-9_/:]+$/.test(note)))throw Error("DEV034_NOTE_DENIED");
  return Object.freeze({projectId,files:Object.freeze(files),
    incomplete:true,truncated:raw.truncated,notes:Object.freeze([...raw.scanNotes])});
}
