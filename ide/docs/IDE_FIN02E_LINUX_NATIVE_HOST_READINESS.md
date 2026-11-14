# FIN-02E — Truthful native Linux host admission before readiness

A new read-only `linux_worker::native_delegated_host_readiness`
checks the **actual process kernel cgroup identity**, exact
`lu-ide-delegated.service/lu-ide-host` topology, all three enabled
delegated controllers, empty delegated parent, fixed root-owned
`/usr/bin/bwrap`, and kernel host leaf files. No directory or
process is created or moved; the check cannot consume a job grant.

The desktop Build/Test and multilang/uv readiness classifications now
require this check on Linux *in addition* to independently hash-pinned
installed resources. A normal uncontained GUI process does not display
a green launchable worker merely because system binaries exist. Windows
still requires per-job Windows recovery-service and Job Object checks;
Windows uv remains disabled for lacking source AppContainer ACL.

The missing production Linux delegated resident host service and
installer-provisioned pinned binary are **not** created by this
observation. The correct release architecture requires a trusted
host process/IPC bridge started by the delegated systemd service;
a Tauri GUI process running outside the host leaf must not execute
privileged worker code or claim Linux native admission. Native and
installed verification remain open.
