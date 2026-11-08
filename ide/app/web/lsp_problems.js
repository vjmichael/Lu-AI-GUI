// DEV-02.1: display-only problems navigator for *verified* LSP diagnostics.
// No LSP worker execution, server URI navigation, filesystem access, or
// synthetic diagnostic sources are authorized by this module.
import { matchesSource, visibleDiagnostics } from './lsp_editor.js';

export const MAX_PROBLEM_TABS = 64;
export const MAX_PROJECT_PROBLEMS = 200;
export const MAX_PROBLEM_QUERY = 128;

export function projectProblems(tabs, nativeByTab, projectId, options = {}) {
  const { severity = 'all', query = '', limit = MAX_PROJECT_PROBLEMS } = options;
  if (!Array.isArray(tabs) || !nativeByTab || typeof nativeByTab !== 'object' ||
      typeof projectId !== 'string' || !projectId ||
      !['all', 'error', 'warning', 'info'].includes(severity) ||
      typeof query !== 'string' || query.length > MAX_PROBLEM_QUERY ||
      !Number.isInteger(limit) || limit < 1 || limit > MAX_PROJECT_PROBLEMS) {
    throw Error('LSP_PROBLEMS_INVALID_SCOPE');
  }
  const search = query.trim().toLocaleLowerCase();
  const sorted = tabs
    .filter(tab => tab && tab.projectId === projectId &&
      typeof tab.key === 'string' && tab.key.length <= 1200 &&
      typeof tab.relativePath === 'string' && tab.relativePath.length <= 1024)
    .sort((a, b) => a.relativePath.localeCompare(b.relativePath) ||
      a.key.localeCompare(b.key));
  const truncatedTabs = sorted.length > MAX_PROBLEM_TABS;
  const files = sorted.slice(0, MAX_PROBLEM_TABS);
  const items = [];
  let verifiedTabs = 0;
  let staleTabs = 0;
  let total = 0;
  let errors = 0;
  let warnings = 0;
  let infos = 0;
  for (const tab of files) {
    const result = nativeByTab[tab.key];
    if (!result) continue;
    if (!matchesSource(tab, result)) {
      staleTabs++;
      continue;
    }
    verifiedTabs++;
    // visibleDiagnostics also rejects invalid UTF-16 offsets, excessive
    // messages, cross-document results and source-revision mismatches.
    for (const diagnostic of visibleDiagnostics(tab, result)) {
      total++;
      const kind = diagnostic.severity === 1 ? 'error' :
        diagnostic.severity === 2 ? 'warning' : 'info';
      if (kind === 'error') errors++;
      else if (kind === 'warning') warnings++;
      else infos++;
      if (severity !== 'all' && severity !== kind) continue;
      if (search && !(
        tab.relativePath.toLocaleLowerCase().includes(search) ||
        diagnostic.message.toLocaleLowerCase().includes(search))) continue;
      if (items.length >= limit) continue;
      items.push({
        tabKey: tab.key, projectId: tab.projectId,
        relativePath: tab.relativePath, diskSha256: tab.revision,
        bufferRevision: tab.bufferRevision, generation: tab.generation,
        kind, line: diagnostic.line, severity: diagnostic.severity,
        from: diagnostic.from, to: diagnostic.to, message: diagnostic.message,
      });
    }
  }
  items.sort((a, b) => a.severity - b.severity ||
    a.relativePath.localeCompare(b.relativePath) ||
    a.line - b.line || a.from - b.from);
  return {
    items, total, errors, warnings, infos,
    verifiedTabs, staleTabs, searchedTabs: files.length,
    truncated: truncatedTabs || items.length < total &&
      severity === 'all' && search === '',
    // Filtering may deliberately hide many diagnostics; indicate when
    // the unfiltered verified count exceeds display capacity.
    capacityLimited: total > limit || truncatedTabs,
  };
}

export function stillCurrentProblem(tab, nativeResult, problem) {
  if (!problem || !tab || problem.tabKey !== tab.key ||
      problem.projectId !== tab.projectId ||
      problem.relativePath !== tab.relativePath ||
      problem.diskSha256 !== tab.revision ||
      problem.bufferRevision !== tab.bufferRevision ||
      problem.generation !== tab.generation ||
      !matchesSource(tab, nativeResult)) return false;
  return visibleDiagnostics(tab, nativeResult).some(item =>
    item.from === problem.from && item.to === problem.to &&
    item.line === problem.line && item.severity === problem.severity &&
    item.message === problem.message);
}
