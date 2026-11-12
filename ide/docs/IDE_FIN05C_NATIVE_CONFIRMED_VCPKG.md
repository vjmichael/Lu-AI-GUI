# FIN-05C — Human-confirmed direct vcpkg manifest change

The separately reviewed FIN-05B exact JSON draft now has a distinct
native one-use confirmation and source replacement action. The operator
sees the original and proposed source SHA and the complete proposed
vcpkg.json in the Packages pane; any existing open vcpkg.json CodeMirror
tab disables the package edit to avoid overwriting an unsaved buffer.

Native Tauri owns a process-local global single-edit gate, revalidates the
package preview and exact source hash before showing a fresh OS modal,
refuses timeout/decline, remeasures all source+package evidence after the
dialog, then uses the existing revision-checked capability-scoped source
save. A WebView boolean is never an approval token. No package tool
is started, and the receipt distinguishes changed declaration from
unresolved/uninstalled dependencies.

Still open: full package Add/Remove/Update **resolution and installation**,
vcpkg/Conan native protected sandboxed workers, wheel provenance closure,
Windows offline uv ACL, and independent native/installed OS-consent matrix.
CI/PR and acceptance deferred by owner.
