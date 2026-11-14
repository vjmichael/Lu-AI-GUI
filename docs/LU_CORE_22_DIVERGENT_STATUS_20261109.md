# R00 — individual disposition of the 22 divergent Lu Core PR tips

**Scope:** PR #200 convergence, comparison against the frozen original Core head `a7d82f13...`. The exact side-branch SHAs and decisions are in [the machine register](LU_CORE_DIVERGENT_TIP_RESOLUTION_20261109.json); original open-PR topology and the Git comparison ledger remain linked. This is a **review of source deltas, not a merge of 22 PRs**.

There is no safe blanket fast-forward or automatic cherry-pick for these 22 branches: old implementations overlap later security-critical Core code. Where they are superseded, the current implementation is retained; where they are future DLCs, they remain explicitly outside the frozen alpha; where useful isolated changes were demonstrated, those changes have been ported into PR #200 without replaying any old permissions.

| PR | Family | Source reconciliation decision |
| --- | --- | --- |
| #3 | REPO-01 | Superseded: exact source scripts already included |
| #6 | NVP inference seal | Deferred: DLC interoperability, not alpha |
| #10 | DLC IPC Rust | Deferred: unaccepted first-party IPC |
| #11 | DLC host RFC | Deferred: proposed schema only |
| #15 | Archive inspect | Superseded by newer native archive reader |
| #16 | JSON Schema tool | Optional parity gap: existing JSON boundary is NOT JSON Schema 2020-12 |
| #17 | Environment check | Host environment context retained; no extra model-facing executable finder |
| #18 | OCR fallback | Current host-owned native OCR/vision route retained |
| #19 | Long tool-loop guard | Future measured-progress improvement; current bounded/identical-result guard retained |
| #20 | Backend/vLLM control | Defer alternate provider; current pinned GGUF host supervisor retained |
| #21 | Cognitum bridge | Deferred research add-on; no memory authority |
| #22 | Archive CI replay | Superseded by current archive integration and grouped gates |
| #26 | Browser research | Newer host-scoped browser and research modules retained |
| #27 | Background/Reader | Newer task and Reader modules retained |
| #28 | Operational-memory v0 | Superseded by ANSF/SQLite durable original memory; no competing store |
| #29 | Foundational audit | Historical design evidence, no runtime import |
| #35 | Image edit intent | Newer non-destructive image intent module retained |
| #44 | Wave2 tools | Newer atomic/no-clobber file/archive/Office source retained |
| #61 | Observe/Wave4 | Selected seven Windows/SQLite/browser test corrections ADOPTED |
| #112 | CI queue hygiene | Superseded by current lease-bound Node cancellation and manual-only CI |
| #133 | Delegated cgroup | AGENT-01L kept; nested cgroup removal and test ADOPTED; kernel-positive OPEN |
| #191 | R00 graph | Newer full graph with canonical main ledger retained |

## Evidence-backed integrated improvements

**Observe #61:** Seven native-test and browser-acceptance paths were reconciled on PR #200 (archive and Office capability handles, DesktopOwner bounded retry, collision-safe ANSF continuity fixtures, explicit browser-candidate requirement). Older workflow fanout and stale assertions were not copied. The current archive-tree and package cleanup tests already have stronger handling.

**Linux custody #133:** Retained AGENT-01L's existing first-party host/custodian and strengthened it using #133's nested child-cgroup case. The custodian performs bounded recursive cgroup removal only after genuine kernel recursive emptiness; independent test-only host now launches a `setsid` descendant in a nested cgroup, and the Python oracle insists both child and root leaf disappear after SIGKILL and a durable OS receipt. **The delegated-kernel positive gate has not passed**, and a same-UID worker migrating into a writable ancestor cgroup is a separate open confinement gate.

**CI queue #112:** No parallel Python cancellation workflow was imported. Existing `lu-ci-superseded-queue.mjs` already enforces a branch/SHA lease, queued-only identity, revalidation and no cancellation of running/current-source work; `lu-ci-group-policy.mjs` excludes heavyweight per-commit fanout. Native group acceptance remains manually initiated.

## Deliberate non-alpha parity gaps

The early optional `validate_schema` implementation from #16 is **not** the same as current JSON syntax validation. The former requires an independently approved JSON Schema 2020-12 dependency, Protobuf schemas, Cargo lock and real runtime negative tests. It is **not implemented in this integration**. #19's source-aware long-horizon loop-progress detector was also not wired into today's bounded `agent_tool_loop` and is not claimed as accepted; current loop control remains an explicit alpha limitation. No substitute or untested mini-application was attached merely to make PR ancestry look clean.

DLC/NVP/Cognitum service protocols, vLLM backend and the retired parallel operational memory store remain deliberately deferred. Original PRs are preserved for audit; none has been automatically merged, closed or classified as native-accepted.

## Acceptance boundary

A completed source integrity test proves only exact tracked bytes and policy invariants. The previously run native memory matrix was pinned to `2ab7a6fa...` and Windows failed a still-open SQLite handle; the fix is included in this convergence branch but **needs exact-head native retest**. Current candidate still requires a grouped Ubuntu/Windows compiler and protected-worker acceptance, then appropriate model, installed app and OS custodian gates. No effect grants, task journals, authority receipts or historical permissions are imported from side branches.
