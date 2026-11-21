import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=path=>readFileSync(new URL(path,import.meta.url),'utf8');
const chat=read('../web/main.jsx');
const settings=read('../web/native_windows.jsx');
const errors=read('../web/unified_tabs.jsx');
const host=read('../src-tauri/src/lib.rs');

test('Everyday chat shows plain model labels, progress and an unchanged load/eject flow',()=>{
  assert.match(chat,/className="modelMenuHeading">LOCAL MODELS/);
  assert.match(chat,/m\.projectorAttached \? 'Vision enabled' : 'Text model'/);
  assert.match(chat,/Getting your model ready…/);
  assert.doesNotMatch(chat,/LOCAL GGUF MODELS/);
  assert.doesNotMatch(chat,/Tensor loading % unavailable/);
  assert.match(chat,/onClick=\{loadModel\}/);
  assert.match(chat,/onClick=\{unloadModel\}/);
  assert.match(chat,/aria-label="Choose a model"/);
  assert.match(chat,/className="spinner" aria-hidden="true"/);
});

test('History labels and reply revisions are understandable without replacing immutable records',()=>{
  assert.match(chat,/<strong>Saved history<\/strong>/);
  assert.match(chat,/placeholder="Search saved messages"/);
  assert.match(chat,/<summary>Review saved facts and corrections<\/summary>/);
  assert.match(chat,/Original saved message · version 0/);
  assert.match(chat,/This changes only how the message appears\. Its original text and earlier approvals stay unchanged/);
  assert.match(chat,/<details><summary>Technical proof<\/summary><small>SHA-256/);
  assert.match(chat,/lu_m4_retract_claim/);
  assert.match(chat,/lu_m3_source_text_page/);
});

test('Original source records are accessible but collapsed by default',()=>{
  assert.match(chat,/<details className="readProposalCard" aria-label="Historical source evidence">/);
  assert.match(chat,/<summary>Original source records<\/summary>/);
  assert.doesNotMatch(chat,/<details className="readProposalCard" aria-label="Historical source evidence" open/);
  assert.match(chat,/lu_native_source_history/);
  assert.match(chat,/item\.sourceSha256/);
  assert.match(chat,/item\.observedSha256/);
});

test('Settings has one user-facing navigation with contextual options instead of a Basic/Advanced mode',()=>{
  assert.match(settings,/const SETTINGS_SECTIONS=\[/);
  for(const section of ["['general','General & appearance']","['models','Models']","['tools','Tools & permissions']","['research','Browser & research']","['memory','Memory']","['privacy','Privacy & Observe']"])
    assert.ok(settings.includes(section),section);
  assert.ok(settings.includes('SETTINGS_SECTIONS.map(([key,label])=>'));
  assert.doesNotMatch(settings,/showAdvanced|setShowAdvanced|nativeAdvancedToggle|Show advanced settings|Hide advanced settings|Advanced runtime/);
  assert.match(settings,/<summary>Model response options<\/summary>/);
  assert.match(settings,/<summary>How Lu runs local models<\/summary>/);
  assert.match(settings,/<summary>Project planning tools<\/summary>/);
  assert.match(settings,/lu_native_route_catalog/);
  assert.match(settings,/cap\.authorization/);
});

test('Model Library keeps genuine GGUF compatibility mechanics with friendly visible language',()=>{
  assert.match(settings,/>My models<\/button>/);
  assert.match(settings,/>Add model file…<\/button>/);
  assert.match(settings,/>Add vision support…<\/button>/);
  assert.match(settings,/Vision support attached/);
  assert.match(settings,/No local models added yet/);
  assert.match(settings,/invoke\('lu_import_model'\)/);
  assert.match(settings,/invoke\('lu_attach_projector'/);
  assert.match(settings,/invoke\('lu_load_model'/);
  assert.match(settings,/invoke\('lu_unload_model'/);
  assert.match(errors,/Check that its vision support is attached/);
});

test('Consent and source custody are not altered by presentation-only work',()=>{
  assert.match(settings,/invoke\('lu_dx_review_supported'\)/);
  assert.match(settings,/invoke\('lu_observe_'\+action,\{chatId\}\)/);
  assert.match(settings,/approval|permission|consent/i);
  assert.match(host,/fn lu_confirm_document_create\(/);
  assert.match(host,/fn lu_unload_model\(/);
});

test('Model discovery offers accessible download controls and opt-in technical records',()=>{
  assert.match(settings,/const \[showTechnicalDetails,setShowTechnicalDetails\]=useState\(false\)/);
  assert.match(settings,/aria-pressed=\{showTechnicalDetails\}/);
  assert.match(settings,/:'Technical details'/);
  assert.match(settings,/showTechnicalDetails&&<p>These are original host-owned transfer receipts/);
  assert.match(settings,/>Check interrupted download<\/button>/);
  assert.match(settings,/>Verify downloaded file<\/button>/);
  assert.match(settings,/Add verified model to library/);
  assert.match(settings,/Downloading never loads a model automatically/);
  assert.match(settings,/invoke\('lu_model_transfer_start'/);
  assert.match(settings,/invoke\('lu_model_transfer_resume'/);
  assert.match(settings,/invoke\('lu_model_transfer_verify'/);
  assert.match(settings,/invoke\('lu_model_transfer_cancel'/);
});

test('Collaborative troubleshooting is individually expanded and never background sharing',()=>{
  assert.match(settings,/<summary>Share a troubleshooting review<\/summary>/);
  assert.match(settings,/onToggle=\{event=>setTroubleshootingOpen\(event.currentTarget.open\)\}/);
  assert.match(settings,/troubleshootingOpen&&<LocalDiagnosticExchange active=\{active\}\/>/);
  assert.match(settings,/const \[supported,setSupported\]=useState\(false\)/);
  assert.match(settings,/const \[sharing,setSharing\]=useState\(false\)/);
  assert.match(settings,/invoke\('lu_dx_review_supported'\)/);
  assert.match(settings,/lu_dx_stop_review/);
});
