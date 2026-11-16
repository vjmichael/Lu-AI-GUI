import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';
const source = file => readFileSync(new URL(file, import.meta.url), 'utf8');
const diagnostic = source('../web/frontend_startup_diagnostic.js');

function browser() {
  const listeners = new Map();
  const root = {
    children: [],
    mounted: false,
    querySelector(selector) { return selector === '.ideShell' && this.mounted ? {} : null; },
    replaceChildren(...nodes) { this.children = nodes; },
  };
  const window = {
    addEventListener(name, cb) { listeners.set(name, cb); },
  };
  const document = {
    getElementById(id) { return id === 'root' ? root : null; },
    createElement() {
      return {
        attributes: new Map(),
        setAttribute(k, v) { this.attributes.set(k, v); },
        textContent: '',
      };
    },
  };
  vm.runInNewContext(diagnostic, { window, document }, { timeout: 1000 });
  return { window, root, listeners };
}

test('frontend startup failure reports only fixed cosmetic category and cannot forge the workspace', () => {
  const { window, root, listeners } = browser();
  assert.equal(root.children.length, 0);
  listeners.get('error')({
    target: { tagName: 'SCRIPT', getAttribute: () => './assets/app.js' },
  });
  assert.equal(root.children.length, 1);
  assert.equal(root.children[0].attributes.get('role'), 'alert');
  assert.equal(root.children[0].attributes.get('aria-label'), 'Lu IDE frontend unavailable');
  assert.equal(root.children[0].attributes.get('data-ide-startup-diagnostic'),
    'module_asset_unavailable');
  assert.doesNotMatch(root.children[0].textContent, /Workspace projects/);
  assert.equal(root.children[0].textContent,
    'Lu IDE could not initialize its interface. Restart the application.');
  root.children = [];
  root.mounted = true;
  listeners.get('unhandledrejection')({});
  assert.equal(root.children.length, 0, 'a healthy mounted UI cannot be replaced by a late background error');
  window.__luIdeReportStartupFailure('untrusted-source-failure', true);
  assert.equal(root.children.length, 0, 'unrecognized external failure claims are refused');
});

test('actual Windows and Linux installed checks still require the original OS-visible workspace heading', () => {
  const windows = source('../fixtures/Test-Wo8NativeDesktopUI.ps1');
  const linux = source('../fixtures/wo8_linux_editor_atspi.py');
  assert.match(windows, /Find-Control 'Workspace projects'/);
  assert.match(windows, /Deny 'installed_webview_accessibility_not_ready_after_native_focus'/);
  assert.match(windows, /WO8_WINDOWS_FRONTEND_DIAGNOSTIC=visible_render_failure/);
  assert.match(linux, /accessible\("Workspace projects", seconds=35\)/);
  assert.match(linux, /deny\("missing_native_accessible_control_" \+ name.replace/);
  assert.match(linux, /WO8_LINUX_FRONTEND_DIAGNOSTIC=/);
  assert.doesNotMatch(diagnostic, /Workspace projects|__TAURI|invoke\(|fetch\(|XMLHttpRequest|localStorage|eval\(/);
});

test('Tauri CSP, reviewed bundle and real root mount are preserved, not replaced by diagnostic', () => {
  const html = source('../web/index.html');
  const build = source('../scripts/build.mjs');
  const main = source('../web/main.jsx');
  const config = source('../src-tauri/tauri.conf.json');
  assert.ok(html.indexOf('frontend_startup_diagnostic.js') < html.indexOf('assets/app.js'));
  assert.match(build, /copyFileSync\(join\(root, 'web', 'frontend_startup_diagnostic.js'\)/);
  assert.match(main, /createRoot\(document.getElementById\('root'\), \{/);
  assert.match(main, /\.render\(<App \/>\)/);
  assert.match(config, /default-src 'self'/);
  assert.match(config, /connect-src ipc: http:\/\/ipc\.localhost/);
  assert.doesNotMatch(config, /unsafe-eval|unsafe-inline/);
});
