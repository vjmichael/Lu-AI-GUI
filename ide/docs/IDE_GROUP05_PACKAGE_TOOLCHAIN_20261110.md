# IDE Group 5 — Package & Toolchain Manager engineering (November 10, 2026)

**Working branch:** `ide/group05-package-review-20261110`, independently stacked on `d01c4b034fc0f9307de12104dc50424e4cface0a`. Group 4 LLVM/native acceptance remains on a different branch and is not an execution dependency for these strictly non-executing source changes.

## Implemented during this Group 5 continuation (source only)

- **5A** `9a8f888`: A requested vcpkg package whose direct declaration has additional features/platform/version metadata is no longer falsely treated as unconditional. Duplicate matching declarations are denied. Native negative regression tests added.
- **5A** `12cf727`: Static vcpkg Sync now parses the current SHA-rechecked manifest rather than accepting malformed or invalid dependency-entry JSON merely because no package name was supplied. Tests cover invalid JSON, unsupported entries and valid no-grant Sync.
- **5C** `8a0e698`: Before reading privately frozen `uv.lock` or installed audit metadata, the private source reader verifies its root is an actual directory, not a symlink/reparse redirect. The existing child-path protections and digest checks remain. Unix symlink-root regression test added.
- No manager process, download, file mutation, approval token or new release entitlement was introduced. There is **no claim** that the new tests have passed a native Windows/Linux runner.

## Remaining exact requirements and acceptance matrix

| Group | Existing foundation | Next concrete engineering gate | Evidence required |
| --- | --- | --- | --- |
| **5A** manifest/preview | Registered-project six-file SHA discovery; typed vcpkg/uv/Conan review and revalidation; new fail-closed parser tests | Consolidated native Rust tests including invalid/ambiguous vcpkg, drift, source-link tests, plus desktop review feedback | One source SHA, both Windows/Linux host tests, no resolver/approval leakage |
| **5B** protected execution | Feature-gated fixed offline uv Sync; Linux cgroup-v2/Bubblewrap; Windows remains denied | Validate true private offline Sync, cancellation/kill/restart, source/manifest tamper, no network/secret access, no child escape | Real kernel/process logs, controlled source and output fingerprints, terminal receipts; no fake approval |
| **5C** provenance | Private installed-byte/PEP 376 RECORD/license verifier; upstream `uv` wheel observation; synthetic CI-only package | Independently pin original runtime and complete transitive wheel closure, prevent root/child substitution, verify installed exact archive-to-binary correspondence | Actual wheel archive bytes/hashes, source and license evidence, bounded hostile audits; no unsupported publisher claim |
| **5D** Windows ACL | Observation-only AppContainer staged-tree DACL/MIC auditor; `uv_sync_offline` fail-closed | Native package SID read-only source/runtime, private write-only output, hostile write/rename/reparse and process escape attempts under real AppContainer/SCM | Independent Windows kernel outcomes and native ACL/Job/owner receipts; **never** replace with unsandboxed launch |
| **5E** recovery | Append-only fsynced lifecycle and private discard-pending states; restart does not restore grants | Fault-inject before/after process exit, audit, discard and fsync; verify actual process tree and bytes on restart | Exact journal-chain replay, no retry, no lost/false terminal receipt, verified source preservation and private cleanup |
| **5F** installed/consent | Feature-gated desktop UX; physical Windows denial fixture; no release-ready installer approval | Real installed Windows and Linux human approve/refuse/timeout/cancel/stale-plan, restart, remove/reinstall/data retention; no CI simulated human | Real OS window interaction, exact installed binary SHA, separate one-use grant/worker/journal receipts |

## Work policy

1. Continue independent development on Group 5; do not hold source work for Group 4 GitHub Ops or alter its running LLVM job/authoritative branch.
2. The initial grouped native tests should include 5A/5C source changes without spawning new per-commit workflow fanout. Test actual 5B/5E protected worker operations only on a reviewed OS runtime. Keep Windows deny-by-default until 5D genuine kernel proof.
3. Do not promote synthetic first-party wheel evidence into third-party release provenance; do not treat a parser, metadata hash, or a CI mock approval as an installed package operation.
4. Defer signing and final legal/publisher approval by owner decision while preserving checks and releaseApproved=false; unsigned development is not signed release acceptance.
5. Lu Core bridge remains deferred. No Docker requirement.

## Additional Group 5 source safeguards (November 10)

- `af40523` (5D): Observation-only Windows staged DACL auditor refuses
  caller-relative roots, even when handed a non-null package SID. A
  deterministic null-SID and relative-root negative test was added.
  This cannot satisfy real AppContainer package-SID/MIC/SCM containment.
- `b44edaa` (5E): The journal's `verify_discard` transition now requires
  a fresh `private_output_cleanup_pending` status, rather than
  accepting a stale status captured before discard preparation. Test
  covers attempts before and after preparation with the old status.
  A fresh pending status is only a prerequisite: actual filesystem
  disposal and native process-tree proof must be independently supplied
  before the caller invokes the transition. The method does not do that
  OS verification itself and this is **not** a cleanup-acceptance claim.

**Current stage:** five source edits with hostile regression tests are
committed. No native compiler or runtime test has been executed on the
latest source SHA; the four-lane Group 4 job is unaffected. Group 5B and
5F still require genuine protected Linux/Windows execution and physical
installed human evidence, and are not closed by the new source checks.

## November 10 continuation: provenance and consolidated native gate

**5B:** Existing Linux protected worker test group contains real modes `--offline-miss`, `--cancel`, `--positive` (CI-only synthetic wheel) and `--hostile-recovery` under a delegated cgroup and unnetworked Bubblewrap. These modes must execute at the newly reconciled source SHA; an earlier success is not a pass for the new 5A–5E source.

**5C:** `fc9ced3`, `cb03be4` and `eba7344` changed the fixed upstream uv intake reader to compare original, opened-descriptor and final path identity, with `O_NOFOLLOW` on supporting OSes. New negative cases cover aliasing the installed binary/license and replacing the selected executable path while a descriptor is held. The fetched PyPI release JSON and wheel hash still do not provide a publisher signature, legal clearance, or complete transitive dependency provenance.

**5D:** The consolidated Windows native source job already compiles and invokes the explicitly disposable `ide056_windows_acl_positive` AppContainer kernel fixture: source read allowed, source write/rename/delete and owner/DACL mutation denied, private output write, containment/Job ownership. This has **not** enabled production Windows package operations; the native job must be inspected for genuine pass/failure. Physical Windows operator approval remains separate.

**5E:** The grouped Linux native fixture exercises SIGABRT/restart and original journal reconciliation. The new journal state guard refuses stale cleanup claims but by itself does not verify physical deletion or process-tree zero. A native cleanup receipt + real filesystem observation is still required before release acceptance.

**5F:** The checked-in physical Windows negative fixture can witness installed application denial only, and may not substitute for positive human approval. A user-controlled physical installed session must separately test approval, refusal, cancellation, stale-plan denial, process death, restart and uninstall with exact binary and journal hashes. An interactive session is not accessible from this GitHub-only execution context. Do not script fake human clicks.

**Integration:** Use the existing consolidated `.github/workflows/ide-dev05-consolidated-native.yml` workflow on this Group 5 branch, rather than creating CI micro-jobs. Every meaningful result must be inspected at one exact SHA. Run disposition is unknown until GitHub reports jobs and logs; no release acceptance is implied by a queued or started workflow.

## Follow-up — independent verification attempt and remaining physical gates

The GitHub connector read on 2026-11-10 confirmed Group 5 was still at
`3d5fea43d915766b4f789cd650ed76573070fa19` when queried. The
commit-associated workflow lookup returned **no workflow runs**. That
lookup only reports PR-triggered runs, so it **cannot prove that no
push-triggered workflow exists**. A separate web Actions-page attempt
was inaccessible. There is **no new verified terminal CI outcome** to
justify closing native Windows/Linux acceptance.

### New 5E private disposal invariant

`e0188cf`: `PackageLifecycleLedger::verify_discard` now takes a
private output path and refuses the durable `discard_verified` stage
while that entry exists, is relative, or has an untrusted parent. The
parent is fsynced on Unix before appending the terminal journal entry.
New tests assert existing package files cannot falsely be declared
destroyed and only an absent private tree permits the transition.
This is **filesystem absence + durable journal only**; separate OS
process-tree zero/SCM/cgroup attestation, project identity binding and
installed restart checks are still required. Do not use this API to
replace those native checks.

### New 5C wheel member closure

`6789b9f`, `1a96ddf`: the fixed upstream `uv` PyPI wheel verifier
now requires a bounded, valid `.dist-info/RECORD` manifest, requires
one entry for every actual non-directory wheel member, recomputes each
member's SHA256/byte count, and denies duplicates, unlisted payloads,
bad RECORD self-hash or changed member hashes. Previously accepted
binary + license + metadata comparison alone could overlook a payload
unlisted inside the wheel. New negative tests regenerate a registry
checksum for the corrupted wheel to demonstrate this additional
cross-check. This is still **not** a publisher signature, original
license legal clearance, source reproducibility, CVE audit, or a
complete transitive project dependency provenance chain.

### 5B, 5D, 5F acceptance boundary

The existing grouped workflow already contains Linux actual cgroup-v2
and Bubblewrap offline-miss, cancel, synthetic positive install and
SIGABRT/restart fixtures, plus an explicitly feature-gated Windows
AppContainer kernel witness with source read, write/rename/delete
denial, private output write and Job Object custody. These are
**candidate tests**, not newly observed results. Windows production
`uv_sync_offline` remains fail-closed.

Physical installed Windows NSIS and Linux Debian human approve,
decline, timeout, stale-plan, cancel, restart/uninstall and retained
project-data tests have **not been performed** in this GitHub-only
session. A real operator must execute them on pinned installed images;
CI-only modal syntax, simulated consent, or the physical Windows
denial witness are not positive human approval.

**Next consolidated action:** use the existing
`.github/workflows/ide-dev05-consolidated-native.yml` at one exact
reconciled Group 5 SHA and inspect *actual* run/job logs. Do not start
independent micro-lanes, promote CI synthetic wheels, relax ACLs or
mark installation accepted without the evidence.
