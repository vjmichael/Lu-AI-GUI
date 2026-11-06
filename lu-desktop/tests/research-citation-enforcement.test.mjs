import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const file=path=>readFileSync(new URL(path,import.meta.url),'utf8');
const core=file('../../src-tauri/lu-core/src/web_research_evidence.rs');
const citation=file('../../src-tauri/lu-core/src/web_research_citation.rs');
const research=file('../src-tauri/src/research_desktop.rs');
const browser=file('../src-tauri/src/browser_model_desktop.rs');
const host=file('../src-tauri/src/lib.rs');
const ui=file('../web/main.jsx');
const lib=file('../../src-tauri/lu-core/src/lib.rs');

test('verified publication-linked passage needs independent exact Crossref URL and exact observed quote',()=>{
  assert.match(research,/fn crossref_publisher_url\(/);
  assert.match(research,/work\.get\("DOI"\)/);
  assert.match(research,/work\.get\("URL"\)/);
  assert.match(research,/ledger\.bind_crossref_publisher_url\(/);
  assert.match(core,/CREATE TABLE IF NOT EXISTS research_publisher_links\(/);
  assert.match(core,/pub fn bind_crossref_publisher_url\(/);
  assert.match(core,/pub fn prove_browser_passage\(/);
  assert.match(core,/url\.as_deref\(\)!=Some\(source\.url\.as_str\(\)\)/);
  assert.match(core,/source\.excerpt\.contains\(quotation\)/);
  assert.match(core,/publisher_link_matched:linked/);
  assert.match(core,/SourceStatus::MetadataCorroborated/);
  assert.match(research,/document_accessed:false/);
  assert.match(research,/citation_verified:false/);
});

test('citation audit denies model URLs, fake labels, unsupported quotations and uncited prose',()=>{
  assert.match(lib,/pub mod web_research_citation;/);
  assert.match(citation,/pub fn audit_browser_research_answer\(/);
  assert.match(citation,/pub fn render_observed_source_appendix\(/);
  assert.match(citation,/CitationAuditError::UnsupportedQuotation/);
  assert.match(citation,/CitationAuditError::UnknownCitation/);
  assert.match(citation,/CitationAuditError::MissingCitation/);
  assert.match(citation,/lower\.contains\("https:\/\/"\)/);
  assert.match(citation,/sources\[\*n-1\]\.excerpt\.contains\(passage\)/);
  assert.match(browser,/audit_browser_research_answer\(&answer,&citation_sources\)/);
  assert.match(browser,/let appendix=render_observed_source_appendix\(&citation_sources\)/);
  assert.match(browser,/trusted host adds source URLs/);
  assert.match(browser,/let mut evidence=BTreeMap::<String,\(String,BrowserCitationSource\)>::new\(\)/);
  assert.match(browser,/let mut source_records=Vec::<String>::new\(\)/);
});

test('unaudited model output never streams or persists as completed Browser research',()=>{
  const start=browser.indexOf('let mut answer=String::new()');
  const audit=browser.indexOf('audit_browser_research_answer(&answer,&citation_sources)');
  const release=browser.indexOf('app.emit("lu-token",ChatEvent{',start);
  const completed=browser.indexOf('owner.chats.complete(&chat_id,&gid)?',start);
  assert.ok(start>=0&&start<audit&&audit<release&&release<completed);
  const between=browser.slice(start,audit);
  assert.doesNotMatch(between,/app\.emit\("lu-token"/);
  assert.doesNotMatch(between,/owner\.chats\.append_progress\(&chat_id,&gid,&answer\)/);
  assert.match(browser,/Place an exact \[\[B1\]\], \[\[B2\]\]/);
});

test('local passage-check UI is chat-scoped and never grants network access',()=>{
  assert.match(host,/fn lu_research_verify_quote\(/);
  assert.match(host,/lu_research_verify_quote,/);
  assert.match(host,/state\.chats\.history\(&chat_id\)/);
  assert.match(research,/pub\(crate\) fn verify_browser_quote\(/);
  assert.match(research,/ledger\.prove_browser_passage\(/);
  assert.match(ui,/Check exact source passage/);
  assert.match(ui,/invoke\('lu_research_verify_quote'/);
  assert.match(ui,/researchQuoteProofByKey/);
  assert.match(ui,/Optional independently corroborated OpenAlex source ID/);
  assert.doesNotMatch(research,/fn verify_browser_quote[\s\S]{0,2200}fetcher\.fetch/);
});

test('stored bibliographic citation is rehashed on every verification and export',()=>{
  assert.match(core,/if digest\(original\.as_bytes\(\)\)!=pinned_sha/);
  assert.match(core,/sha==digest\(text\.as_bytes\(\)\) && text\.contains\(quote\)/);
  assert.match(core,/record\.content_sha256\.as_deref\(\)==Some\(sha\.as_str\(\)\)/);
  assert.match(core,/stored_document_tampering_revokes_passage_and_reviewed_bibliography/);
});
