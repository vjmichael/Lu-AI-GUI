# Lu IDE — Current authoritative project state

**Updated:** 2026-11-05 (America/Phoenix). **Project:** standalone Lu IDE, not Lu Core.
**Repository:** `vjmichael/Lu-AI-GUI`
**Development branch:** `ide/workspace-development`
**Last inspected implementation HEAD before continuity documentation:** `eb0aefdb1f4e954044b353e64b8e308d13b0e6f6`
**Working tree / local delivery folder:** UNKNOWN — **not inspected in this session**.
**Local ahead/behind remote:** UNKNOWN — never assume a remote checkout is the most complete copy.
**Live Lu integration:** DEFERRED by user; contract work may proceed independently, but the IDE must run without Lu.

## Evidence grades

| Area | Implemented | GitHub source | Automated tests | Native Windows GUI | Installer/artifact |
| --- | --- | --- | --- | --- | --- |
| R3 editor, project lifecycle | Yes | Yes | Prior CI reported; see handoff | User accepted earlier R3 milestones | Prior artifacts in linked evidence; availability must be rechecked |
| R3.4 diff/review | Yes | Yes | Prior CI passed; see handoff | User accepted interactive multi-hunk smoke | Prior unsigned Windows artifact documented |
| R3.5 proposal inbox | Yes | Yes | Prior CI passed; see handoff | User reported functional flow; hostile gates still open | Prior build references in handoff; reverify availability |
| R3.6 success banner | Yes | Yes | Regression assertions added, **exact-head run unverified** | **Not yet tested by user** | **No R3.6 package verified** |
| R3.7 in-file Find | Yes | Yes | Regression assertions added, **exact-head run unverified** | **Not yet tested by user** | **No R3.7 package verified** |
| Find and Replace | No | No | No | No | No |
| Live Lu↔IDE connection | Deferred | No | No | No | No |

**WARNING:** The ledger describes observed evidence at this checkpoint; Git commits alone never establish local build success.

## Work to resume next

1. **Reconcile branch and local copies**: identify all checkouts / `Delivered` folders / generated Windows packages, capture source roots, hashes and output locations. This step requires access to the relevant local filesystem; no remote connector can certify unseen files.
2. **Verify R3.6/R3.7**: run `cd ide/app && npm run check`, native host tests and Windows Tauri build. Review exact-head CI outcomes and test Find/navigation + banner dismissal on Windows. Resolve failures before calling either release-accepted.
3. **Next standalone feature: Find and Replace**, with explicit editor-buffer edits, undo support and no implicit Save. Keep all Lu bridge activation deferred.
4. Work on source control and build/test UX only under the documented authorization/security gates. No untrusted execution on project-open.

## Artifact manifest — do not infer missing paths

| Artifact | Source or expected location | Verified existence now? | Verification |
| --- | --- | --- | --- |
| Repository source | GitHub, `ide/workspace-development` | Yes, connector read | Fetch exact HEAD before editing |
| Windows R3.5 unsigned release | GitHub Actions release reference in `ENGINEERING_HANDOFF.md` | Prior evidence only | Re-fetch run, artifact, checksum before offering a download |
| R3.6 or R3.7 installer | UNKNOWN | **No** | Do not claim an installer exists |
| Local NSIS tooling / `Delivered` directory | UNKNOWN | **No** | Enumerate real filesystem; record absolute path and checksum |
| Windows installation used for user screenshots | User machine, path unknown | User screenshot confirms application UI, path not verified | User Windows acceptance only |

## Safety and scope

- Human-owned edits; approval is not authority to save; proposals are data, not commands.
- Editing, reviewing and staging do not execute project code.
- Do not change Lu Core for standalone IDE milestones.
- Never equate GitHub, local workspace, CI artifacts, or user-installed build versions without evidence.

## Continuation links

[Chronological changes](CHANGELOG.md) · [Engineering handoff](ENGINEERING_HANDOFF.md) · [Continuity rules](CONTINUITY_PROTOCOL.md) · [Implementation gates](docs/IMPLEMENTATION_GATES.md).
