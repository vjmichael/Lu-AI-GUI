import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=name=>readFileSync(new URL(`../web/${name}`,import.meta.url),'utf8');
const settings=read('native_windows.jsx');
const chat=read('main.jsx');
const tasks=read('task_board.jsx');
const styles=read('styles.css');

test('Lu has one settings navigation and no second interface mode',()=>{
  assert.ok(settings.includes('SETTINGS_SECTIONS.map(([key,label])=>'));
  for(const source of [settings,chat,tasks,styles])
    assert.doesNotMatch(source,/showAdvanced|setShowAdvanced|nativeAdvancedToggle|advancedSettingsOpen|Show advanced settings|Hide advanced settings|Advanced project review/i);
  assert.doesNotMatch(settings,/\['runtime','Advanced runtime'\]|\['diagnostics','Diagnostics'\]/);
});

test('User options and project reviews are individually discoverable, with no global mode',()=>{
  assert.match(settings,/<summary>Model response options<\/summary>/);
  assert.match(settings,/<summary>Check image understanding<\/summary>/);
  assert.match(settings,/<summary>How Lu runs local models<\/summary>/);
  assert.match(settings,/<summary>Project planning tools<\/summary>/);
  assert.match(tasks,/<summary>Original source records<\/summary>/);
  assert.match(tasks,/<\/details>\s*<section className="plan07Refresh"/);
  assert.match(tasks,/<section className="plan07HumanReview"/);
  assert.match(settings,/Open Model Library/);
});

test('Sharing remains opt-in and protected by the existing native consent and cancellation flow',()=>{
  assert.match(settings,/<summary>Share a troubleshooting review<\/summary>/);
  assert.match(settings,/troubleshootingOpen&&<LocalDiagnosticExchange active=\{active\}\/>/);
  assert.match(settings,/invoke\('lu_dx_review_supported'\)/);
  assert.match(settings,/lu_dx_stop_review/);
  assert.match(settings,/lu_observe_forget_notes/);
  assert.doesNotMatch(chat,/Settings → Advanced|\(advanced\)/);
});
