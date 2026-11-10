# IDE Group 2 — Consolidated installed Windows/Linux acceptance

**Scope and owner:** Group 2 only, independent standalone Lu IDE. Stacked
source on Group 1 documentation head `1134531f0425b693a535ae1f46b53fd1d2692bc5`, with the Group 1 actual
native/NSIS witness pinned to `c827a94def0a936177e2c6aa16dc565739dfe61a`.
Do not call Group 1's pending installed Linux job a pass.

## Goal and exact group boundary

One authoritative, CI-only **unsigned** installed-app evidence group on
Windows 2022 (NSIS/WebView2) and Ubuntu 24.04 (Debian/GTK). It must:
- use one final source SHA and review exact npm/Cargo locks and independently
  bundled upstream notices;
- build actual NSIS and Debian packages, verify their SHA-256/notice inventory,
  install and launch each graphical native application;
- operate a genuine OS picker/CodeMirror editor, explicitly Save, restart
  into the real reopened project, reject an external-file collision while
  preserving the unsaved draft;
- verify actual native package uninstall removes installed program bytes,
  while preserving the exact installed app's `projects.json` and project
  `main.rs` without mutation, witnessed by separate read-only before/after
  digests and exact canonical catalog root/host ID;
- retain real Linux dpkg failed-upgrade/previous-version restoration tests,
  including durable user state; retain the default-disabled package/debugger
  security posture and prevent accidental execution on project selection;
- run source and hostile negative contract checks before expensive native
  build; no shell/IPC/JS substitution for genuine GUI evidence.

The shared `group02_user_data_preservation.py` witness requires a disposable
`RUNNER_TEMP` project, a single non-symlink native OS app-data catalog,
a well-formed version-one catalog and exact registered project root/ID. Only
then is a SHA receipt written under `RUNNER_TEMP`. The same data must
survive uninstall unchanged. This is **not** a full signed upgrade or a
physical-PC human-authorized protected worker test.

## Baseline evidence, not current Group 2 acceptance

- [Group 1 native Windows/Linux + protected Linux uv run 42766269202](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42766269202):
  **SUCCESS** at `c827a94...`.
- [Group 1 installed run 42766269459](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42766269459):
  Windows **SUCCESS** at `c827a94...`; Ubuntu still at Linux prerequisite
  stage when Group 2 was started. Do not promote an open job.
- Historical separate B2 installed Windows private draft recovery
  had passed at its own older SHA. It does not automatically prove the
  current installed Group 2 source, and does not authorize protected tools.

## Required report

For each OS record source SHA, exact job/run link, actual package SHA, native
install/launch/UI outcome, explicit Save, restart, file conflict, old-version
rollback where supported, real uninstall, unchanged catalog/source receipts
and any failed step with its original logs. Anything pending or skipped
stays OPEN. A successful code check alone is never an installed pass.

## Exclusions / stop rule

Group 3 security approval, Group 4 protected Run/Debug, Group 5 LSP/IDE
integration and Group 6 final branch convergence are separate work.
`ADR_0003_LU_CORE_DLC_VERSION_BOUNDARIES_20261110.md` keeps all live
Core-DLC integration deferred until after Lu Core v1.0. No signing/legal
release approval, physical approval, PR mass merge or Core source changes
are authorized by Group 2.
