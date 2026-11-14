# Lu CORE-MERGE-C4 — operator-controlled isolated Linux VM build

**Engineering status (2026-11-14): VM provisioning source prepared; no VM
created in this chat, no native kernel receipt and no runner registration.**
Lu Core remains immutable at
\`2fb253ab35bc3b451b9c07b89f5549aec26c32e3\` (existing PR #235).
These setup files live on \`main\` and do not change the accepted source.

## VM design and threat boundary

- **Hypervisor:** native Linux **KVM/libvirt**, not Docker/LXC or a shared
  container. A root/admin-controlled Linux KVM host is required; the current
  ChatGPT container has NO \`/dev/kvm\`, \`qemu-system-x86_64\`, or libvirt VM
  control. Therefore it cannot allocate this VM itself.
- **Guest:** fresh Ubuntu 24.04 LTS amd64 cloud image, 4 vCPU, 8 GiB RAM,
  80 GiB private qcow2 overlay, systemd PID 1, unified kernel cgroup-v2.
- **Admin:** separate, SSH-ed25519-key-only \`lu-admin\` account with
  administrator permission. Its public key is injected by cloud-init
  *locally*; private keys and passwords are NEVER committed to GitHub.
- **Worker:** \`lu-c4\` account without sudo/admin groups; private
  \`/srv/lu-c4/runner\` directory, no GitHub registration at boot.
- **Network:** dedicated hypervisor-enforced egress-only network. An operator
  must inspect firewall/route rules that **deny any access from VM to host
  management, local/private subnets, cloud metadata, privileged control
  systems or other tenants**. Do not use libvirt \`default\` NAT as proof of
  this. Outbound DNS/HTTPS for Ubuntu updates, GitHub and pinned toolchain
  downloads must be explicitly reviewed. No inbound internet exposure and
  no sensitive files on the guest. The bootstrap script requires an
  explicit network-attestation environment variable but **cannot audit the
  host firewall from inside the VM**.
- **Release policy:** GitHub repository is public. Even a one-job ephemeral
  self-hosted runner can execute hostile PR workflow code. An operator
  must first validate isolation; the recommended strongest alternative
  is an **offline native acceptance VM**, transferring original
  source-locked results out through administrator-reviewed media rather than
  attaching the privileged host to the public repository. Any runner
  registration is optional and **never a kernel acceptance by itself**.

## 1. Obtain a verified base image and isolated network (Linux KVM host)

Install/administer \`qemu-kvm\`, \`virtinst\`, \`libvirt-daemon-system\`,
\`cloud-image-utils\`, \`qemu-utils\`, \`python3\`, \`virsh\` and \`git\`
through the Linux host OS. Create a dedicated network with upstream host
firewall policy as described above. Verify the network is active with
\`virsh net-info YOUR_LU_C4_EGRESS_NETWORK\`; do not treat “active” as
a firewall assessment.

Obtain the official Ubuntu 24.04 *released* amd64 qcow2 image from
\`https://cloud-images.ubuntu.com/releases/noble/release/\`:
\`ubuntu-24.04-server-cloudimg-amd64.img\`. Verify the image against
Ubuntu's **signed** \`SHA256SUMS\` /\`SHA256SUMS.gpg\` with a separately
trusted Ubuntu signing key. Preserve the exact 64-character checksum and
provide it to the provisioning script. Do not download an unverified VM
image, use a guessed digest or skip the check.

The administrator must have an existing **public** ED25519 SSH key file
(not a private key). Provisioner rejects malformed public keys and any
previously used VM directory.

## 2. Define and start the disposable VM

On the KVM host from an administrator-reviewed checkout of this repository's
\`main\`, use the actual verified image checksum and private egress-only
network. Here the variables are **placeholders**, not runnable values:

\`\`\`bash
# Run only after verifying the upstream host firewall and signed Ubuntu image.
sudo env LU_C4_EGRESS_BOUNDARY_ATTESTED=YES \
  bash scripts/lu-c4-create-kvm-vm.sh \
  --ubuntu-qcow2 /verified/ubuntu-24.04-server-cloudimg-amd64.img \
  --image-sha256 YOUR_VERIFIED_IMAGE_SHA256 \
  --admin-ed25519-pubkey /secure/operator/id_ed25519.pub \
  --egress-only-network YOUR_LU_C4_EGRESS_NETWORK \
  --vm-directory /var/lib/libvirt/images/lu-c4-new-disposable \
  --qemu-service-group libvirt-qemu
\`\`\`

The QEMU service group may have a different name on another distribution.
The script checks that group exists, independently hashes the base image,
checks qcow2 format, creates a *new* disk directory, protects the base and
seed from other host users, creates a qcow2 overlay and cloud-init ISO,
then calls \`virt-install --import --virt-type kvm\` with virtio networking.
The administrator must ensure the full VM storage path is searchable by the
actual QEMU service group. It refuses overwriting an existing VM or directory.

If the script fails, the VM must NOT be considered ready. Inspect and
delete any partial staging resources **only after confirming they belong to
this attempted VM**, not by a broad destructive cleanup command.

## 3. Complete guest bootstrap and audit

Use the VM administrator's own restricted access path. After first boot:

\`\`\`bash
cloud-init status --wait
sudo git clone https://github.com/vjmichael/Lu-AI-GUI.git /srv/lu-c4/lu-main
python3 /srv/lu-c4/lu-main/scripts/lu-c4-vm-audit.py
\`\`\`

The audit confirms it is a real systemd VM, not a container, that kernel
cgroup-v2 is unified and writable, that the bootstrap marker references
the exact original Core SHA, that \`lu-c4\` has no sudo/adm group, that the
runner directory is owner-only and unregistered, and that first-party native
tool prerequisites exist. It reports
\`networkBoundaryVerified=false\`,
\`systemdDelegationVerified=false\`,
\`githubRunnerRegistered=false\`,
\`originalKernelCgroupReceipt=false\` even on success. The network
must be independently audited on the hypervisor.

## 4. Prove OS custody BEFORE any runner registration

The host administrator must next run Lu's actual
\`scripts/lu-c4-ephemeral-runner-preflight.py\` inside a
\`Delegate=yes\` service using the non-root \`lu-c4\` worker, per
\`docs/LU_CORE_C4_EPHEMERAL_LINUX_RUNNER.md\`. It must return a genuine
protected-ancestor and writable-child result, not merely a systemd service
startup. Do not use \`sudo\` as the Lu worker, grant arbitrary
\`systemd-run\`, or \`chmod\` any kernel \`cgroup.procs\` to force success.

**Known architectural conflict:** ordinary \`Delegate=yes\` gives the
same user writable control over the service's delegated root, while Lu's
legacy \`lu-linux-cgroup-guard\` requires a non-writable source ancestor
to deny worker escape. Cgroup-v2's common-ancestor migration rule may
make these requirements unsatisfiable under the same UID. Do not work
around this by changing file permissions alone. The source-level
integration path is the existing **independently root-owned SEC-06C
service boundary**, which must authorize/attach workers BEFORE untrusted
execution. The frozen original C4 candidate must not be claimed to pass
unless it genuinely does; a source repair would require a new coherent
source SHA and full same-SHA acceptance, without a new branch/PR.

## 5. Optional one-use public-repo GitHub runner, only after the gates above

Use \`scripts/lu-c4-register-ephemeral-runner.sh\` and the root-reviewed
systemd unit template in the linked runner guide. The workflow on \`main\`
requires both the stable label \`lu-cgroup-v2-delegated\` and a unique
\`lu-c4-<24hex>\` label, with \`config.sh --ephemeral\`. The operator
generates the one-hour GitHub registration token **locally in GitHub UI**,
never provides it to this chat and never puts it in a workflow. Do not
enable the runner as a permanently connected public-repo machine.
Destroy the VM after one job.

The original custody test must actually prove SIGKILL of the Lu process,
\`setsid\` escaped descendant containment, kernel \`cgroup.kill\`, recursive
\`populated 0\`, unaffected control process, original exact-turn receipt,
Git cold restart and replay denial. Linux SEC-06F, physical Windows
Job/SCM/WFP, NSIS and owner Qwen27B/Vulkan/MMProj remain separately open.

## Sources and administrative constraints

- Ubuntu official cloud image listing:
  https://cloud-images.ubuntu.com/releases/noble/release/
- Ubuntu local cloud-init datasource:
  https://ubuntu.com/docs/public-images/public-images-how-to/use-local-cloud-init-ds/
- Linux native cgroup v2 delegation and common-ancestor rule:
  https://docs.kernel.org/admin-guide/cgroup-v2.html
- GitHub public-repo self-hosted runner warning:
  https://docs.github.com/en/actions/reference/security/secure-use
- GitHub ephemeral runner registration:
  https://docs.github.com/en/actions/reference/runners/self-hosted-runners

**Result classification:** provisioner files committed and statically
reviewed; no KVM device or VM administrator is connected to this ChatGPT
environment, so VM_CREATED=NO, RUNNER_REGISTERED=NO, NATIVE_KERNEL_RECEIPT=NO,
FULL_C4_ACCEPTANCE=OPEN.
