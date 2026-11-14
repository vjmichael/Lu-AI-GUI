# MEMORY-LOOP-02B — Host-classified safe continuation

Status: implemented source candidate; native OS and installed-app acceptance open.
Parent: MEMORY-LOOP-02 PR #178.

## Objective
The Qwen2.5-0.5B Q2_K genuine local diagnostic yielded 0/18 correct strict evidence/action judgments, despite copying the stored no-reuse flags. A smaller model must not need to decide whether a historical source, effect or OS custodian justifies a new action.

## What belongs in Lu
The first-party Rust host_safe_continuation module classifies only independently retrieved historical evidence. The loaded AI is free to explain the host's classification, but cannot modify it or use it as a permission grant.

The four ordered next actions, strictest first:
- RECONCILE_ORIGINAL_HISTORY_BEFORE_NEW_ACTION: missing original ANSF source, unbound journal, empty original effect list, or unread additional operations.
- MANUAL_RECONCILIATION_BEFORE_NEW_ACTION: conflicting, unsupported or missing independent source verdict; uncertain/failed original worker; bad digest, links or incomplete effect journals.
- RECONCILE_OS_THEN_RECHECK_SOURCE_AND_REQUEST_FRESH_AUTHORIZATION: original source is independently VERIFIED_AT_OBSERVATION but native Windows Job or delegated Linux cgroup-v2 descendant-empty evidence cannot be proven.
- RECHECK_SOURCE_AND_REQUEST_FRESH_AUTHORIZATION: original semantic and OS custody evidence passed historically, but current resource freshness and fresh human approval remain unknown.

The host aggregates all inspected operations and all inspected original turns by the most restrictive class. Historical success cannot override an earlier observed conflict, missing witness or truncated read. Raw model instructions, generated paths, old human text and historical tool grants are not passed to the policy.

The next loaded-model normal Send receives hostSafeNextAction, hostSafeNextActionExplanation, and modelMayOverrideHostSafeDecision=false alongside bounded historical statuses. If original evidence cannot be reopened, the desktop supplies a fixed neutral reconciliation response instead. The model can explain but never execute, schedule, resume, or authorize a next step based on this snapshot.

## Tests
- Six pure native Rust unit tests assert all four classifications, opposite-order precedence, incomplete evidence and strict denial of replay.
- Eight Node source-contract guards assert the reviewed SQLite/oracle/custody source pipeline, safe decision integration and its nonexecuting API.
- Existing real SQLite reopening test was strengthened to verify source-verified-but-OS-unproven classification, tampered digest downgrade and model no-override flag.
- The genuine Git oracle Qwen3 fixture now exports and checks the host decision for verified/conflicted/interrupted histories.
- Separate exact-head Windows/Linux workflow compiles the pure Rust policy test and runs the native Tauri/SQLite test. Its results MUST be independently observed before this feature is accepted.

## Open gates
This is not an installed Lu pass, AMD Vulkan acceptance, proven escape-capable descendant cleanup, or a new measurement of the larger Qwen 3.8 27B. Cognitum stays deferred to Lu 2.0/3.0 as previously decided. The first-party Lu core remains unsandboxed; only reviewed effects/workers require native supervision.
