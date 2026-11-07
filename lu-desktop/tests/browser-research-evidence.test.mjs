import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

const file=relative=>readFileSync(new URL(relative,import.meta.url),'utf8');
const core=file('../../src-tauri/lu-core/src/web_research_evidence.rs');
const research=file('../src-tauri/src/research_desktop.rs');
const browser=file('../src-tauri/src/browser_model_desktop.rs');
const desktop=file('../src-tauri/src/lib.rs');
const ui=file('../web/main.jsx');

test('only native Browser READ/ACCESSIBILITY results can reach Research evidence',()=>{
  assert.match(browser,/registry\.dispatch\(invocation,policy\.as_ref\(\)\)/);
  assert.match(browser,/catalog\.decode_result\(/);
  assert.match(browser,/source_url=url\.as_deref\(\)/);
  assert.match(browser,/result_scope\.allows\(u\)/);
  assert.match(browser,/scope\.allows\(url\)/);
  assert.match(browser,/worker_truncated=body\.get\("truncated"\)/);
  assert.match(browser,/owner\.research\.record_browser_read\(/);
  assert.equal((browser.match(/owner\.research\.record_browser_read\(/g)||[]).length,2);
  assert.match(browser,/let host_call=next_id\("browser-call-"\)/);
  assert.doesNotMatch(browser,/&call\.call_id,None/);
  assert.match(browser,/grant_current\(/);
});

test('observed Browser snapshots are immutable and distinct from verified bibliography',()=>{
  assert.match(core,/CREATE TABLE IF NOT EXISTS research_browser_observations\(/);
  assert.match(core,/PRIMARY KEY\(generation_id,call_id\)/);
  assert.match(core,/INSERT OR IGNORE INTO research_browser_observations/);
  assert.match(core,/if stored!=expected\{return Err\(EvidenceError::Invalid\);\}/);
  assert.match(core,/pub fn recent_browser_observations\(/);
  assert.match(core,/pub fn has_browser_excerpt\(/);
  assert.match(core,/eligible_for_reviewed_bibliography/);
  assert.match(research,/verification_status:"browser_observed_untrusted"/);
  assert.match(research,/citation_verified:false/);
  assert.match(core,/SourceStatus::Discovered/);
  assert.doesNotMatch(core,/UPDATE research_sources SET status='PASSAGE_VERIFIED'.{0,200}research_browser_observations/s);
});

test('Research view has no network requirement and never crosses chats',()=>{
  assert.match(research,/pub\(crate\) fn browser_evidence\(/);
  assert.match(research,/if !safe_id\(chat_id\)/);
  assert.match(research,/recent_browser_observations\(24\)/);
  assert.match(research,/format!\("\{chat_id\}\.sqlite"\)/);
  assert.match(desktop,/fn lu_research_browser_evidence\(/);
  assert.match(desktop,/state\.chats\.history\(&chat_id\)/);
  assert.match(desktop,/lu_research_browser_evidence,/);
  assert.match(ui,/invoke\('lu_research_browser_evidence',\{chatId:selectedChat\}\)/);
  assert.match(ui,/browserEvidenceByChat\[chatId\]/);
  assert.match(ui,/Observed Browser evidence/);
  assert.match(ui,/citation verified: no/);
  assert.doesNotMatch(ui,/Review observed Browser evidence.{0,500}Check independent Crossref metadata/s);
});
