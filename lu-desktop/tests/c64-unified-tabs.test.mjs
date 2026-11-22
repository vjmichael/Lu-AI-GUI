import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=p=>readFileSync(new URL(p,import.meta.url),'utf8');
const ui=read('../web/main.jsx'),tabs=read('../web/unified_tabs.jsx'),css=read('../web/styles.css');
const settings=read('../web/native_windows.jsx'),host=read('../src-tauri/src/lib.rs'),store=read('../src-tauri/src/ansf_chat_store.rs');
const smoke=read('../../scripts/p0-windows-installed-gui-smoke.ps1');
test('one persistent main chat and five tab sections',()=>{
 assert.match(tabs,/\['chat','Chat'\],\['history','History'\],\['tasks','Tasks'\],\['models','Models'\],\['settings','Settings'\]/);
 assert.match(ui,/hidden=\{activeTab!=='chat'\} className=\{'shell'/);
 for(const id of ['history','models','settings','tasks'])assert.ok(ui.includes("visitedTabs.includes('"+id+"')"));
 assert.match(css,/\.luTabShell \[hidden\]\{display:none!important\}/);
 assert.match(ui,/captureHistoryAnchor\(el\)/);
 assert.match(ui,/restoreHistoryAnchor\(el,stored.anchor\)/);
 assert.match(ui,/\[history, visibleStream, chatId,activeTab\]/);
 assert.doesNotMatch(ui,/Independent desktop<\/small>|className="m2MemoryPane"|aria-label="Draft plan with Lu"|<AnsfHistoryPane activeChatId=/);
 assert.ok(ui.indexOf('className="chatSearchRow"')<ui.indexOf('className="newChatButton"'));
});
test('history searches profile-owned current and other chats without permission grants',()=>{
 assert.match(tabs,/Search all conversations/);
 assert.doesNotMatch(tabs,/<select/);
 assert.match(tabs,/invoke\('lu_history_search'/);
 assert.match(tabs,/invoke\('lu_m3_source_text_page'/);
 assert.match(host,/fn lu_history_search\(/);
 assert.match(store,/pub fn search_history_page\(/);
 assert.match(store,/chats\.iter\(\)\.skip\(offset\)\.take\(12\)/);
 assert.match(store,/self\.require\(&chat\.chat_id\)/);
 assert.match(store,/human_history_search_includes_current_and_other_books_without_recall_grants/);
});
test('task controls keep the original scope and actual security checks',()=>{
 assert.match(ui,/invoke\('lu_background_cancel', \{taskId,chatId:scopeChatId\}\)/);
 assert.match(ui,/cancelBackgroundTask\(task\.taskId,task\.scopeChatId\)/);
 assert.match(ui,/invoke\('lu_model_transfer_recent'\)/);
 assert.match(ui,/Object\.entries\(jobs\)/);
 assert.match(host,/scheduler\.cancel_owned\(&task_id,&chat_id\)/);
 assert.match(host,/Read-only Browser grant/);
 assert.match(host,/owner\.observe\.deny_unscoped_network_while_observing\(\)\?/);
 assert.match(settings,/section==='tools'&&<Section title="Tools & permissions">/);
 assert.match(settings,/<summary>Project planning tools<\/summary>/);
});
test('installed oracle demands real same HWND and all five tabs',()=>{
 assert.match(smoke,/PASS_INSTALLED_UNIFIED_TABS/);
 for(const name of ['Settings','Models','History','Tasks','Chat'])assert.ok(smoke.includes("Wait-Element $window '"+name+"' $true"),name);
 assert.match(smoke,/PASS_ONE_INSTALLED_WINDOW/);
 assert.match(smoke,/MainWindowHandle/);
 assert.match(smoke,/UIAutomationClient/);
 // A visible screenshot alone never passes: require WebView2's real UIA
 // subtree through an OS-verified child HWND and an explicit accessibility mode.
 assert.match(smoke,/Get-InstalledUIAElements/);
 assert.match(smoke,/EnumChildWindows/);
 assert.match(smoke,/IsChild/);
 assert.match(smoke,/WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS/);
 assert.match(smoke,/--force-renderer-accessibility/);
 assert.match(smoke,/TryGetCurrentPattern/);
 assert.match(smoke,/Installed WebView2 did not expose UI Automation element/);
 assert.match(smoke,/CopyFromScreen/);
});
