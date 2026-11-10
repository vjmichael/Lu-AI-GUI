# PLAN-02 — Host-verified project evidence graph (2026-11-10)

**Scope:** Lu Core + desktop in existing `vjmichael/Lu-AI-GUI`, stacked on PLAN-01 PR #207. **No IDE work.** PLAN-03 classifier, PLAN-04 GGUF planner, other products and remote CI auth are not enabled.

## Graph model
New `src-tauri/lu-core/src/project_evidence_graph.rs`: schema `lu-project-evidence-v1`, exact host-selected local repository identity, project root fingerprint, source-hash and line-provenance. Typed nodes: requirement, document, component/source, integration path, test, run, acceptance, artifact, repository snapshot. Typed bounded edges: original document, exact text path mention, declared source/test link, reported run and outcome, claimed artifact, verified local Git tip ancestry.

States: `reported` (untrusted document or report assertion), `inspected` (host read bytes/hash or selected Git HEAD), `independently_verified` (ONLY host-checked Git ancestry), `stale` (report references missing/another commit), `unknown` (no trusted inspection or accepted verification). Even a pass on the current HEAD does not establish native or installed acceptance. Missing paths are unknown, not proof of missing features. No task classifier or model is involved.

## Host path and evidence inputs
`project_evidence_desktop.rs` requires a **new native Settings folder picker** and uses the bounded PLAN-01 scanner. It reads selected files via `cap_std::Dir` with symlink and parent traversal checks, uses capped hashes, and accepts only local typed JSON `lu-project-native-run-v1` metadata from `reports/`, `test-results/` or `evidence/`. Any unsupported JUnit/XML/TAP/log is explicitly partial and NEVER passed off as a verified run. Remote GitHub/CI remains unknown; adding a trusted external collector is a later reviewed work order. A metadata-only path does not claim original source contents.

Local branch ancestry is checked by the existing reviewed native Git wrapper (fixed `for-each-ref`, `merge-base`, no network/hooks/config effects), and separately identifies branches that are not contained in the selected HEAD. This is NOT an acceptance verdict and cannot establish that isolated PR feature work was merged into the app.

Each graph is canonically hashed and revalidated; append-only snapshots live in an additional table of the **existing ANSF `ansf/library.db`**, not a second DB. Transactional insertion does not replace historic evidence. Re-reading a changed graph digest, source hash, foreign project scope or untrusted acceptance assertion fails. The UI shows evidence nodes, explicit edges, current partial coverage and source digest. No model activation, background work, external fetch, or permission grant.

## Demonstrated PLAN-01 prerequisite defects
Previous exact-head native run #42919682209 failed on both OS: `E0282` ambiguous filename vector, `E0034` ambiguous `File::by_ref`, and a Tauri `frontendDist` build-stage ordering error. PLAN-02 repairs the first two in source, and builds frontend `dist` BEFORE Tauri cargo testing in grouped CI.

## Test/acceptance
Rust graph tests: full typed chain, forged-label digest tampering, stale SHA and skipped result, absent Git/CI, unsafe paths, no false verification. Desktop fixtures: strict JSON schema and unknown-key rejection, bounded source access, append-only preservation in existing ANSF database. Native Git acceptance: actual clean/dirty/detached repository plus two separately committed branch tips before/after merging, with HEAD-change invalidation. Node test asserts no WebView paths or agent/model execution. One exact-head Windows/Linux native job; installed UIA/physical approval remains open and is **not** inferred from compiled source.

## Next
PLAN-03 deterministic classification must consume this graph without allowing reports to manufacture verified evidence. PLAN-04 will only receive bounded evidence after host validation and is **not** part of this work group.
