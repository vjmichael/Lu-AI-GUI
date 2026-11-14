# IDE-05.6 Task 5 installed desktop acceptance group

**Test configuration:** one consolidated Windows 2022 + Ubuntu 24.04
native installed-app group, not release promotion. The jobs reuse the
previously authored real unsigned WO8 installers and UI instrumentation
with exact current source checkout, resolved upstream license notices,
pre/post installed program evidence and actual OS package/uninstall
checks. They use the **default-disabled** Tauri feature set, never
`native-package-uv`, `ide055-ci-wheelhouse`, or an unapproved Windows
AppContainer package worker.

## Gates

- Windows: build an actual per-user unsigned NSIS bundle, verify real
  bundled third-party license inventory and original upstream notice
  bytes, install and launch `lu-ide-desktop.exe`, operate genuine OS
  UI Automation / WebView navigation, project folder picker, persistent
  editor and external-source-conflict controls; verify default protected
  debugger execution denied and uninstall removal.
- Linux: build the actual unsigned Debian package with verified license
  notices, use native `dpkg` install and tested hostile upgrade/rollback,
  launch an actual GTK/X11 desktop and project smoke, and confirm package
  removal without treating personal project state as disposable.
- Both: require exact source SHA, actual build and executable evidence;
  do not use source-only assertions as proof of a real installed app.

**Scope deliberately limited:** passing those jobs proves default
unsigned desktop installation and UI baseline on hosted Windows/Linux
runners. It does **not** prove native protected uv execution in that
installed app, independent real human approve/decline/timeout/cancel,
a reviewed third-party wheel inventory, Windows package-SID read-only
ACLs, real positive AppContainer write-denial, physical operator PC
acceptance or signed release. These must be separate Task 5 gates.

The installed workflow is
`.github/workflows/ide-dev056-installed-native.yml`.
Use the actual exact-head job logs and uploaded installer/receipt
artifacts before marking this CI-only subgate PASS. A failed or queued
job remains OPEN. This work is kept in one installed task-group
workflow, not a queue of tiny CI probes.
