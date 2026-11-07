import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=(path)=>readFileSync(new URL(path,import.meta.url),'utf8');
const tasks=read('../../src-tauri/lu-core/src/tasks.rs');
const lane=read('../../src-tauri/lu-core/src/native_awareness_lane.rs');
const host=read('../src-tauri/src/observe_desktop.rs');
const desktop=read('../src-tauri/src/lib.rs');
const ui=read('../web/main.jsx');
const native=read('../../src-tauri/lu-core/src/native_observe_learn.rs');

test('P06 observation is off-by-default and never grants ambient desktop capture',()=>{
  assert.match(native,/mode:Mode::Off/);
  assert.match(host,/can_see_desktop:false/);
  assert.match(host,/screen_capture_enabled:false/);
  assert.match(host,/allow_redacted_snapshots:false/);
  assert.match(host,/inference_performed:0/);
  assert.match(desktop,/lu_observe_start/);
  assert.match(desktop,/blocking_show\(\)/);
});
test('P06 old native_runtime CHECK schema is atomically migrated without dropping rows',()=>{
  assert.match(tasks,/source TEXT NOT NULL CHECK/);
  assert.match(tasks,/native_accessibility','native_runtime','native_test_receipt/);
  assert.match(tasks,/if !awareness_schema\.contains\("'native_runtime'"\)/);
  assert.match(tasks,/INSERT INTO lu_awareness_notes_p06/);
  assert.match(tasks,/FROM lu_awareness_notes;/);
  assert.match(tasks,/p06_native_runtime_notes_migrate_and_private_deletion_is_chat_scoped/);
  assert.match(host,/p06_trusted_runtime_error_persists_but_cannot_claim_verified_gui_failure/);
});
test('P06 seven-day retention and chat-only deletion cannot mint tool or learning grant',()=>{
  assert.match(tasks,/datetime\('now','-7 days'\)/);
  assert.match(tasks,/delete_awareness_notes_for_workspace/);
  assert.match(tasks,/WHERE workspace_id=\?1 AND task_id LIKE 'obs-%'/);
  assert.match(tasks,/p06_note_retention_prunes_stale_only/);
  assert.match(host,/pub\(crate\) fn forget_notes/);
  assert.match(host,/awareness\.discard_observations\(\)/);
  assert.match(lane,/pub fn discard_observations/);
  assert.match(lane,/p06_forget_clears_handoff_without_regrant_or_sequence_replay/);
  assert.match(desktop,/async fn lu_observe_forget_notes/);
  assert.match(desktop,/state\.chats\.history\(&chat_id\)/);
  assert.match(desktop,/Forget diagnostic notes/);
  assert.match(ui,/invoke\('lu_observe_forget_notes'/);
  assert.match(ui,/Forget observation notes for this chat/);
  assert.match(ui,/diagnostic codes expire after seven days/i);
  assert.doesNotMatch(host,/grant_learning_review\(true\)/);
});
test('P06 capture receipts never imply verified visual interpretation or retained pixels',()=>{
  assert.match(host,/source_identity_attested:source_attested/);
  assert.match(host,/capture_guardian:Option<GuardianLease>/);
  assert.match(host,/session\.capture_guardian\.take\(\)/);
  assert.match(host,/release_guardian_when_reaped/);
  assert.match(host,/fn reconcile_revoked\(&self,session:&mut Session\)/);
  assert.match(host,/p06_revoked_consent_task_receipt_reconciles_after_native_cleanup/);
  assert.match(host,/p06_rejected_capture_plan_drops_guardian_reservation_without_spawning_worker/);  assert.match(host,/release_guardian_when_reaped/);
  assert.match(host,/capture_processes\.poll\(\)\s*\.map_err\(/);
  assert.doesNotMatch(host,/let _=self\.capture_processes\.poll\(\)/);
  assert.match(host,/let receipt_ready=verified_reaped_frame_receipt\(/);
  assert.match(host,/let worker_reaped=!self\.capture_processes\.can_cancel\(&task_id\)/);
  assert.match(host,/frame_sha256:if receipt_ready\{sha\}else\{None\}/);
  assert.match(host,/p06_capture_receipt_requires_supervised_os_reap_before_evidence/);
  assert.match(host,/fully_redacted_receipt:receipt_ready/);
  assert.match(host,/pixels_retained:false/);
  assert.match(host,/visual_interpretation_verified:false/);
  assert.match(desktop,/lu_observe_capture_one_shot/);
  assert.match(ui,/No observation data/);
});

test('P06 native WGC process instance is verified, generic picker stays unverified',()=>{
  const worker=read('../../src-tauri/lu-observe-capture-worker/src/main.rs');
  const scheduler=read('../../src-tauri/lu-core/src/native_background_scheduler.rs');
  assert.match(worker,/OpenProcess\(PROCESS_QUERY_LIMITED_INFORMATION,false,pid\)/);
  assert.match(worker,/GetProcessTimes/);
  assert.match(worker,/struct ProcessQueryHandle\(HANDLE\)/);
  assert.match(worker,/CloseHandle\(self\.0\)/);
  assert.match(worker,/process_instance::born_at\(pid\)\?!=born/);
  assert.match(worker,/visible_window::attested\(window\.as_raw_hwnd\(\) as usize\)/);
  assert.match(worker,/GetAncestor\(native,GA_ROOT\)/);
  assert.match(worker,/DwmGetWindowAttribute/);
  assert.match(worker,/IsWindow\(Some\(native\)\)/);
  assert.match(worker,/DWMWA_CLOAKED/);
  assert.equal((worker.match(/process_instance::born_at\(pid\)\?!=born/g)||[]).length,2);
  assert.match(worker,/Foreground window instance changed during readback/);
  assert.match(worker,/foreground_identity:Option<\(usize,u32,u64\)>/);
  assert.match(worker,/\(nonce\.into_owned\(\),None\)/);
  assert.match(worker,/padded\.fill\(255\)/);
  assert.match(worker,/Lu-WGC-whole-frame-redacted-v2/);
  assert.match(scheduler,/pub fn capture_one_shot\(\)/);
  const supervisor=read('../../src-tauri/lu-core/src/native_background_process.rs');
  assert.match(supervisor,/--lu-observe-one-shot"\|"--lu-observe-foreground-one-shot/);
  assert.match(supervisor,/job\.set_memory_limit\(512\*1024\*1024\)/);  assert.match(scheduler,/pub fn capture_one_shot\(\)/);
  assert.match(desktop,/guardian\.admit_host_worker\(/);
  assert.match(desktop,/observer\.launch_one_shot\(&chat_id,plan,lease\)/);
  assert.match(ui,/fullyRedactedReceipt/);
  assert.match(ui,/sourceIdentityAttested/);
});

test('P06 denied Windows WGC child is never misreported as workflow failure',()=>{
  const focused=read('../../.github/workflows/lu-p06-observe-privacy.yml');
  const legacy=read('../../.github/workflows/lu-v1-observe-learn.yml');
  assert.match(focused,/if\(\$LASTEXITCODE -ne 1\)\{throw 'Native WGC worker failed closedness check'\}[\s\S]*?exit 0/);
  assert.match(legacy,/throw 'Capture worker did not reject unauthorized invocation without pixel output'[\s\S]*?exit 0/);
});

test('P06 physical Windows child-process evidence is opt-in and cannot claim WGC UI or redaction',()=>{
  const script=read('../../scripts/verify-observe-installed-windows.ps1');
  assert.match(script,/\[switch\]\$ExerciseCaptureCancellation/);
  assert.match(script,/Get-CimInstance -ClassName Win32_Process/);
  assert.match(script,/ParentProcessId/);
  assert.match(script,/uiAutomationWindowProcessAttested='PASS'/);
  assert.match(script,/root\.Current\.ProcessId/);
  assert.match(script,/\[switch\]\$ExerciseRestart/);
  assert.match(script,/restartOffByDefaultUi='PASS_UIA_OFF_ON_RELAUNCH_ONLY'/);
  assert.match(script,/FAIL_OBSERVED_OS_PROCESS_STILL_ACTIVE/);
  assert.match(script,/ExecutablePath/);
  assert.match(script,/captureWorkerOsReap='PASS_OS_CHILD_REAP_ONLY'/);
  assert.match(script,/wgcPickerAndCancellation='NOT_TESTED'/);
  assert.match(script,/sensitiveRegionRedaction='NOT_TESTED'/);
  assert.match(script,/visionOrGuiTestAcceptance='NOT_TESTED'/);
  assert.doesNotMatch(script,/Press-ObservedButton 'Approve'/);
});

test('P06 bounded region mask is not a shortcut around native OS source and privacy attestation',()=>{
  const worker=read('../../src-tauri/lu-observe-capture-worker/src/main.rs');
  assert.match(worker,/fn mask_sensitive_bgra_regions/);
  assert.match(worker,/regions\.is_empty\(\)\|\|regions\.len\(\)>MAX_MASKS/);
  assert.match(worker,/rect\.left\.checked_add\(rect\.width\)/);
  assert.match(worker,/invalid_or_untrusted_mask_never_partially_writes_raw_frame/);
  assert.match(worker,/future_sensitive_region_mask_is_stride_correct_without_touching_padding/);
  // Only fully blanked, nonce-bound receipts remain connected to WGC frame events.
  assert.match(worker,/fn frame_receipt\(/);
  assert.match(worker,/Lu-WGC-whole-frame-redacted-v2/);
  assert.doesNotMatch(worker,/mask_sensitive_bgra_regions\(width,height,row_pitch,buffer/);
});

test('P06 native UIA privacy probe is OS identity bound and cannot authorize image release',()=>{
  const ps=read('../../scripts/probe-observe-sensitive-window-windows.ps1');
  assert.match(ps,/GetWindowThreadProcessId/);
  assert.match(ps,/GetAncestor\(\$hwnd,2\)/);
  assert.match(ps,/DwmGetWindowAttribute/);
  assert.match(ps,/ExpectedBirthFileTime/);
  assert.match(ps,/IsPasswordProperty/);
  assert.match(ps,/detectorCoverage='INCOMPLETE_UIA_ONLY'/);
  assert.match(ps,/wgcFrameCoordinatesVerified=\$false/);
  assert.match(ps,/pixelReleaseAllowed=\$false/);
  assert.doesNotMatch(ps,/Current\.Name|Current\.Value|TextPattern|ValuePattern/);
});

test('P06 real native OS birth and sensitive UIA refusal tests run on Windows',()=>{
  const worker=read('../../src-tauri/lu-observe-capture-worker/src/main.rs');
  const ci=read('../../.github/workflows/lu-p06-observe-privacy.yml');
  assert.match(worker,/native_win32_process_birth_is_live_stable_and_query_only/);
  assert.match(worker,/process_birth_for_test\(own_pid\)/);
  assert.match(ci,/Refuse untrusted native HWND privacy probe without accessibility scan/);
  assert.match(ci,/if\(\$child\.ExitCode -eq 0\)\{throw 'Invalid OS HWND incorrectly accepted/);
});

test('P06 zero-default approved BGRA region extraction and external GPU counter do not bypass native privacy',()=>{
  const worker=read('../../src-tauri/lu-observe-capture-worker/src/main.rs');
  const script=read('../../scripts/verify-observe-installed-windows.ps1');
  const gui=read('../../src-tauri/lu-core/src/native_gui_verification.rs');
  const ci=read('../../.github/workflows/lu-p06-observe-privacy.yml');
  assert.match(worker,/fn approved_regions_only_bgra/);
  assert.match(worker,/approved\.is_empty\(\)\|\|approved\.len\(\)>MAX_APPROVED/);
  assert.match(worker,/reviewed_region_export_defaults_to_black_and_never_copies_pitch_padding/);
  assert.match(worker,/invalid_or_unreviewed_region_cannot_produce_any_pixel_buffer/);
  assert.doesNotMatch(worker,/approved_regions_only_bgra\(width,height,row_pitch,buffer/);
  assert.match(script,/\[switch\]\$SampleGpuMemory/);
  assert.match(script,/Get-Counter '\\GPU Process Memory\(\*\)\\Local Usage'/);
  assert.match(script,/gpuQuotaVerified=\$false/);
  assert.match(script,/OS_GPU_PROCESS_COUNTER_OBSERVED_ONLY/);
  assert.match(gui,/Evidence contains checks not in intended plan/);
  assert.match(ci,/native_gui_verification::tests/);
});

test('P06 independently hash-reviewed native NSIS install/uninstall is a real opt-in test, not a scripted pass',()=>{
  const script=read('../../scripts/verify-observe-nsis-cycle-windows.ps1');
  assert.match(script,/\$DisposableMachineConfirmed/);
  assert.match(script,/Match-Sha \$installer \$InstallerSha256/);
  assert.match(script,/Lu-Registrations/);
  assert.match(script,/Get-Process -Name Lu/);
  assert.match(script,/Invoke-BoundedReviewedExe \$installer/);
  assert.match(script,/Match-Sha \$app \$LuSha256/);
  assert.match(script,/Match-Sha \$worker \$WorkerSha256/);
  assert.match(script,/Invoke-BoundedReviewedExe \$uninstaller/);
  assert.match(script,/uninstallRegistration='PASS'/);
  assert.match(script,/wgcPicker='NOT_TESTED'/);
  assert.match(script,/gpuHardwareQuota='NOT_TESTED'/);
  assert.doesNotMatch(script,/Remove-Item -Recurse|Invoke-WebRequest|Start-BitsTransfer/);
});

test('P06 source-instance domain bound in redacted receipt; generic remains unverified',()=>{
  const worker=read('../../src-tauri/lu-observe-capture-worker/src/main.rs');
  assert.match(worker,/native-foreground-window-v1/);
  assert.match(worker,/unverified-generic-picker-v1/);
  assert.match(worker,/nonce_bound_foreground_provenance_is_distinct_and_privacy_preserving/);
  assert.match(worker,/hash\.update\(\(hwnd as u64\)\.to_le_bytes\(\)\)/);
  assert.match(worker,/hash\.update\(born\.to_le_bytes\(\)\)/);
  assert.match(worker,/self\.foreground_identity/);
});

test('P06 observed WGC frame is never presented as an image delivered to the model',()=>{
  assert.match(host,/pub frame_observed:bool/);
  assert.match(host,/image_received:false/);
  assert.match(host,/frame_observed:receipt_ready/);
  assert.match(host,/visual_interpretation_verified:false/);
  assert.match(ui,/captureReceipt\.frameObserved/);
  assert.doesNotMatch(host,/image_received:receipt_ready/);
});

test('P06 independent UIA runner verifies reviewed native process and plan but cannot claim pixels or effects',()=>{
  const script=read('../../scripts/verify-observe-ui-contract-windows.ps1');
  assert.match(script,/Checked-File \$InstalledExe \$ArtifactSha256/);
  assert.match(script,/Checked-File \$PlanPath \$PlanSha256/);
  assert.match(script,/MainWindowHandle/);
  assert.match(script,/AutomationIdProperty/);
  assert.match(script,/Current\.IsEnabled/);
  assert.match(script,/Current\.IsOffscreen/);
  assert.match(script,/PASS_ACCESSIBILITY_ONLY/);
  assert.match(script,/visualPixelProof=\$false/);
  assert.match(script,/nativeEffectProof=\$false/);
  assert.match(script,/modelImageReceived=\$false/);
  assert.doesNotMatch(script,/ValuePattern|TextPattern|Current\.Name|InvokePattern/);
});

test('P06 reviewed native UIA checks require a pinned source commit and explicit button-only fallback',()=>{
  const script=read('../../scripts/verify-observe-ui-contract-windows.ps1');
  assert.match(script,/plan\.sourceCommit -cnotmatch/);
  assert.match(script,/plan\.artifactSha256 -ine \$ArtifactSha256/);
  assert.match(script,/PSObject\.Properties\['buttonName'\]/);
  assert.match(script,/AutomationElement\]::NameProperty/);
  assert.match(script,/AutomationElement\]::AutomationIdProperty/);
  assert.match(script,/INCONCLUSIVE_NOT_NATIVE_BUTTON/);
  assert.match(script,/sourceCommit=\(\[string\]\$plan\.sourceCommit\)/);
  assert.doesNotMatch(script,/Current\.Name|ValuePattern|TextPattern|InvokePattern/);
});

test('P06 native foreground selected window belongs to same Windows session as supervised worker',()=>{
  const worker=read('../../src-tauri/lu-observe-capture-worker/src/main.rs');
  const manifest=read('../../src-tauri/lu-observe-capture-worker/Cargo.toml');
  assert.match(worker,/ProcessIdToSessionId/);
  assert.match(worker,/fn same_session\(pid:u32\)/);
  assert.match(worker,/process_instance::same_session\(pid\)\?/);
  assert.match(worker,/native_foreground_source_must_share_interactive_session/);
  assert.match(manifest,/Win32_System_RemoteDesktop/);
});

test('P06 OS UIA probes require reviewer session and CI runs a single branch-push matrix',()=>{
  const workflow=read('../../.github/workflows/lu-p06-observe-privacy.yml');
  const sensitive=read('../../scripts/probe-observe-sensitive-window-windows.ps1');
  const checker=read('../../scripts/verify-observe-ui-contract-windows.ps1');
  assert.match(workflow,/branches: \['lu\/p06-observe-privacy-\*'\]/);
  assert.match(workflow,/workflow_dispatch:/);
  assert.doesNotMatch(workflow,/\n  pull_request:/);
  assert.match(sensitive,/\$proc\.SessionId -ne \$auditor\.SessionId/);
  assert.match(sensitive,/\$after\.SessionId -ne \$auditor\.SessionId/);
  assert.match(checker,/\$process\.SessionId -ne \$auditor\.SessionId/);
  assert.match(checker,/\$after\.SessionId -ne \$auditor\.SessionId/);
});

test('P06 actual NSIS process run is time-bounded and never auto-cleans uncertain writes',()=>{
  const script=read('../../scripts/verify-observe-nsis-cycle-windows.ps1');
  assert.match(script,/\[ValidateRange\(30,1800\)\]\[int\]\$StageTimeoutSeconds=600/);
  assert.match(script,/function Invoke-BoundedReviewedExe/);
  assert.match(script,/WaitForExit\(\$StageTimeoutSeconds\*1000\)/);
  assert.match(script,/\$child\.Kill\(\$true\)/);
  assert.match(script,/installationStateUncertain=\$false/);
  assert.match(script,/installationStateUncertain=\(\$stage -ne 'preflight'\)/);
  assert.match(script,/reviewedStageProcessExit='BOTH_REVIEWED_STAGE_PROCESSES_EXITED'/);
  assert.doesNotMatch(script,/Remove-Item -Recurse/);
});

test('P06 actual two native dialogs and Linux administrator OS agent keep activation off by default',()=>{
  const auth=read('../src-tauri/src/observe_linux_auth.rs');
  const polkit=read('../../scripts/linux/org.lu.desktop.observe-learn.policy');
  assert.match(desktop,/EXPERIMENTAL Observe & Learn warning/);
  assert.match(desktop,/Explicit local observation permission/);
  assert.match(desktop,/if !experimental\{return false;\}/);
  assert.match(desktop,/if !approved\{return observer\.status\(&chat_id\);\}/);
  assert.match(desktop,/observe_linux_auth::require_admin_for_observation\(\)\?/);
  assert.match(desktop,/active_research_network_tasks/);
  assert.match(auth,/--allow-user-interaction/);
  assert.match(auth,/--process/);
  assert.match(auth,/rsplit_once\("\) "\)/);
  assert.match(auth,/Duration::from_secs\(90\)/);
  assert.match(polkit,/<allow_active>auth_admin<\/allow_active>/);
  assert.match(polkit,/<allow_any>no<\/allow_any>/);
  assert.match(ui,/Enable experimental Observe & Learn/);
});
test('P06 tagged local-only observations cannot be declassified into network sinks or research/browser tools',()=>{
  const policy=read('../../src-tauri/lu-core/src/native_observe_egress.rs');
  const browser=read('../src-tauri/src/browser_model_desktop.rs');
  const memory=read('../../src-tauri/lu-core/src/native_observe_ansf.rs');
  assert.match(policy,/pub const OBSERVE_LEARN/);
  assert.match(policy,/Destination::NetworkSocket/);
  assert.match(policy,/Destination::RemoteModel/);
  assert.match(policy,/fn joined/);
  assert.match(tasks,/pub fn has_observe_provenance/);
  assert.match(tasks,/observe_session_provenance_is_durable_after_stop_and_note_forget/);
  assert.match(host,/DataProvenance::OBSERVE_LEARN/);
  assert.match(host,/network_egress_allowed:false/);
  assert.match(desktop,/deny_observation_network_egress\(&chat_id\)/);
  assert.match(desktop,/deny_unscoped_network_while_observing\(\)/);
  assert.match(browser,/deny_observation_network_egress\(chat_id\)/);
  assert.match(memory,/network_egress=deny/);
  assert.match(ui,/LOCAL ONLY/);
});

test('P06 Tauri Linux DEB/RPM embeds real admin polkit action and compiles native subject check',()=>{
  const cfg=JSON.parse(read('../src-tauri/tauri.linux.conf.json'));
  const path='/usr/share/polkit-1/actions/org.lu.desktop.observe-learn.policy';
  assert.deepEqual(cfg.bundle.targets,['deb','rpm']);
  assert.equal(cfg.bundle.linux.deb.files[path],'../../scripts/linux/org.lu.desktop.observe-learn.policy');
  assert.equal(cfg.bundle.linux.rpm.files[path],'../../scripts/linux/org.lu.desktop.observe-learn.policy');
  const ci=read('../../.github/workflows/lu-p06-observe-privacy.yml');
  assert.match(ci,/rustc --edition=2021 --test lu-desktop\/src-tauri\/src\/observe_linux_auth\.rs/);
  assert.match(ci,/ET\.parse\(policy\)/);
});

test('P06 durable SQLite awareness rows enforce local-only origin and prevent metadata declassification',()=>{
  assert.match(tasks,/ALTER TABLE lu_awareness_notes ADD COLUMN data_origin TEXT NOT NULL DEFAULT 'observe_learn'/);
  assert.match(tasks,/network_egress_allowed INTEGER NOT NULL DEFAULT 0/);
  assert.match(tasks,/CHECK\(network_egress_allowed=0\)/);
  assert.match(tasks,/pragma_table_info\('lu_awareness_notes'\)/);
  assert.match(tasks,/migrated\[0\]\.data_origin/);
  assert.match(host,/data_origin:n\.data_origin,network_egress_allowed:n\.network_egress_allowed/);
});

test('P06 native primary-monitor route is OS-identified and never inferred from the generic picker',()=>{
  const worker=read('../../src-tauri/lu-observe-capture-worker/src/main.rs');
  const supervisor=read('../../src-tauri/lu-core/src/native_background_process.rs');
  assert.match(worker,/monitor::Monitor/);
  assert.match(worker,/Monitor::primary\(\)/);
  assert.match(worker,/as_raw_hmonitor/);
  assert.match(worker,/primary_monitor_matches/);
  assert.match(worker,/native-primary-monitor-v1/);
  assert.match(worker,/native_monitor_source_receipt_is_domain_separated_from_unknown_picker/);
  assert.match(worker,/native_primary_monitor_identity_is_real_os_handle_or_refused/);
  assert.match(worker,/--lu-observe-primary-monitor-one-shot/);
  assert.match(supervisor,/--lu-observe-primary-monitor-one-shot/);
  assert.match(host,/--lu-observe-primary-monitor-one-shot/);
  assert.match(desktop,/primary_display:Option<bool>/);
  assert.match(ui,/observeCaptureOneShot\(false,true\)/);
});

test('P06 native local WGC image adapter enforces consent, PNG bounds and tainted non-network origin',()=>{
  const bridge=read('../../src-tauri/lu-core/src/native_observe_image_admission.rs');
  const core=read('../../src-tauri/lu-core/src/lib.rs');
  const workflow=read('../../.github/workflows/lu-p06-observe-privacy.yml');
  assert.match(core,/pub mod native_observe_image_admission;/);
  assert.match(bridge,/pub fn admit_wgc_frame_locally/);
  assert.match(bridge,/consent_active_and_os_source_attested:bool/);
  assert.match(bridge,/DataProvenance::OBSERVE_LEARN/);
  assert.match(bridge,/Destination::LocalInference/);
  assert.match(bridge,/Destination::RemoteModel/);
  assert.match(bridge,/Destination::NetworkSocket/);
  assert.match(bridge,/rgba\.fill\(0\)/);
  assert.match(bridge,/admit_data_url\(&payload\)/);
  assert.match(bridge,/MAX_VISUAL_HEIGHT:u32=720/);
  assert.match(workflow,/native_observe_image_admission::tests/);
});

test('P06 host-owned WGC stdout BGRA one-use pipe does not persist image bytes or enable arbitrary workers',()=>{
  const src=read('../../src-tauri/lu-core/src/native_background_process.rs');
  const workflow=read('../../.github/workflows/lu-p06-observe-privacy.yml');
  assert.match(src,/pub struct CapturedBgra/);
  assert.match(src,/fn read_worker_progress\(/);
  assert.match(src,/allow_local_image:bool/);
  assert.match(src,/if !allow_local_image/);
  assert.match(src,/input\.read_exact\(&mut pixels\)/);
  assert.match(src,/Sha256::digest\(&pixels\)/);
  assert.match(src,/pub fn take_local_observe_frame/);
  assert.match(src,/pub fn discard_local_observe_frame/);
  assert.match(src,/impl Drop for CapturedBgra/);
  assert.match(src,/self\.bytes\.fill\(0\)/);
  assert.match(src,/native_local_observe_image_pipe_is_private_one_shot_bounded_and_not_in_sqlite/);
  assert.match(workflow,/native_local_observe_image_pipe_is_private_one_shot_bounded_and_not_in_sqlite/);
});
