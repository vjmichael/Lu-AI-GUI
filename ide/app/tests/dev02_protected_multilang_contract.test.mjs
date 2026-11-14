import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {
  languageServiceCapabilities, languageMethodAvailable,
} from '../web/lsp_language_capabilities.js';
const read = path => readFileSync(new URL(path, import.meta.url), 'utf8');
const ui=read('../web/main.jsx');
const tauri=read('../src-tauri/src/lib.rs');
const backend=read('../src-tauri/src/lsp_backend.rs');
const cargo=read('../src-tauri/Cargo.toml');
const linux=read('../../supervisor/src/linux_lsp_worker.rs');
const windows=read('../../supervisor/src/windows_lsp_worker.rs');

test('Rust, Python and C++ native provider capabilities are explicit and not overpromised',()=>{
  for(const language of ['rust','c','cpp']) {
    const capabilities=languageServiceCapabilities(language);
    assert.ok(capabilities);
    for(const kind of ['diagnostics','completion','hover','definition'])
      assert.equal(languageMethodAvailable(language,kind),true,language+' '+kind);
  }
  const python=languageServiceCapabilities('python');
  assert.equal(python.provider,'Ruff');
  assert.equal(languageMethodAvailable('python','diagnostics'),true);
  for(const kind of ['completion','hover','definition'])
    assert.equal(languageMethodAvailable('python',kind),false,kind);
  assert.match(python.status,/reviewed Pyright-compatible provider/);
  for(const unknown of ['javascript','typescript','',null,'__proto__']) {
    assert.equal(languageMethodAvailable(unknown,'diagnostics'),false);
    assert.equal(languageServiceCapabilities(unknown),null);
  }
  assert.equal(languageMethodAvailable('rust','execute'),false);
});

test('Linux and Windows use same reviewed native LSP permission boundary, not PATH programs',()=>{
  assert.ok(cargo.includes('"lu-ide-execution-supervisor/linux-worker"'));
  assert.ok(cargo.includes('default = []'));
  assert.ok(tauri.includes('all(any(windows, target_os = "linux"), feature = "native-lsp")'));
  assert.ok(backend.includes('linux_worker::lsp_native::{'));
  assert.ok(backend.includes('windows_worker::lsp_native::{'));
  for(const name of ['start_linux_lsp(', 'start_windows_lsp(',
    'native_lsp_bundle_digest(&bundle, recipe)',
    'record_human_approval(', 'validated_registered_text(',
    'publish_lsp_input(', 'LSP_FILE_CHANGED_ON_DISK',
    'LSP_SOURCE_SCOPE_MISMATCH', 'LSP_METHOD_DENIED',
    'LSP_HUMAN_APPROVAL_DENIED']) assert.ok(backend.includes(name),name);
  for(const name of ['ensure_root_owned_tool(Path::new(BWRAP))',
    'consume_grant_after_attestation', 'cgroup.procs', '--unshare-all'])
    assert.ok(linux.includes(name),name);
  for(const name of ['AppContainer','JobPlan','start_windows_lsp']) {
    assert.ok(windows.includes(name) || backend.includes(name),name);
  }
  for(const forbidden of ['std::process::Command::new(', 'Path::new("clangd")',
     'trustedProjectFromBrowser', 'eval(']) {
    assert.ok(!backend.includes(forbidden),forbidden);
  }
});

test('real UI distinguishes Ruff diagnostic-only and validates native source after every request',()=>{
  for(const marker of [
    "import { languageServiceCapabilities, languageMethodAvailable }",
    "languageMethodAvailable(currentLanguageId, kind)",
    "currentLanguageId === 'python'",
    'Ruff · diagnostics only',
    "invoke('ide_lsp_start_session'",
    "invoke('ide_lsp_change_document'",
    "invoke('ide_lsp_read_diagnostics'",
    "kind === 'completion'",
    "kind === 'hover'",
    "kind === 'definition'",
    'matchesSource(tab,response)',
    'visibleDefinition(tab,response)',
    'tabIdentity: tab.key +',
  ]) assert.ok(ui.includes(marker),marker);
  assert.ok(!cargo.includes('default = ["native-lsp"]'));
  assert.ok(!backend.includes('Command::new('));
});
