# FIN-06 — Human-owned integrated IDE workflow

Source implementation: seven explicitly navigable stages, saved-source
review → protected language diagnostics → reviewed Build/Test →
original-receipt test reporting → protected debugger → local Git review →
human task acceptance. Each stage navigates to the existing IDE
interface only; no stages can auto-save, launch a native process,
move source between projects, stage Git, install a dependency, or
approve/close a task.

Progress labels never count editor text, framework reports, DAP
acknowledgments, public CI or a completed native receipt as human
release acceptance. A current in-process native job receipt is
correlated with the original structured reporter before a test report
is exposed. Project switches invalidate the source Workbench and
therefore the journey.

The optional terminal continues as FIN-05/INT05's fixed typed
command broker, with no arbitrary shell or PTY. Production debugger
action authority and full shell PTY remain separate security
work needing native OS approval and genuine acceptance. No CI,
PR, native or physical installed tests run.

## FIN-06B — consolidated project-bound operational workflow

The existing seven-stage human navigation now has a separate read-only
native-readiness projection. It correlates the selected project with
local Git observations, native build-worker status, protected debugger
launch status, package-manifest readiness and global recovery/custody.
A project switch, foreign project signal or unresolved worker recovery
cannot be interpreted as a successful prior operation. Missing signals
remain unavailable. No simulated tool availability, native authorization,
package installation, finished debugger action or owner task acceptance
can be inferred from the projection.

The panel does not use Git status, compile results or debugger traces to
grant new permissions. The original Tauri per-action native confirmation
and host-side evidence validation remain mandatory. An interactive PTY,
installed accessibility, physical owner approval and production worker
admission remain separate open tasks. Source implementation does not
constitute installed acceptance.
