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
  // Source comments may document the fixed Crossref endpoint. Network access
  // must remain in the separately authorized native Research host.
  assert.doesNotMatch(src,/reqwest::|std::process::Command|curl_binary|fetcher\\.fetch/);
  assert.match(src,/catalog_work_from_crossref\\(/);
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

test('P07 immutable DOI-checked Crossref catalog snapshots never silently rewrite metadata',()=>{
  const evidence=read('../../src-tauri/lu-core/src/web_research_evidence.rs');
  const host=read('../src-tauri/src/research_desktop.rs');
  assert.match(evidence,/CREATE TABLE IF NOT EXISTS research_citation_catalog_snapshots/);
  assert.match(evidence,/pub fn pin_citation_catalog\(/);
  assert.match(evidence,/INSERT OR IGNORE INTO research_citation_catalog_snapshots/);
  assert.match(evidence,/saved\.response_sha256!=hash\|\|saved\.response_json!=response/);
  assert.match(evidence,/digest\(&value\.response_json\)!=value\.response_sha256/);
  assert.match(evidence,/citation_catalog_source_ids\(/);
  assert.match(evidence,/p07_citation_catalog_snapshot_is_chat_scoped_immutable_and_hash_checked/);
  assert.match(host,/ledger\.pin_citation_catalog\(source_id,&page\.bytes\)/);
});
test('P07 bibliography previews all reviewed sources offline, exports with trusted OS picker',()=>{
  const host=read('../src-tauri/src/research_desktop.rs');
  const desktop=read('../src-tauri/src/lib.rs');
  const ui=read('../web/main.jsx');
  assert.match(host,/pub\(crate\) fn bibliography_preview\(/);
  assert.match(host,/citation_catalog_source_ids\(121\)/);
  assert.match(host,/if ids\.len\(\)>120/);
  assert.match(host,/ledger\.citation_catalog\(&id\)/);
  assert.match(host,/render_bibliography\(&ledger,&works,style\)/);
  assert.match(host,/crossref_response_sha256/);
  assert.match(desktop,/async fn lu_research_bibliography_preview\(/);
  assert.match(desktop,/async fn lu_research_bibliography_export\(/);
  assert.match(desktop,/lu_research_bibliography_preview,/);
  assert.match(desktop,/lu_research_bibliography_export,/);
  assert.match(desktop,/File::create_new\(&temp\)/);
  assert.match(desktop,/std::fs::hard_link\(&temp,&destination\)/);
  assert.match(desktop,/if reread\.catalog_manifest_sha256!=reviewed\.catalog_manifest_sha256/);
  assert.match(ui,/Compile reviewed bibliography offline/);
  assert.match(ui,/Save new bibliography Markdown/);
  assert.match(ui,/Save new bibliography CSL-JSON/);
  assert.match(ui,/researchBibliographiesByChat\[chatId\]/);
  assert.match(ui,/publication details remain unverified/i);
});

test('P07 SBL2 primary-source locator is explicit, bounded, offline and not a verified quote',()=>{
  const native=read('../src-tauri/src/lib.rs');
  const ui=read('../web/main.jsx');
  assert.match(src,/pub fn format_sbl_scripture_reference\(/);
  assert.match(src,/SBL2_CANONICAL_BIBLE_LOCATOR_ONLY_UNVERIFIED/);
  assert.match(src,/passage_verified:false,translation_verified:false/);
  assert.match(src,/bibliographic_entry_required:false/);
  assert.match(src,/sbl_scripture_locator_formats_only_user_supplied_book_verse_translation/);
  assert.match(src,/sbl_canonical_book_abbreviation/);
  assert.match(native,/fn lu_research_sbl_scripture_locator\(/);
  assert.match(native,/lu_research_sbl_scripture_locator,/);
  assert.match(ui,/invoke\('lu_research_sbl_scripture_locator'/);
  assert.match(ui,/SBL2 canonical Scripture locator/);
  assert.match(ui,/Copy Scripture reference/);
  assert.match(ui,/This is not a bibliography item or scholarly source attestation/);
});
test('P07 reviewed SBL2 secondary abbreviation map and Chicago 18 six-plus author rule remain bounded',()=>{
  assert.match(src,/pub fn sbl_known_journal_abbreviation\(/);
  assert.match(src,/Journal of Biblical Literature/);
  assert.match(src,/Catholic Biblical Quarterly/);
  assert.match(src,/sbl_small_reviewed_journal_map_never_uses_unreviewed_short_title/);
  assert.match(src,/p07_chicago18_many_authors_preserves_first_three_before_et_al/);
  assert.match(src,/Style::Chicago18 if xs\.len\(\)>6/);
  assert.match(src,/journal_abbreviation=journal\.as_deref\(\)/);
});

test('P07 native Research report optionally includes only sealed bibliography evidence',()=>{
  const desktop=read('../src-tauri/src/lib.rs');
  const host=read('../src-tauri/src/research_desktop.rs');
  const ui=read('../web/main.jsx');
  assert.match(desktop,/include_bibliography:Option<bool>,citation_style:Option<String>/);
  assert.match(desktop,/if include_bibliography\.unwrap_or\(false\)/);
  assert.match(desktop,/Some\(state\.research\.bibliography_preview\(&chat_id,profile\)\?\)/);
  assert.match(desktop,/lu_reviewed_bibliography/);
  assert.match(desktop,/bibliography_sources:bibliography\.as_ref\(\)/);
  assert.match(host,/pub bibliography_sources:usize/);
  assert.match(host,/p07_bibliography_reopens_sealed_crossref_catalog_offline_after_restart/);
  assert.match(ui,/Include reviewed P07 bibliography/);
  assert.match(ui,/includeBibliography:researchReportIncludeBibliography/);
  assert.match(ui,/citationStyle:researchCitationStyle/);
  assert.match(wf,/p07_bibliography_reopens_sealed_crossref_catalog_offline_after_restart/);
});

test('P07 catalog revision requires native side-by-side comparison with durable SHA-chain receipts',()=>{
  const ledger=read('../../src-tauri/lu-core/src/web_research_evidence.rs');
  const host=read('../src-tauri/src/research_desktop.rs');
  const commands=read('../src-tauri/src/lib.rs');
  const ui=read('../web/main.jsx');
  assert.match(ledger,/CREATE TABLE IF NOT EXISTS research_citation_catalog_revisions/);
  assert.match(ledger,/pub fn replace_citation_catalog_after_review\(/);
  assert.match(ledger,/previous\.response_sha256!=expected_sha256/);
  assert.match(ledger,/WHERE source_id=\?1 AND response_sha256=\?2/);
  assert.match(ledger,/pub fn citation_catalog_revision_receipts\(/);
  assert.match(ledger,/active\.response_sha256!=expected/);
  assert.match(ledger,/p07_catalog_re_review_preserves_history_denies_stale_receipts_and_recovers/);
  assert.match(host,/pub\(crate\) fn prepare_citation_revision\(/);
  assert.match(host,/pub\(crate\) fn apply_citation_revision\(/);
  assert.match(host,/let old=catalog_work_from_crossref/);
  assert.match(host,/let next=catalog_work_from_crossref/);
  assert.match(host,/replace_citation_catalog_after_review\(/);
  assert.match(commands,/async fn lu_research_citation_review_revision\(/);
  assert.match(commands,/lu_research_citation_review_revision,/);
  assert.match(commands,/Approve exact catalog revision/);
  assert.match(commands,/Reject and keep old/);
  assert.match(commands,/next_id\("research-citation-revision-"\)/);
  assert.match(ui,/invoke\('lu_research_citation_review_revision'/);
  assert.match(ui,/Review changed Crossref metadata/);
  assert.match(ui,/researchCitationRevisionByChat/);
});
