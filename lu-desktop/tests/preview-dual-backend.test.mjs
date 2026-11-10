import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read = p => readFileSync(new URL('../'+p, import.meta.url),'utf8');

test('the current NSIS bundles two distinct source-built and SHA-pinned engines',()=>{
  const builder=read('../scripts/build-current-windows-nsis.ps1');
  const nsi=read('packaging/NSIS_CURRENT_DEV_PREVIEW.nsi');
  const host=read('src-tauri/src/lib.rs');
  const cargo=read('src-tauri/Cargo.toml');
  assert.match(cargo,/dual_win_preview = \["physical_win_cpu_eval"\]/);
  assert.match(builder,/--features physical_win_cpu_eval,dual_win_preview/);
  assert.match(builder,/-DGGML_VULKAN=OFF/);
  assert.match(builder,/'-DGGML_VULKAN=ON'/);
  assert.match(builder,/LU_PREVIEW_VULKAN_ENGINE_SHA256=\$vkHash/);
  assert.match(builder,/LU_PREVIEW_VULKAN_ENGINE_BYTES=\[string\]\$vkBytes/);
  assert.match(builder,/engineVariant='cpu-and-vulkan'/);
  assert.match(builder,/LU_VULKAN_ENGINE_EXE/);
  assert.match(nsi,/File \/oname=llama-server-vulkan\.exe/);
  assert.match(nsi,/Delete "\$INSTDIR\\lu-inference\\llama-server-vulkan\.exe"/);
  assert.match(host,/const VULKAN_ENGINE_SHA256/);
  assert.match(host,/check_file\(&engine_dir\.join\(name\), sha, bytes\)/);
  assert.match(host,/preview_engine_choice::read_choice/);
  assert.match(host,/selected_engine_sha/);
  assert.doesNotMatch(builder,/NOT_AN_ENGINE_CI_PLACEHOLDER/);
});

test('backend selection stays a human controlled native preference, not a model supplied path',()=>{
  const host=read('src-tauri/src/lib.rs');
  const choice=read('src-tauri/src/preview_engine_choice.rs');
  const ui=read('web/main.jsx');
  const settings=read('web/native_windows.jsx');
  assert.match(host,/async fn lu_model_select_backend/);
  assert.match(host,/MessageDialogButtons::OkCancelCustom/);
  assert.match(host,/lu_model_select_backend,/);
  assert.match(choice,/BackendChoice::from_id/);
  assert.match(choice,/Unknown engine mode/);
  assert.match(choice,/read_choice/);
  assert.match(choice,/write_choice/);
  assert.match(settings,/invoke\('lu_model_select_backend'/);
  assert.match(settings,/Use on next start/);
  assert.match(settings,/selectedOnNextStart/);
  assert.match(settings,/Saving a preference never grants GPU access/);
  assert.doesNotMatch(ui,/invoke\('lu_model_select_backend'/);
  assert.doesNotMatch(choice,/Command::new|std::process::Command|unsafe/);
});

test('both engines required in the actual installed lifecycle and Vulkan SDK build is pinned',()=>{
  const workflow=read('../.github/workflows/lu-repo01-full-source.yml');
  const cycle=read('../scripts/m3-windows-nsis-cycle.ps1');
  const manifest=read('scripts/verify_win_v6_manifest.py');
  assert.match(workflow,/choco install vulkan-sdk --version=1\.4\.350/);
  assert.match(workflow,/ExpectedVulkanSha256 \$f.vulkanEngineSha256/);
  assert.match(cycle,/installedVulkanEngine='PASS_SHA256'/);
  assert.match(cycle,/PASS_UNTOUCHED_APPDATA_SENTINEL/);
  assert.match(manifest,/parse_manifest_resource/);
  assert.match(manifest,/no Common Controls v6 SxS activation/);
  assert.match(manifest,/TaskDialogIndirect/);
  assert.match(manifest,/resource_bytes/);
});
