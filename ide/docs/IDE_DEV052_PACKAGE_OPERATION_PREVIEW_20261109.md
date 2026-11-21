# IDE-05.2 — Revision-bound package operation previews

**November 9, 2026 — source implementation in the IDE-05 workstream.**
Base: IDE-05.1 commit `68e63498ec2c975ed89c4249001ee2270f513908`,
branch `ide/dev05-1-package-discovery-20261109`. Native Windows/Linux
compilation, executable manager acceptance, installed UI and signing
**are not accepted** merely by implementing the preview.

## Intent and guarantees

The standalone Lu IDE's **master manager of managers** does not implement a
general ecosystem dependency solver. IDE-05.2 adds a new **static, data-only**
review layer between manifest discovery and any future protected package worker:

1. A **fixed typed request** selects exactly one detected manager
   (`uv`, `vcpkg`, `conan2`), one operation (`add`, `remove`, `sync`),
   and an optionally required literal package name/exact requested version.
   `add` requires both; `remove` requires a name and forbids a version;
   `sync` forbids all package arguments. A constrained ASCII grammar denies
   path traversal, shell words, version constraints, URLs and unrecognized
   extra fields. No arbitrary command-line flags, manager binary, execution
   scope, network registry or credentials are accepted by Tauri.
2. The **native project registry** reopens the OS-picker-registered project
   capability and performs IDE-05.1's fixed six-file content-SHA inspection.
   The selected candidate must be `candidate_needs_review`: missing,
   ambiguous, conflicting or unreadable managers and stale/unverifiable
   lockfiles are refused, not silently switched to another manager.
3. The preview contains the exact native evidence fingerprint, all six
   individual file observations (including missing and SHA-256 revisions),
   an operation/manager/source-bound **deterministic preview digest**, the
   intended direct declaration adjustment, optional lockfile impact and a
   list of security risks. **No actual file diff is computed**, and
   every proposed change carries `expectedResultSha256=null`,
   `actualDiffComputed=false` and `requiresProtectedResolver=true`.
   A named version is the *requested version*, not a verified installable or
   transitive resolution.
4. For `vcpkg.json`, the host can additionally parse the **saved JSON
   data only**, revalidate the exact manifest SHA before interpreting static
   top-level dependency names, and distinguish already-declared from absent
   direct dependencies. Duplicate Add and missing Remove are refused. This
   parser does NOT process vcpkg registries, version-baseline inheritance,
   feature activation or CMake scripts. Python `pyproject.toml`,
   Conan `conanfile.txt` and executable `conanfile.py` declaration
   semantics are deliberately marked **unparsed** rather than guessed or
   dynamically evaluated.
5. Risks identify transitive dependency/license uncertainty, install hooks,
   Python build backend execution, native builds, registry/network and
   credentials, unverified toolchains and Conan 2 version detection. Removal
   warns of reverse-dependency breakage; Sync warns of environment mutation.
   This is a **risk classification**, not a completed vulnerability scan.
6. **Mandatory fresh revalidation:** The separate native
   `revalidate_package_operation` call takes the original *typed request*
   plus expected preview SHA and evidence fingerprint, reopens and rehashes
   native manifest and lock evidence and rebuilds the entire proposal from
   current source. Any drift, different manager, operation, package,
   version, ambiguous candidate or digest mismatch rejects with an explicit
   stale/error status. It will not rely on cached UI input.
7. A matching revalidation returns only
   `matchedFreshNativeEvidence=true`; always
   `readyForApproval=false`, `authorizationIssued=false`,
   `processStarted=false`, `filesModified=false`.
   Even perfect matching source data **does not authorize Apply**. The
   future IDE-05.3 native worker must conduct another atomic admission
   recheck, obtain a separately reviewed OS-native human approval,
   sandbox untrusted managers and prove trusted cleanup before making
   changes. No IDE-05.2 install/apply Tauri API exists.

## Implementation boundaries

- `ide/host/src/package_preview.rs` — fixed request type, conservative
  declaration projection, danger classification, digest and independent
  revalidation. Ten+ native unit tests cover exact revisions, lock/source
  edits, source identity, conflicting C++ managers, unsupported tools,
  malicious arguments, Conan Python script not evaluated, vcpkg JSON
  parsing and duplicate/no-op protection, and Unix symlink replacement.
- `ide/app/src-tauri/src/lib.rs` — two nonexecuting commands:
  `ide_preview_package_operation` and
  `ide_revalidate_package_operation`, both native registry-only and
  `spawn_blocking`. There is **no** apply/install execution command.
- `ide/app/web/main.jsx` — selected-manager/operation/literal requirement
  editor; source digest and intended changes, risk list, exact reinspection
  button; input and project changes invalidate prior preview, and stale
  reinspection erases both preview and verification rather than granting
  false approval. Explicit **Install/Apply unavailable** notice.
- `ide/app/tests/dev052_package_operation_preview.test.mjs` — source
  boundary regression cases, not a substitute for native execution.

Each file sample is still **non-atomic** across multiple files. A stale
check can only reduce reviewer error; a future operation cannot become
approved until the native admission layer creates an immutable effect plan
and rehashes the current source/lock and installer/toolchain before
dispatch. CI code comments, a SHA fingerprint and a saved history do not
create OS authority.

## Consolidated IDE-05 acceptance (open)

At the completion of IDE-05.1–05.4, run **one** grouped exact-head
Windows/Linux gate, not one GitHub workflow for every commit. It should
include Cargo fmt/clippy/test for host, Tauri native compile, Node frontend
test/build and installed UI evidence, followed by hostile native fixtures:
malformed/overlarge JSON and TOML; lockfile alias, replace and mtime races;
manager conflicts; wrong project ID; argument/flag injection; operation
and version changes after review; denial/expiry/cancellation; untrusted
Conan/Python/npm hooks; network/credential denial; protected worker
termination, reboot recovery and independent installed artifact receipts.
The IDE-05.2 implementation is source-only until those exact OS results
arrive. Keep unrelated DEV-04.6D LLDB-human-dialog Windows acceptance
open separately.

**Next:** IDE-05.3 protected package worker admission **without** expanding
permissions from this deterministic preview. Deferred: full package
resolution, transitive dependency diff, writable manifests/lockfiles and
signed/installed manager execution until separately verified.
