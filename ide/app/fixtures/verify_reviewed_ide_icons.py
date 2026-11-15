#!/usr/bin/env python3
"""Fail closed unless exact, tracked first-party Lu IDE icon bytes are present.

This is asset provenance only. It is not native custody, installed package,
signing, legal review, or human release authorization. No synthetic CI icons.
"""
import argparse
import hashlib
from pathlib import Path
import re
import stat
import struct
import subprocess
import sys

PINS = {
    "ide/app/scripts/make-icon.mjs": (
        "ef7790ac656ea6b59067f00bf4be959eec3f99bb", None
    ),
    "ide/app/src-tauri/icons/icon.ico": (
        "9ef9713a2ab32ea29244c9b65b04f1f16245b92a",
        "c7cffbab57aac9ca8ccf2182a1d1130d401448c7e8be3849a4228f049e9e8391"
    ),
    "ide/app/src-tauri/icons/icon.png": (
        "66d0c1b7fe0448f50055995ef3a49bddb34262a6",
        "7849faf6dd0e2aa0417cee24b9f09c841ae8e4374f1486ccc59418c6784d6327"
    ),
}
ICON_PATH = "ide/app/src-tauri/icons/icon.ico"
PNG_PATH = "ide/app/src-tauri/icons/icon.png"


class Denied(ValueError):
    pass


def git_head_blob(root, path):
    proc = subprocess.run(
        ["git", "ls-tree", "HEAD", "--", path], cwd=root,
        text=True, capture_output=True, timeout=12, check=False
    )
    if proc.returncode:
        raise Denied("git_source_tree_unavailable")
    row = proc.stdout.strip().split("\t", 1)
    if len(row) != 2 or row[1] != path:
        raise Denied("reviewed_icon_source_not_git_tracked:" + path)
    fields = row[0].split()
    if len(fields) != 3 or fields[0] != "100644" or fields[1] != "blob":
        raise Denied("untrusted_icon_git_source_mode:" + path)
    return fields[2]


def verify_ico(raw):
    if len(raw) != 16958:
        raise Denied("reviewed_windows_icon_length_drift")
    if raw[:6] != b"\x00\x00\x01\x00\x01\x00":
        raise Denied("reviewed_windows_icon_header_invalid")
    width, height, colors, reserved, planes, bits, size, offset = struct.unpack_from(
        "<BBBBHHII", raw, 6
    )
    if (width, height, colors, reserved, planes, bits, size, offset) != (
        64, 64, 0, 0, 1, 32, len(raw) - 22, 22
    ):
        raise Denied("reviewed_windows_icon_directory_invalid")
    if struct.unpack_from("<IiiHH", raw, 22) != (40, 64, 128, 1, 32):
        raise Denied("reviewed_windows_icon_bitmap_header_invalid")


def verify_png(raw):
    if (len(raw) < 33 or raw[:8] != b"\x89PNG\r\n\x1a\n"
            or raw[12:16] != b"IHDR"
            or struct.unpack_from(">II", raw, 16) != (64, 64)
            or raw[24:26] != b"\x08\x06"):
        raise Denied("reviewed_linux_icon_png_invalid")


def verify(root):
    root = Path(root).resolve(strict=True)
    if not (root / ".git").exists():
        raise Denied("real_git_checkout_required")
    checked = {}
    for path, (git_pin, sha256_pin) in PINS.items():
        file = root / path
        try:
            meta = file.lstat()
        except OSError as exc:
            raise Denied("reviewed_icon_file_missing:" + path) from exc
        if not stat.S_ISREG(meta.st_mode) or meta.st_size > 200_000:
            raise Denied("reviewed_icon_file_not_regular:" + path)
        if git_head_blob(root, path) != git_pin:
            raise Denied("reviewed_icon_git_blob_changed:" + path)
        if sha256_pin is not None:
            data = file.read_bytes()
            raw_blob = b"blob " + str(len(data)).encode() + b"\0" + data
            if (hashlib.sha1(raw_blob).hexdigest() != git_pin
                    or hashlib.sha256(data).hexdigest() != sha256_pin):
                raise Denied("reviewed_icon_worktree_bytes_changed:" + path)
            if path == ICON_PATH:
                verify_ico(data)
            if path == PNG_PATH:
                verify_png(data)
        checked[path] = git_pin
    return {
        "schema": "lu.ide.windows-reviewed-firstparty-icon.v1",
        "source": "git_head_and_remeasured_worktree",
        "gitSourceIdentities": checked,
        "windowsTauriIconPresent": True,
        "firstPartySourceReproducibility": "checked_in_node_regression",
        "nativeOsCustodyAccepted": False,
        "installedAppAccepted": False,
        "releaseApproved": False,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo-root", type=Path, required=True)
    args = parser.parse_args()
    try:
        result = verify(args.repo_root)
    except (Denied, OSError, ValueError, TypeError, subprocess.TimeoutExpired) as exc:
        print("IDE_REVIEWED_ICON=DENIED:" + str(exc), file=sys.stderr)
        return 2
    print("IDE_REVIEWED_ICON=SOURCE_BYTES_VERIFIED_NOT_INSTALLED_ACCEPTANCE")
    print("IDE_REVIEWED_ICON_GIT_ICO=" + result["gitSourceIdentities"][ICON_PATH])
    print("IDE_REVIEWED_ICON_RELEASE=NOT_APPROVED")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
