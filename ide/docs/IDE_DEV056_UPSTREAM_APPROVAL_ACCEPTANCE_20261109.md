# IDE-05.6 / Task 5 — upstream wheel provenance, operator approval and installed acceptance

Task 4's **CI-only first-party wheel/native recovery** is accepted at
`7c9075478f6428630e5980aad549da1b10a370d0` and its separate
documentation-only closeout `42790be97ab0afbc354cfcab7bc74ee241dc5d21`.
This Task 5 work does **not** silently promote that synthetic wheel into a
release, nor treat CI synthetic consent as a real person's action.

## Task 5 group A — independent upstream runtime archive intake

The actual `uv==0.9.9` Linux CLI used by the Task 4 acceptance runner was
previously pip-installed and measured only **after** staging. Task 5 adds a
separate **read-only upstream archive observation** outside the protected
worker, executed only by the trusted CI host, not on model/user-provided
URLs. The fixed `https://pypi.org/pypi/uv/0.9.9/json` release record is
fetched using TLS. The test also downloads exactly one corresponding
Linux x86-64 wheel, requires a non-yanked `bdist_wheel` at the expected
`files.pythonhosted.org/packages` origin, and verifies its actual bytes
and size against the separately retrieved release JSON checksum. The
installed executable, original MIT license bytes and wheel internal
Name/Version are compared with exact wheel members. The archive is scanned
for malicious paths, links, expansion, duplicate members, wrong CPU,
version and platform. Changed executable, license, registry URL, yanked
status or archive digest fails closed.

A bounded receipt distinguishes observations:
`registryWheelHashAndInstalledBinaryMatched=true` is meaningful **only
when the actual runner succeeds**. It always sets
`upstreamPublisherSignatureVerified=false`,
`independentUpstreamAttestationVerified=false`,
`licensesLegallyCleared=false`, `humanApprovalGiven=false`,
`releaseApproved=false`. PyPI release metadata delivered over TLS does
**not** prove publisher identity, signature provenance, advisory closure or
immutable source control. The Task 4 synthetic `ide05_demo` wheel likewise
has NO upstream origin and remains CI-only.

The retained `ide055-ci-wheelhouse` feature is unavailable in the
installed app. This intake does not trust a `uv.lock` URL or the package
installer's own RECORD as authority; it only adds independent runtime
archive-to-installed-bytes corroboration. A real production install still
needs an OS-installer-pinned, separately reviewed full dependency source
inventory, trusted download/mirror, publisher identity policy, license
interpretation and advisory review **before** any private environment may
be promoted.

## Task 5 remaining independent gates (not proven by compilation)

1. **Human approval:** real installed Tauri OS dialog on physical Windows
   and Linux, explicit approve, decline, timeout, cancel, stale source/
   preview after display, process launch and one-use grant negative controls.
   Real operator evidence must include GUI, process/job and journal receipts.
   The CI fixture's `record_human_approval(true)` is not a human witness.
2. **Installed app:** exact NSIS/.deb installation, no compiled-in
   `ide055-ci-wheelhouse`, real registered project, source preservation,
   frozen dependency plans, private output and explicit discard, interrupted
   OS child recovery, restart, uninstall and retained project data. CI
   installed desktop smoke alone is not evidence of fresh operator consent.
3. **Windows AppContainer:** keep `uv_sync_offline` fail-closed before
   profile creation, consent and child launch until the source snapshot,
   runtime and output ACLs are independently evaluated against the actual
   package SID: source/runtime must be denied write, output permitted only
   its designated private tree. Must prove hostile file-write attempts,
   Job/SCM ownership and escaped-process cleanup on a genuine Windows
   kernel. No automatic ACL relaxation or unsandboxed fallback.
4. **Original third-party wheel closure:** full transitive wheelhouse hashes
   against separate, approved upstream manifests, native ABI, scripts/
   entry points, licenses and known-advisory review. Static origin records
   alone never authorize code execution, update or deployment.
5. **Packaging/release:** release-level Windows/Linux attestations,
   rollback and signed/unsigned distribution policy remain distinct from
   this task-group native CI acceptance.

The existing physical Windows negative witness is
`ide/app/fixtures/Test-IDE055-PhysicalWindowsPackageDenial.ps1`.
It witnesses only the installed app **refusing** package execution and
cannot substitute for the missing positive AppContainer/approval gates.

## Consolidated acceptance and evidence rule

Use the existing grouped IDE-05 Windows/Ubuntu source matrix and one
dependent real Linux delegated-worker gate; do not create a new micro-CI
queue. The Task 5 upstream observation runs in that **same** native group,
outside Bubblewrap, and the fixed negative test suite runs on both OS
matrices. Never mark a gate PASS until exact code-head logs and the actual
artifact/receipt file are inspected.

When work progresses, append the exact SHA, workflow run ID, job IDs,
positive or negative result and independently remaining human/installed/
Windows release gates here.

## Actual native Task 5 upstream-corroboration failure and repair

At exact head `f34f6d20d32e0b6bc85341465473aa9d0fd6a2c6`,
[run 42739837059](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42739837059)
passed the Windows 2022 and Ubuntu 24.04 source matrices including the
new negative intake suite. The real Linux upstream-wheel comparison failed
`IDE056_INSTALLED_UV_BYTES_DIFFER_FROM_WHEEL`. The inherited staging step
chose a `find ... -name uv -print -quit` basename result from a pip
`--target` tree which may include both wheel-shipped native ELF and
pip-generated console entrypoints. Selection by traversal order is NOT an
upstream binary identity.

The next exact-head correction pins the **one fixed wheel package member**
`$RUNNER_TEMP/ide055-uv/uv/uv`, requires it to be executable and to
have native ELF magic, and applies this path in both runtime staging and
independent archive comparison. This does NOT relax exact wheel archive
hashes, the license check, OS containment, CI synthetic no-network worker
or any release/human authority. It requires new native runner evidence.
