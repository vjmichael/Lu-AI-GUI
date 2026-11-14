#!/usr/bin/env python3
"""Render Ubuntu 24.04 VM cloud-init for an isolated, disposable Lu C4 host.

This is ONLY VM bootstrap; it never registers a runner, gets a GitHub token,
executes an original kernel test, or claims systemd delegation.

Inputs are local: an operator's PUBLIC ed25519 SSH key and a new output path.
No private keys, passwords, cloud API credentials, or GitHub tokens are read.
The VM must have a dedicated, host-enforced egress-only network with no
route to the operator's private LAN or metadata/management interfaces.
"""
import argparse
import json
import os
import pathlib
import sys

SOURCE_SHA = "2fb253ab35bc3b451b9c07b89f5549aec26c32e3"


def render(public_key: str) -> str:
    key = " ".join(public_key.strip().split())
    parts = key.split()
    if len(parts) < 2 or parts[0] != "ssh-ed25519":
        raise ValueError("An operator SSH ed25519 PUBLIC key is required")
    import base64
    try:
        raw = base64.b64decode(parts[1], validate=True)
    except ValueError as exc:
        raise ValueError("Invalid SSH public key base64") from exc
    if len(raw) < 48 or len(raw) > 256:
        raise ValueError("Implausible SSH public key length")
    if any(c in key for c in "\r\n\x00"):
        raise ValueError("Invalid public key characters")

    # YAML double-quoted string produced using JSON string encoding; source
    # SHA, packages, users and paths are deliberately fixed.
    return f"""#cloud-config
# Lu C4 disposable native Ubuntu VM. The operator must independently
# restrict the VM network and verify the base image before first boot.
hostname: lu-c4-disposable
preserve_hostname: false
disable_root: true
ssh_pwauth: false
package_update: true
package_upgrade: true
users:
  - name: lu-admin
    gecos: Lu C4 VM administrator
    shell: /bin/bash
    groups: [sudo]
    sudo: ['ALL=(ALL) NOPASSWD:ALL']
    lock_passwd: true
    ssh_authorized_keys:
      - {json.dumps(key)}
  - name: lu-c4
    gecos: Lu C4 single-use native CI worker, unprivileged
    shell: /bin/bash
    groups: []
    lock_passwd: true
packages:
  - ca-certificates
  - curl
  - git
  - build-essential
  - pkg-config
  - libssl-dev
  - clang
  - cmake
  - python3
  - python3-venv
  - python3-pip
  - jq
  - nodejs
  - npm
  - qemu-guest-agent
  - unzip
  - tar
  - xz-utils
  - zstd
write_files:
  - path: /etc/lu-c4-vm-role
    owner: root:root
    permissions: '0644'
    content: |
      role=disposable-c4-native-linux
      originalCoreSha={SOURCE_SHA}
      githubRunnerRegistration=NOT_AUTHORIZED_BEFORE_OS_PREFLIGHT
      rootOwnedBroker=NOT_PROVISIONED
runcmd:
  - [install, -d, -m, '0755', -o, root, -g, root, /srv/lu-c4]
  - [install, -d, -m, '0755', -o, root, -g, root, /srv/lu-c4/lu-main]
  - [install, -d, -m, '0700', -o, lu-c4, -g, lu-c4, /srv/lu-c4/runner]
final_message: 'Lu C4 VM bootstrapped. No runner registered and no kernel pass claimed.'
"""


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--admin-pubkey-file", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()

    keypath = pathlib.Path(args.admin_pubkey_file)
    if keypath.is_symlink() or not keypath.is_file():
        parser.error("public SSH key must be a regular file, not a symlink")
    if keypath.stat().st_size > 4096:
        parser.error("oversized public SSH key")
    try:
        key = keypath.read_text(encoding="ascii")
        output = render(key)
    except (ValueError, UnicodeError) as exc:
        parser.error(str(exc))

    target = pathlib.Path(args.output)
    target.parent.mkdir(parents=True, exist_ok=True)
    fd = os.open(target, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as stream:
            stream.write(output)
            stream.flush()
            os.fsync(stream.fileno())
    except BaseException:
        target.unlink(missing_ok=True)
        raise
    print("LU_C4_VM_CLOUD_INIT_RENDERED=1")
    print(f"CORE_SHA={SOURCE_SHA}")
    print(f"OUTPUT={target}")
    print("RUNNER_REGISTERED=0")
    print("KERNEL_EVIDENCE_ACCEPTED=0")
    return 0


if __name__ == "__main__":
    sys.exit(main())
