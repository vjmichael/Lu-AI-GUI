# IDE-PRECORE-CLOSEOUT-R3 — signed Windows RC identity / custody handoff

**2026-11-21. Status: SOURCE-ONLY PREPARATION; native signed installation, protected worker and physical acceptance NOT RUN.** The existing [draft PR #236](https://github.com/vjmichael/Lu-AI-GUI/pull/236) is the single IDE integration line at exact observed `f1f0ed4bc1e740d2a9f85615b988b779a46651db`; no new PR, branch, GitHub CI dispatch, promotion, merge or Lu Core modification. Issue #245's live Health dispatcher is an external dependency, not part of this pre-Core closeout.

## 1. Accepted evidence versus current gaps

- **R2 reviewed source:** `IDE_PRECORE_CLOSEOUT_R2_cumulative_ide_only.patch` SHA-256 `b5a4c4dffb4c85b1603039940b9f42c2a018c8b4e5b1f6388bc45f525cad246d`, verified against the recovered R2 receipt. R2 reports 437 passing packaging tests, two expected skips, 73 passing installed-package preflight tests, locked/offline Rust 1.99.1 check PASS. Those are local tests and synthetic installed-package preflight, **not** Windows signed release or physical process custody.
- **R2 lock successors:** host `f9fc6178178bf2d56c6ca308472b9ba2e35a27910e3a30f6b72e9339bb4d10b2`, desktop `77e317747fdf2743f55363c9809f866a770255`; historical MERGE02C original remains immutable. No registry dependency change was accepted in R2.
- **WO7 checked-in source:** verified exact Git blobs on PR #236: `wo7-release.manifest.json` `bd2f9334717e44956f4da36da93605ed25384ed6`; `wo7-bundle.inputs.json` `7927440108a4bfa239ce022dafcec40f2bb1824b`; `wo7-evidence.inputs.json` `b73a54fe08bf2099183d218f75c35ed99681abe2`. All remain explicitly unapproved, with no real signer/tool/certificate or binary SHA pins.
- **WO8/WO9 checked-in source:** verified blob `wo8-installed-evidence.json` `87439190cfd880c2c3c48a642f8b0e8c03b30924`; `wo9-release-evidence.json` `54771c7c46f4468fa78dd22a0e7a8a11f274efcc`. Both contain empty observations. Their source gates must not be upgraded to native installed evidence by copying older CI results or populating JSON flags.
- **FIN01D source:** protected terminal requires original Job empty and independent SCM signed stop/release ACK after durable `ReleasePending`; cancel is first journaled, terminal cancellation requires verified descendant cleanup and independent release; uncertain outcomes stay quarantined across restart. The source document describes these semantics. Actual current installed Windows `Job`/SCM/Cancel/Revoke/Restart receipts are **not** in the reviewed records.
- **Windows app identity:** checked-in default standalone desktop `org.lu.ide`, version `0.1.0-alpha.1`, `bundle.active=false`; protected features opt-in and default off. A reviewed signed WO7 NSIS overlay and protected worker service require a distinct explicit candidate.

## 2. R3 implementation

`ide/packaging/verify_ide_precore_r3.py` reconciles the original R2 patch bytes, base/head/lock identities, current manifest schemas and source SHA, checked-out Tauri/Cargo identity and default-off features (once the successor is committed), and signed candidate byte pins (when supplied). It rejects changed release flags, missing or wrong source lineage, cross-manifest drift, mixed roles, unsafe paths, malformed signer data, duplicate protected receipt hashes and source or lock drift. It emits an explicit blocker register and **never** turns metadata consistency into signed Windows, kernel Job or release approval. `--require-admission` exits nonzero while independent native acceptance remains open. No automatic signing or certificate selection.

`Test-IdePrecoreR3WindowsCandidate.ps1` is a read-only physical Windows probe. Against a **clean exact candidate commit**, it checks R2 Cargo.lock bytes, standalone desktop identity/default-off package, a separately produced WO7 candidate manifest, three real SHA-pinned Authenticode signatures and optionally the exact installed per-machine `LuIdeRecoveryBroker` LocalService account, restricted service SID, signed installed service/desktop bytes and non-user-writable install ACLs. It does **not** install, launch, approve, cancel or recover workers. Even if its read-only observations pass, the output explicitly states original Job/SCM custody terminal receipts, cancellation/revocation/restart and full physical/release acceptance remain unverified. The POSIX test environment does not have PowerShell/Windows, so this script has not been parsed or run natively.

`ide-precore-r3-admission-register.json` is a safe checked-in **template**. Its `candidateSha` intentionally remains `null`: including a successor SHA inside the source commit whose hash it names would cause a self-reference. For actual candidate preparation, put a separately reviewed copy of the register and actual WO7 signed manifest **outside** the clean checkout and pin the final published exact SHA there. Do not edit approval fields to silence a blocker.

## 3. Exact physical Windows acceptance sequence (still open)

1. Reconcile R2 and R3 IDE-only source against the *same* existing PR #236 review line. Commit once as a coherent successor, preserve main-ledger ancestry/source-tree lock, do not force-update PR #236 or stack CI. Recheck its actual new SHA, then independently qualify that SHA in original Windows/Linux source suites.
2. Have a real release reviewer approve exact native desktop, LocalService service, debugger/LSP/runtime import closure and redistributed dependency licenses, pinned SHA-256 and publisher certificate; obtain a real signing tool and timestamp authority. Never create a test certificate or synthesize signing inputs for production.
3. Produce a distinct candidate with existing reviewed `stage_wo7_windows_release.py`, `Build-Wo7ReviewedNsis.ps1`, real `Sign-Wo7ReviewedArtifact.ps1`; run `verify_wo7_candidate.py` (candidate-only), then the R3 read-only Windows probe on the actual machine. The checked-in unapproved manifests remain unchanged.
4. On the **physical Windows 10 Home owner system**, with actual signed package identity, independently observe install/uninstall, SCM service principal/image/DACL, original Windows Job membership and empty-Job before terminal receipt, permitted one-use approval, cancellation/revocation, forced service/worker loss, restart and original journal recovery, tamper refusal, versioned upgrade/rollback and power-loss recovery. The read-only probe does not itself perform those actions; original supervised and human-approved native acceptance owns them. Failed or uncertain operations must remain quarantined, not automatically retried or recorded successful.
5. Correlate each genuine signed operating-system and original-worker receipt with **same exact source SHA, installer/service/desktop SHA, original operation ID, immutable external artifact and native witness**, then independently inspect run logs and evidence. Only a separate authorized release review can mark any gate accepted. Existing `WO7`/`WO8`/`WO9` gate checkers remain active.

An unsigned personal-alpha install may be tested separately with protected features disabled; it is **not** a signed protected release candidate and cannot satisfy the requirements above. Keep Lu Core #245 and IDE-side IPC positive Health admission separate from this pre-Core packaging task.

## 4. Reproducible source-only checks

From the recovered R2 patch/receipt and byte-exact original WO7/WO8/WO9 manifests, run:

```sh
python -m unittest discover -s ide/packaging -p 'test_verify_ide_precore_r3.py' -v
python ide/packaging/verify_ide_precore_r3.py \
  --register ide/packaging/ide-precore-r3-admission-register.json \
  --r2-receipt /independent/R2_verification.json \
  --r2-patch /independent/R2_cumulative_ide_only.patch \
  --wo7 /independent/wo7-release.manifest.json \
  --bundle /independent/wo7-bundle.inputs.json \
  --wo7-evidence /independent/wo7-evidence.inputs.json \
  --wo8-evidence /independent/wo8-installed-evidence.json \
  --wo9-evidence /independent/wo9-release-evidence.json
```

Optional `--checkout` and `--candidate-root` enable read-only local source and candidate **byte** measurement after a reviewed successor is committed. Absence of these arguments is reported as a blocker, never silently counted as native acceptance.

**Result of this R3 pass:** 24/24 Python hostile tests PASS. The real unapproved original manifests reconcile with R2 and report `blocked=true`, `physicalAdmissionApproved=false`, `protectedExecutionAccepted=false`, `releaseApproved=false`. Windows PowerShell parsing, genuine signing, physical Windows native probe, service execution, worker Job and installed recovery **NOT RUN**. No GitHub source or CI publication.

## R4 audit correction — immutable R2 receipt discrepancy

The R2 verification receipt's `desktop_lock_successor.sha256` contains only 38
characters (`77e317747fdf2743f55363c9809f866a770255`) and is not a SHA-256.
The reviewed R2 source lock and its verifier contain the full 64-character pin
`77e317747fdf2743f55363c9809f7baeca40d949675ad547d6909f866a770255`.
R4 keeps the original receipt unchanged, requires the source-owned lock review
and measured Cargo.lock bytes to match the full digest on a clean exact checkout,
and leaves a blocker if only the malformed historical receipt is available.
This source correction does not prove signing, SCM/Job custody or release.
