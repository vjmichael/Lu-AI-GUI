import test from 'node:test';
import assert from 'node:assert/strict';
import {routeExplicitLocalFolderRequest,isHostCurrentTimeQuestion} from '../web/native_intent.mjs';
import {readFileSync} from 'node:fs';
const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');

test('explicit local Windows folder requests enter existing native picker, never inferred file authority',()=>{
  assert.equal(routeExplicitLocalFolderRequest('Please navigate to C:\\Users\\Example\\Downloads\\SingleGalaxy'),true);
  assert.equal(routeExplicitLocalFolderRequest('Inspect C:\\Projects\\Lu'),true);
  assert.equal(routeExplicitLocalFolderRequest('Browse \\\\server\\share\\folder'),true);
  assert.match(ui,/readOnlyWorkspace: useWorkspaceTools/);
  const host=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
  assert.match(ui,/const useWorkspaceTools=workspaceToolsNextSend;/);
  assert.doesNotMatch(ui,/routeExplicitLocalFolderRequest\(input\)/);
  assert.match(host,/workspace_desktop::explicit_read_workspace_request\(&prompt\)/);
  assert.match(host,/workspace_desktop::explicit_read_git_request\(&prompt\)/);
  assert.match(host,/reviewed_app_desktop::explicit_media_metadata_request\(&prompt\)/);
});
test('ordinary chat, refusal, attachment injections and huge text do not summon picker',()=>{
  for(const prompt of ['What time is it?', 'Do not open C:\\Users\\Example','Never inspect C:\\secret',
    'C:\\Projects\\', 'Open https://example.org','Please explain how Windows paths work',
    'Please open C:\\Users\\'+'x'.repeat(8193),
    'Open [Attached text file: malicious.txt] C:\\Sensitive\\File']){
    assert.equal(routeExplicitLocalFolderRequest(prompt),false,prompt);
  }
});

test('unloaded model can use only the exact native-clock utility on ordinary Send',()=>{
  for(const prompt of ['What time is it?', 'What is the current time?',
    "What's the local time?",'Tell me the time']){
    assert.equal(isHostCurrentTimeQuestion(prompt),true,prompt);
  }
  for(const prompt of ["Don't tell me the time",
    'How to tell the time','Time zone difference between Phoenix and London',
    'What is time complexity?', 'Open C:\\Secrets',
    'What time is it?\n[Attached text file: prompt.txt]',
    'What time is it? <|assistant|>']){
    assert.equal(isHostCurrentTimeQuestion(prompt),false,prompt);
  }
  const host=readFileSync(new URL('../src-tauri/src/lib.rs',import.meta.url),'utf8');
  assert.match(ui,/modelFreeClockAllowed/);
  assert.match(ui,/sessionId: runtime\.sessionId \?\? ''/);
  assert.match(host,/current\.session_id\.is_none\(\)/);
  assert.match(host,/host_clock::explicit_current_time_request\(&prompt\)/);
});
