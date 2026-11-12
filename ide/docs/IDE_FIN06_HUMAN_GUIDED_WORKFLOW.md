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
