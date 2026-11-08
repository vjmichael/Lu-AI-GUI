import assert from 'node:assert/strict';
import test from 'node:test';
import { readFileSync } from 'node:fs';
const ui = readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
const logic = readFileSync(new URL('../web/quick_open.js',import.meta.url),'utf8');

test('real editor Ctrl+P and visible quick-open affordance use native source APIs only',()=>{
  for (const marker of [
    "collectQuickOpenFiles, matchQuickOpenFiles",
    "if (key === 'p' && !event.shiftKey && !event.altKey)",
    "Quick open · Ctrl+P", 'role="dialog"', 'aria-modal="true"',
    'aria-label="Matching project files"', 'onClick={() => activateQuickOpen(path)}',
    "invoke('ide_list_source_directory'", "void selectEntry({kind: 'file', relativePath: path})",
    "activeProjectIdRef.current !== projectId",
    "quickOpenProjectId !== activeProjectIdRef.current",
    "setQuickOpenLimited(result.truncated)",
  ]) assert.ok(ui.includes(marker),marker);
  for(const banned of ['window.__TAURI_INTERNALS__','eval(','new Function(',
                         'document.execCommand(','navigator.clipboard.writeText(']){
    assert.ok(!logic.includes(banned),'quick-open must not use '+banned);
  }
});

test('native preview remains source/authority checked, not a synthetic open',()=>{
  const start=ui.indexOf('async function selectEntry(entry)');
  assert.ok(start>=0);
  const end=ui.indexOf('function closeTab(key)',start);
  const source=ui.slice(start,end);
  assert.ok(source.includes("invoke('ide_preview_source_file'"));
  assert.ok(source.includes("projectId: activeProject.projectId"));
  assert.ok(source.includes("relativePath: entry.relativePath"));
  assert.ok(!source.includes("setTabs(previous => [...previous, {"));
});
