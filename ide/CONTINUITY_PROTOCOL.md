# Project Continuity Protocol (PCP-01)

**Purpose:** Prevent cross-conversation rediscovery and false claims that GitHub, a working directory, a build artifact and an installed application are interchangeable. Use this protocol for Lu IDE; copy/adapt it to Lu, NVP, Cognitum and Single-Galaxy independently.

## Required project documents

- `PROJECT_STATE.md`: short, **current** status; next work, branch, exact implementation commit, verification matrix, blockers, local/Git divergence, artifact manifest, dependencies.
- `CHANGELOG.md`: append-only chronological record of meaningful changes, with provenance.
- `ENGINEERING_HANDOFF.md`: durable architecture, build commands, source-map, CI paths, recovered decisions.
- Detailed gate/evidence documents remain in `docs/`; these are linked, not duplicated.

**Before work:** read state and handoff, inspect actual branch HEAD and available local files, identify whether local or remote contains newer changes. Do not overwrite or abandon uncommitted work. If a filesystem is inaccessible, record `UNKNOWN / NOT INSPECTED`, not `clean` or `missing`.

**After each meaningful development checkpoint:** update CHANGELOG and PROJECT_STATE alongside code; record actual file paths, branch, commit SHA (or `PENDING COMMIT` during write), tests attempted and results with exact runner/OS, artifact IDs/paths and SHA-256 if generated, pending tasks and blockers. Correct stale handoff claims. Distinguish code written vs CI passed vs locally compiled vs packaged vs user-tested vs production accepted.

**Local vs remote reconciliation:** use a two-way inventory:
1. Local workspace root, `git status`, current branch/HEAD, tracked/untracked work, `Delivered` and other artifacts, checksums.
2. Git remote branch/HEAD, PRs, actions runs, release artifacts, recorded checksums.
3. Classify each item: LOCAL_ONLY, REMOTE_ONLY, MATCHED, DIVERGED, UNVERIFIED. Never silently prioritize one source.
4. Record resolutions without deleting unique local files; require explicit approval for destructive reconciliation.

**Artifact entry fields:** logical name; absolute local path or GitHub artifact URL/ID; target OS; source commit; build job ID; generated UTC timestamp (if known); file size; SHA-256; verified accessible (yes/no/unknown); user installed/tested (yes/no/unknown). Never invent package paths.

**Evidence vocabulary:** DESIGN_ONLY, IMPLEMENTED_UNVERIFIED, UNIT_VERIFIED, CI_VERIFIED_EXACT_HEAD, NATIVE_VERIFIED, PACKAGED_VERIFIED, USER_ACCEPTED, RELEASE_ACCEPTED. These statuses can coexist and should not be collapsed into a single "done".

**Conversation handoff:** a new developer/AI should read these documents, verify current Git/local facts, choose the next unblocked gate, and update the ledger at the end. Old documents may be stale: actual evidence wins. The user explicitly deferred any live Lu-to-IDE connection; standalone development is independent.

## Enforcement

Repository policy: a code or workflow PR touching `ide/**` must include a meaningful `ide/CHANGELOG.md` update and refresh `ide/PROJECT_STATE.md` when status changes. CI should reject missing updates on such PRs, excluding ledger-only and purely administrative modifications. A real Git-aware diff gate must compare PR base and head and must not be bypassable by merely editing timestamps. The PR diff gate is implemented in `ide/scripts/check_continuity.py` and `.github/workflows/ide-continuity.yml`, but its first passing run is unverified and branch protection must explicitly require its `continuity` check before merges are actually blocked.

## Minimal entry template

```text
Date / gate / branch / implementation commit:
Changed files and behavior:
Local workspace and remote divergence:
Verification (command, OS, exact SHA, passed/failed/not run):
Artifacts (location, SHA-256, accessibility):
Unfinished / blockers:
Next exact action:
```
