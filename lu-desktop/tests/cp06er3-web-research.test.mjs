import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {test} from 'node:test';
const read=(p)=>readFileSync(new URL(p,import.meta.url),'utf8');
const host=read('../src-tauri/src/lib.rs');
const adapter=read('../src-tauri/src/model_web_research_desktop.rs');
const core=read('../../src-tauri/lu-core/src/model_web_research_turn.rs');
const transport=read('../../src-tauri/lu-core/src/native_web_research.rs');
test('ordinary current human Send joins the existing turn before capability negotiation',()=>{
  const start=host.indexOf('let no_negotiated_selection=if origin==GenerationOrigin::Human');
  const route=host.indexOf('model_web_research_desktop::try_model_web_research(',start);
  assert.ok(start>=0&&route>start&&route<host.indexOf('capability_negotiation::try_read_turn(',start));
  assert.match(host.slice(start,route),/images\.is_empty\(\)&&linked\.is_empty\(\)&&manifest\.is_empty\(\)/);
  assert.match(adapter,/PersistedHumanTurn::current_chat/);
  assert.match(adapter,/verify_active_foreground/);
  assert.match(adapter,/deny_observation_network_egress/);
  assert.match(adapter,/p0_auth_requested_watermark/);
});
test('native consent precedes one-use registration and verifier-bound model dispatch',()=>{
  const review=adapter.indexOf('.blocking_show()');
  const decline=adapter.indexOf('if !approved',review);
  const register=adapter.indexOf('ApprovedWebTurn::new_after_review',decline);
  const run=adapter.indexOf('native.run(',register);
  assert.ok(review>=0&&decline>review&&register>decline&&run>register);
  assert.match(adapter,/pinned_transfer_client\(\)/);
  assert.match(core,/bind_current_network_input/);
  assert.match(core,/require_single_observed_tool_result/);
  assert.match(core,/&call\.input!=self\.input/);
  assert.match(core,/used\.swap\(true,Ordering::AcqRel\)/);
  assert.match(adapter,/complete_reviewed_if_not_revoked/);
});
test('both original transport lanes are cancellable and preserve source limitations',()=>{
  assert.match(transport,/fetcher\.fetch_cancellable/);
  assert.match(transport,/search_openalex_cancellable/);
  assert.match(transport,/page\.http_status!=200\|\|page\.redirect_target\.is_some\(\)/);
  assert.match(transport,/"source_is_untrusted":true/);
  assert.match(transport,/"leads_verified":false/);
  assert.match(transport,/leads_truncated/);
});

test('ordinary Chat projects one plain-language activity without exposing the model tool inventory',()=>{
  const ui=read('../web/main.jsx');
  assert.doesNotMatch(ui,/turnCapabilitiesByChat|setTurnCapabilitiesByChat|lu-native-turn-capabilities/);
  for(const name of ['workspace_desktop','reviewed_app_desktop']){
    const native=read(`../src-tauri/src/${name}.rs`);
    assert.doesNotMatch(native,/lu-native-turn-capabilities/);
    assert.match(native,/\.capabilities\(\)/);
    assert.match(native,/matches_live/);
  }
  const progress=ui.slice(ui.indexOf('{visibleJob && <div'),ui.indexOf('{!atBottom && <button'));
  assert.match(progress,/working on your request/);
  assert.match(progress,/reviewing the result/);
  assert.doesNotMatch(progress,/native tool|tool result|thinking with tools|Using approved tools/);
  assert.match(progress,/spinner/);
  assert.match(adapter,/title\("Allow web access\?"\)/);
  assert.match(adapter,/"Allow once"/);
  assert.match(adapter,/No web request was made/);
});
