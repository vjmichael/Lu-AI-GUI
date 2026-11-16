import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import vm from 'node:vm';
import { build } from 'esbuild';

const mainPath = fileURLToPath(new URL('../web/main.jsx', import.meta.url));
// Windows checkouts can use CRLF; only normalize line endings for source inspection.
// The complete original React entry and empty-workspace assertions stay required.
const source = readFileSync(mainPath, 'utf8').replace(/\r\n/g, '\n');

const providerExpression = source.match(
  /const lspProviderForTab = ([\s\S]+?);\s*const lspMayAddCurrentFile/
);
const auditBody = source.match(
  /const diskAuditCounts = useMemo\(\(\)=>\{([\s\S]+?)\n  \},\[diskAuditReport,activeProject\?\.projectId\]\);/
);

function provider(session, tab, language, capabilities, serves = () => true) {
  assert.ok(providerExpression, 'the real provider expression must remain directly testable');
  return vm.runInNewContext('(' + providerExpression[1] + ')', {
    lspSession: session, activeTab: tab, currentLanguageId: language,
    lspCapabilities: capabilities, providerServesLanguage: serves,
  }, { timeout: 1000 });
}
function audit(report, project) {
  assert.ok(auditBody, 'the real saved-source audit must remain directly testable');
  return vm.runInNewContext('(()=>{' + auditBody[1] + '\n})()', {
    diskAuditReport: report, activeProject: project,
  }, { timeout: 1000 });
}

test('empty initial workspace never dereferences a null LSP session', () => {
  assert.equal(provider(null, null, null, null), 'unavailable language provider');
  assert.equal(provider(null, {projectId:'p'}, 'rust', {provider:'rust-analyzer'}),
    'rust-analyzer');
  assert.equal(provider({projectId:'p',providerId:'clangd'}, null, 'cpp', {provider:'clangd'}),
    'clangd');
  assert.equal(provider({projectId:'foreign',providerId:'clangd'},
    {projectId:'trusted'}, 'cpp', {provider:'no cross-project session'}),
    'no cross-project session');
  assert.equal(provider({projectId:'p',providerId:'clangd'},
    {projectId:'p'}, 'cpp', {provider:'fallback'}, () => false), 'fallback');
  assert.equal(provider({projectId:'p',providerId:'clangd'},
    {projectId:'p'}, 'cpp', {provider:'fallback'},
    (candidate,language) => candidate === 'clangd' && language === 'cpp'), 'clangd');
});

test('empty initial workspace audit is zero and does not leak another project', () => {
  const zero = {changed:0,current:0,unavailable:0,stale:0,unsaved:0};
  assert.deepEqual({...audit(null,null)}, zero);
  assert.deepEqual({...audit(null,{projectId:'p'})}, zero);
  assert.deepEqual({...audit({projectId:'p',rows:[{status:'changed'}]},null)}, zero);
  assert.deepEqual({...audit({projectId:'foreign',rows:[{status:'changed'}]},
    {projectId:'trusted'})}, zero);
  assert.deepEqual({...audit({projectId:'p',rows:[
    {status:'changed',liveUnsaved:true},
    {status:'current'},
    {status:'unavailable'},
    {status:'stale'},
  ]},{projectId:'p'})}, {changed:1,current:1,unavailable:1,stale:1,unsaved:1});
});

test('real esbuild production entry renders first project view with no native project/session', async () => {
  const mount = `createRoot(document.getElementById('root'), {
  onUncaughtError: () => window.__luIdeReportStartupFailure?.('react_render_failed', true),
}).render(<App />);`;
  assert.equal(source.split(mount).length-1, 1, 'real React entry must not change unexpectedly');
  const injected = source.replace(mount,
    'globalThis.__luIdeSmokeApp = App; globalThis.__luIdeSmokeReact = React;');
  const compiled = await build({
    stdin: {contents: injected, resolveDir: dirname(mainPath),
      sourcefile: 'main.jsx', loader: 'jsx'},
    bundle: true, platform: 'browser', format: 'iife', target: 'es2020',
    jsx: 'automatic', minify: true, write: false, outfile: 'lu-ide-probe.js',
    logLevel: 'silent',
  });
  assert.equal(compiled.outputFiles.length, 1);
  const document = {
    createElement: () => ({
      style: {}, classList: {add() {}, remove() {}},
      setAttribute() {}, getAttribute() { return null; },
      appendChild() {}, removeChild() {},
    }),
    head: {appendChild() {}},
    documentElement: {style: {}},
  };
  const sandbox = {
    console, setTimeout, clearTimeout, setInterval, clearInterval,
    queueMicrotask, performance, URL, TextEncoder, TextDecoder,
    navigator: {platform:'Linux', userAgent:'Lu IDE local smoke'},
    document, window: {},
  };
  sandbox.globalThis = sandbox;
  vm.runInNewContext(compiled.outputFiles[0].text, sandbox,
    {timeout: 5000, filename: 'real-lu-ide-production-entry.js'});
  assert.equal(typeof sandbox.__luIdeSmokeApp,'function');
  const hooks = {
    useState: value => [typeof value === 'function' ? value() : value, () => {}],
    useRef: value => ({current: value}),
    useMemo: calculate => calculate(),
    useCallback: value => value,
    useEffect() {}, useLayoutEffect() {}, useImperativeHandle() {},
    useContext: () => ({}),
    useReducer: (_reducer,value) => [value, () => {}],
    useId: () => 'id',
    useDeferredValue: value => value,
    useSyncExternalStore: (_subscribe,getSnapshot) => getSnapshot(),
    useTransition: () => [false, () => {}],
  };
  sandbox.__luIdeSmokeReact
    .__CLIENT_INTERNALS_DO_NOT_USE_OR_WARN_USERS_THEY_CANNOT_UPGRADE.H = hooks;
  const tree = sandbox.__luIdeSmokeApp();
  assert.equal(tree.type, 'div');
  assert.equal(tree.props.className, 'ideShell');
  const strings = [];
  const visit = value => {
    if (value === null || value === undefined || value === false) return;
    if (typeof value === 'string') strings.push(value);
    else if (Array.isArray(value)) value.forEach(visit);
    else if (value?.props) visit(value.props.children);
  };
  visit(tree);
  assert.ok(strings.includes('Workspace projects'),
    'the real, empty-workspace React first render must expose the original heading');
  assert.ok(!strings.includes('Lu IDE frontend unavailable'),
    'the failure diagnostic must never substitute for the workspace');
});

// The actual installed GTK AT-SPI/WebView2 UIA drivers search for this
// stable contenteditable name. Pane identity must augment, not replace it.
test('mounted CodeMirror retains native editor accessibility identity', () => {
  const editor = readFileSync(fileURLToPath(new URL('../web/code_editor.jsx', import.meta.url)), 'utf8');
  assert.match(editor, /'aria-label': 'Source editor text'/);
  assert.ok(editor.includes("view.contentDOM.setAttribute('aria-label','Source editor text · '+label);"));
  assert.ok(!editor.includes("view.contentDOM.setAttribute('aria-label',label);"),
    'mount may not erase the OS-visible source editor name');
  assert.match(editor, /role: 'textbox'/);
});
