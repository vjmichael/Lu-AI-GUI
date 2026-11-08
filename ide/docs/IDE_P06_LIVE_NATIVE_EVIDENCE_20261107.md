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
