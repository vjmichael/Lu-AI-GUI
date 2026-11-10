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

## 2026-11-10 follow-up — installed gate `79d0643` and actual defects

At [run 42747082451](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42747082451),
Windows/Ubuntu native compilation and the real delegated Linux protected
`uv` OS gate all **passed** at exact head
`79d0643961bbfa39419dec98c78a7c7ee6814017`.
That does not promote the installed app or human permission gates.

At [installed run 42747082943](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42747082943)
the genuine unsigned Windows NSIS again installed, launched and bundled
license evidence, but its WebView UIA tree became empty after the user-facing
Job requests navigation, despite successful initial native focus recovery.
Inspecting the **actual React source** found a deterministic render failure:
when `activeProject===null` and e.g. `projectRunProfiles===null`,
`projectRunProfiles?.projectId===activeProject?.projectId` evaluates
`undefined===undefined` → true, so the Jobs page tries to call
`projectRunProfiles.profiles.map` on null and kills the React render.
The same incorrect guard affected `testDiscovery`, `nativeJobOutput`,
package panels and other project-only data. The product fix uses the
existing `belongsToSelectedProject(activeProject, record)` helper,
requiring two real, nonempty matching project identities; it also corrects
one inverted identity guard and tests missing/cross-project data.
It does **not** substitute UIA clicks, fake any WebView state, or remove
installed source/restore/uninstall requirements.

The real Linux Debian install, graphical launch, bundled licenses,
failed-upgrade rollback and user-state preservation passed, but GTK
`SELECT_FOLDER` did not close or register the fixture after attempts.
The `79d0643` fixture's attempt 1 and 3 mouse geometry calls succeeded
and returned before the native `Alt+S` acceptance mnemonic was ever sent.
The next correction prioritizes independently X11-title-verified actual
GTK keyboard `Alt+S`, then `Alt+O`, before the limited UIA/AT-SPI
geometry fallback, and continues to require both actual modal closure
and the exact registered project; keystroke delivery is never a pass.

## Task 5 one-use approval binding and Windows ACL status

The protected offline-uv approval dialog now includes the freshly prepared
native **Job ID** and full sealed **JobPlan SHA-256** alongside frozen
source, manifest, reviewed operation and runtime SHA. The dialog still
refuses timeout, denial, concurrent cancellation and stale reviewed source
and grants once only after OS/native revalidation. This is a **binding
implementation and source-test gate**, not proof that a real human has
personally clicked Approve/Refuse on a signed installed release.

Windows `uv_sync_offline` is still **refused before creating an AppContainer
profile**: the copied source and toolchain under `LocalState` have not
been independently sealed against writes by the exact AppContainer SID.
The fail-closed `NoContainment` branch is explicitly regression-tested
ahead of profile creation and staging. A genuine positive Windows security
gate must independently prove native DACL and mandatory integrity
read-only source/runtime, writable private output only, no alternate
token/capability write route, reparse and rename denial, native SCM/Job
custody and restart recovery. No CI or installed-app smoke run to date
grants that permission. Do **not** remove the early denial until the
actual native positive tests and physical approval witness exist.

**Pending:** next exact-head grouped Windows/Ubuntu+delegated Linux source gate
and NSIS/Debian installed group. Full Task 5 remains open until the real
installed checks pass and separately all human/Windows ACL gates pass.

## Exact installed gate 42751072907 (2026-11-10 UTC)

The Windows NSIS job **PASSED** including source-resolved licenses, actual
installed WebView2 UIA shell/navigation/native picker, source edit and
explicit save, read-only Jobs discovery, default-disabled debugger,
persisted restart, external edit/draft conflict preservation and real
NSIS uninstall. The source-level React missing-project guard fix at
`c32d2c1b345a82a02f84c4df70c41ee7169679c1` therefore has independent
native installed Windows acceptance. The default unsigned app still does
**not** demonstrate protected worker execution, independent real human
consent or Windows AppContainer ACL positive acceptance.

Linux built, installed, launched under GTK/X11, verified license inventory,
failed-upgrade rollback and user-state preservation, but its OS file chooser
**FAILED** exact project selection. Detailed logs:
`P04_GTK_PARENT_LOCATION=exact_native_parent_submitted`;
`P04_GTK_FIXTURE_DIRECTORY_CANDIDATES=0`;
`P04_GTK_CHILD_TYPEAHEAD=native_keyboard_attempt_not_witness`;
`P04_GTK_EXACT_PATH_ENTRY=reasserted_and_submitted_in_native_chooser`;
`P04_GTK_NATIVE_CONFIRM_KEY=alt_select`;
`P04_NATIVE_GTK_FOLDER_ACCEPT=visible_dialog_still_open_after_bounded_real_actions`.
The fixture navigated back inside the target after detecting zero AT-SPI
rows **before** attempting native Select. Follow-up preserves the candidate
at parent-level until native confirmation; exact chooser closure and
registered project are still mandatory. No acceptance from a keystroke.

The Windows package ACL code remains deliberately fail-closed prior to
AppContainer creation. A separate read-only DACL inspection module is
a *diagnostic* and cannot grant execution; it requires additional
effective access checks and real hostile child proof before promotion.
