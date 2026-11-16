# CORE-MERGE-C4 — ephemeral delegated-Linux runner registration

**Status 2026-11-14:** registration is NOT COMPLETE. The Core source remains the
unchanged `2fb253ab35bc3b451b9c07b89f5549aec26c32e3` on existing PR #235.
This page and the companion registration helper are infrastructure preparation
on `main`, not execution receipts, another Core branch or a release approval.

## Provisioning before registration — admin-owned native VM

The reproducible VM preparation is now in
[`docs/LU_CORE_C4_ISOLATED_KVM_VM.md`](LU_CORE_C4_ISOLATED_KVM_VM.md),
backed by `scripts/lu-c4-create-kvm-vm.sh`,
`scripts/lu-c4-vm-cloud-init.py` and `scripts/lu-c4-vm-audit.py`.

**Do not attempt registration on the current ChatGPT container**: it lacks
`/dev/kvm`, libvirt VM control and a delegated cgroup-v2 mount. The VM
provisioner is intentionally Linux KVM-specific, and requires a separate
administrator-controlled hypervisor with an approved isolated network and
a checksum-verified Ubuntu cloud base. A generated cloud-init file, or a
VM bootstrap audit PASS, **does not prove** Lu's cgroup-v2 worker-kill
contract. The protected-ancestor/migration conflict below must still be
resolved with the original independent SEC-06C custody model.

## Important security boundary: Lu-AI-GUI is PUBLIC

GitHub expressly advises against leaving self-hosted runners on public repos.
Any workflow running on a self-hosted runner executes code on that machine.
For this one original kernel test, use **only a fresh, isolated, disposable
Ubuntu 24.04 (or equivalent) Linux VM**, with no personal files, other
workloads, SSH keys, long-lived cloud credentials, private network access, or
repository secrets. **No Docker, no permanent self-hosted runner, no runner
sudo access, no raw unrestricted `systemd-run` sudoers exception.**

GitHub references:
- https://docs.github.com/en/actions/how-tos/manage-runners/self-hosted-runners/add-runners
- https://docs.github.com/en/actions/reference/runners/self-hosted-runners
- https://docs.github.com/en/actions/reference/security/secure-use
- https://docs.kernel.org/admin-guide/cgroup-v2.html

## What is ready

- Manual-only workflow on `main`:
  `.github/workflows/lu-core-c4-delegated-kernel-original-sha.yml`.
  It checks out PR #235 at its **exact frozen SHA**, not current main.
- One native Linux job, consuming exactly one **ephemeral** runner and requiring
  both `lu-cgroup-v2-delegated` and a newly generated one-use
  `lu-c4-<24 lowercase hex>` label. No follow-on runner job remains.
- `scripts/lu-c4-ephemeral-runner-preflight.py`: fail-closed original-cgroup
  identity, nonroot account, rw cgroup2, systemd `user.delegate=1`, protected
  ancestor `cgroup.procs`, actual child `cgroup.kill` and kernel
  `populated 0`. It NEVER writes a PID to the protected ancestor.
- `scripts/lu-c4-register-ephemeral-runner.sh`: uses a hidden,
  short-lived token pasted **locally** by the VM operator. Registers with
  GitHub `config.sh --ephemeral`; never stores the token in the repository,
  creates an elevated service, or grants sudo.
- `scripts/lu-c4-ephemeral-runner.service.example`: a root-reviewed unit
  with `Delegate=yes`, a nonroot user, no added capabilities and an
  `ExecStartPre` check before the runner may receive code.

## Crucial blocker discovered before registration

**Plain systemd `Delegate=yes` is NOT sufficient for Lu's original legacy
kernel-custody test.** The exact original
`lu-linux-cgroup-guard` requires the runner's parent
`cgroup.procs` to be independently owned and non-writable by the nonroot
runner; normal systemd delegation ordinarily gives that same user access to
the delegated service root. Linux also requires permission to the common
ancestor's `cgroup.procs` when migrating a worker to a new child cgroup.

That means simply changing the owner/mode of the cgroup file to satisfy
Lu's protected-ancestor check can break its own worker migration. **Do not
relabel or chmod kernel interfaces, run the test as root, fake a witness, or
silently revert to same-UID process groups.** A source-level reconciliation
with the **independently root-owned SEC-06C process broker** may be needed to
prove both protected ancestry and real worker migration. Keep this as an
engineering gate in the canonical ledger until a genuine native run proves it.
The original source test is allowed to fail closed.

## Operator registration procedure (requires control of a Linux VM)

1. Prepare a clean disposable native Linux VM with systemd PID 1, unified
   cgroup-v2 and an actual read-write cgroup mount. Allocate enough disk and
   RAM for Rust native tests; do not give its runner user sudo. As VM
   administrator, create the dedicated `lu-c4` nonroot user and a private
   `/srv/lu-c4` directory. Place a clean checkout of this repo's `main`
   at `/srv/lu-c4/lu-main` and ensure the runner account can read the
   preflight and registration scripts. This is not a personal development VM.
2. In the VM administrator terminal, test the REAL systemd-delegated service
   boundary **before** fetching a registration token:

   ```bash
   sudo systemd-run --system --wait --pipe --collect --service-type=exec \
     --property=Delegate=yes --property=User=lu-c4 \
     --working-directory=/srv/lu-c4 \
     /usr/bin/python3 \
     /srv/lu-c4/lu-main/scripts/lu-c4-ephemeral-runner-preflight.py
   ```

   Only `DELEGATION_PREFLIGHT_ONLY` permits proceeding. A `BLOCKED`
   result is a real blocker, **not** a registration success. Normal
   `Delegate=yes` may fail for the protected-ancestor conflict above.
   Resolve that engineering/custody problem first rather than bypassing it.
3. In GitHub, go to **Actions → Lu C4 exact-SHA Linux delegated kernel
   acceptance → Run workflow**. Set a unique one-time label. Generate it
   locally as the runner account with:

   ```bash
   bash /srv/lu-c4/lu-main/scripts/lu-c4-register-ephemeral-runner.sh --new-label
   ```

   Enter that `lu-c4-...` value in the workflow's `one_time_label` input
   and queue the job. It will wait for its matching runner.
4. In GitHub, go to **Settings → Actions → Runners → New self-hosted
   runner → Linux → x64**. Follow GitHub's current download, **SHA-256
   verification**, unpacking and dependency instructions on that VM.
   Place the resulting GitHub runner files in `/srv/lu-c4/runner` with
   owner `lu-c4` and directory mode `0700`. Do NOT put the registration
   token in this document, a ticket, chat or GitHub Actions secrets.
5. In the VM's private runner directory as `lu-c4`, register once using
   GitHub's one-hour token from the UI (the script prompts without echoing):

   ```bash
   cd /srv/lu-c4/runner
   LU_C4_DISPOSABLE_VM_CONFIRM=YES bash \
     /srv/lu-c4/lu-main/scripts/lu-c4-register-ephemeral-runner.sh \
     --label lu-c4-REPLACE_WITH_THE_24_HEX_VALUE
   ```

   This creates a runner labeled `self-hosted`, `linux`, `x64`,
   **`lu-cgroup-v2-delegated`**, and the matching one-time label. It is
   **ephemeral** and automatically de-registers after one job.
6. Have the VM administrator review and install
   `scripts/lu-c4-ephemeral-runner.service.example` as a root-owned
   `/etc/systemd/system/lu-c4-runner.service`, then run `systemctl
   daemon-reload` and `systemctl start lu-c4-runner.service`. Do NOT use
   `svc.sh install` to create an unreviewed service or run the runner
   from an ordinary terminal without the delegated service boundary.
   Confirm the GitHub runner shows **Online** and has the required labels.
7. Require actual original GitHub artifacts with
   `AGENT01L_NATIVE_CGROUP_ESCAPED_DESCENDANT_PASS=1` AND passing reviewed
   Git cold-restart/replay-denial, matched to the exact unchanged Core SHA.
   Job `queued`, `offline`, `NOT_AVAILABLE`, skipped, or a green
   preflight alone does not close the gate. When the single job ends,
   verify automatic de-registration and **destroy/reset the VM**.

## Boundaries / independent gates

The one-shot public-repo runner has **no sudo permission**. The original
root-owned **SEC-06C full Linux matrix** invokes a privileged systemd
service and must be accepted separately on an operator-controlled native
host with reviewed root-owned provisioning. Do not grant unrestricted
`sudo /usr/bin/systemd-run` to a runner reachable from a public repository.
SEC-06F original hostile browser/Node and security matrix, Windows
SCM/Job/WFP, NSIS/EFS installation and physical Radeon Qwen 27B/Vulkan/MMProj
remain independently open.

GitHub runner registration requires the repository owner to create an
expiring token and a VM operator to install/start the actual runner. This
GitHub connection cannot administer repository Actions runners or access
your Linux VM, so neither registration nor positive kernel custody is
claimed merely because these setup files are committed.
