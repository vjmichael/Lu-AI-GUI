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
 assert.match(share,/cfg!\(target_os="linux"\)/);assert.match(receive,/cfg!\(target_os="linux"\)/);
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
 assert.match(view,/onClick=.*invoke\('lu_dx_stop_share'\)/);
 assert.match(view,/onClick=.*navigator\.clipboard\.writeText\(code\)/);
 assert.match(view,/type="password"/);assert.doesNotMatch(view,/localStorage|sessionStorage/);
});
