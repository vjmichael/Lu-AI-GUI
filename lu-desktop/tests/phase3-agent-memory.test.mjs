import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=path=>readFileSync(new URL('../'+path,import.meta.url),'utf8');

test('Phase 3 uses original ANSF memory as bounded data for actual planning',()=>{
 const source=read('src-tauri/src/agent_memory_reasoning.rs');
 const app=read('src-tauri/src/lib.rs');
 assert.match(source,/m4_procedure_context/);
 assert.match(source,/m4_belief_context/);
 assert.match(source,/project_goal_events/);
 assert.match(source,/latest_authenticated_user_turn_id/);
 assert.match(source,/contradicted_human_review_required/);
 assert.match(source,/automaticEffectRetry/);
 assert.match(source,/modelMayOverrideHumanCorrection/);
 assert.match(app,/agent_memory_reasoning::for_planning/);
 assert.match(app,/plan_memory_context\.as_deref\(\)!=Some\(now\.as_str\(\)\)/);
 assert.match(app,/stage_completed_model_plan/);
});

test('Phase 3 guards reviewed worker with original memory and explicit authority',()=>{
 const app=read('src-tauri/src/reviewed_app_desktop.rs');
 const mem=read('src-tauri/src/agent_memory_reasoning.rs');
 assert.match(app,/agent_memory_reasoning::for_reviewed_execution/);
 assert.match(app,/current_step_clarifications_resolved/);
 assert.match(app,/reverify_child_before_execution/);
 assert.match(app,/link_verified_scheduled_worker_outcome/);
 assert.match(mem,/disputed_or_stale_beliefs/);
 assert.match(mem,/human_review_required/);
 assert.match(mem,/executionAuthority/);
 assert.doesNotMatch(mem,/TaskStore::resume|spawn\(|std::process::Command/);
});

test('Current preview and real Qwen3.8 are distinct from the deferred 0.6B benchmark',()=>{
 const wf=read('../.github/workflows/lu-repo01-full-source.yml');
 const build=read('../scripts/build-current-windows-nsis.ps1');
 const testScript=read('../scripts/run-qwen38-27b-local.ps1');
 assert.match(wf,/vars\.LU_ENABLE_OLD_0_6B_DIAGNOSTIC/);
 assert.match(wf,/LU_R00_PHASE3_NSIS_SHA/);
 assert.match(wf,/build-current-windows-nsis\.ps1/);
 assert.match(build,/e0f142d5ddc1461ce721644d92af33db23545105/);
 assert.match(build,/0\.2\.0-rc\.1/);
 assert.match(build,/NSIS_CURRENT_DEV_PREVIEW\.nsi/);
 assert.match(testScript,/BuildVulkan/);
 assert.match(testScript,/--mmproj/);
 assert.match(testScript,/--list-devices/);
 assert.match(testScript,/REAL_MODEL_RESPONSE_OBSERVED/);
 assert.match(testScript,/installedLuIntegration='NOT_TESTED_BY_STANDALONE_SERVER'/);
});
