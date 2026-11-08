# Lu IDE DEV-01.6 — bounded native large-project quick-open indexing

**Status:** source-implemented; Windows/Linux exact-head stress and installed large-project pilot OPEN.
**Dependency:** DEV-01.5 two-pane editor #125; this remains a separate stacked draft, not merged to Lu Core or release candidate.

## Architecture

The previous DEV-01.1 Quick Open performed sequential WebView-to-Tauri directory calls for at most 24 directories/500 filenames. DEV-01.6 introduces `IdeWorkspaceRegistry::index_project_files`: a single `spawn_blocking` Tauri request that enumerates file **names only**, through an already-open native project capability. No source content, script execution, model call, shell, ambient project path, or external service is involved.

The native worker enforces **128 directories / 4,000 files / 1,000 entries per directory / 20,000 entries total / 8-second elapsed budget**, returning a `truncated` result whenever work is incomplete. It excludes git metadata, temporary recovery/trash, dependency/build caches and symlinks; validates each relative path and never descends an unregistered root. Its response contains measured directory/entry/file counts and actual elapsed milliseconds. The frontend validates project ID, path structure and all count limits, rejects responses from an earlier project/Quick Open request, memoizes search matching and renders at most 30 matches. Quick Open explicitly displays partial results and native metrics rather than pretending to have completely searched a huge repository.

There is **no durable search cache** yet: indexing an active project again retrieves current names instead of trusting potentially stale path listings. Results are bounded and ephemeral. Existing native `ide_preview_source_file` still verifies and opens the actual selected file; an indexed filename never itself grants filesystem access.

## Source regression and performance evidence

- Real native stress test creates 864 source files across 24 subdirectories (beyond old 500-file cap) and emits the exact observed OS elapsed time and bounded counters. Additional tests inject an oversized directory, ignored source-control/build folders, Unix symlink escapes, fresh external changes and closed-project denial.
- Exact-head Windows2022/Ubuntu24 workflow runs `cargo fmt --check`, real Rust filesystem tests with visible timing output, full frontend build and Node scope tests, and the pinned Tauri type check.
- The elapsed-time metric is **observed evidence**, not a predeclared or fabricated improvement. Do not conclude it outperforms old behavior without a same-machine three-arm baseline (old WebView loop vs native bounded call vs indexed cache), identical projects and repeat counts.

## Remaining gaps

A single 8-second host deadline is advisory between synchronous filesystem calls, not preemptive kernel cancellation; in-flight old scans can continue after browser navigation until the bound expires. Large directories above 1,000 entries and repos above 128 directories/4,000 files intentionally yield partial search. A future native cursor/index supervisor and safe invalidation mechanism could extend complete indexing without putting the entire repository in browser memory.

Real **installed Windows/Linux GUI** large-repository test, indexing energy/memory and p95 latency, user typing responsiveness, external mutation race, cancellation and crash/restart acceptance remain open. Signer/legal/P07/BLK-05/06/previous source gates are separate.
