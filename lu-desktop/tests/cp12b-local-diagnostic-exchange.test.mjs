import { readFileSync } from 'node:fs';
import test from 'node:test';
import assert from 'node:assert/strict';
const read=p=>readFileSync(new URL(p,import.meta.url),'utf8');
const host=read('../src-tauri/src/lib.rs'), ipc=read('../src-tauri/src/dx_ipc.rs'), ui=read('../web/native_windows.jsx'), arc=read('../src-tauri/src/diagnostic_archive.rs');
test('fresh explicit native approval on both ends, and platform fail-closed',()=>{
 const share=host.slice(host.indexOf('async fn lu_dx_share('),host.indexOf('async fn lu_dx_receive('));
 const receive=host.slice(host.indexOf('async fn lu_dx_receive('),host.indexOf('fn lu_dx_supported('));
 assert.ok(share.indexOf('.blocking_show()')<share.indexOf('diagnostic_exchange.start('));
 assert.ok(receive.indexOf('.blocking_show()')<receive.indexOf('dx_ipc::receive(&code)'));
 assert.match(share,/dx_ipc::supported\(\)/);assert.match(receive,/dx_ipc::supported\(\)/);assert.match(ipc,/all\(target_os = "windows", feature = "windows_dx_eval"\)/);
 assert.match(host,/state\.diagnostic_exchange\.shutdown\(\)/);
});
test('exchange is a fixed count-only snapshot without live archive or tool interface',()=>{
 const summary=ipc.slice(ipc.indexOf('struct Summary'),ipc.indexOf('impl Summary'));
 assert.equal((summary.match(/pub \w+: u32/g)||[]).length,5);
 assert.doesNotMatch(ipc,/ToolCall|Command::new\(.*shell|TcpListener|TcpStream/);
 assert.match(arc,/pub fn exchange_summary\(&self\)/);
 assert.match(ipc,/SO_PEERCRED/);assert.match(ipc,/ct_eq/);assert.match(ipc,/canonical/);
});
test('Settings invitation only on click, explicit copy and stop, masked input',()=>{
 const view=ui.slice(ui.indexOf('function LocalDiagnosticExchange'),ui.indexOf('export function SettingsWindow'));
 assert.match(view,/onClick=.*invoke\('lu_dx_share'\)/);
 assert.match(view,/onClick=\{\(\)=>void stopSharing\(\)\}/);assert.match(view,/await invoke\('lu_dx_stop_share'\)/);
 assert.match(view,/onClick=.*navigator\.clipboard\.writeText\(code\)/);
 assert.match(view,/type="password"/);assert.doesNotMatch(view,/localStorage|sessionStorage/);
});
test('Windows evaluation preserves explicit opt-in, private local kernel endpoints and bounded shared protocol',()=>{
 const win=read('../src-tauri/src/dx_windows_pipe.rs'), manifest=read('../src-tauri/Cargo.toml');
 assert.match(manifest,/windows_dx_eval\s*=\s*\[\]/);
 assert.match(ipc,/all\(target_os = "windows", feature = "windows_dx_eval"\)/);
 assert.match(ipc,/mod protocol/);assert.match(win,/protocol::\*/);
 assert.match(win,/FILE_FLAG_FIRST_PIPE_INSTANCE/);assert.match(win,/PIPE_REJECT_REMOTE_CLIENTS/);
 assert.match(win,/SE_DACL_PROTECTED/);assert.match(win,/size\.AceCount != 1/);
 assert.match(win,/verify_endpoint\(&h, expected\)\?/);assert.match(win,/ImpersonateNamedPipeClient/);
 assert.match(win,/SECURITY_IDENTIFICATION/);assert.match(win,/RevertToSelf/);
 assert.match(win,/auth_request_for\(&key/);assert.match(win,/response_full\(&key/);assert.match(win,/auth_response_full\(&key/);
 assert.doesNotMatch(win,/TcpListener|TcpStream|ToolCall/);
});
