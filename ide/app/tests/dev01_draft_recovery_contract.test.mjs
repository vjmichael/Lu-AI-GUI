import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=path=>readFileSync(new URL(path,import.meta.url),'utf8');
const ui=read('../web/main.jsx');
const tauri=read('../src-tauri/src/lib.rs');
const vault=read('../../host/src/draft_recovery.rs');
test('unsaved drafts never use browser storage or project source paths',()=>{
  for(const text of [
    "invoke('ide_recovery_checkpoint'", "invoke('ide_recovery_list'",
    "invoke('ide_recovery_adopt'", "invoke('ide_recovery_forget'",
    "draftText: tab.draft", "baseRevision: tab.revision",
    "bufferRevision: tab.bufferRevision", "}, 700)", "recoveryBlocked.current",
  ]) assert.ok(ui.includes(text),text);
  for(const forbidden of ['localStorage','sessionStorage','IndexedDB','navigator.clipboard.writeText']) {
    assert.ok(!vault.includes(forbidden),forbidden);
  }
  assert.ok(tauri.includes('data.join("draft-recovery")'));
  assert.ok(tauri.includes('registry.check_recovery_project(&project_id)'));
});
test('previous-session record never silently replaced; native commits stay inside vault',()=>{
  for(const text of [
    'DraftRecoveryError::PreviousSession', 'MAX_RECORDS: usize = 64',
    'MAX_RECORD_BYTES', 'self.write_lock.lock()', 'previous.session_id != self.session_id',
    'self.dir.rename(&temp, &self.dir, file_name(',
    'record.buffer_revision == 0', 'revision_for(record.draft_text.as_bytes())',
    'record.draft_sha256 != expected_draft_sha256',
    'opts.mode(0o600)', 'fs::Permissions::from_mode(0o700)',
    'meta.file_type().is_symlink()'
  ]) assert.ok(vault.includes(text),text);
  for(const forbidden of ['std::process::Command','std::fs::write(', 'fs::rename(', 'remove_dir_all(']) {
    assert.ok(!vault.includes(forbidden),forbidden);
  }
});
test('recovery refuses changed saved revision and dirty-tab replacement',()=>{
  for(const text of [
    "saved.revision !== item.baseRevision",
    "checked.revision !== saved.revision",
    "tab.key === key && tab.dirty",
    "restored.draftSha256 !== item.draftSha256",
    "dirty: restored.draftText !== saved.text",
    "setHint('Recovered ' + item.relativePath",
    "Nothing was written to project source."
  ]) assert.ok(ui.includes(text),text);
  assert.ok(!ui.includes('window.localStorage'));
});
