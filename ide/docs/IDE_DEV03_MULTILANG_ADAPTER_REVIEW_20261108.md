# Lu IDE DEV-03.2B — Python, Node and CMake adapter review

Date: November 8, 2026
Branch: ide/dev03-multilang-native-adapter-review-20261108
Base: DEV-03.2A commit 1d6b1325967cfa20e8be7bba395e33ac2e506ff4
Scope: Native adapter-review contracts, NOT runnable multi-language workers.

## Why the runtime boundary matters

Pytest imports, JavaScript/TypeScript tests, and CMake configuration all
execute project code. A manifest name, a test file, or Git HEAD alone is
not sufficient authorization to run them. The original WO6 protected
Cargo worker cannot simply use npm, Python or CMake on host PATH.

This stage adds immutable native-host contracts and exact project
preflight, not a general execution command, script runner, or task
completion claim.

## Native implementation

The standalone host module at ide/host/src/reviewed_adapters.rs
defines three exact, hardcoded non-Rust contracts:

- python_pytest: a pinned-Python candidate with fixed arguments
  -I -m pytest -q -p no:cacheprovider. Python isolated mode alone
  does NOT prevent arbitrary side effects from tests/plugins.
- js_tests: a pinned-Node candidate with builtin --test and
  --test-reporter=tap. It never invokes package.json scripts or npm.
  TypeScript support is unverified pending the exact bundled runtime.
- cpp_cmake_build: three ordered fixed logical steps, CMake configure
  with Ninja and Debug private output, bounded CMake build, and CTest.
  /workspace and /output are logical worker mounts, not operating-system
  paths ready to use in a Windows executable command line.

The new ide_review_multilang_adapter command accepts only projectId,
a fixed profileId, and an expected saved-manifest SHA-256. The native
host checks an active registered project, clean Git HEAD, no tracked or
untracked changes, and the real file's bounded registered cap-std
read-only source hash. It rechecks both Git state and manifest bytes
before producing a deterministic review fingerprint.

This fingerprint is not an authorization token. The host result
always states:
- reviewState = native_contract_observed_not_approved
- snapshotState = not_sealed
- toolchainState = not_pinned_or_installed
- nativeWorkerState = no_reviewed_platform_adapter
- executionAuthorized = false
- commandDispatched = false
- sourceTextExported = false
- installedAppAccepted = false

Eight explicit future admission gates require independently pinned
runtime bundles, a fully frozen source snapshot, denied network and
credentials, read-only project/output separation, kernel admission
and quotas, fresh native OS approval, bounded output/artifacts, and
durable verified descendant termination/recovery.

The Jobs UI allows an explicit read-only adapter review for these
three profiles, showing the clean HEAD, manifest SHA, immutable
candidate steps, and missing runtime/acceptance gates. It creates
no runnable button for Python, JS/TS or CMake. The frontend mirrors
all exact fixed step arguments and validates result shape, preventing
forged command text or execution-ready claims. New capability
job.reviewAdapter and closed schemas make 49 catalog operations and
47 shared definitions.

## Verification

Six native Git/filesystem cases are committed in
ide/host/tests/reviewed_multilang_adapters.rs:
genuine clean tracked manifests, hash CAS, deterministic review,
script content non-disclosure, changed/untracked repo denial,
unknown/foreign/closed project denial, oversized/missing files, and
Unix symlink denial.

Eleven focused JavaScript tests are committed in
ide/app/tests/reviewed_multilang_adapters.test.mjs. All 11 passed
in a V8 harness emulating Node assertions and reading the exact
GitHub source. THIS IS NOT genuine npm/Node, Rust or installed testing.

The dedicated exact-head Windows2022/Ubuntu24.04 workflow is
.github/workflows/ide-dev03-multilang-adapter-review-native.yml.
It checks native Rust tests, full frontend tests, default Tauri build,
and exact-source diff proof that the existing OS worker, build backend
and source browser did not change. CI results must actually pass
before technical source acceptance. Installed acceptance remains OPEN.

## Follow-up execution implementation required

DEV-03.2B.2: Stage exact versioned, reviewed, offline interpreter
and compiler bundles with original licenses, transitive runtime
dependencies, hashes, ABI and per-OS redistribution review.

DEV-03.2B.3: Build platform-specific native multi-language worker
adapters under the original protected one-use supervisor. Require
source snapshots, pinned tools, fixed arguments and private outputs;
never shell/host PATH, npm install, pip install or untrusted hooks.

DEV-03.2B.4: Real Windows/Linux isolated workers and adversarial
native tests proving normal, cancel, crash, descendant kill, bounded
stderr/stdout and durable journal result with artifact SHA.
Then installed NSIS/Debian normal+restart acceptance.

Other open gates: Groups A/B native GTK and debugger repairs, Group C
consolidation/integrated acceptance, and DEV-03.3 live log streaming.
Do not promote this contract-only tranche to runnable feature status.
