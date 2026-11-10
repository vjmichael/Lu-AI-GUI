# IDE-05.6 Task 5 — installed NSIS/Debian acceptance and platform gating

## Verified baseline (2026-11-10 UTC)

- Linux upstream `uv==0.9.9` registry wheel, ELF and license bytes:
  **PASS**, [consolidated native run 42742430229](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42742430229)
  at `b7fa7dc2f1d52a558653674f86e167a9e4a95f5d`.
  Real protected Linux `uv` offline miss, cancellation, synthetic positive wheel,
  and SIGABRT/restart recovery also passed in that group. No signature,
  complete third-party wheelhouse, release or human approval is implied.
- Original P06 Cargo review hash
  `d4312b60209f89181766b60c2982fb8b199e8ed9d6be1a9dcbdcb3dd07abf6b4`
  (Git blob `87d0045a4be1c78651f080300aba71c14dbe1f30`)
  differs from current lock
  `c7995fb9957bee2fb0f8247e232c5f7f9db3b3ef5d77d3afcbdc09f1ed9b36aa`
  (Git blob `69bc41f0cf739c3d77989531b14b28a472b16e4a`).
  Independent blob comparison established *exactly one* changed local
  `lu-ide-execution-supervisor@0.1.0` dependency block, adding previously
  locked `base64 0.22.1` and `toml 0.8.2`. All other 499 package blocks,
  resolved versions and registry checksums are byte-identical.
  Their common non-supervisor SHA-256 is
  `cbe657f1a2b150d0a45744e4a855a36382f6572f2fb9f6893e49758cc705d2e7`.
  The original P06 approval is unchanged; an **exact-scoped** successor
  manifest and seven hostile tests were added at `5788f9a`.
  A different lockfile still fails, and all release/human approvals are false.

## Installed gate — first post-reconciliation OS failures

[Run 42745481444](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42745481444),
exact tested code commit `5788f9a198425c7c31057e9496b77b37ae7289d9`.

### Actual Windows NSIS job 129596428726

The successor review and Tauri/Cargo/frontend/native lock graph passed.
The exact unsigned NSIS package built, bundled independently fetched
original license notices, installed into the disposable Windows runner,
and launched as a real WebView2 window. During **real UI Automation**
the initial `Workspace projects` heading was recovered by OS Tab focus.
Clicking `Job requests` then left an active real app HWND and WebView2
renderer, but the accessibility tree temporarily contained only three
root nodes. The test failed
`WO8_NATIVE_UI_DENIED:ui_element_not_exposed_Execution_requests`.
The source/installer is not accepted merely because a native window
existed; project editing, restart and uninstall have not been proven.

The next fixture repair adds at most two further native Tab focus-recovery
attempts **only for known IDE navigation headings** after the window remains
owned by the correct installed process and is verified foreground.
It still requires the genuine UIA heading, never uses JS or Tauri IPC,
never fabricates UIA nodes, and never sends keys to a native permission dialog.

### Actual Linux .deb job 129596429100

The successor review, transitive license inventory, Debian build,
real dpkg failed-upgrade rollback and preservation of user data passed.
The real unsigned .deb installed, launched and focused a GTK/X11 desktop.
The separate AT-SPI native folder chooser was visible but its file list
exposed no exact fixture directory row; after four bounded genuine native
keyboard/chooser-geometry actions the chooser remained visible. The test
correctly failed `WO8_LINUX_EDITOR_DENIED:native_folder_chooser_not_accepted`.
The package was removed in the cleanup trap; source editing, restart and
complete installed project acceptance remain unproven.

The next fixture repair navigates to the **exact temporary fixture's
parent folder** using GTK's OS location entry (instead of relying on
`Alt+Up`), then attempts an exact child-name native keyboard selection.
It requires real chooser closure and exact registered project exposure
before acceptance. No hidden catalog mutation, direct IPC, JS,
unscoped generic Open-click retry or fake folder selection is introduced.

## Independent gates still OPEN

- Genuine **human** per-job approval, refusal, timeout, cancellation,
  changed-preview denial and reviewed one-use receipts in the installed
  desktop. Automated CI synthetic consent is not an independent human.
- Windows `uv_sync_offline` **remains NoContainment denied before AppContainer
  profile/approval/launch** until immutable staged workspace/runtime ACLs
  are independently attested against the exact child package SID and
  tested through native hostile write/rename, output-only write and
  SCM/Job recovery cases. This code does not enable the Windows worker.
- Real third-party dependency publisher signatures, complete wheel
  archive provenance and advisory/legal approvals, and release-grade
  signed/unsigned distribution remain independently unaccepted.

No Task 5 **full release/physical human** acceptance should be claimed
from a successful default-disabled NSIS or .deb smoke run.
