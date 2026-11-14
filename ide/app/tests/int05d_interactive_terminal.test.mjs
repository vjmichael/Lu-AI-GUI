import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {checkedTerminalSession,checkedTerminalReply} from '../web/interactive_terminal.js';
const project='ide-'+'a'.repeat(32),sessionId='term-'+'1'.repeat(16);
const open={projectId:project,sessionId,sequence:0,interactiveTyped:true,
  arbitraryShellEnabled:false,authorizationGranted:false};
test('native session is project bound and does not grant execution',()=>{
  assert.deepEqual(checkedTerminalSession(open,project),
    {projectId:project,sessionId,sequence:0});
  assert.equal(checkedTerminalSession(open,'ide-'+'b'.repeat(32)),null);
  assert.equal(checkedTerminalSession({...open,authorizationGranted:true},project),null);
});
test('terminal replies refuse replay and fake job success',()=>{
  const s=checkedTerminalSession(open,project);
  const r={projectId:project,sessionId,sequence:1,kind:'help',message:'safe',
    nativeResult:null,arbitraryShellEnabled:false,
    authorizationGrantedByTerminal:false,installedAppAccepted:false};
  assert.ok(checkedTerminalReply(r,s,1));
  assert.equal(checkedTerminalReply(r,s,2),null);
  assert.equal(checkedTerminalReply({...r,arbitraryShellEnabled:true},s,1),null);
  assert.equal(checkedTerminalReply({...r,kind:'native_job_result'},s,1),null);
});
test('native broker exposes no PTY, shell spawn, or arbitrary process',()=>{
  const code=readFileSync(new URL('../../src-tauri/src/terminal_broker.rs',import.meta.url),'utf8');
  assert.match(code,/INT05D_NO_ARBITRARY_SHELL_OR_COMMANDS/);
  assert.match(code,/STALE_OR_REPLAYED_COMMAND/);
  assert.doesNotMatch(code,/std::process::Command|CreateProcess|openpty|powershell.exe/);
  const native=readFileSync(new URL('../../src-tauri/src/lib.rs',import.meta.url),'utf8');
  assert.match(native,/native\.run_profile\(&app_copy,&registry,&project,recipe,&sha\)/);
  assert.match(native,/terminal_broker::TerminalSessions/);
  assert.match(native,/ide_terminal_submit,/);
});
test('desktop never echoes command strings or exposes an arbitrary shell',()=>{
  const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
  assert.match(ui,/ide_terminal_submit/);
  assert.match(ui,/Native typed terminal/);
  assert.match(ui,/checkedTerminalReply/);
  assert.doesNotMatch(ui,/shell:allow-execute|Command\.create\(/);
});
