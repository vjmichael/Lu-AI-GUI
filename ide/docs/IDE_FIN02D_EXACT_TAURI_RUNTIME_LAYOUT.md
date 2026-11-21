# FIN-02D — Exact packaged runtime placement for Tauri

Additional *opt-in* packaging configs:
- `tauri.fin02.windows.conf.json`: per-machine NSIS with existing WO7
  protected-service lifecycle hooks; resource tree goes to the app's fixed
  native resource directory.
- `tauri.fin02.linux.conf.json`: Debian package maps the source-staged
  Python/Node/CMake/LLDB trees to the exact root-owned `/opt/lu-ide/`
  paths that the protected Linux worker already hardcodes. The LSP
  resource tree is also explicitly bundled.

On Linux, FIN-02 compile-time admission now rehashes **both** the
source-staged payloads and the fixed provisioned `/opt` trees and
requires digest parity. This prevents a staged installer from silently
carrying different bytes from those whose pins were embedded. The
offline source verifier enforces the same relationship.

This is only a package layout/source gate. Tauri/DPKG cannot by itself
certify root-owned immutable modes, delegated cgroup-v2, protected
Bubblewrap process custody, physical native consent, unattended upgrades,
uninstall preservation, installed accessibility or dependencies. Those
remain blocked until independently exercised on actual Linux/Windows
installations. No binaries were staged in this repository commit.
