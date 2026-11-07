# IDE Execution Security Model — design, 2026-11-04

## Threats
Malicious cloned projects, source code, package installer hooks, build systems, test suites, LSP/debug adapters, extensions, compromised compilers, prompt injection embedded in project files, hostile repositories/branches and credential exfiltration; accidental destructive operations and runaway resource use.

## Trust levels
UNTRUSTED (default): allow preview/read/edit under explicit project scope but no automatic tasks, terminals, builds, installs, LSP workers requiring execution, debug adapters or extensions.
REVIEWED: specific worker operation may be approved with narrow target/path/network/time/resource scope; project-wide trust is not an unlimited shell grant.
TRUSTED DEVELOPMENT: normal workflows under bounded host grants with UI warning for high-impact operations. Trust can be revoked, expires/revalidates on changed provenance and must never imply admin/root access.
ISOLATED HIGH-RISK: use disposable VM/microVM or equivalent OS-backed stronger isolation for deliberately hostile code when supported; fail closed if unavailable, do not silently fall back.

## Required enforcement
- Source of authority: authenticated human turn/user click; manager/agent/repo metadata never upgrades grants.
- Host-brokered handles and path validation including symlink/reparse, traversal, TOCTOU and race tests.
- OS-enforced separation: Linux namespace/mount/PID/network restrictions via bubblewrap-like launcher where verified, plus Landlock/seccomp/no-new-privileges/cgroup quotas as available; verify kernel and distribution features. Bubblewrap is a toolkit, not a security policy or automatic guarantee. Windows use restricted tokens/AppContainer when viable, Job Objects for child cleanup/quotas, and explicit ACL/network restrictions; Job Objects alone are not an execution sandbox.
- Worker no direct access to `~/.ssh`, cloud keys, browser profiles, Lu private databases, system settings, unrelated projects or privileged sockets. Isolate temp/home; sanitize environment; close inherited handles/fds; no root/admin.
- Default no network for code execution. For fetching dependency metadata/downloads use constrained broker and registry policy; avoid passing tokens into untrusted workers. Network availability must be visible and grantable per operation.
- Host controls child kill, timeouts, CPU/memory/file/pid constraints as available; preserve logs and record terminal job outcome.
- IDE extension/plugin host separate from trusted shell; publisher/install review and explicit scoped capabilities. Never assume an extension respects UI trust labels.
- Protect disk: readonly source view as default for unknown code, writable project overlay/copy for risky runs, immutable backup/snapshot before destructive changes; restrict output paths.
- Static scanners, dependency advisories and malware checks are defense in depth, not evidence that code is safe.
- No permission laundering through prompt injection: untrusted file text and tool outputs cannot authorize new tools, hosts, files or networks.

## Nonnegotiable adversarial acceptance
Open hostile repo and prove zero executed commands/network requests; package postinstall tries to read `~/.ssh` and is denied; build tries to delete home/outside root and is denied; symlink replacement attack; runaway fork bomb/CPU/RAM stress bounded; rogue child process killed on cancellation; network exfiltration blocked; prompt injection in README denied; malicious LSP and debug adapter contained; failed sandbox setup rejects execution; crash recovery does not retry uncertain external effects.
Each claim requires real OS-level tests on Linux and Windows, not mocks.

## Important limitations
No IDE can guarantee arbitrary native code is harmless. Users can intentionally grant risky powers; kernel/driver faults and sandbox escape vulnerabilities remain possible. An approved command running as the host user without OS isolation cannot be made safe by AI review or JavaScript checks. Prefer stronger disposable environments for malicious code.

## Research
- https://code.visualstudio.com/docs/editing/workspaces/workspace-trust
- https://code.visualstudio.com/docs/configure/extensions/extension-runtime-security
- https://github.com/containers/bubblewrap
- https://docs.kernel.org/userspace-api/landlock.html
- https://contribute.cncf.io/projects/best-practices/security/supply-chain/supply-chain-best-practices/
