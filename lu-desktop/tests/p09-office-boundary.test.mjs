import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=path=>readFileSync(new URL(path,import.meta.url),'utf8');
const ui=read('../web/main.jsx');
const revision=read('../../src-tauri/lu-core/src/native_office_revision.rs');
const desktop=read('../src-tauri/src/office_revision_desktop.rs');
const batch=read('../src-tauri/src/office_batch_desktop.rs');
const tests=read('../../src-tauri/lu-core/tests/native_office_revision.rs');
const reader=read('../../src-tauri/lu-core/src/native_reader_office.rs');

test('P09 Office reviews disclose bounded fidelity and require separate human publication',()=>{
  assert.match(ui,/P09 Office fidelity boundary:/);
  assert.match(ui,/P09 fidelity: this changes one exact text run or simple typed cell/);
  assert.match(ui,/not ZIP container bytes or guaranteed/);
  assert.match(ui,/Formulas are not recalculated/);
  assert.match(ui,/dedicated Office application/);
  assert.match(desktop,/native modal/i);
  assert.match(batch,/Final native consent is one-use/);
});
test('P09 ordinary Office revisions never mutate originals or imply complex formatting fidelity',()=>{
  assert.match(revision,/NEW version \(never silently overwrite\)/);
  assert.match(revision,/compression headers\/timestamps are NOT byte-for-byte preserved/);
  assert.match(revision,/pub fn render_office_revision_batch\(/);
  assert.match(revision,/pub fn publish_office_revision_batch_new\(/);
  assert.match(revision,/if edits\.is_empty\(\)\|\|edits\.len\(\)>32/);
  assert.match(tests,/revised_word_text_retains_other_package_parts_and_original/);
  assert.match(tests,/revised_existing_numeric_cell_retains_workbook_sheets_and_text/);
  assert.match(tests,/simple_powerpoint_text_edit_preserves_untouched_slide/);
  assert.match(tests,/batch_word_changes_are_atomic_and_preserve_original_and_unmodified_parts/);
  assert.match(tests,/batch_rejects_ambiguous_cells_and_partial_change_without_publication/);
});
test('P09 rejects unsupported rich structures and reads cells/slides with bounded selectors',()=>{
  assert.match(revision,/UnsupportedPackage/);
  assert.match(revision,/UnsafePackage/);
  assert.match(tests,/package_edits_fail_on_changed_baseline_unsupported_cell_and_signed_parts/);
  assert.match(tests,/inline_xlsx_text_edit_preserves_worksheet_other_cells_and_rejects_rich_text/);
  assert.match(reader,/pub fn read_xlsx_range\(/);
  assert.match(reader,/pub fn read_pptx_slide\(/);
  assert.match(reader,/pub fn read_pptx_notes\(/);
});
