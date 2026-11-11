// IDE-INT02-A: connect the actual CodeMirror text and revision to native LSP.
// No process launches, disk writes, authorization, network or source parsing.
import { captureLiveEditorIntent, stillCurrentEditorIntent } from './editor_live_write.js';
import { languageForPath, lspPositionOffset } from './lsp_editor.js';

export const MAX_LSP_TEXT_BYTES=512*1024;
const languageIds=new Set(['rust','c','cpp','python']);
const encodedByteLength=text=>new TextEncoder().encode(text).length;

export function captureLiveLspSource(tab,identity,editorState,projectId) {
  const live=captureLiveEditorIntent(tab,identity,editorState,projectId);
  if(live.status!=='current') return null;
  const languageId=languageForPath(live.relativePath);
  if(!languageIds.has(languageId) ||
      typeof live.text!=='string' || live.text.includes('\0') ||
      encodedByteLength(live.text)>MAX_LSP_TEXT_BYTES ||
      !Number.isSafeInteger(live.bufferRevision) || live.bufferRevision<0)
    return null;
  return Object.freeze({...live,languageId});
}

export function stillCurrentLspSource(source,tab,identity,editorState,projectId) {
  if(!source || !languageIds.has(source.languageId) ||
      !stillCurrentEditorIntent(source,tab,identity,editorState,projectId))
    return false;
  return languageForPath(source.relativePath)===source.languageId;
}

const providersByLanguage=Object.freeze({
  rust:['rust-analyzer'],c:['clangd'],cpp:['clangd'],
  python:['ruff','pyright'],
});
export function lspNativeReplyMatches(source,reply) {
  const observed=reply?.source;
  return !!source && !!observed &&
    providersByLanguage[source.languageId]?.includes(reply.providerId) &&
    observed.projectId===source.projectId &&
    observed.relativePath===source.relativePath &&
    observed.diskSha256===source.savedRevision &&
    observed.bufferRevision===source.bufferRevision &&
    observed.languageId===source.languageId;
}

export function lspRequestForSource(source,kind=null,position=null) {
  if(!source || !languageIds.has(source.languageId) ||
      !['completion','hover','definition','diagnostics',null].includes(kind))
    return null;
  if(kind==='completion'||kind==='hover'||kind==='definition'){
    if(lspPositionOffset(source.text,position)===null) return null;
  }
  return {
    projectId:source.projectId,relativePath:source.relativePath,
    expectedRevision:source.savedRevision,bufferRevision:source.bufferRevision,
    languageId:source.languageId,text:source.text,
    kind,position:kind==='completion'||kind==='hover'||kind==='definition'
      ? {line:position.line,character:position.character}:null,
  };
}

export function sourceMatchesSession(source,session) {
  return !!source && !!session &&
    session.projectId===source.projectId &&
    session.relativePath===source.relativePath &&
    session.languageId===source.languageId &&
    session.bufferId===source.bufferId &&
    session.savedRevision===source.savedRevision;
}
