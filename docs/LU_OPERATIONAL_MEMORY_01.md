# LU-OPMEM-01 — Operational memory and verified resource reuse

**Status (2026-11-05):** First source-only slice on
`lu/aiw-operational-memory-20261105`. It is not yet connected to live desktop
task routing, ANSF context assembly or an executable runner. It is not proof
that Windows NSIS, model backends, or a Cognitum dataset exist on a given host.

## Purpose

LU-AIW-01 R3 requires persistent, *verified* operational knowledge. An AI should
not repeatedly rediscover existing NSIS binaries, compilation tools, datasets,
fixtures or tested build procedures between conversations. But recalling a path
does not establish that a file, dependency or executable is still usable.

Keep these categories separate:

- **ANSF memory** — human decisions, conversation history and provenance.
- **Operational memory** — resource locator, platform, version, source digest,
  previously successful verified recipe and last-successful command summary.
- **Authorization** — current task's authenticated origin, specific permitted
  tools, target/effect checks and expiration. Never inferred from memory.
- **Execution journal** — real prepare/apply/uncertain effect receipts, not
  merely historically successful instructions.

## LU-OPMEM-01A implemented API

`src-tauri/lu-core/src/operational_memory.rs` contains:

- `open`: app-profile-owned SQLite storage, WAL with durable sync.
- `remember`: workspace-scoped immutable resource ID, display name, kind
  (file, executable or directory), absolute local path, OS/architecture,
  version, optional pinned SHA-256; idempotent replay, conflicting redirects
  rejected.
- `recall` and `list`: bounded queries that return **KnownHistorically**,
  never execution privileges or current resource readiness.
- `probe`: explicit read-only local check for platform mismatch, existence,
  ordinary file/directory versus symlink, and optional SHA-256 comparison
  against the admitted or last-success digest. Hash work capped at 512 MiB.
- `record_host_success`: trusted-host-only recording after an independently
  verified command or task. Records recipe ID, redacted summary, timestamp and
  current file digest. It performs **no execution** itself.
- A current-process host attestation can make a matching file
  **VerifiedUsable**, but opening the database in a new process clears that
  live attestation. The resource then becomes **PresentUnverified** until the
  host revalidates its runtime usability. Directories remain present/unverified.

All returned paths are metadata, not validated execution targets.
The host must restrict DB location to the application profile, check access
before disclosing paths, and prevent the model/WebView from invoking
`remember` or `record_host_success` as a self-attestation route.
Never blindly execute a stored command summary or include secrets in it.

## Next-session example

1. The host remembers a previous NSIS location and recipe ID after an actual
   verified build.
2. On the next chat's Windows installer task, it checks the inventory first,
   before web search or re-downloading packages.
3. `probe` detects absent, modified, mismatched or unverified local resources.
4. The trusted host runner checks versions/dependencies and executes an
   approved lightweight verification under current task authority.
5. A successful verification records new evidence and permits subsequent
   authorized operations via existing tool/process and effect-journal gates.
6. The final output records exact artifact hash, source revision and acceptance
   status. Missing prerequisites are explicit, not silently reinstalled.

## Open gates

**LU-OPMEM-01B:** wire the ledger into desktop profile state, host router and
task preflight; discover and verify through *host-owned* probes (no model-origin
claims); add project access checks, safe command receipts, bounded orientation
summaries, and a user-visible remembered/present/verified status.

**LU-OPMEM-01C:** connect resource dependency graph and ANSF references,
verified recipe registry, task checkpoint evidence, robust stale/corrupt data
handling, version migration, cross-platform executable detection, and installed
Windows acceptance with real NSIS and Cognitum benchmark workflows.

The first implementation covers persistence/probing only. It is deliberately
not yet operationally useful through Lu's installed GUI.

## Acceptance expectations

- Source tests: cross-restart retention; successful recipe preservation;
  old success downgraded after reopening; changed/deleted file detection;
  workspace isolation; conflicting paths; OS mismatch; digest mismatch;
  symlink refusal and no false positive for directories.
- Integration tests: fresh chat finds the old resource, avoids unnecessary
  downloads, safely detects deliberate removal, and cannot auto-execute a
  remembered command without an authenticated current grant.
- Benchmark: number of repeated searches/downloads, time to first action,
  false readiness, proper provenance, task completion, authorization
  violations, and human clarification burden.

A source test pass is not installed desktop acceptance.
