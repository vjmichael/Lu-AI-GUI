# Windows Job Object containment probe — Wave D2-W1

**Experimental, fixed-fixture only, not a production worker.** This standalone
Rust binary is part of `ide/sandbox`, but it is **not** imported by Lu IDE's
Tauri app or host and cannot execute project code or arbitrary user commands.
It is not a Windows AppContainer, restricted token, or filesystem/network
sandbox.

The fixture exercises three actual Windows native OS behaviors:

1. **Suspended admission:** `CreateProcessW(CREATE_SUSPENDED)` for a fixed
   copy of its own executable, set Job Object limits, assign process, then
   `ResumeThread`. Any creation/assignment/limit failure aborts without
   uncontained process startup.
2. **Deadline kill:** a fixed sleeping child is terminated with
   `TerminateJobObject`, then the supervisor verifies it exits.
3. **Descendant cleanup:** the fixed child spawns a fixed grandchild, which
   is verified terminated after the Job Object is killed.

The Job Object uses `KILL_ON_JOB_CLOSE`, a 128 MiB job-memory cap and a
four-active-process cap. The probe clears all inherited environment variables
except sanitized `SystemRoot`, `TEMP` and `TMP` needed for its test marker.
It passes no inherited handles. Only compiled-in probe flags are accepted.

Run on Windows:

```powershell
cargo fmt --manifest-path ide/sandbox/Cargo.toml -- --check
cargo run --manifest-path ide/sandbox/Cargo.toml --bin windows_probe --release
```

The GitHub workflow `.github/workflows/ide-sandbox-windows.yml` runs these
tests on a real Windows runner. Its success proves only these fixed OS
behaviors on that runner; it is not a license to run arbitrary builds, language
servers or package scripts. Windows native Job Objects provide lifecycle and
quota containment, **not** privileged filesystem or network isolation.
D2's remaining gates include restricted token or equivalent access-control
policy, verified identity/working-directory admission, job cancellation race
tests, log and artifact limits, crash recovery, and trusted input snapshots.
Linux has a separately tested Bubblewrap fixture and still needs stronger
resource quotas and full descendant/native worker tests.

**No UI or Lu Core bridge is attached to this probe.**
