import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const source=p=>readFileSync(new URL(p,import.meta.url),'utf8');
const shared=source('../src-tauri/src/global_native_custody.rs');
const build=source('../src-tauri/src/build_backend.rs');
const debug=source('../src-tauri/src/debug_backend.rs');
const backend=source('../src-tauri/src/lib.rs');

test('FIN01 build and debugger share the exact profile-wide cross-process admission fence',()=>{
 assert.match(shared,/OpenOptions::new\(\)\.create_new\(true\)/);
 assert.match(shared,/fin01-global-native-custody-v1\.lock/);
 assert.match(shared,/fn sibling_rows/);
 assert.match(shared,/debug-fixture-receipts\.jsonl/);
 assert.match(shared,/build-job-receipts\.jsonl/);
 assert.match(build,/GlobalAdmissionFence\s*::acquire\(&self\.global_custody_profile,"build"\)/);
 assert.match(debug,/GlobalAdmissionFence\s*::acquire\(&self\.global_custody_profile,"debug"\)/);
 assert.match(backend,/mod global_native_custody;/);
});
test('FIN01 all terminal receipts required before global fence release',()=>{
 assert.match(shared,/pub fn release_if_terminal/);
 assert.match(shared,/durable_busy\(own\) \|\| durable_busy\(other\)/);
 assert.match(shared,/FIN01_GLOBAL_CUSTODY_NOT_RECONCILED/);
 assert.match(build,/global_fence\.release_if_terminal\(&own,&sibling\)/);
 assert.match(debug,/global_fence\.release_if_terminal\(&own,&sibling\)/);
 assert.doesNotMatch(shared,/impl Drop for GlobalAdmissionFence/);
});
test('FIN01 installed UI readiness observes interrupted fence and other supervisor',()=>{
 for(const s of [build,debug]){
  assert.match(s,/global_custody_unresolved\(&self\.global_custody_profile\)/);
  assert.match(s,/sibling_rows\(/);
  assert.match(s,/other_custody\.recovery_required/);
 }
 assert.match(build,/global_worker_busy = global_worker_busy \|\|/);
 assert.match(debug,/!other_custody\.worker_busy_or_unreconciled/);
});
