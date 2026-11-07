import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=p=>readFileSync(new URL(p,import.meta.url),'utf8');
const src=read('../../src-tauri/lu-core/src/web_research_styles.rs');
const rust=read('../../src-tauri/lu-core/src/lib.rs');
const wf=read('../../.github/workflows/lu-p07-research-citation-styles.yml');
test('P07 formatting uses native corroborated evidence identity and no model-controlled reference minting',()=>{
  assert.match(rust,/pub mod web_research_styles;/);
  assert.match(src,/ledger\.record\(&work\.source_id\)/);
  assert.match(src,/SourceStatus::MetadataCorroborated/);
  assert.match(src,/SourceStatus::DocumentAccessed/);
  assert.match(src,/source\.record_url\.as_deref\(\)!=Some\(work\.record_url\.as_str\(\)\)/);
  assert.match(src,/IdentityMismatch/);
  assert.match(src,/factual_claim_verified:false/);
  assert.match(src,/quotation_or_page_verified:false/);
  assert.match(src,/BASIC_BOOK_JOURNAL_ONLY_REVIEW_REQUIRED/);
  assert.doesNotMatch(src,/https?:\/\/api\.|reqwest::|curl|std::process::Command/);
});
test('P07 names five explicitly versioned citation styles while refusing unsupported SBL data',()=>{
  for(const label of ['APA 7','MLA 9','Chicago 18 Notes-Bibliography',
    'Turabian 9 Notes-Bibliography','SBL Handbook 2 Notes-Bibliography']){
    assert.ok(src.includes(label),label);
  }
  assert.match(src,/Kind::AncientSource/);
  assert.match(src,/Err\(StyleError::Unsupported\)/);
  assert.match(src,/field\(work\.journal_abbreviation\.as_ref\(\)\)/);
  assert.match(src,/sbl_does_not_invent_journal_abbreviations/);
  assert.match(src,/uncorroborated_mismatched_or_ancient_source_is_never_fabricated/);
});
test('P07 native Linux Windows tests retain exact source lock and evidence-verification gates',()=>{
  assert.match(wf,/ubuntu-latest, windows-latest/);
  assert.match(wf,/node scripts\/verify-source-tree\.mjs/);
  assert.match(wf,/web_research_styles::tests/);
  assert.match(wf,/web_research_evidence::tests/);
});

test('P07 CSL JSON has ledger-verified author identity but discloses incomplete secondary metadata',()=>{
  assert.match(src,/ledger\.discovered_candidate\(&work\.source_id\)/);
  assert.match(src,/original!=attested_one&&original!=attested_other/);
  assert.match(src,/pub fn export_csl_json\(/);
  assert.match(src,/author_identity_corroborated:true/);
  assert.match(src,/publication_details_verified:false/);
  assert.match(src,/quotation_or_page_verified:false/);
  assert.match(src,/csl_json_preserves_attested_identity_and_never_invents_locator/);
  assert.doesNotMatch(src,/"page":|page_locator:\s*Some/);
});

test('P07 existing evidence ledger rejects real newlines, not a literal backslash-n fixture',()=>{
  const src=read('../../src-tauri/lu-core/src/web_research_evidence.rs');
  assert.ok(src.includes('discovered_candidate("source-1\\nOTHER")'));
  assert.doesNotMatch(src,/discovered_candidate\("source-1\\\\nOTHER"\)/);
});

test('P07 bibliography preview is bounded, sorted, atomic and cannot duplicate corroborated sources',()=>{
  assert.match(src,/pub fn render_bibliography\(/);
  assert.match(src,/MAX_BIBLIOGRAPHY_ITEMS:usize=120/);
  assert.match(src,/if !seen\.insert\(&work\.source_id\)/);
  assert.match(src,/rendered\.push\(render_correlated\(ledger,work,style\)\?\)/);
  assert.match(src,/rendered\.sort_by/);
  assert.match(src,/bounded_bibliography_sorts_only_existing_correlated_sources/);
  assert.doesNotMatch(src,/factual_claim_verified:true/);
});

test('P07 Crossref metadata adapter refuses DOI/title/year/author substitution and SBL guesswork',()=>{
  assert.match(src,/pub fn catalog_work_from_crossref\(/);
  assert.match(src,/canonical_doi\(&candidate\.identifier\)/);
  assert.match(src,/if actual!=expected/);
  assert.match(src,/if years\.len\(\)!=1/);
  assert.match(src,/if !bounded\(title,1024\)\|\|title!=source\.title/);
  assert.match(src,/journal_abbreviation:None/);
  assert.match(src,/render_correlated\(ledger,&record,Style::Chicago18\)/);
  assert.match(src,/crossref_adapter_attests_doi_title_author_year_and_book_identity/);
  assert.match(src,/crossref_adapter_refuses_conflicting_or_untrusted_secondary_metadata/);
  assert.match(src,/crossref_journal_metadata_never_guesses_sbl_abbreviation/);
});
test('P07 only the user-approved native Research host can preview citations from pinned Crossref metadata',()=>{
  const host=read('../src-tauri/src/research_desktop.rs');
  const desktop=read('../src-tauri/src/lib.rs');
  const ui=read('../web/main.jsx');
  const tasks=read('../../src-tauri/lu-core/src/tasks.rs');
  assert.match(host,/pub\(crate\) fn citation_preview\(/);
  assert.match(host,/ledger\.discovered_candidate\(source_id\)/);
  assert.match(host,/encode_doi_component\(&doi\)/);
  assert.match(host,/fetcher\.fetch_bounded\(&url,1024\*1024\)/);
  assert.match(host,/page\.http_status!=200\|\|page\.redirect_target\.is_some\(\)/);
  assert.match(host,/catalog_work_from_crossref\(&ledger,source_id,&page\.bytes\)/);
  assert.match(host,/Sha256::digest\(&page\.bytes\)/);
  assert.match(desktop,/async fn lu_research_citation_preview\(/);
  assert.match(desktop,/lu_research_citation_preview,/);
  assert.match(desktop,/state\.observe\.deny_observation_network_egress\(&chat_id\)/);
  assert.match(desktop,/next_id\("research-citation-"\)/);
  assert.match(desktop,/request_owner\.observe\.deny_observation_network_egress\(&chat_id\)/);
  assert.match(tasks,/pub fn active_research_network_tasks/);
  assert.match(ui,/invoke\('lu_research_citation_preview'/);
  assert.match(ui,/Preview evidence-bound citation/);
  assert.match(ui,/Crossref-backed citation preview/);
  assert.match(ui,/REVIEW REQUIRED/);
});

test('P07 native Crossref citation preview includes provenance-preserving CSL JSON and completed task custody',()=>{
  const host=read('../src-tauri/src/research_desktop.rs');
  const desktop=read('../src-tauri/src/lib.rs');
  const ui=read('../web/main.jsx');
  assert.match(host,/pub csl_json:serde_json::Value/);
  assert.match(host,/let csl_json=export_csl_json\(&ledger,&record\)/);
  assert.match(host,/catalog_response_sha256:catalog_sha/);
  assert.match(desktop,/owner\.background_tasks\.complete\(&task_id\)/);
  assert.match(desktop,/owner\.background_tasks\.fail\(&task_id\)/);
  assert.match(ui,/Copy reviewed CSL JSON/);
  assert.match(ui,/publication particulars, quotations,/);
  assert.match(ui,/JSON\.stringify\(researchCitationsByChat/);
  assert.match(wf,/Compile actual P07 React and native Research\/Tauri host on Windows/);
  assert.match(wf,/cargo check --locked --manifest-path lu-desktop\/src-tauri\/Cargo.toml/);
});
