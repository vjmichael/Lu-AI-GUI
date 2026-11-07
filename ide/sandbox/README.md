# Lu IDE — Linux execution isolation probes (Task 2)

**Status: experimental, not production sandbox acceptance.**

This crate implements *only* a fixed set of Linux Bubblewrap isolation probes. It is intentionally **not** linked into Lu IDE's UI, package manager, build runner, terminal, LSP or debugger. There is **no API to run arbitrary commands**.

## Security properties under test

- Run only with a root-owned, non-setuid, non-group/world-writable `/usr/bin/bwrap` launcher.
- Create new namespaces including network, PID and user namespaces, and explicitly disable nested user namespace creation; no fallback to unsandboxed execution.
- Bind minimal system tool directories read-only, stage the selected project fixture at `/workspace` read-only, and provide ephemeral `/tmp` and empty `/home`.
- Clear the inherited environment; do not mount host HOME, process broker sockets, SSH keys or Lu data.
- Terminate a fixed long-running test process on timeout; use a new terminal session and parent-death cleanup. Full descendant cleanup and resource quota guarantees remain unverified.

## Verification

```sh
cargo test --manifest-path ide/sandbox/Cargo.toml --all-targets
cargo run --manifest-path ide/sandbox/Cargo.toml --bin probe
```

The second command is meaningful only on a Linux host where `/usr/bin/bwrap` exists and unprivileged namespace creation is allowed. AppArmor and distribution policies sometimes deny this. Such a result must be reported as **isolation unavailable**, not a reason to loosen security settings or execute the command on the host.

The static tests verify argument selection, but do not prove namespace isolation. The [five native probe results](../docs/IDE_02_SECURITY_PROBE_EVIDENCE.md) test namespace setup, host secrets, read-only mounts, network isolation and timeout behavior on a real Linux kernel. The executable probe attempts to verify hidden host paths, read-only project files, and blocked external connectivity. **The project should not enable arbitrary code workers until stronger adversarial testing, cancellation, disk/memory/pid limits, mounted-root race checks, and independent Windows containment pass.** Bubblewrap is only one layer.

References: https://github.com/containers/bubblewrap and https://docs.kernel.org/userspace-api/landlock.html
