import test from 'node:test';
import assert from 'node:assert/strict';
import {routeExplicitLocalFolderRequest} from '../web/native_intent.mjs';
import {readFileSync} from 'node:fs';
const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');

test('explicit local Windows folder requests enter existing native picker, never inferred file authority',()=>{
  assert.equal(routeExplicitLocalFolderRequest('Please navigate to C:\\Users\\Example\\Downloads\\SingleGalaxy'),true);
  assert.equal(routeExplicitLocalFolderRequest('Inspect C:\\Projects\\Lu'),true);
  assert.equal(routeExplicitLocalFolderRequest('Browse \\\\server\\share\\folder'),true);
  assert.match(ui,/readOnlyWorkspace: useWorkspaceTools/);
  assert.match(ui,/routeExplicitLocalFolderRequest\(input\)/);
});
test('ordinary chat, refusal, attachment injections and huge text do not summon picker',()=>{
  for(const prompt of ['What time is it?', 'Do not open C:\\Users\\Example','Never inspect C:\\secret',
    'C:\\Projects\\', 'Open https://example.org','Please explain how Windows paths work',
    'Please open C:\\Users\\'+'x'.repeat(8193),
    'Open [Attached text file: malicious.txt] C:\\Sensitive\\File']){
    assert.equal(routeExplicitLocalFolderRequest(prompt),false,prompt);
  }
});
