# B4 — Split editor: distinct native-registered CodeMirror source tabs

**Status:** source implementation committed on a separate B4 branch stacked on B2 #145.
Windows/Linux exact-head native source/build tests and real installed GUI acceptance
must pass before the feature is considered accepted.

## Implemented human workflow

In an open project with an existing source tab, choose **Split editor**. Select
a *different* already-open source file in the **Second editor** chooser to show
it beside the main editor. Both panes are real editable CodeMirror instances
with separate project-scoped paths, independent states, revisions, cursors,
undo/redo stacks and B2 private unsaved-draft identities. The user can select
**Save second file** or use Ctrl+S while typing in the second CodeMirror pane;
the keyboard event resolves to the **focused pane** rather than always saving
the primary tab. The other pane remains unsaved. **Make primary** exchanges
the selected tabs rather than mounting the same state into two EditorViews.

Closing a file or project invalidates its pane and editor identity. Switching
projects closes the split layout and prevents using a secondary tab belonging
to the previous registered project. The renderer refuses a duplicate pane
for the same tab. The original single-pane editor remains full-width when
the split is disabled. Narrower windows stack the two panes vertically.

## Boundaries

- No new native IPC operation, model invocation, execution grant, native
  source path or filesystem capability.
- No hidden Save or automatic merge. All writes still use existing B2 native
  optimistic file revision checking.
- No copy of source text or B2 buffer identity is made for the second pane.
  Split panes are two *different* already-open, already-authorized tabs.
- The currently selected primary tab owns the existing Find/Replace and LSP
  actions. For operations on the second tab, choose **Make primary** first.
- This B4 source work is independent of the B2 compiled, private-recovery
  and installed-app gates and cannot be used to close them.

## Acceptance

1. Pure JavaScript tests in `ide/app/tests/editor_layout.test.mjs` prove
   project scoping, distinct identities, close/switch invalidation and focused
   Save targeting.
2. Native CodeMirror frontend tests assert two independent EditorViews,
   updated source identity, and no changes to worker/process/tool rights.
3. New `.github/workflows/ide-b4-split-editor-native.yml` executes the
   actual Windows 2022 and Ubuntu 24.04 frontend, host Rust tests and locked
   native Tauri compile, retaining unchanged reviewed dependencies.
4. **Open:** Genuine unsigned installed NSIS + Debian/GTK UI interaction:
   select/open two files, edit in each, Undo independently, save secondary
   without touching primary, reopen both and verify source hashes and clean
   worker recovery. Native package install/OS UI evidence required. Missing
   installed UI or a pending CI run is not evidence of acceptance.

## Actual installed OS acceptance harness committed

- **Windows:** `.github/workflows/ide-b4-installed-editor-windows.yml`
  packages an unsigned NSIS with exact upstream and third-party notice
  verification and invokes `Test-Wo8NativeDesktopUI.ps1 -Scenario B4SplitEditor`
  after genuine install. That new scenario requires two distinct accessible
  installed CodeMirror surfaces, physically edits the secondary source and
  proves that neither project file was implicitly saved, then explicitly
  clicks **Save second file** and verifies only the secondary project's bytes
  changed. Finally, it physically edits the primary view, sends Ctrl+S to
  the focused CodeMirror view, verifies only that file changed, and requires
  actual NSIS uninstall.
- **Linux:** `.github/workflows/ide-b4-installed-editor-linux.yml`
  packages actual Debian, verifies original notice bytes, installs with dpkg
  and launches GTK/WebKit under real X11/AT-SPI. The
  `b4_linux_installed_split_atspi.py` fixture reuses the genuine native
  GTK folder chooser and native editor controls, selects the second source,
  edits each visible CodeMirror surface, verifies source hashes before Save,
  checks exact independently saved files, closes the desktop process, and
  requires real Debian package removal.
- Both workflows fail closed if the operating system does not expose two
  actual editable surfaces, if focus cannot be verified, if a file is
  unexpectedly written, if licensing evidence is absent, or if the installed
  desktop cannot launch/uninstall. Source implementations are committed, but
  real **passing exact-head OS job outcomes have not been obtained** and the
  B4 acceptance gate remains **OPEN**.

## Protection against misleading source gate checks

The B4 native workflow checks the exact immutable B2 parent
`97940239e1af4a6c8edbdfa3a176579d01ae5f18` as an ancestor and compares
actual changed source trees, including reviewed native worker and file-write
modules. It no longer checks only that the freshly checked-out working tree
is clean, which would be true of unrelated potentially dangerous changes.
