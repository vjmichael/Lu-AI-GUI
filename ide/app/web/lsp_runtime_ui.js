// INT02-F: display-only, source-bound LSP availability and presentation.
// Never grants execution, opens a URI, writes a file, or trusts server HTML.
import { lspNativeReplyMatches, sourceMatchesSession } from './lsp_live_source.js';

const KNOWN_PROVIDERS = new Set(['rust-analyzer', 'clangd', 'ruff', 'pyright']);
const VALID_STATES = new Set(['idle', 'running', 'reconciliation_required', 'unavailable']);
const DEFAULT = Object.freeze({
  ready:false, code:'LSP_NATIVE_STATUS_UNVERIFIED',
  detail:'Native offline language-server state could not be verified.',
  supportedLanguages:[], semanticLanguages:[],
  installedProviders:[], sessionState:'unavailable',
});

export function checkedLspWorkerStatus(status) {
  if (!status || typeof status!=='object' ||
      typeof status.ready!=='boolean' ||
      typeof status.code!=='string' || status.code.length>100 ||
      typeof status.detail!=='string' || status.detail.length>512 ||
      !VALID_STATES.has(status.sessionState) ||
      !Array.isArray(status.supportedLanguages) ||
      !Array.isArray(status.semanticLanguages) ||
      !Array.isArray(status.installedProviders)) return DEFAULT;
  const languages=new Set(['rust','c','cpp','python']);
  if ([...status.supportedLanguages,...status.semanticLanguages]
      .some(id=>!languages.has(id)) ||
      status.installedProviders.some(id=>!KNOWN_PROVIDERS.has(id)) ||
      status.installedProviders.length>4 ||
      new Set(status.installedProviders).size!==status.installedProviders.length)
    return DEFAULT;
  const usable=status.ready &&
    (status.sessionState==='idle'||status.sessionState==='running');
  return Object.freeze({
    ready:usable,
    code:status.code,detail:status.detail,
    supportedLanguages:usable ? status.supportedLanguages.slice(0,4):[],
    semanticLanguages:usable ? status.semanticLanguages.slice(0,4):[],
    installedProviders:status.installedProviders.slice(0,4),
    sessionState:status.sessionState,
  });
}

export function invalidateNativeLspPresentation(state, session, busy) {
  return !!session && !busy && (
    state==='reconciliation_required' || state==='unavailable' ||
    state==='idle'
  );
}

// Native enumerates only currently attached, registered project sources.
// Match every reply to a *fresh* live CodeMirror capture and the exact OS
// approved provider before showing it; reject duplicates and bounded overflow.
export function checkedProjectDiagnosticReplies(replies, session, liveSourceForPath) {
  if (!Array.isArray(replies) || replies.length>16 || !session ||
      typeof liveSourceForPath!=='function' || session.cleanupRequired)
    return {};
  const next={};
  const visited=new Set();
  for (const reply of replies) {
    const path=reply?.source?.relativePath;
    if (typeof path!=='string' || path.length>1024 || visited.has(path))
      return {};
    visited.add(path);
    const source=liveSourceForPath(path);
    if (!source || !sourceMatchesSession(source,session) ||
        !lspNativeReplyMatches(source,reply)) continue;
    next[source.key]=reply;
  }
  return next;
}

export function accessibleDiagnosticLabel(path, diagnostic) {
  const kind=diagnostic?.severity===1 ? 'Error' :
    diagnostic?.severity===2 ? 'Warning' : 'Information';
  const message=typeof diagnostic?.message==='string'
    ? diagnostic.message.slice(0,240).replace(/\s+/g,' ') : '';
  const line=Number.isInteger(diagnostic?.line) && diagnostic.line>0
    ? diagnostic.line : 1;
  const file=typeof path==='string' ? path.slice(0,256) : 'current file';
  return kind+' in '+file+', line '+line+'. '+message;
}
