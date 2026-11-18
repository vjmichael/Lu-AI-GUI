import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const native=name=>readFileSync(new URL(`../src-tauri/src/${name}.rs`,import.meta.url),'utf8');
test('supervised workspace, Reader, utility and reviewed routes use common progress and queue fences',()=>{
  for(const name of ['workspace_desktop','reader_desktop','core_utility_desktop','reviewed_app_desktop']){
    const src=native(name);
    assert.match(src,/NativeTurnProgress::new\(/,name);
    assert.match(src,/progress\.model_observer\(\)/,name);
    assert.match(src,/progress\.dispatch_observer\(\)/,name);
    assert.match(src,/enqueue_with_class_committing\([^;]*append_(?:reader_user|user_with_turn_id)\(/,name);
    assert.match(src,/ticket\.commit_completed\(&session_id,\|\|\{[\s\S]*?host\.chats\.complete(?:_reviewed_if_not_revoked)?\(/,name);
    assert.doesNotMatch(src,/struct (ReaderToolProgress|CoreUtilityProgress|ReviewedToolProgress|NativeToolProgress)\b/,name);
  }
  const browser=native('browser_model_desktop');
  assert.match(browser,/enqueue_with_class_committing\([^;]*append_user_with_turn_id\(/);
  assert.match(browser,/ticket\.commit_completed\(&session_id,\|\|\{/);
});
