import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=path=>readFileSync(new URL(path,import.meta.url),'utf8');
const ui=read('../web/main.jsx');
const style=read('../web/styles.css');

test('split renders two different original tabs; never clones a buffer',()=>{
 for(const token of [
  'const [splitEnabled, setSplitEnabled] = useState(false)',
  'const [secondaryTabKey, setSecondaryTabKey] = useState(null)',
  'tab.key === secondaryTabKey && tab.key !== activeTabKey',
  'const alternative = projectTabs.find(tab => tab.key !== activeTabKey)',
  'editorStates.current.get(activeTab.key)',
  'editorStates.current.get(secondaryTab.key)',
  'tabIdentity={secondaryTab.key +',
  'setSecondaryTabKey(event.target.value)',
  'role="region"',
  'aria-label="Secondary source editor pane"',
  'Save secondary',
  'onClick={() => void saveTab(secondaryTab)}',
 ]) assert.ok(ui.includes(token),token);
 assert.ok(style.includes('.sourceEditorPair.split'));
 assert.ok(style.includes('.sourceEditorSecondary'));
});
test('Ctrl+S saves focused tab and split resets when project closes',()=>{
 for(const token of [
  'setFocusedEditorKey(activeTab.key)',
  'setFocusedEditorKey(secondaryTab.key)',
  'projectTabs.find(tab => tab.key === focusedEditorKey) || activeTab',
  'if (target) void saveTab(target)',
  'setSplitEnabled(false)',
  'setSecondaryTabKey(null)',
  'if (secondaryTabKey === key) setSecondaryTabKey(null)'
 ]) assert.ok(ui.includes(token),token);
});
test('split UI cannot execute, save implicitly, or overwrite source on view switch',()=>{
 const start=ui.indexOf('// DEV-01.5: two different existing editor tabs');
 const end=ui.indexOf('useEffect(() => {\n    const shortcut',start);
 assert.ok(start>0 && end>start);
 const logic=ui.slice(start,end);
 for(const bad of ["invoke('ide_save_source_file'",'setTabs(','createEditorState(',
   'editorStates.current.set(','window.localStorage','window.__TAURI_INTERNALS__']) {
  assert.ok(!logic.includes(bad),bad);
 }
});
