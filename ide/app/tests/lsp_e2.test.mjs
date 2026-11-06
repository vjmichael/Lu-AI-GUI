import test from 'node:test';
import assert from 'node:assert/strict';
import {
  languageForPath, lspPositionOffset, matchesSource, visibleDiagnostics,
  visibleCompletions, visibleHover, visibleDefinition, safePlainText,
} from '../web/lsp_editor.js';

function tab(overrides = {}) {
  return {
    projectId: 'ide-' + 'a'.repeat(32),
    relativePath: 'src/main.rs',
    revision: 'b'.repeat(64),
    bufferRevision: 3,
    draft: '😀 hello\nsymbol();\n',
    ...overrides,
  };
}
function result(t = tab(), rest = {}) {
  return {
    source: { projectId: t.projectId, relativePath: t.relativePath,
      diskSha256: t.revision, bufferRevision: t.bufferRevision,
      languageId: languageForPath(t.relativePath) },
    ...rest,
  };
}

test('E2 language mappings are descriptive and strictly bounded', () => {
  assert.equal(languageForPath('src/main.rs'), 'rust');
  assert.equal(languageForPath('source/file.tsx'), 'typescriptreact');
  assert.equal(languageForPath('source/file.jsx'), 'javascriptreact');
  assert.equal(languageForPath('app/data.jsonc'), 'jsonc');
  assert.equal(languageForPath('README.md'), null);
  assert.equal(languageForPath('x'.repeat(1025)), null);
});

test('E2 rejects stale disk and buffer revisions before presenting LSP data', () => {
  const t = tab();
  const r = result(t, { diagnostics: [{
    range: { start: {line:0,character:0}, end:{line:0,character:2} },
    severity: 1, message: 'Type error' }] });
  assert.equal(matchesSource(t,r), true);
  assert.deepEqual(visibleDiagnostics(t,r).map(d=>[d.from,d.to,d.line]), [[0,2,1]]);
  assert.equal(visibleDiagnostics(tab({bufferRevision:4}), r).length, 0);
  assert.equal(visibleDiagnostics(tab({revision: 'c'.repeat(64)}), r).length, 0);
  assert.equal(visibleDiagnostics(tab({relativePath:'src/other.rs'}), r).length, 0);
});

test('E2 interprets LSP UTF-16 positions without splitting surrogate pairs', () => {
  const text = '😀 hello\nsymbol();\n';
  assert.equal(lspPositionOffset(text,{line:0,character:0}),0);
  assert.equal(lspPositionOffset(text,{line:0,character:1}),null);
  assert.equal(lspPositionOffset(text,{line:0,character:2}),2);
  assert.equal(lspPositionOffset(text,{line:1,character:3}),12);
  assert.equal(lspPositionOffset(text,{line:3,character:0}),null);
});

test('E2 only accepts plain limited content; rejects remote URI and executable edits', () => {
  const t = tab();
  const visible = result(t, {
    hover:'**hello** <script>alert(1)</script>',
    completions:[
      {label:'hello',insertText:'hello()',detail:'function'},
      {label:'snippet',insertText:'x$1',insertTextFormat:2},
      {label:'command',insertText:'x',command:{command:'rm -rf /'}},
      {label:'edit',insertText:'x',textEdit:{newText:'surprise'}},
    ],
    definition:{relativePath:'src/main.rs',range:{
      start:{line:1,character:0},end:{line:1,character:6}
    }},
  });
  assert.equal(visibleCompletions(t,visible).length,1);
  assert.equal(visibleHover(t,visible),visible.hover);
  assert.deepEqual(visibleDefinition(t,visible),{from:9,to:15});
  assert.equal(visibleDefinition(t,{
    ...visible,definition:{...visible.definition,relativePath:'../outside.rs'},
  }),null);
  assert.equal(safePlainText('<img onerror=alert(1)>'),'<img onerror=alert(1)>');
});
