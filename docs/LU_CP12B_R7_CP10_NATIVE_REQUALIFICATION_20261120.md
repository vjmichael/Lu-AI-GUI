# CP-12B-R7 / CP-10 — Windows native CI requalification, 2026-11-20

**Source branch:** `lu/cp12b-r7-cp10-native-repair-20261120`, based exactly on published R7 commit `2e0ed022d03bacae73a669e362c41f72a5c1d145`.

## Observed failure, not inferred

Hosted run [44830831568](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/44830831568) at R7 SHA had two Windows 2025 jobs: `windows-desktop-source-check` **PASS**, `windows-native-protocol` **FAIL** in the initial UI/source-contract step (13 passed, 1 failed). The failing source regex required bare `\n` while the Git Windows checkout read `\r\n`. Source-tree verification and the other 13 contracts passed. Since the early contract failed, the job **did not execute** Windows named-pipe native protocol tests; no claim of a native failure or pass can be made from that job.

## Focused repair

- Normalize only the workflow text *in memory* before line-anchored Node assertions; never rewrite checkout bytes, normalize security tokens or relax the actual expected branch/commit conditions.
- Schedule exactly one source-locked Windows 2025 push qualification on the new isolated candidate branch and exactly one exact-matching commit message, using the existing two jobs. No push on unrelated paths, no automatic model downloads, no installed acceptance claim.
- The script verifies current checked-out SHA, complete immutable source lock, frontend/contract checks, actual native Windows `windows_dx_eval` and default-off Rust tests, and separately Windows desktop source checking. First failed step remains a hard failure.
- No native Rust/transport/security code, release/installer, model provisioning, IDE scope or published R7 head changed.

## Remaining P0 / CP-10 conditions

1. Complete this exact candidate's hosted Windows two-job acceptance and investigate any actual failure on its own source SHA; never use prior older CI to close it.
2. Reconcile CP-13-R1 docs and UX-R2 single-interface frontend commit `b7fbd61f098127d1c6d8b16786bb37f6f843a196` into a later independently qualified consolidated source. The current repaired CI candidate deliberately excludes the unqualified UX change.
3. Build exact-source installed NSIS including authentic runtime dependencies; observe actual two independently owned GGUF processes and both user consents, cancellation, revocation, replay, crash/restart and negative second-account test. A hosted type-check is not installed graphical acceptance.
4. On the physical Windows/Radeon R9700 machine, qualify GPU/CPU model lifecycle, startup, inference, shutdown, tool calls, multimodal, installed recovery, upgrade/rollback and data integrity. Record block/skip/fail separately.
5. Repair/reconcile two historical UI assertion failures in consolidated acceptance: workspace-routing and scheduler priority, without reclassifying them as passes.

**Release status:** Lu v1.0 NOT qualified. CP-13's 98 tracked design IDs are evidence-map entries, not 98 passes; no new open-ended autonomous repair or broader sandbox features authorized.
