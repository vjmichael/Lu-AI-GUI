// RESEARCH-05..10: structural security regression across native entrypoints.
// Actual Rust, Windows, Chromium, GGUF and installed-app probes remain separate.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const file=relative=>readFileSync(new URL(relative,import.meta.url),'utf8');
const http=file('../../src-tauri/lu-core/src/web_http_fetch.rs');
const document=file('../../src-tauri/lu-core/src/web_research_document.rs');
const evidence=file('../../src-tauri/lu-core/src/web_research_evidence.rs');
const analysis=file('../../src-tauri/lu-core/src/web_research_analysis.rs');
const report=file('../../src-tauri/lu-core/src/web_research_report.rs');
const desktop=file('../src-tauri/src/research_desktop.rs');
const host=file('../src-tauri/src/lib.rs');
const ui=file('../web/main.jsx');
const lib=file('../../src-tauri/lu-core/src/lib.rs');

test('RESEARCH-05 bounded nonexecuting PDF/HTML parser is invoked after fixed network retrieval',()=>{
  assert.match(document,/read_pdf_text\(/);
  assert.match(document,/MAX_RESEARCH_DOCUMENT_BYTES:usize=12\*1024\*1024/);
  assert.match(document,/token,"head"\|"script"\|"style"/);
  assert.match(document,/document_candidate:kind==ResearchDocumentKind::Pdf && !truncated/);
  assert.match(lib,/pub mod web_research_document;/);
  assert.match(desktop,/fetch_document_chain\(/);
  assert.match(desktop,/extract_research_document\(/);
  assert.match(desktop,/record_document_retrieval\(/);
  assert.match(evidence,/format=="PDF"&&!truncated&&final_url==pinned/);
  assert.match(host,/async fn lu_research_retrieve_document\(/);
  assert.match(host,/Retrieve bounded PDF\/HTML document/);
  assert.match(ui,/Retrieve bounded PDF\/HTML document/);
});

test('RESEARCH-06 redirect chains require independently pinned HTTPS public DNS for every hop',()=>{
  assert.match(http,/pub fn fetch_document_chain\(/);
  assert.match(http,/let page=self\.fetch_bounded\(&current,max_bytes\)/);
  assert.match(http,/let \(_,next\)=validated_host\(&target\)/);
  assert.match(http,/trusted_resolution\(/);
  assert.match(http,/--max-redirs","0"/);
  assert.match(http,/--resolve/);
  assert.match(http,/seen\.insert\(current\.clone\(\)\)/);
  assert.match(http,/chain_sha256/);
  assert.match(desktop,/publisher_url_matched:proof\.final_url==proof\.catalog_url/);
  assert.match(desktop,/document_accessed:proof\.document_accessed/);
});

test('RESEARCH-07 conflicting passages stay visible without a model fact verdict',()=>{
  assert.match(analysis,/pub fn summarize_claims\(/);
  assert.match(analysis,/self\.export_rechecked\(ledger\)\?/);
  assert.match(analysis,/unresolved_disagreement/);
  assert.match(analysis,/MODEL_INTERPRETATIONS_NOT_FACTS/);
  assert.match(host,/fn lu_research_add_claim\(/);
  assert.match(desktop,/ledger\.has_verified_passage\(source_id,quote\)/);
  assert.match(ui,/Save my evidence interpretation/);
  assert.match(ui,/Unresolved: contrary or qualifying evidence remains/);
});

test('RESEARCH-08 campaign is durable and bounded to 120 distinct DOI records',()=>{
  assert.match(evidence,/CREATE TABLE IF NOT EXISTS research_campaigns/);
  assert.match(evidence,/CREATE TABLE IF NOT EXISTS research_campaign_pages/);
  assert.match(evidence,/CREATE TABLE IF NOT EXISTS research_campaign_leads/);
  assert.match(evidence,/pub fn finish_campaign_page\(/);
  assert.match(evidence,/INSERT OR IGNORE INTO research_campaign_leads/);
  assert.match(evidence,/count>=state\.candidate_ceiling/);
  assert.match(evidence,/candidate_ceiling BETWEEN 1 AND 120/);
  assert.match(desktop,/pub\(crate\) fn run_campaign\(/);
  assert.match(desktop,/search_openalex\(&fetcher,&query,page\)/);
  assert.match(host,/async fn lu_research_campaign_start\(/);
  assert.match(host,/async fn lu_research_campaign_resume\(/);
  assert.match(ui,/Restore saved research jobs/);
  assert.match(ui,/Cancel campaign/);
});

test('RESEARCH-09 reports contain native evidence and do not overwrite a user file',()=>{
  assert.match(lib,/pub mod web_research_report;/);
  assert.match(report,/analysis\.export_verified_json\(ledger\)\?/);
  assert.match(report,/interpretations_independently_verified":false/);
  assert.match(report,/No eligible document-linked interpretations exist/);
  assert.match(host,/async fn lu_research_export_report\(/);
  assert.match(host,/\.blocking_save_file\(\)/);
  assert.match(host,/std::fs::File::create_new\(&temp\)/);
  assert.match(host,/std::fs::hard_link\(&temp,&destination\)/);
  assert.match(ui,/Save Markdown report/);
  assert.match(ui,/Save evidence JSON/);
});

test('RESEARCH-10 all new commands registered and chat-scoped; no model URL or path grant',()=>{
  for(const cmd of [
    'lu_research_retrieve_document','lu_research_verify_document_passage',
    'lu_research_campaign_start','lu_research_campaigns',
    'lu_research_campaign_resume','lu_research_campaign_cancel',
    'lu_research_add_claim','lu_research_claims','lu_research_export_report'
  ]) assert.match(host,new RegExp(cmd+','));
  assert.match(host,/state\.chats\.history\(&chat_id\)/);
  assert.match(desktop,/safe_id\(chat_id\)/);
  assert.match(desktop,/Pinned Research HTTPS client unavailable/);
  assert.doesNotMatch(host,/fn lu_research_fetch_arbitrary_url\(/);
  assert.doesNotMatch(host,/fn lu_research_export_to_unapproved_path\(/);
});
