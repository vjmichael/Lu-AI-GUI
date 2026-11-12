# IDE-INT06 — Integrated project workbench and workflow completion

**Status:** INT06-A/B/C/D integrated in source only on `ide/int05-toolchains-packages-terminal-20261111`. New exact-head native builds, regression execution, PR/CI, installed Windows/Linux and physical-human acceptance remain deferred under the owner's build-first instructions. This document does not certify delivery of a released IDE.

## Scope and design
- The standalone IDE already has protected editor/LSP/build/debugger/Git/terminal/package subsystems. INT06 connects their **existing observation and review entrypoints** in a single selected-project Workbench. It does **not** introduce another executor, copy private source content, inspect arbitrary paths, or make a process grant.
- Navigation remains explicit: inspect or navigate to the existing panel, then use that panel's existing separate, OS-reviewed action. No automatic Run, Save, Git change, package resolution, debugger step, shell, acceptance or task completion.

## INT06-A — Single-project workbench
- Added Workbench to the IDE's navigation, showing Editor, Git inventory, LSP, Build/Test, Debugger, Packages/Toolchains, Terminal and Release Acceptance.
- Displays conservative `observed`, `review_required`, `unverified`, `blocked`, or `reconciliation_required` states and bounded explanatory text. A release gate is never shown as passed because of a completed job or local source observation.

## INT06-B — Independent native evidence connections
- `integration_workbench.js` invokes only six fixed **read-only** existing native commands: saved Build/Test profiles, local Git repository inventory, package manager inspection, protected Build/Test status, protected debugger status, and global protected LSP worker status.
- Each result is revalidated at its own native/source trust boundary. Git observation is bound to the current registered project and source HEAD SHA; it is **not** remote status or a worktree-clean claim. Package and profile evidence is project-scoped. Runtime toolchain fingerprints come from the independently verified native release pins.
- These reads are intentionally **non-atomic** and only describe conditions at observation time. Neither an observed runtime nor reviewed manifest can attest to subsequent execution or kernel cleanup.

## INT06-C — Review-first workflow guidance
- The workbench prioritizes any directly observed OS reconciliation requirement before ordinary review and presents per-subsystem Open actions that only change the visible panel. A workflow row cannot embed paths, shell arguments, executable names or native authorization tokens.
- Operator can continue in the existing protected Jobs, Languages, Source, Packages and Terminal views, where per-turn approvals, immutable receipts, source rechecks and cancellation remain enforced by the original subsystem.

## INT06-D — Project-switch, restart and accessibility
- In-flight multi-source reads use a monotonically increasing request ticket and the current registered project ID; project selection changes invalidate previously returned information. No result is persisted across IDE restart.
- The Workbench is keyboard reachable, labelled as a single section, with a live status region for refresh errors/availability and explicit buttons to inspect or open underlying panels. Warnings are text, not untrusted markup.
- Added source-contract and model regression tests for cross-project identity, stale read suppression, foreign/untrusted source state, non-execution, recovery precedence and fixed native-read allowlist. These tests are staged **but not yet run**.

## Remaining consolidated acceptance and gaps
1. Reconcile this work with the final exact-head compiler and Node test results; fix real diagnostic failures and verify all panel routes.
2. Validate native Windows AppContainer/SCM and Linux cgroup-v2/Bubblewrap custody/recovery with real original receipts. Include concurrent project switching, locked worker, interrupted receipt, command replay and installed restart.
3. Owner-operated keyboard/screen-reader and installed NSIS/Debian acceptance. Keep tests requiring actual OS approvals deferred.
4. Treat native debug single-step, offline uv Windows ACL, publisher/license/version provenance, toolchain bundle distribution and terminal project command restrictions as open release gates until independent physical evidence is obtained.
5. Do not merge to main or issue release binaries without the owner's explicit authorization.
