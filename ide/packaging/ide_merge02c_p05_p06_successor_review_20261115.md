# IDE-MERGE-02C — P05 historical workflows and P06 scoped installed verification

**Review date:** November 15, 2026. **Scope:** source comparison only; unpublished follow-up to prepared `7e3ea2c6` on existing draft PR #236. No original acceptance gate, release approval, native custody, installed GUI result, or merge into `main` is asserted.

## P05 independent Git blob comparison

Fetched all 38 actual workflow Git blobs from the exact candidate tree and compared each against the **unchanged** historical P05 coverage manifest (Git blob `5299b1ca6981f341d2855f549e567b8c2747dc13`, snapshot `bc4fd78685c342675e4a80556970e934125a0e20`). **31 are byte-identical. Seven differ.** Reconstructed the original source of all seven at that snapshot and performed line-by-line deletion/addition comparisons:

| Workflow | Changed source behavior | Remaining authorization |
|---|---|---|
| D7 Windows worker | Removed `push` path trigger, retained PR/manual and original native source steps | Independent original-trigger reduction decision required, original historical P05 red |
| D7B Windows acceptance | Three extra PR paths and a genuine Cargo-produced AppContainer executable receipt step; no original steps deleted | Original OS worker acceptance not yet passed at new source |
| D7D protected service | Feature-correct `dap-transport` Cargo check/test plus exit-code and provenance checks | Actual SCM kernel custody not certified |
| Supervisor core | Removed `push` paths, added separate opt-in DAP audit test, retained default core test | Independent trigger reduction decision required |
| WO6 native OS/DAP | Windows image 2025→2022 for fixed MSVC/SDK; exact-head checkout, symbolizer, project debugger, Linux pinned LLDB additions | Toolchain identity, physical custody and genuine debugger outcomes open; original Windows 2025 semantic equivalence not certified |
| WO8 installed Windows | Existing current UI/Jobs denials and upstream notices; **two formerly bare P06 lock checks are now explicitly scoped to `--ide-merge02-successor`** | Original baseline remains denied; actual unsigned native NSIS/WebView2 not yet run against this candidate |
| WO8 signed protected | Manual exact SHA/artifact/environment retained; extra LLDB bundle hash, running-process leak and project-debug/restart checks | No signed release or independent legal/human authorization, cancellation/crash separately open |

The new **`audit_p05_reviewed_workflow_successor.py`** pins each of the seven exact **old and new** Git blobs, requires the other 31 originals to remain exactly identical, verifies working-tree bytes as well as Git HEAD, and rejects unreviewed eighth-workflow changes, changed original hashes, workflow deletion, or approval promotion. Its only successful result is **source identities reconciled / historical P05 STILL FAILED**. It does not modify `verify_p05_pr_inventory.py`, the original immutable 38-Git-blob manifest, original hostile tests, trigger policy (`workflowTriggerReductionAuthorized=false`) or release flags.

## P06 corrected installed Windows source path

The original P06 immutable graph **must** deny the one-local-`rand` changed desktop Cargo.lock. The reviewed MERGE-02 successor is independently bound to whole Cargo SHA256 `b3b8c5bf056163c2747c28473b583b996b520917eb63e9a1b4eeb05437abca5a`, local desktop block `a5e4a1867eeb99c821b9e0a144aae01fe2895c49717b37756f7519f5b72594e3`, all remaining blocks `73afb4fa1fcfc63f2e30b15ecdb06570c55804088dcfa4bfbac78e9f41240cb4`, 500 packages, zero changed external dependencies, and both OS lock checks; release flags remain false.

Original WO8 installed Windows added the immutable P06 checker but **omitted** `--ide-merge02-successor` twice. That would deterministically deny current reviewed Cargo.lock even after the corrected digest. The new reviewed source repairs both pre/post-install checks without removing `cargo fetch --locked`, exact source checkout, npm `ci`, upstream notice/artifact receipts, native UIA tests or `WO8_NATIVE_PROTECTED_FEATURES=NOT_ACCEPTED`. A source-only negative regression rejects a bare predecessor verification, an omitted second check, unlocked Cargo, or missing upstream notice gate.

## Required unchanged-SHA acceptance (still open)

Complete the prior Windows LLVM18 job and review its original terminal diagnostics. Only then group the prepared source repairs into the same existing #236 branch by non-force expected-head CAS and inspect exact-head Windows/Linux native, lock, P05 successor source-only witness, original P05 historical denial, P06 original dependency+license evidence and real installed NSIS/WebView2/GTK tests. Source-only audit **cannot make original P05 pass**; independent trigger/workflow review or preserving distinct original-gate proof is still required. Physical Windows/R9700, real SCM/AppContainer/Job/WFP and Linux delegated kernel custody, legal/signing and final `main` authorization remain separate.
