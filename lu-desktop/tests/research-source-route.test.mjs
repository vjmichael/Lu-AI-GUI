import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

const host=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
const research=readFileSync(new URL('../src-tauri/src/research_desktop.rs',import.meta.url),'utf8');
const ledger=readFileSync(new URL('../../src-tauri/lu-core/src/web_research_evidence.rs',import.meta.url),'utf8');
const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');

test('research pagination is bounded and every network page requires a human native confirmation',()=>{
  assert.match(host,/page:Option<u32>/);
  assert.match(host,/\(1\.\.=5\)\.contains\(&page\)/);
  assert.match(host,/Native research approval unavailable/);
  assert.match(host,/worker\.search_page\(&chat,&query,page\)/);
  assert.match(research,/search_openalex\(&fetcher,query,number\)/);
  assert.match(research,/next_page:page\.next_page\.filter\(/);
  assert.match(ui,/Previous discovery page/);
  assert.match(ui,/Next discovery page/);
  assert.match(ui,/researchResult\?\.chatId===chatId/);
});
test('Crossref metadata always comes from the discovered per-chat DOI and cannot mint a verified citation',()=>{
  assert.match(host,/async fn lu_research_crossref\(/);
  assert.match(host,/Lu — Authorize Crossref metadata check/);
  assert.match(host,/lu_research_crossref,/);
  assert.match(research,/ledger\.discovered_candidate\(source_id\)/);
  assert.match(research,/SourceStatus::Discovered/);
  assert.match(research,/Crossref DOI identity mismatch/);
  assert.match(research,/ledger\.corroborate\(source_id,&record\)/);
  assert.match(ledger,/pub fn discovered_candidate\(/);
  assert.match(research,/citation_verified:false/);
  assert.match(ui,/Check independent Crossref metadata/);
  assert.match(ui,/researchMetadataByChat\[chatId\+'\|'\+l\.sourceId\]/);
  assert.doesNotMatch(host,/fn lu_research_open_url\(/);
});
