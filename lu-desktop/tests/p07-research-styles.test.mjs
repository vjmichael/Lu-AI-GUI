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
