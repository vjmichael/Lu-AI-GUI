# IDE-MERGE-02C frozen-source repair register — 2026-11-15

Published draft PR #236 is frozen at `cdef9ac364b6856e225fec5e7ebe17ce4d7ab91b` during its original 62-workflow Windows/Linux acceptance wave. **This follow-up commit is unreferenced and its repairs have not been tested in CI. No branch update, workflow restart, weakening of historical assertions or main merge.**

## Source corrections independently demonstrated by original runs

1. **LSP test fixture, Windows/Linux:** real clangd17 wire test panics in original job #133000942416 at `lsp_session.rs:916:32`, index 0 into empty events. Source review confirms `LspSession::request` clears prior pending semantic request; fixture requested hover then definition before consuming hover, making hover response stale. Correct fixture ordering to consume hover first and **add denial of replayed stale hover**. Keep actual single-inflight protocol and positive hover/definition/completion assertions untouched.
2. **FIN01 custody negative fixture, Windows:** original job #133000944672 expected live-owner refusal but got `FIN01_GLOBAL_MARKER_FORMAT_INVALID`. Its manually fabricated marker contained literal backslash-n instead of newline. Correct fixture to valid marker; no change to kernel proof, original denial or corrupted-marker tests.
3. **Rustfmt, Windows/Linux:** original jobs #133000944564 and #133000944779 report only three `cargo fmt --check` diffs in the new `quick_open.rs` original 864-file/1,005-entry native tests. Apply their exact formatting. Keep all original large-project/budget/path/symlink assertions.

## Separate historical gates (not a generic repair)

- P05 historical Git workflow blob assertion rejects `.github/workflows/ide-d7-windows-worker.yml` on both OSes as source demonstrably differs; preserve failure and independent trigger-policy authorization. Current P05 source-only successor cannot replace it.
- P06 exact-source Windows/Linux review **passed** in [run #43775561213](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/43775561213). Several historical installed jobs still stop *before installing* at the prior immutable desktop Cargo.lock refusal `DENIED: candidate_cargo_lock_not_reviewed`. Their historical checks stay strict; only explicitly reviewed scoped successor builds may use the newer local-desktop graph.
- Older B4/B5 performance-only/protected-source scopes reject the larger integrated tree; original historical denial valid, not evidence that current editor fails. Older standalone desktop containment regex also rejects the now-enabled authorized worker; historical scope must not be widened by removing the worker.
- Original Tauri receipt test jobs #133000942984/#133000943088 report `frontendDist="../dist" but this path doesn't exist`. Genuine locked frontend build must precede Cargo, never an invented dist or disabled check.
- Windows LSP native jobs repeatedly fail the same clangd test; original HLS/result checks are unaltered. Windows Tauri global custody tests fail only the malformed marker fixture described above.

## Pending exact-head acceptance

[LLVM18 Windows native run #43775561350](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/43775561350), [six-job integrated Windows/Linux+installed run #43775561446](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/43775561446), and [G06D installed NSIS/DEB run #43775561345](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/43775561345) were not terminal at this reconciliation snapshot. Real Windows SCM/AppContainer/Job/WFP, Linux kernel custody, installed WebView2/GTK/restart/uninstall, legal/signing and authorized main merge remain independent gates. The frozen CI SHA must remain unchanged until those original results have been inspected.
