# Lu IDE P06-R1 — exact-head live GitHub Actions native evidence (read-only)

**Branch:** `ide/p06-live-native-receipts-20261107`, deliberately stacked on P06 draft PR #95.  
**Authority:** read-only GitHub Actions inspection. No branch rulesets, release approval, native execution grant or user-selected job.

## Why this exists

The original `p06-native-acceptance-matrix.json` is a frozen, **default-denied source record**. It pins historic source coverage but contains no fresh kernel/GUI measurements. A source document must not be able to set `protectedExecutionAccepted` to `true` or claim that historical green CI proved the current release candidate.

`p06_live_native_jobs.py` reads actual public GitHub Actions workflow runs and job statuses for **one full source SHA** from `vjmichael/Lu-AI-GUI`. It requires the exact workflow path, repository and head repository, same 40-character commit SHA, recognized push/PR/manual event, latest run for each workflow, and every expected native job. It classifies queued, missing, interrupted, cancelled, red, incomplete, cross-repo and stale runs without borrowing historic green checks. Empty or untrusted API results fail closed. A rerun is observed as a separate newer attempt; an older successful run cannot rescue a newer failed attempt.

The offline tests use fabricated scenarios **only to test the verifier**; production observations are fetched from GitHub's official Actions API, and the resulting JSON is an *unapproved diagnostic artifact*, not a user-editable release manifest.

## Observed gates (not authorization)

| Observation | Required native jobs | Meaning |
| --- | --- | --- |
| P02_source | Ubuntu 24.04 and Windows 2022 exact-source | Locked frontend/host/Tauri checks, not a genuine installer |
| P03_debugger | Windows protected LLDB; Linux delegated LLDB; Linux protected Cargo | Actual protected OS workflows still independently required |
| P04_windows_installed | Windows installed UI | Unsigned native installer smoke only |
| P04_linux_installed | Ubuntu installed Debian/GTK | Unsigned native installer smoke only |
| P06_dependency | Ubuntu 24.04 and Windows 2022 dependency/license evidence | Native-resolved source inventories, not legal approval |

**All rows must be from the exact same commit to display `allObservedGatesSuccessful`.** That Boolean describes only the observed job subset. The report always keeps `releaseApproved=false`, `signingIdentityReviewed=false`, `licenseLegalReviewApproved=false`, `protectedExecutionAccepted=false`, `nativeInstalledReleaseAccepted=false`, and `gatePromotionAuthority=none_read_only_snapshot`.

P05 B2/B3 are still separate, divergent draft branches: do not report their source/native results as though integrated at the P06 source SHA, and do not reconcile B3 before the new B2 installed Save/restart acceptance passes. P03 and P04 phase gates remain independently open. P07/WO10 Lu Core interface remains deferred until Lu Core v1.0 and explicit user approval.

## Execution

```bash
python -m unittest discover -s ide/packaging -p 'test_p06_live_native_jobs.py' -v
python ide/packaging/p06_live_native_jobs.py \
  --sha FULL_FORTY_CHARACTER_SOURCE_SHA \
  --output ide/p06-observed-native-actions.json
```

The standalone `.github/workflows/ide-p06-live-native-evidence.yml` runs the same tests and reports actual public OS job statuses. It uses `actions: read`, `contents: read`, an exact-head checkout, a short network timeout, bounded API pagination, no code execution capability, and a 14-day unapproved artifact. A report of `missing` is expected when native jobs were never run on this exact candidate; it is **not** a CI failure to hide or an OS pass. API errors deny the report instead of publishing stale evidence.

**Important limits:** This does not attest the actual installed binary checksum, native process tree/Job membership, DAP stop/stack/cancel behavior, signed two-version upgrades, license obligations, GitHub branch protection, a physical Windows environment or humans approving the release. Those remain separate required gates, not items to mark true here.

**Project state:** P06 source instrumentation implemented; native CI results pending. No existing PR, legacy workflow or security job may be removed or silently retargeted.


## Verified earlier native provenance ZIP inspection and byte-exact repair (Nov 8 UTC)

Downloaded **both actual GitHub Actions artifacts** from historic completed P06 [run 42427078307](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42427078307), exact source `55f70b66bdbdfded57ee416011881eb9965904f6`: archive IDs `13596405657` (Windows 2022) and `13598643673` (Ubuntu 24.04). Both OS jobs finished **success** on that old commit. Both ZIPs contain 918 paths; **500 Cargo packages, 62 npm lockfile package entries, 521 license-inventory package rows**, and independently archived notices. Both lockfile raw SHA-256s were equal on both OS: Cargo `d4312b60209f89181766b60c2982fb8b199e8ed9d6be1a9dcbdcb3dd07abf6b4`; npm `3c137aede8749007c062b80e53bef0c83241b73d21e4c692ae1718882e81a316`. The resolved npm semantic digest `9a61ac00a82c8f8ca57ce6e56b2e9eb355fd66a0d9af6081b199ddf0573e97de` and Cargo lock were the same in both parity and locked-rerun receipts.

The actual target metadata intentionally differs: Linux `x86_64-unknown-linux-gnu` resolved 348 target packages; Windows `x86_64-pc-windows-msvc` resolved 308. Each native review queue reported **zero missing license-text records** but **44 independently open legal cases across five nonoverlapping review categories**: seven special-license reviews, 19 Apple SDK obligations, nine embedded compiler payloads, seven external native libraries/SDKs, three WebView2 runtime obligations. (These category counts sum to 45: 7+19+9+7+3=45; do not conflate the review count with legal violations or missing notices.) Both release/legal approval fields remained false.

**Concrete discrepancy:** The Windows ZIP contained all six complete upstream core notice files converted from LF to CRLF by Git checkout, whereas Ubuntu preserved the original LF bytes. Each Windows `p06-upstream-receipt.json` recorded `matchesUpstream=true` under a *semantic* newline normalization but `bundledSourceSha256 != sourceSha256`. For example, the LLVM notice Windows local digest `a9ed3eae930adbc631b83300b78ab5d228c9b1e4560d64ec4b4ea2bcf69f4569` differs from upstream `8d85c1057d742e597985c7d4e6320b015a9139385cff4cbae06ffc0ebe89afee`; converting its 279 CRLF sequences back to LF reproduces upstream exactly. This was **not a legal text change or a binary checksum failure**, but the old "exact upstream bytes" claim was too strong.

**Source repair:** Added `.gitattributes` LF enforcement for `ide/packaging/upstream-licenses/*.txt`. Changed `upstream_notice_receipt.py` to require byte-for-byte original equality and reject CRLF normalization, `collect_verification_licenses.py` and `verify_p04_license_bundle.py` to require BOTH remote and bundled SHA-256 to equal the staged notice SHA, with hostile tests rejecting forged remote digests and CRLF. The P06 Windows/Linux native matrix now verifies all checked-out six original Git blob bytes and `eol=lf` before network retrieval and bundling. **These repairs require a NEW completed same-head native matrix**; the historical success is retained as diagnostic evidence, not retroactively reclassified as byte-exact Windows acceptance. Release/installed/native worker/signer/legal gates remain default-denied.
