#!/usr/bin/env bash
# Administrative provisioning for a fresh native Ubuntu cloud-image KVM VM.
# Does NOT register a runner, configure a privileged cgroup broker or claim a
# positive kernel receipt. Do not use Docker/LXC/container virtualization.
set -euo pipefail
umask 077

die() { printf 'LU_C4_VM_CREATE_BLOCKED: %s\n' "$*" >&2; exit 1; }
usage() {
  cat <<'EOF'
Usage:
  scripts/lu-c4-create-kvm-vm.sh --ubuntu-qcow2 IMAGE \
    --image-sha256 64_HEX --admin-ed25519-pubkey PUBLIC_KEY_PATH \
    --egress-only-network ADMIN_APPROVED_LIBVIRT_NETWORK \
    --vm-directory EMPTY_NEW_DIRECTORY

Prerequisites: Ubuntu 24.04 x86_64 cloud qcow2 image with independently
verified provenance, virt-install/libvirt/KVM/cloud-localds/qemu-img, and a
dedicated egress-only network whose HOST firewall denies LAN, metadata,
hypervisor management and other internal addresses. No password or token.
The host administrator must set LU_C4_EGRESS_BOUNDARY_ATTESTED=YES only after
checking the firewall outside the guest.
EOF
  exit 2
}

[[ "\${LU_C4_EGRESS_BOUNDARY_ATTESTED:-}" == YES ]] ||
  die 'admin must first attest dedicated hypervisor-enforced egress isolation'
image='' sha='' pubkey='' network='' workdir=''
while (($#)); do
  case "$1" in
    --ubuntu-qcow2) image="\${2:-}"; shift 2;;
    --image-sha256) sha="\${2:-}"; shift 2;;
    --admin-ed25519-pubkey) pubkey="\${2:-}"; shift 2;;
    --egress-only-network) network="\${2:-}"; shift 2;;
    --vm-directory) workdir="\${2:-}"; shift 2;;
    *) usage;;
  esac
done
[[ -n "$image" && -n "$sha" && -n "$pubkey" && -n "$network" && -n "$workdir" ]] || usage
[[ "$sha" =~ ^[a-f0-9]{64}$ ]] || die 'invalid full base-image SHA256'
[[ "$network" =~ ^[a-zA-Z0-9][a-zA-Z0-9_-]{0,47}$ ]] || die 'unsafe network name'
for tool in python3 virsh virt-install cloud-localds qemu-img sha256sum readlink stat; do
  command -v "$tool" >/dev/null || die "host prerequisite missing: $tool"
done
[[ -r "$image" && -f "$image" && ! -L "$image" ]] ||
  die 'base image must be an existing, regular, non-symlink qcow2 file'
[[ -r "$pubkey" && -f "$pubkey" && ! -L "$pubkey" ]] ||
  die 'admin ED25519 public key must be a regular, non-symlink file'
base="$(readlink -f "$image")"
[[ -n "$base" ]] || die 'cannot canonicalize base image'
printf '%s  %s\n' "$sha" "$base" | sha256sum --check --status ||
  die 'original Ubuntu cloud image digest mismatch; do NOT boot'
info="$(qemu-img info --output=json "$base")"
python3 -c 'import json,sys; info=json.load(sys.stdin); sys.exit(0 if info.get("format")=="qcow2" else 1)' <<< "$info" ||
  die 'trusted base image is not qcow2'
[[ -x scripts/lu-c4-vm-cloud-init.py || -f scripts/lu-c4-vm-cloud-init.py ]] ||
  die 'run from an administrator-reviewed Lu repository checkout'

# GitHub's public repo must never be able to run arbitrary workflows in a
# privileged host. This VM has its own disposable disk and separate egress.
if [[ "$(virsh net-info "$network" 2>/dev/null | awk '/^Active:/{print $2}')" != yes ]]; then
  die 'admin-reviewed isolated egress network is not active in libvirt'
fi
if virsh dominfo lu-c4-disposable >/dev/null 2>&1; then
  die 'VM name already exists; never reuse a previous GitHub runner VM'
fi
[[ ! -e "$workdir" ]] || die 'VM workspace must be a brand-new path'
mkdir -m 0700 "$workdir"
workdir="$(readlink -f "$workdir")"
mkdir -m 0700 "$workdir/metadata"
python3 scripts/lu-c4-vm-cloud-init.py \
  --admin-pubkey-file "$pubkey" --output "$workdir/metadata/user-data"
printf 'instance-id: lu-c4-disposable-%s\nlocal-hostname: lu-c4-disposable\n' \
  "$(date -u +%Y%m%d%H%M%S)" > "$workdir/metadata/meta-data"
chmod 0600 "$workdir/metadata/"*
cloud-localds "$workdir/seed.iso" "$workdir/metadata/user-data" "$workdir/metadata/meta-data"
chmod 0600 "$workdir/seed.iso"
qemu-img create -f qcow2 -F qcow2 -b "$base" "$workdir/disk.qcow2" 80G >/dev/null

# KVM is kernel-backed; --import avoids booting a mutable installer from this
# public repository. Egress network policy is still host-admin responsibility.
virt-install --name lu-c4-disposable --memory 8192 --vcpus 4 \
  --import --os-variant ubuntu24.04 --virt-type kvm \
  --disk "path=$workdir/disk.qcow2,format=qcow2,bus=virtio" \
  --disk "path=$workdir/seed.iso,device=cdrom,readonly=on" \
  --network "network=$network,model=virtio" \
  --graphics none --console pty,target_type=serial --noautoconsole

printf 'LU_C4_ISOLATED_KVM_VM_DEFINED_AND_STARTED=1\n'
printf 'VM=lu-c4-disposable\nIMAGE_SHA256=%s\nVM_DIR=%s\n' "$sha" "$workdir"
printf 'CORE_CANDIDATE_SHA=2fb253ab35bc3b451b9c07b89f5549aec26c32e3\n'
printf 'RUNNER_REGISTERED=0\nKERNEL_CUSTODY_ACCEPTED=0\n'
printf 'Next: verify cloud-init completion, run Lu VM audit, and inspect the\n'
printf 'original protected-ancestor preflight BEFORE any GitHub registration.\n'
