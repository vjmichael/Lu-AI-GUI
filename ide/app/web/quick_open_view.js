// INT01-E: typed read-only filename / directory-page UI boundaries.
// Neither a Quick Open result nor a page cursor grants write permission.
import { verifiedRelativeSourcePath } from './editor_working_set.js';

const MAX_FILENAME_RESULTS=60;
const MAX_DIRECTORY_PAGE=120;
const MAX_VISITED=30000;
const validNonnegative=value=>Number.isSafeInteger(value)&&value>=0;
const validProject=value=>typeof value==='string'&&/^ide-[a-f0-9]{32}$/.test(value);
const validName=value=>typeof value==='string'&&value.length>0&&
  value.length<=255&&verifiedRelativeSourcePath(value)===value&&!value.includes('/');

export function checkedQuickOpenResult(raw,projectId,query) {
  const normalized=String(query||'').trim().toLowerCase();
  if (!validProject(projectId)||!normalized||
      raw?.schemaVersion!==1||raw.projectId!==projectId||
      raw.query!==normalized||raw.readOnly!==true||
      raw.sourceFilesRead!==false||raw.worktreeModified!==false||
      raw.processStarted!==false||!Array.isArray(raw.matches)||
      raw.matches.length>MAX_FILENAME_RESULTS||
      !validNonnegative(raw.entriesExamined)||raw.entriesExamined>MAX_VISITED||
      !validNonnegative(raw.directoriesExamined)||
      raw.directoriesExamined>3000||typeof raw.truncated!=='boolean')
    throw Error('INT01E_UNTRUSTED_QUICK_OPEN_RESULT');
  const paths=new Set();
  for(const match of raw.matches){
    if (verifiedRelativeSourcePath(match?.relativePath)!==match.relativePath||
        paths.has(match.relativePath)||!validNonnegative(match.size)||
        !Number.isInteger(match.score)||match.score<0||match.score>3)
      throw Error('INT01E_UNTRUSTED_QUICK_OPEN_PATH');
    paths.add(match.relativePath);
  }
  return raw;
}

export function checkedDirectoryPage(raw,projectId,relativeDir,after) {
  if (!validProject(projectId)||
      (relativeDir!==''&&verifiedRelativeSourcePath(relativeDir)!==relativeDir)||
      (after!==null&&!validName(after))||
      raw?.schemaVersion!==1||raw.projectId!==projectId||
      raw.relativeDir!==relativeDir||raw.readOnly!==true||
      !Array.isArray(raw.entries)||raw.entries.length>MAX_DIRECTORY_PAGE||
      typeof raw.truncated!=='boolean'||typeof raw.scanLimited!=='boolean'||
      !validNonnegative(raw.entriesExamined)||raw.entriesExamined>MAX_VISITED||
      (raw.nextAfter!==null&&!validName(raw.nextAfter))||
      (!raw.truncated&&raw.nextAfter!==null)||
      (raw.scanLimited&&raw.nextAfter!==null))
    throw Error('INT01E_UNTRUSTED_DIRECTORY_PAGE');
  let last=after||'';
  const seen=new Set();
  for(const entry of raw.entries){
    if (!validName(entry?.name)||entry.name<=last||
        seen.has(entry.name)||
        entry.relativePath!==(relativeDir?relativeDir+'/'+entry.name:entry.name)||
        !['directory','file','symlink','other'].includes(entry.kind)||
        !validNonnegative(entry.size))
      throw Error('INT01E_UNTRUSTED_DIRECTORY_ENTRY');
    last=entry.name;
    seen.add(entry.name);
  }
  if (raw.nextAfter!==null&&raw.nextAfter!==last)
    throw Error('INT01E_DIRECTORY_CURSOR_MISMATCH');
  return raw;
}
