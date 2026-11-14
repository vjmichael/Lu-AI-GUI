# IDE-05.4 — Local installed-artifact evidence, provenance limits and durable private rollback

**Implementation checkpoint: November 9, 2026.** Stacked after IDE-05.3 commit
`83ab98c61b214d371fe665c9eb5c050892ded1d3` on
`ide/dev05-1-package-discovery-20261109`.

**Status: SOURCE IMPLEMENTED, NOT NATIVELY ACCEPTED.** There is no claim of
real Linux uv installation, Windows package execution, downloaded wheel,
human operator acceptance, signed release, or 7B+ local-model installation.
The same owner CI policy applies: run a consolidated exact-head Windows/Linux
acceptance at the **end of the IDE-05 task group**, not one workflow per edit.

## Verified semantics versus claims

IDE-05.3 freezes `pyproject.toml` and `uv.lock`, records an exact D3
JobPlan, and runs (on a properly provisioned Linux native installation)
a fixed offline uv Sync under cgroup-v2/Bubblewrap. IDE-05.4 now has a
**second, native, read-only verifier** that cannot be driven by a WebView
path, execute Python, import modules, call uv, download artifacts, or
promote the private environment.

The verifier tests a *full* output tree (bounded 6,000 files / 256 MiB),
not the 96 visible entries emitted by the normal WO6 GUI artifact list.
The original WO6 auditor also uses no-follow file opens against symlink
swaps. The new private package audit:

1. Refuses symlinks/junctions, nonregular files, unsafe names, unexpected
   path traversal, excess tree size/depth, metadata races and duplicate
   installed-file ownership. Reads using native no-follow handles and
   rehashes the complete output tree after inspecting package metadata.
2. Finds installed `.venv/lib/python3.x/site-packages/*.dist-info` and
   rehashes bounded `METADATA`, `WHEEL`, and `RECORD` files.
   It verifies the PEP 376 comma/quote-aware RECORD entries against the
   **actual installed file bytes**, using URL-safe base64-encoded SHA-256
   and exact file sizes; unhashed RECORD self-entry and optional bytecode
   are treated separately. Every other installed site-packages file must
   be claimed by exactly one RECORD, never by an untrusted annotation.
3. Requires a matching `Name` and `Version` in a hash-checked
   `uv.lock` registry package with a declared SHA-256 wheel hash.
   Direct URL, VCS, local directory/editable origin and unexpected
   installed files are refused. Importantly, the **frozen lockfile is
   checked before execution** against these rules, not only after.
   First-party virtual workspace root packages may be omitted because
   the fixed worker uses `--no-install-workspace`.
4. Requires a bounded `License-Expression` and corresponding
   `License-File` declarations with actual, hashed license text in
   `.dist-info/licenses/`. A license string alone or an absent license
   file is NOT sufficient; this checks file evidence, **not legal
   interpretation of license obligations**.
5. Rehashes the frozen manifest/lock with the originally reviewed
   digests. The native host separately rechecks **the registered original
   project** through IDE-05.2 revalidation and the entire frozen input
   after the worker finishes. The returned
   `postrunOriginalManifestLockUnchanged` flag is true only for a
   successful fresh original-project/source-snapshot recheck.
6. Requires an original D3 terminal receipt, non-quarantined Completed,
   native exit-zero, and matching WO6 output-tree SHA before publishing
   local artifact evidence. Failure means **blocked private output**,
   not installed or accepted. The reviewer never reuses a Cargo-only
   build/test receipt to bless package output.

**Crucial source/provenance distinction:** An installed RECORD validates
local bytes *relative to the installer-produced RECORD*; it is not an
independent trusted wheel archive or proof of a PyPI download. The
lockfile's registry URL and wheel SHA-256 are source **declarations**.
Until the exact offline wheel archive is independently hashed and
matched to the approved hash and a trusted source inventory, both
`wheelArchivesVerifiedAgainstLock=false` and
`sourceProvenanceVerified=false` are returned. Therefore
`approvedForProjectPromotion=false` is unconditional.
No registry/network availability, advisory closure, license clearance,
reproducible packaging or accepted app installation is implied.

## Fixed admission restrictions

The Linux uv worker remains opt-in via `native-package-uv` and
`package-uv-worker`. Its fixed arguments are offline + locked, no build,
no workspace project install, no default groups, no dev group, no
`tool.uv.sources` substitution, no external Python downloads, no
configuration discovery and **copy** link mode. The bundle is pinned by
installer SHA, source is read-only, the environment/cache are private
under /output, the network is unshared and the original D3 one-use
native human consent and cgroup process-tree cancellation remain required.
An offline cache miss or invalid wheel must FAIL; it is not allowed
to opportunistically contact a registry or fall back to host Python.

The fixed Windows uv recipe still denies `NoContainment` until
read-only source ACL + package-SID tests are accepted, even if the
Windows runtime is hash-pinned. Default builds have no package worker.

**Conservative pilot limitations:** A normal venv may contain symlinked
interpreters and modern metadata often uses non-PEP639 or folded fields.
The first pilot rejects symlinks, missing license-file evidence and
unsupported metadata instead of guessing that they are safe. Real
native fixtures must establish compatible, defensible exceptions
without weakening containment. The preflight currently supports
registry wheel declarations, not original archive proof; actual wheel
provenance remains a separate acceptance gate.

## Durable audit, restart and rollback/discard

A separate native app-profile journal
`package-output-lifecycle-v1.jsonl`, with domain-separated SHA-256
entry chaining, bounded size and **fsync on every append**, records:

- `package_prepared`: immediately after D3 prepare, **before**
  approval or OS worker launch. Restart renders the package outcome
  unresolved. This never grants authority to resume or retry.
- `audit_local_complete` or `audit_blocked`: after native terminal
  evidence and postrun verification. Only matching exact project,
  job, plan and D3 terminal receipt hashes can be displayed as a
  joined native status.
- `discard_prepared`: after a **new native OS dialog** approves
  destroying this one original job's private output. The host checks
  original D3 terminal state, no quarantine, fixed job ID, exact
  app-profile output parent and a bounded complete tree audit. If the
  audit was never recorded because of a crash, this route is allowed
  only after the original OS job reached a proved terminal state; it
  performs cleanup **without promoting/accepting** the artifacts.
- `discard_verified`: written only after the exact scoped private
  directory is absent and, on Unix, parent removal is fsynced.

A crash between `discard_prepared` and `discard_verified` leaves
**private-output cleanup pending** on restart, requires fresh consent,
and never auto-deletes or starts another package worker. If a partial
deletion changed the originally sealed tree, the current conservative
implementation refuses unattended continuation; it needs a separately
reviewed OS cleanup path. A corrupted or truncated journal halts
replay rather than silently clearing the record. SHA chaining detects
accidental/journal corruption; it is **not a cryptographic signature
against a same-user attacker** and never creates worker authority.

"Rollback" has a precise meaning here: **discard the private,
unpromoted output environment**. The original project was never
mutated by the package worker. No global package installation,
environment activation or transactional project-file swap has been
implemented, so there is nothing valid to "roll back" in the source
project. Never represent deleting private output as an uninstall
of the user's system Python.

The Packages & Workflows UI now shows bounded per-distribution names,
versions, source-declaration hash counts, verified local licenses,
unproven archive provenance, the original manifest/lock effect flag,
replayed package receipt states and separate human-confirmed private
discard. UI inputs cannot supply arbitrary deletion paths or
reusable approval tokens.

## Test plan / evidence status

New Rust tests include real synthetic fixture bytes for positive
`METADATA/WHEEL/RECORD/License-File` verification and negative
altered installed file, forged RECORD, absent license, direct_url,
unsupported lock source, changed manifest/lock, unclaimed payload,
path traversal and symlink. Journal tests cover prepared restart,
two-phase discard, crash-pending restart, altered/partial journal,
wrong project/plan, blocked-audit denial, and symlinked journal.
Node source-boundary checks verify strict cargo features, exact
runtime flags, non-truncated audit, frozen lock preflight, host
source reinspection, D3 receipt join, and no arbitrary cleanup API.

**Still OPEN — must be real, independent native evidence:**
- Windows/Linux Cargo compilation, host/unit/test and Tauri frontend build;
- Real Linux pinned root-owned uv/Python bundle and Bwrap/cgroup state;
- Real offline wheelhouse installed into private output, ABI, native
  script/bytecode, metadata and hash/RECORD acceptance;
- Reconciliation of normal interpreter symlinks and package license
  metadata without following untrusted links;
- Exact wheel archive SHA and approved license/source provenance,
  including known advisories and transitive completeness;
- Cross-process source/lock mutation before consent and after execution;
- Actual human approve/refuse/timeout/cancel during uv Sync;
- Forced worker and IDE crash before/during/after output audit,
  OS process-tree kill, app restart and pending-discard recovery;
- Installed Windows ACL protected worker, Windows native runtime proof,
  and any packaged/signed release.

This checkpoint **does not close IDE-05.4 native acceptance**.
Next is IDE-05.5 consolidated exact-head platform and installed
acceptance with demonstrated failures repaired before promotion.

Standards:
- https://packaging.python.org/en/latest/specifications/recording-installed-packages/
- https://packaging.python.org/en/latest/specifications/binary-distribution-format/
- https://docs.astral.sh/uv/reference/cli/
