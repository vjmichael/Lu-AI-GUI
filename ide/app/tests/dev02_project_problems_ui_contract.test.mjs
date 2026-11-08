import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
const model=readFileSync(new URL('../web/lsp_problems.js',import.meta.url),'utf8');
const backend=readFileSync(new URL('../src-tauri/src/lsp_backend.rs',import.meta.url),'utf8');

test('real editor integrates read-only Problems with accessible severity/query and project reset',()=>{
  for(const text of [
    "import { projectProblems, stillCurrentProblem } from './lsp_problems.js'",
    'const [projectProblemsOpen, setProjectProblemsOpen] = useState(false)',
    "const [projectProblemSeverity, setProjectProblemSeverity] = useState('all')",
    "const [projectProblemSearch, setProjectProblemSearch] = useState('')",
    'projectProblems(projectTabs, lspNativeDiagnostics',
    'stillCurrentProblem(tab, lspNativeDiagnostics[tab.key], item)',
    "setProjectProblemsOpen(false)",
    'aria-label="Open-project verified language-server problems"',
    'aria-controls="projectProblemsPanel"',
    'aria-expanded={projectProblemsOpen}',
    'onClick={() => navigateProjectProblem(item)}',
    'onChange={event => setProjectProblemSeverity(event.target.value)}',
    'onChange={event => setProjectProblemSearch(event.target.value)}',
    'No verified language-server diagnostics yet',
    'stale responses hidden',
    'Partial results: open tab or problem display limit reached',
  ]) assert.ok(ui.includes(text),text);
});

test('cross-tab jump stays inside existing project tab, matches saved/buffer revision and never grants URI authority',()=>{
  const begin=ui.indexOf('function navigateProjectProblem(item)');
  const end=ui.indexOf('const anyDirty = ',begin);
  assert.ok(begin>0 && end>begin);
  const navigation=ui.slice(begin,end);
  for(const marker of [
    'candidate.projectId === activeProjectIdRef.current',
    'setActiveTabKey(tab.key)',
    "setSection('explorer')",
    'tabIdentity: tab.key +',
    'nonce: ++problemJumpNonce.current'
  ]) assert.ok(navigation.includes(marker),marker);
  for(const forbidden of ['invoke(', 'window.open(', 'openUrl(', 'eval(',
    'document.execCommand(', 'location.href', 'window.__TAURI_INTERNALS__']) {
    assert.ok(!navigation.includes(forbidden),forbidden);
  }
  assert.ok(model.includes('matchesSource(tab, result)'));
  assert.ok(model.includes('visibleDiagnostics(tab, result)'));
  assert.ok(model.includes('MAX_PROBLEM_TABS = 64'));
  assert.ok(model.includes('MAX_PROJECT_PROBLEMS = 200'));
  assert.ok(!model.includes('innerHTML'));
  assert.ok(backend.includes('feature = "native-lsp"'));
});

test('Problems display is not a hidden trigger for LSP session creation',()=>{
  const begin=ui.indexOf('aria-label="Open-project verified language-server problems"');
  const end=ui.indexOf('aria-label="Language server problems"',begin);
  assert.ok(begin>0&&end>begin);
  const section=ui.slice(begin,end);
  for(const disallowed of ["startLspSession(", "invoke('ide_lsp_", "requestLspFeature(",
    'new Worker(', 'dangerouslySetInnerHTML']) {
    assert.ok(!section.includes(disallowed),disallowed);
  }
});
