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
  return Object.freeze({...live,languageId,generation:tab.generation});
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

// One approved, single-project process may own a bounded set of live
// CodeMirror buffers. Provider ownership is fixed when the native OS grants
// the sealed workspace; this never authorizes a new provider or project.
export function providerServesLanguage(provider, languageId) {
  return (provider==='rust-analyzer' && languageId==='rust') ||
    (provider==='clangd' && (languageId==='c'||languageId==='cpp')) ||
    ((provider==='pyright'||provider==='ruff') && languageId==='python');
}
export function sessionMemberForSource(source) {
  if (!source || typeof source.key!=='string') return null;
  return Object.freeze({
    key:source.key, projectId:source.projectId,
    relativePath:source.relativePath,languageId:source.languageId,
    bufferId:source.bufferId,savedRevision:source.savedRevision,
    generation:source.generation,
  });
}
export function sourceMatchesSession(source,session) {
  if (!source || !session || session.cleanupRequired ||
      source.projectId!==session.projectId ||
      !providerServesLanguage(session.providerId,source.languageId)) return false;
  const member=session.members?.[source.key];
  return !!member && member.projectId===source.projectId &&
    member.relativePath===source.relativePath &&
    member.languageId===source.languageId &&
    member.bufferId===source.bufferId &&
    member.savedRevision===source.savedRevision &&
    member.generation===source.generation;
}
export function sessionCloseRequest(member) {
  if (!member || typeof member.projectId!=='string' ||
      typeof member.relativePath!=='string') return null;
  return {
    projectId:member.projectId,relativePath:member.relativePath,
    expectedRevision:member.savedRevision,bufferRevision:0,
    languageId:member.languageId,text:'',kind:null,position:null,
  };
}
