// Source-only safety contract. NOT Windows compiler, installer or real OS proof.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const get=(rel)=>readFileSync(new URL(rel,import.meta.url),'utf8');
const nsis=get('../packaging/NSIS_AGENT01G_PHYSICAL_WINDOWS_CPU_TEST.nsi');
const build=get('../../scripts/build-agent01g-physical-windows-nsis.ps1');
const workflow=get('../../.github/workflows/lu-agent01g-physical-windows-nsis.yml');
const cargo=get('../src-tauri/Cargo.toml');
const rust=get('../src-tauri/src/lib.rs');
const readme=get('../packaging/README_AGENT01G_PHYSICAL_WINDOWS_CPU_TEST.txt');

test('the current source produces a NEW isolated NSIS test package, never R22/alpha',()=>{
  assert.match(nsis,/Name "Lu AGENT-01G Physical Windows CPU Test \(NOT ALPHA\)"/);
  assert.match(nsis,/InstallDir "\$LOCALAPPDATA\\Programs\\LuAGENT01GPhysicalTest"/);
  assert.match(nsis,/RequestExecutionLevel user/);
  assert.match(nsis,/SetCompressor lzma/);
  assert.match(build,/git rev-parse HEAD/);
  assert.match(build,/node scripts\/verify-source-tree\.mjs/);
  assert.match(build,/cargo rustc --release --locked/);
});
test('test engine is compiled from exact pinned upstream, never CI fake resources',()=>{
  assert.match(build,/7fe450e19305b828c199d602c23a8337aaa1f03b/);
  assert.match(build,/git -C \$vendor fetch --depth=1 origin \$upstream/);
  assert.match(build,/--target llama-server/);
  assert.match(build,/-DGGML_VULKAN=OFF/);
  assert.match(build,/Get-FileHash -LiteralPath \$engine.FullName -Algorithm SHA256/);
  assert.doesNotMatch(build,/NOT_AN_ENGINE_CI_PLACEHOLDER/);
});
test('explicit CPU test-only feature pins real binary at compile time',()=>{
  assert.match(cargo,/physical_win_cpu_eval = \[\]/);
  assert.match(rust,/#\[cfg\(feature = "physical_win_cpu_eval"\)\]\s+const ENGINE_SHA256: &str = env!\("LU_01G_PHYSICAL_CPU_ENGINE_SHA256"\)/);
  assert.match(rust,/LU_01G_PHYSICAL_CPU_ENGINE_BYTES/);
  assert.match(rust,/fn runtime_engine_files\(\)/);
  assert.match(rust,/check_file\(&engine_dir\.join\(name\), sha, bytes\)/);
  assert.match(build,/LU_01G_PHYSICAL_CPU_ENGINE_SHA256=\$engineHash/);
  assert.match(build,/--features physical_win_cpu_eval/);
});
test('Windows build has real Win32 manifest and a separate app-data identity',()=>{
  assert.match(build,/org\.lu\.agent01g\.devtest/);
  assert.match(build,/build_win_manifest_res\.py/);
  assert.match(build,/verify_win_v6_manifest\.py/);
  assert.match(build,/lu-agent01g-common-controls-v6\.res/);
  assert.match(build,/modelBundled=\$false/);
});
test('only packaged app and source-pinned engine are installed; user data survive uninstall',()=>{
  assert.match(nsis,/File \/oname=lu-desktop\.exe/);
  assert.match(nsis,/File \/oname=llama-server\.exe/);
  assert.match(nsis,/File \/oname=BUILD_PROVENANCE\.json/);
  assert.match(nsis,/Delete "\$INSTDIR\\lu-inference\\llama-server\.exe"/);
  assert.match(nsis,/never delete %APPDATA%, chats, GGUF or journals/i);
  assert.doesNotMatch(nsis,/RMDir \/r "\$APPDATA/);
});
test('Windows runner only uploads real built installer on success and distinguishes acceptance',()=>{
  assert.match(workflow,/runs-on: windows-2025/);
  assert.match(workflow,/choco install nsis --version=3\.11\.0/);
  assert.match(workflow,/Build pinned CPU engine and current AGENT-01G Windows Lu/);
  assert.match(workflow,/scripts\/build-agent01g-physical-windows-nsis\.ps1/);
  assert.match(workflow,/installer-checksums\.json/);
  assert.match(workflow,/actions\/upload-artifact@v4/);
  assert.match(workflow,/if-no-files-found: error/);
  assert.match(readme,/does not prove AMD Vulkan\/ROCm GPU offload/);
});
