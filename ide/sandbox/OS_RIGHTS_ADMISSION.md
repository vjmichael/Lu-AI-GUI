# Wave D4 — Native OS rights and quota admission probes

These are experimental *fixed* OS checks under `ide/sandbox`, **not a
production runner**. Their only purpose is to establish what the native
platform can actually enforce. They have no Tauri integration or user/model
command input.

## Windows: SID-restricted token

`src/bin/windows_restricted_token_probe.rs` opens its own process token,
uses `CreateRestrictedToken` with `DISABLE_MAX_PRIVILEGE` and the fixed
Restricted Code SID `S-1-5-12`, and requires `IsTokenRestricted` to succeed.
The probe does not create any child process or use an external executable.

**Limits:** restricted SID presence/privilege removal alone is not proof of
low integrity, AppContainer confinement, secure filesystem/network ACLs, or
that a child can be safely started with that token. These remain explicit
unverified gates. The separate Windows Job Object probe covers process
lifecycle and descendant kill, but not rights confinement.

## Linux: delegated cgroup-v2 controllers

`src/bin/linux_cgroup_probe.rs` derives the **current process's** cgroup
from `/proc/self/cgroup`, ensures its canonical path remains under
`/sys/fs/cgroup`, verifies cpu/memory/pids controllers and attempts to
create a unique, empty sub-cgroup. It writes and reads back fixed CPU,
memory and PID ceilings, ensures `cgroup.procs` and `cgroup.kill` exist,
then removes the empty cgroup.

When delegation is absent or quotas cannot be applied, the probe explicitly
prints **DENIED** and does not start anything. A workflow step completing
after that denial **does not mean** Linux containment was accepted.

**Limits:** configuring an *empty* cgroup does not prove attaching a
suspended worker atomically before exec, namespace/bwrap integration,
cgroup-wide kill across host failure, or safe file/network credentials.
Linux Bubblewrap's earlier fixed OS probes remain separate.

## Acceptances still open

- Windows low integrity/AppContainer or equivalent, restricted child
  activation, filesystem+network rights tests, OS quotas, and hostile fixtures.
- Linux delegated pre-exec cgroup placement, PID/memory/CPU enforcement under
  actual load, child/descendant cancellation and crash orphan cleanup.
- A trusted source/toolchain snapshot and immutable output staging; protected
  effect journal broker; one-shot grants consumed by a real OS-bound worker.
- Installed Windows GUI acceptance.

The production supervisor remains hard-disabled via
`production_worker_admission()`. A probe success does **not** add `job.run`
or enable any project execution in Lu IDE.
