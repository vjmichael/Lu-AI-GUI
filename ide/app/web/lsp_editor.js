// Wave E2 presentation boundary for native-confirmed LSP replies.
// Server-provided Markdown/HTML and commands NEVER reach innerHTML or eval.
// A returned URI is data, not a file capability; the Rust host must resolve
// and authorize cross-file targets before returning a view location.
export const MAX_UI_DIAGNOSTICS = 200;
export const MAX_UI_COMPLETIONS = 100;
export const MAX_UI_DETAIL = 4096;

export function languageForPath(path = '') {
  if (typeof path !== 'string' || path.length > 1024) return null;
  const ext = path.toLowerCase().split('.').pop();
  if (ext === 'rs') return 'rust';
  if (ext === 'c' || ext === 'h') return 'c';
  if (['cpp', 'cc', 'cxx', 'hpp', 'hxx'].includes(ext)) return 'cpp';
  if (ext === 'py' || ext === 'pyi') return 'python';
  if (['js', 'jsx', 'mjs', 'cjs'].includes(ext)) return ext === 'jsx' ? 'javascriptreact' : 'javascript';
  if (ext === 'ts' || ext === 'tsx') return ext === 'tsx' ? 'typescriptreact' : 'typescript';
  if (ext === 'json' || ext === 'jsonc') return ext;
  if (['html', 'htm'].includes(ext)) return 'html';
  if (['css', 'scss', 'less'].includes(ext)) return ext;
  return null;
}

export function matchesSource(tab, result) {
  const source = result?.source;
  return !!tab && !!source && source.projectId === tab.projectId &&
    source.relativePath === tab.relativePath &&
    source.diskSha256 === tab.revision &&
    Number.isSafeInteger(source.bufferRevision) &&
    source.bufferRevision === tab.bufferRevision &&
    source.languageId === languageForPath(tab.relativePath);
}

// LSP positions are UTF-16 units, matching CodeMirror offsets. Reject
// surrogate halves instead of moving the caret into an invalid position.
export function lspPositionOffset(text, position) {
  if (typeof text !== 'string' || text.length > 512 * 1024 ||
    !Number.isSafeInteger(position?.line) || !Number.isSafeInteger(position?.character) ||
    position.line < 0 || position.character < 0 || position.line > 250000 ||
    position.character > 250000) return null;
  let from = 0;
  let lineNo = 0;
  while (lineNo < position.line) {
    const next = text.indexOf('\n', from);
    if (next < 0) return null;
    from = next + 1;
    ++lineNo;
  }
  const end = text.indexOf('\n', from);
  const raw = text.slice(from, end < 0 ? text.length : end);
  const line = raw.endsWith('\r') ? raw.slice(0, -1) : raw;
  if (position.character > line.length) return null;
  if (position.character > 0 && position.character < line.length) {
    const last = line.charCodeAt(position.character - 1);
    const next = line.charCodeAt(position.character);
    if (last >= 0xd800 && last <= 0xdbff && next >= 0xdc00 && next <= 0xdfff) {
      return null;
    }
  }
  return from + position.character;
}

export function visibleDiagnostics(tab, nativeResult) {
  if (!matchesSource(tab, nativeResult) || !Array.isArray(nativeResult.diagnostics) ||
      nativeResult.diagnostics.length > MAX_UI_DIAGNOSTICS) return [];
  const output = [];
  for (const item of nativeResult.diagnostics) {
    const start = lspPositionOffset(tab.draft, item?.range?.start);
    const end = lspPositionOffset(tab.draft, item?.range?.end);
    if (start === null || end === null || start > end ||
        typeof item.message !== 'string' || item.message.length > MAX_UI_DETAIL) continue;
    output.push({ from: start, to: end, line: item.range.start.line + 1,
      severity: [1, 2, 3, 4].includes(item.severity) ? item.severity : 3,
      message: item.message });
  }
  return output;
}

export function visibleCompletions(tab, nativeResult) {
  if (!matchesSource(tab, nativeResult) || !Array.isArray(nativeResult.completions) ||
      nativeResult.completions.length > MAX_UI_COMPLETIONS) return [];
  return nativeResult.completions.filter(item =>
    typeof item.label === 'string' && item.label.length > 0 && item.label.length <= 256 &&
    typeof item.insertText === 'string' && item.insertText.length <= 512 &&
    !item.insertText.includes('\0') && !item.label.includes('\n') &&
    item.insertTextFormat !== 2 && !item.textEdit && !item.command
  ).map(item => ({ label: item.label,
    detail: typeof item.detail === 'string' ? item.detail.slice(0, 256) : '',
    insertText: item.insertText }));
}

export function visibleHover(tab, nativeResult) {
  if (!matchesSource(tab, nativeResult)) return '';
  return typeof nativeResult.hover === 'string' && nativeResult.hover.length <= MAX_UI_DETAIL
    ? nativeResult.hover : '';
}

export function visibleDefinition(tab, nativeResult) {
  if (!matchesSource(tab, nativeResult) ||
      nativeResult.definition?.relativePath !== tab.relativePath) return null;
  const from = lspPositionOffset(tab.draft, nativeResult.definition?.range?.start);
  const to = lspPositionOffset(tab.draft, nativeResult.definition?.range?.end);
  if (from === null || to === null || from > to) return null;
  return { from, to };
}

export function safePlainText(value, max = MAX_UI_DETAIL) {
  return typeof value === 'string' ? value.slice(0, max).replaceAll('\0', '\uFFFD') : '';
}
