# Lu IDE DEV-02.3 — independent language-server review and Python semantic selection

**Status: candidate intake/source review, NOT approved, NOT launched.** This group is stacked on DEV-02.2 PR #131 and does not give the IDE, user or AI any new executable right.

## Physical native-provider assets required for genuine installed acceptance

The project's existing `ide/packaging/language-servers.manifest.json` still has `releaseApproved=false`, all native providers in `planned` mode, and **no approved executable asset hashes**. The two installed acceptance workflows require *real* offline source-provenance and distributor evidence. We must not turn this field true by assertion, infer approval from permissive licensing, fetch arbitrary `latest` binaries into a release, or bootstrap a special uncontained worker merely to get green CI.

The new `ide/packaging/prepare_lsp_candidate_review.py` consumes a candidate spec and operator-supplied offline binaries/notices for the three existing providers: rust-analyzer, clangd, Ruff. It strictly rejects unrecognized providers, missing Windows/Linux binary, traversal/symlink/special file/empty or oversized input and absent original notices; it **hashes each real binary and license notice** into a deterministic immutable review packet. All returned approval/containment flags remain **false** and the packet is NOT a substitute for signed release provenance. Exact final binary digests and complete dynamic dependencies remain to be verified by an independent reviewer.

Minimal *example schema* for external input, with placeholder file paths and exact owner-observed upstream versions (not a factual candidate release):

```json
{
  "schema": "lu.ide.e2.lsp-offline-intake.v1",
  "servers": [
    {
      "id": "clangd",
      "releaseVersion": "REPLACE_WITH_OBSERVED_VERSION",
      "assetByPlatform": {
        "windows-x86_64": "clangd/windows-x86_64/clangd.exe",
        "linux-x86_64": "clangd/linux-x86_64/clangd"
      },
      "licenseNoticeFiles": ["clangd/NOTICES/LICENSE.txt"]
    },
    {
      "id": "rust-analyzer",
      "releaseVersion": "REPLACE_WITH_OBSERVED_VERSION",
      "assetByPlatform": {
        "windows-x86_64": "rust-analyzer/windows-x86_64/rust-analyzer.exe",
        "linux-x86_64": "rust-analyzer/linux-x86_64/rust-analyzer"
      },
      "licenseNoticeFiles": ["rust-analyzer/NOTICES/LICENSE.txt"]
    },
    {
      "id": "ruff",
      "releaseVersion": "REPLACE_WITH_OBSERVED_VERSION",
      "assetByPlatform": {
        "windows-x86_64": "ruff/windows-x86_64/ruff.exe",
        "linux-x86_64": "ruff/linux-x86_64/ruff"
      },
      "licenseNoticeFiles": ["ruff/NOTICES/LICENSE.txt"]
    }
  ]
}
```

Run locally with a deliberate user-approved source folder (no Docker, no network, no shell from a project):

```bash
python ide/packaging/prepare_lsp_candidate_review.py \
  --bundle-root /path/to/reviewed-assets \
  --candidate-spec /path/to/candidate-spec.json \
  --out /path/to/NEW-lsp-review-packet.json
```

The file produced is **evidence for human review only**. The existing release verifier still rejects the shipped manifest. Its tests cover per-platform SHA, original notices, no-release approval, duplicate/foreign provider, path traversal, symlinked parents and missing assets.

## Python semantic alternative research (do not confuse Ruff with Pyright)

| Candidate | Language-server capabilities | Runtime consequences | Decision |
| --- | --- | --- | --- |
| Microsoft Pyright | Type checking, completion, hover, definition and editor navigation | A pinned Node runtime and audited npm transitive dependency/legal inventory must be bundled into the *same* Lu installer; native worker needs a fixed `node server.js --stdio` recipe | **Deferred**, original manifest node provider |
| BasedPyright | Compatible Pyright-family language server, semantic completion/hover/navigation | Its npm or PyPI distribution still requires exact resolved runtime/Node executable, any wheel bootstrap, transitive notices and an OS-specific native read-only worker recipe | **Review candidate**, not installed |
| Astral `ty` | Rust-native language server, completion, hover, definitions, auto imports and diagnostics | Could fit Lu's single-installer native bundle model without a bundled Node runtime; **beta 0.0.x** and potentially changing type/diagnostic behavior require comparative acceptance vs BasedPyright | **Promising alternative**, not approved |
| Ruff | Linting and diagnostics from the current reviewed fixed native recipe | No claim that Ruff performs Pyright semantic intelligence | **Existing diagnostic-only implementation** |

Official sources as of November 8, 2026: https://github.com/microsoft/pyright/blob/main/docs/installation.md ; https://docs.basedpyright.com/latest/installation/ides/ ; https://docs.astral.sh/ty/editors/ ; https://github.com/astral-sh/ty . Third-party documentation does **not** itself establish the distributor's authority to package an executable or its dependencies.

**Decision criteria before activation**: exact reproducible Windows/Linux asset and full license graph, real in-snapshot LSP initialize/diagnostics/completion/hover/definition from a clean project, finite memory+time limits, malicious URI and cross-project denial, no network/process spawn within the protected OS worker, bounded shutdown/cancellation, and genuine installed WebView interaction. Compare completion precision, hover relevance, definition accuracy and latency for the same 20 Python fixtures. Native `ty` is technically attractive, but is not yet selected as a new Lu execution provider.

## Prerequisites for actual installed OS acceptance

- **Windows**: pre-provisioned protected self-hosted Windows runner with administrator/service rights; independently verified authenticode **signed** LocalService broker with pinned SHA, reviewed binaries for all three native engines (plus matching LLVM DLLs) and legal notices; user-reviewed release manifest; exact OS service installation/uninstall test. Do not infer commercial publisher identity from unsigned NSIS success.
- **Linux**: self-hosted runner under the exact delegated `lu-ide-delegated/lu-ide-host` cgroup hierarchy, root-owned bundle installed outside project home, root-owned `/usr/bin/bwrap`, enabled cgroup-v2 process/memory/CPU controllers and usable GTK/AT-SPI; reviewer-approved binaries and notices; real Debian install/process isolation/native GUI/uninstall.
- **Both**: CI executes the full exact-head native Tauri source matrix, checksum locks, hostile source tests, genuine installed LSP protocol and physical source integrity tests, then captures immutable receipts. Owner legal release approval and future signing identity remain separate, explicitly user-deferred tasks.

**No protected self-hosted runner availability has been independently verified, none was provisioned or executed by this task, and no real provider binaries were supplied to it. Installed LSP acceptance remains open.**

## Pinned upstream candidate discovery — official release SHA (not approval)

To avoid repeated rediscovery of upstream packages, the first-party `ide/packaging/lsp_upstream_archive_candidates_20261108.json` retains the exact official 2026-11-08 lookup results for **six x86-64 archive assets** (Windows + Linux) from these GitHub release pages:

- `clangd/clangd` `23.1.0`: Windows `clangd-windows-23.1.0.zip` and Linux `clangd-linux-23.1.0.zip`, with original GitHub release SHA256 fields.
- `rust-lang/rust-analyzer` `2026-11-02`: `rust-analyzer-x86_64-pc-windows-msvc.zip` and `rust-analyzer-x86_64-unknown-linux-gnu.gz`.
- `astral-sh/ruff` `0.17.4`: `ruff-x86_64-pc-windows-msvc.zip` and `ruff-x86_64-unknown-linux-gnu.tar.gz`.

`review_upstream_lsp_archives.py` can verify these exact archive SHA256s offline or fetch only the pinned release URLs with an explicit `--fetch-locked` on GitHub Actions, inspect archives without extracting arbitrary paths, stream only one bounded native binary per archive, compute extracted binary SHA256 and delete binary staging afterward. `.github/workflows/ide-dev02-upstream-native-asset-intake.yml` runs malicious-archive tests on Windows/Ubuntu, then a real Ubuntu hosted run downloads and verifies the six pinned archives, uploads **only a JSON digest receipt** with 7-day retention (no public redistribution of executable files). **The official release metadata is evidence of byte identity, not independent review of upstream copyright, redistribution license, transitive dynamic dependency, OS containment, or service signing.** It cannot set the main manifest `releaseApproved` flag.
