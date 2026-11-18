import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
const read = (path) => fs.readFileSync(new URL("../"+path, import.meta.url),"utf8");

test("C6 heavy source + native packaging is manual-only and exact-SHA bound",()=>{
  const workflow=read(".github/workflows/lu-core-c6-hosted-dual-nsis.yml");
  const trigger=workflow.slice(workflow.indexOf("\non:\n"),workflow.indexOf("\npermissions:"));
  assert.match(trigger,/workflow_dispatch:/);
  assert.match(trigger,/workflow_call:/);
  assert.doesNotMatch(trigger,/push:|pull_request:|pull_request_target:/);
  assert.match(workflow,/github\.sha/);
  assert.match(workflow,/node scripts\/verify-source-tree\.mjs/);
  assert.match(workflow,/Build exact-current native Lu and separately pinned CPU plus Vulkan llama\.cpp NSIS/);
  assert.match(workflow,/scripts\/build-current-windows-nsis\.ps1/);
  assert.match(workflow,/choco install nsis --version=3\.11\.0/);
  assert.match(workflow,/choco install vulkan-sdk --version=1\.4\.350/);
  assert.match(workflow,/scripts\/m3-windows-nsis-cycle\.ps1/);
  assert.match(workflow,/ExerciseInstalledGui=\$true/);
  assert.match(workflow,/ExpectedVulkanSha256/);
  assert.match(workflow,/PASS_INSTALLED_UNIFIED_TABS/);
  assert.match(workflow,/PASS_UNTOUCHED_APPDATA_SENTINEL/);
  assert.match(workflow,/actions\/upload-artifact@v4/);
  assert.doesNotMatch(workflow,/continue-on-error: true/);
});

test("C6 physical evidence only runs against genuine enumerated R9700 and matching MMProj",()=>{
  const source=read("scripts/p0-qwen38-radeon-physical.ps1");
  assert.match(source,/PhysicalWindowsConfirmed/);
  assert.match(source,/Get-CimInstance Win32_VideoController/);
  assert.match(source,/R9700/);
  assert.match(source,/R9700 adapter not enumerated by Windows/);
  assert.match(source,/C6 image acceptance requires a matching pinned MMProj GGUF/);
  assert.match(source,/r9700HardwareObserved=\$true/);
  assert.match(source,/modelArchitectureFromGGUF='NOT_INDEPENDENTLY_PARSED_BY_THIS_RECORDER'/);
  assert.doesNotMatch(source,/function Gpu-Pid-Bytes\(\[int\]\$pid\)/i);
  assert.match(source,/function Gpu-Pid-Bytes\(\[int\]\$engineProcessId\)/);
  assert.match(source,/ownerPid/);
  assert.match(source,/ownedVulkanEngineSamples/);
  assert.match(source,/INCOMPLETE_NO_LU_OWNED_VULKAN_ENGINE_OBSERVED/);
  assert.match(source,/human report, not automated oracle/);
  assert.match(source,/ggufTensorLoadedPercent='NOT_MEASURED_BACKEND_TENSOR_TELEMETRY_REQUIRED'/);
  assert.match(source,/acceptance='NOT_ACCEPTED_REQUIRES_REVIEW'/);
});

test("C6 installed package never claims physical offload or silently bundles an unreviewed browser",()=>{
  const build=read("scripts/build-current-windows-nsis.ps1");
  const nsis=read("lu-desktop/packaging/NSIS_CURRENT_DEV_PREVIEW.nsi");
  const workflow=read(".github/workflows/lu-core-c6-hosted-dual-nsis.yml");
  assert.match(build,/e0f142d5ddc1461ce721644d92af33db23545105/);
  assert.match(build,/-DGGML_VULKAN=OFF/);
  assert.match(build,/'-DGGML_VULKAN=ON'/);
  assert.match(build,/LU_PREVIEW_VULKAN_ENGINE_SHA256/);
  assert.match(build,/LU_BROWSER_PACKAGE_DIR/);
  assert.match(build,/LU_REQUIRE_BROWSER/);
  assert.match(workflow,/LU_REQUIRE_BROWSER: '0'/);
  assert.match(workflow,/browserRuntime -ne 'NOT_PACKAGED'/);
  assert.match(nsis,/File \/oname=llama-server\.exe/);
  assert.match(nsis,/File \/oname=llama-server-vulkan\.exe/);
  assert.match(nsis,/RMDir \/r "\$INSTDIR\\lu-browser"/);
  assert.doesNotMatch(nsis,/RMDir \/r "\$APPDATA/);
});
