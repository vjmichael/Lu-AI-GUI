# CP-04B4: physical Windows acceptance handoff (2026-11-18)

## Disposition

Hosted run [44274413309](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/44274413309), exact commit `2ea78b23963830de90a2a9951d00c674dc13ef06`, remains **failed**: the strict HTML-provider live test returned `SEARCH_UNAVAILABLE`, `PUBLIC_SEARCH_NOT_COMPLETED`, HTTP 202. The diagnostic contract worked; neither the HTTP status nor the available evidence establishes the reason for the provider's response. Do not rewrite this as a successful search. Native routing (job 134698768545), real React build, browser provisioning, canonical worker and Chromium fixture gates passed. The browser job (134698769038) reports 20 passing tests and one failed live test in its final cohort. Google compatibility remains separately unverified. No automatic retries, alternate provider submission, provider challenge bypass or JavaScript/CA changes. No repeated hosted diagnostic run.

## Exact preserved inputs

Download the original ZIPs, not repackaged extracts. The browser is **unapproved** until separate review. The engine was genuinely compiled and exercised, but its original smoke receipt is standalone and not a browser or synthesis acceptance receipt.

| Input | Artifact | ZIP SHA-256 |
|---|---|---|
| Browser candidate (563,548,251 bytes) | [14414415865](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/44274413309/artifacts/14414415865) | `15756d62a692f73b5da8b9f2adf63e7fa6b9aafdc54f3b5cbc51735c6c0f942f` |
| Genuine Windows engine + Qwen model (475,202,779 bytes) | [14409181478](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/44261533230/artifacts/14409181478) | `f47145fe94b88188b0625fb6a043b6b154f9ce67d8e060ab00fc02b82c9d340b` |
| Native routing | [14414916154](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/44274413309/artifacts/14414916154) | `206847a1616906f7edd7a326af769ca9f1d77973d60f4e5a64b5c9252098ed56` |
| Provider failure evidence | [14413003905](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/44274413309/artifacts/14413003905) | `fd5ba0a64f7ce539482be8c8145e8568671db7ab1ccf00df1f51aa1d9ea84e39` |

Browser manifest `f511bfa8a1b01530a41915f64ae018b28575f3dd8fd45eb660f584895383f97f`, 482 payload files, Playwright 1.64.1, canonical worker SHA-256 `c9ea3d8a6a6fe5d9864b095e1276c4284c296dcaedf8692b57cc958ecbf63dd9`. Browser artifact exceeds the GitHub connector's 512 MiB transfer maximum; full downloaded archive has **not** been inspected locally. Use authenticated GitHub artifact download while retained (14 days from creation); no replacement of original evidence with newly assembled bytes. Engine archive and both payloads have been checked against original pins locally.

## Stage 1 — offline preparation only

Use a clean checkout of the CP-04 integration branch containing this handoff. Requirements: Python 3.10+, Node 22, Git, at least 4 GB free, both original ZIPs locally available, a fresh output directory outside the checkout. The normal source inventory check must pass. From the repository root in PowerShell, adjust only the three paths:

```powershell
python scripts/lu-cp04b4-prepare-physical.py --browser-archive 'C:\Downloads\browser.zip' --engine-archive 'C:\Downloads\engine.zip' --output 'C:\LuAcceptance\CP04B4'
```

The preparation utility checks exact archive and payload hashes, the source-current canonical worker, archive layout, browser candidate status and standalone inference receipt. It extracts into a newly created directory, rehashes copied payloads, and writes `preparation.json` with status `UNAPPROVED_BYTES_VERIFIED`. It does not start executables, make provider requests, change source/approval records, or certify provenance of upstream binaries. It refuses mismatches and existing destinations. Preserve the original ZIPs and the preparation receipt. Synthetic tests are not a substitute for checking the actual 482 browser payloads.

## Stage 2 — independent exact-byte review

Review the actual transport and browser payload. Establish the correct original source/run/artifact identity, original ZIP digest, file manifest digest, file count and proposed reviewed manifest digest. Do not use the historical C6.2 approval (`d9af093e`) for this package. A separate source review/approval must update `docs/acceptance/LU_C62_WINDOWS_BROWSER_REVIEW.json` and regenerate the source inventory as a distinct reviewed commit. Existing promotion and staging checks require the exact independently approved digests. No CI pass, successful preparation receipt, or proposed review digest is itself approval. If review cannot be performed, stop here.

## Stage 3 — one explicit protected native live run

On ordinary network-capable native Windows x64 with C++/Rust build prerequisites and a clean inference resource directory, use the reviewed checkout and previously verified package. The native test uses a fixed public query (`duckduckgo Rust programming language`) and test-scoped operator authorization, not the interactive Tauri dialog. The following command explicitly opts in to sending that public query to DuckDuckGo. No switching providers or automatic retries. The script calls the existing production-native test harness, not a fake browser or GGUF server.

```powershell
pwsh -File scripts/lu-cp04b3-run-reviewed-native.ps1 `
  -CandidateDir 'C:\LuAcceptance\CP04B4\browser-candidate' `
  -EnginePath 'C:\LuAcceptance\CP04B4\engine\llama-server.exe' `
  -EngineSha256 '48a3b2a09e5bc732d97f061a7de04a556b6dc26b9bbd30a995eb4636d52ebd53' `
  -ModelPath 'C:\LuAcceptance\CP04B4\engine\Qwen3-0.6B.Q4_K_M.gguf' `
  -ModelSha256 '7af3fdf842f87b24672f8a7f1dd50404043f0bfb71093ff91c31d2b49df4631d' `
  -EvidencePath 'C:\LuAcceptance\CP04B4\native-live.log' -Provider duckduckgo
```

Acceptance requires the exact provider-origin receipt, verified result observation and cleanup, genuine model generation and successful citation audit, plus `test result: ok. 1 passed; 0 failed;`. Preserve failures and terminal output as well. In particular, an ongoing HTTP 202 is an unresolved provider dependency, not evidence of result extraction failure and not a reason to keep reissuing requests. If blocked, defer live-search acceptance and continue unrelated local Core work if approved. User-facing native consent UI and physical installed R9700 acceptance remain separate. No installer, service-access guarantee, automatic review, or protected end-to-end acceptance is claimed by this handoff.
