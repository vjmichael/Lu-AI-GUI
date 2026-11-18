# Lu M5-03–05 — physical Windows, effect authority and comparative evidence

**Work-order implementation:** source submitted; native/real-model results must pass on the final exact source head. **No installed Windows acceptance is claimed.** Parent M3 PR #96, M4 PR #98 and M5 PR #101 remain draft and independently gated.

## M5-03 — genuine installed Windows model-swap witness

`scripts/m5-windows-installed-memory-pilot.ps1` is an opt-in pilot for a **dedicated disposable test profile on a physical interactive Windows PC**. It refuses CI/headless execution, an existing Lu process, missing original bytes, a mismatched native `Lu.exe` or `lu-desktop.exe`, the wrong bundled native `lu-inference/llama-server.exe`, symlink/reparse-point substitutions and non-distinct or unhashed GGUF models. No installer, model downloader, inferred authorization, capture of private transcripts or arbitrary effect runs are embedded.

After independent actual SHA-256 reviews and an installed Lu/NSIS cycle (the earlier `scripts/m3-windows-nsis-cycle.ps1` is a separate disposable-Windows gate), a tester may choose to execute:

```powershell
.\scripts\m5-windows-installed-memory-pilot.ps1 `
  -RealInteractiveWindowsAndOwnerConsent -DedicatedDisposableTestProfileConfirmed `
  -InstalledExe '<actual installed Lu.exe absolute path>' `
  -ExeSha256 '<independently measured Lu exe SHA-256>' `
  -EngineSha256 '<independently measured bundled llama-server exe SHA-256>' `
  -ModelA '<absolute path to first reviewed real GGUF>' `
  -ModelASha256 '<actual first GGUF SHA-256>' `
  -ModelB '<absolute path to distinct second real GGUF>' `
  -ModelBSha256 '<actual second GGUF SHA-256>'
```

These are placeholders and **must never be treated as a runnable command or evidence** until reviewed local values exist. The script starts *three real native app processes sequentially*: primary model, same model after cold process restart, and a different reviewed model after swap. It captures real Win32 process ID, UIAutomation identity, window startup time, installed native inference-worker SHA and PID, OS worker absence after normal app close and Windows GPU adapter identity. It prompts the operator to exercise genuine model output, original ANSF source identity/revisions, reported-vs-verified claims, conflict/stale-version, background/hot-scroll, cancellation, context-change monitoring, and historical permission denial.

**Evidence boundaries:** every model answer, source ID and functional assertion typed by the tester is explicitly marked **OPERATOR_REPORTED**. UI Automation process identity and native worker process disappearance are OS-observed but do **not** independently establish local model answer quality or actual AMD Vulkan GPU offload. A GPU enumeration record is **not** Vulkan inference verification. Thus the script may produce a readiness report for separate review, but cannot itself approve M5 release or close physical Windows acceptance. A failed or skipped scenario leaves the gate open. This does not modify existing user data by design: use a disposable test profile.

## M5-04 — real Core authority and effect crash/revocation

`src-tauri/lu-core/tests/m5_effect_authority_acceptance.rs` compiles and exercises the existing `conversation_authority`, native `ToolAuthorizer`, persisted original `AnsfStore`, `EffectJournal` and `ContinuityStore` without duplicating the authorization implementation.

Native regression checks:
- a USER message physically stored in ANSF is eligible for *host-verified task admission*, whereas assistant text, wrong session, invented task origin, changed workspace, wrong effect type and host veto are rejected;
- a bounded `with_authorized_worker_effect` performs an **actual disposable local file write**, checks its genuine bytes, then an explicit task revocation prevents any second write despite an old model/tool invocation; a fresh authority registry after restart cannot revive the grant;
- a previously EXECUTING native effect is genuinely persisted in SQLite, reopened, reconciled to UNCERTAIN and cannot link as APPLIED; newly admitted *read-only* intent cannot authorize an old write, elevated privilege, same call ID or task authority;
- real task revocation listener fires synchronously; a source-linked checkpoint retains its known blocker and reference.

This verifies the **native short, bounded broker and journal boundary**. It is *not* proof of Windows Job Object descendant kill, model-to-UI Tauri button clicks, a complete native per-effect OS-recovery flow, cryptographic installer signing or a verified independent final user-visible model answer. Those remain installed M5-03/04 gates.

## M5-05 — repeat-task comparative pilot, strict evidence schema

`benchmarks/memory/m5_comparative_acceptance.py` is the no-guess scorer. It accepts optional exact-source artifacts from the real GitHub-hosted M5 Windows/Linux native replay, real Qwen M5-02 paired generations, reviewed physical Windows model-swap session, actual authorization test and separate NSIS/Vulkan installation trial. It validates schema shape, paired control/memory cases, actual raw model text versus expected answer, source ID provenance, measured local-model latency and version/unknown safety. It separately records p50/p95 latency and lookup cost, known-answer improvement, overall correctness, prompt tokens, and the source/engine SHA-256.

The report explicitly leaves as `null`: **human interruption time, rediscovery tool-call counts, unnecessary questions, actual external tool effect counts, installed first-token latency, installed model-swap time and verified GPU offload** until someone instruments those outcomes in a real Lu session. Unavailable inputs are `NOT_SUBMITTED`, malformed inputs `INVALID_OR_INCOMPLETE`, user-supplied records `SUBMITTED_NOT_INDEPENDENTLY_ATTESTED`. `release_accepted` is always `false`; final release acceptance **requires independent source, native OS, installer and reviewer sign-off** rather than trusting self-reported JSON.

Run the report after obtaining real M5 data, for example:

```bash
python3 benchmarks/memory/m5_comparative_acceptance.py \
  --model '<actual completed M5-02 results.json>' \
  --physical '<actual reviewed M5-03 m5-physical-windows.json>' \
  --output '<new evidence directory>/m5-gate.json'
```

Without real measurements, run it only to produce an explicitly **open / no inputs submitted** acceptance record. The GitHub M5 workflow executes the real native authority tests on Linux/Windows, parses the physical PowerShell acceptance file on Windows without pretending to install Lu, exercises scorer regression contracts, and attaches its incomplete-by-default report alongside actual Qwen run results if those ever arrive.

## Remaining release blockers

1. Final exact-head M5 Windows/Linux native replay and authority tests and genuine GGUF generated-answer artifacts; repair any actual failing jobs before closing source gates.
2. Genuine original-model -> restart -> distinct replacement-model Windows WebView/hot-scroll/UI and worker process trial, plus independent AMD Vulkan inference/offload proof. No physical PC is attached to this GitHub-only workflow.
3. Actual installer/uninstaller with engine and CRT bytes reviewed, genuine app UI and two locally available models, before considering this a distributable alpha.
4. Real task activity instrumentation for tool rediscovery counts, operator interruption, unnecessary clarification, first-token latency and permission-effect outcomes. A headless answer-recall test **cannot** measure these.
5. Keep M3/M4/M5 draft until evidence gates separately close; don't treat old user approvals, old revisions, source snippets, CI placeholders or a model-generated answer as trust authority.
