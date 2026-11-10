#!/usr/bin/env python3
"""IDE-05.6 independent, read-only upstream uv wheel intake (NOT release approval).

Runs OUTSIDE the protected package worker. An HTTPS PyPI JSON observation is
an untrusted registry assertion, not a publisher signature or legal clearance.
Only the fixed uv 0.9.9 Linux wheel candidate is eligible; rejects aliases,
malformed archives and mismatched installed executable/license bytes.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import urllib.request
import zipfile

PINNED_NAME = "uv"
PINNED_VERSION = "0.9.9"
PYPI_URL = "https://pypi.org/pypi/uv/0.9.9/json"
MAX_METADATA_BYTES = 5 * 1024 * 1024
MAX_ARCHIVE_BYTES = 90 * 1024 * 1024
MAX_EXTRACTED_BYTES = 350 * 1024 * 1024
MAX_MEMBERS = 400


class Denied(ValueError):
    pass


def require(condition: bool, code: str) -> None:
    if not condition:
        raise Denied(code)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def regular_bytes(path: Path, max_size: int) -> bytes:
    # A prior lstat alone cannot protect an installed binary or license:
    # a hostile filesystem could replace it with a symlink before open.
    info = path.lstat()
    require(stat.S_ISREG(info.st_mode) and 0 <= info.st_size <= max_size,
            "IDE056_SOURCE_FILE_UNTRUSTED")
    flags = os.O_RDONLY | getattr(os, "O_BINARY", 0)
    require(hasattr(os, "O_NOFOLLOW"), "IDE056_NOFOLLOW_UNAVAILABLE")
    flags |= os.O_NOFOLLOW
    fd = os.open(path, flags)
    try:
        opened = os.fstat(fd)
        identity = lambda x: (x.st_dev, x.st_ino, x.st_size, x.st_mtime_ns)
        require(stat.S_ISREG(opened.st_mode) and identity(opened) == identity(info),
                "IDE056_SOURCE_REPLACED_BEFORE_OPEN")
        with os.fdopen(fd, "rb", closefd=False) as stream:
            data = stream.read(max_size + 1)
        after_handle = os.fstat(fd)
        after_path = path.lstat()
        require(len(data) == info.st_size and
                identity(info) == identity(after_handle) == identity(after_path) and
                stat.S_ISREG(after_path.st_mode),
                "IDE056_SOURCE_MUTATED_DURING_READ")
        return data
    finally:
        os.close(fd)


def public_pypi_record() -> dict:
    request = urllib.request.Request(PYPI_URL, headers={
        "User-Agent": "lu-ide-05-upstream-intake/1",
        "Accept": "application/json",
    })
    with urllib.request.urlopen(request, timeout=20) as response:
        require(response.geturl() == PYPI_URL and response.status == 200,
                "IDE056_PYPI_REDIRECT_OR_STATUS_DENIED")
        raw = response.read(MAX_METADATA_BYTES + 1)
    require(len(raw) <= MAX_METADATA_BYTES, "IDE056_PYPI_METADATA_OVERSIZE")
    return json.loads(raw)


def verify_pypi_uv_wheel(
    metadata: dict, wheel: Path, installed_binary: Path, installed_license: Path,
) -> dict:
    require(isinstance(metadata, dict) and isinstance(metadata.get("info"), dict)
            and isinstance(metadata.get("urls"), list),
            "IDE056_PYPI_RESPONSE_INVALID")
    require(metadata["info"].get("name") == PINNED_NAME and
            metadata["info"].get("version") == PINNED_VERSION,
            "IDE056_PYPI_RELEASE_IDENTITY_DENIED")
    name = wheel.name
    require(re.fullmatch(
        r"uv-0\.9\.9-py3-none-(?:manylinux|musllinux)[A-Za-z0-9_.]*x86_64\.whl",
        name,
    ) is not None, "IDE056_WRONG_PLATFORM_OR_WHEEL_NAME")
    require(not wheel.is_symlink(), "IDE056_WHEEL_ALIAS_DENIED")
    archive = regular_bytes(wheel, MAX_ARCHIVE_BYTES)
    matching = [x for x in metadata["urls"] if isinstance(x, dict)
                and x.get("filename") == name]
    require(len(matching) == 1, "IDE056_PYPI_WHEEL_IDENTITY_AMBIGUOUS")
    record = matching[0]
    require(record.get("packagetype") == "bdist_wheel"
            and record.get("yanked") is False
            and isinstance(record.get("url"), str)
            and record["url"].startswith("https://files.pythonhosted.org/packages/")
            and record["url"].endswith("/" + name)
            and isinstance(record.get("digests"), dict)
            and re.fullmatch(r"[0-9a-f]{64}", record["digests"].get("sha256", "")),
            "IDE056_PYPI_FILE_SOURCE_DENIED")
    digest = sha256(archive)
    require(record["digests"]["sha256"] == digest,
            "IDE056_PYPI_ARCHIVE_SHA_MISMATCH")
    require(isinstance(record.get("size"), int) and record["size"] == len(archive),
            "IDE056_PYPI_ARCHIVE_SIZE_MISMATCH")
    binary = regular_bytes(installed_binary, MAX_ARCHIVE_BYTES)
    license_bytes = regular_bytes(installed_license, 512 * 1024)
    # The wheel's executable can be installed as a native package member
    # or a data/scripts payload. Its actual content, not the guessed path
    # or a pip-generated wrapper, establishes identity.
    license_path = ""
    names: set[str] = set()
    payload: dict[str, bytes] = {}
    total = 0
    with zipfile.ZipFile(wheel) as zipped:
        members = zipped.infolist()
        require(0 < len(members) <= MAX_MEMBERS, "IDE056_WHEEL_MEMBER_COUNT_DENIED")
        for entry in members:
            path = entry.filename
            parts = path.split("/")
            require(path not in names and not path.startswith("/") and
                    "\\" not in path and ":" not in path and
                    all(part not in ("", ".", "..") for part in parts if part != "") and
                    (not path.endswith("/") or parts[-1] == "") and
                    not (entry.external_attr >> 16 & 0o170000 == stat.S_IFLNK),
                    "IDE056_WHEEL_MEMBER_PATH_OR_LINK_DENIED")
            names.add(path)
            total += entry.file_size
            require(total <= MAX_EXTRACTED_BYTES and entry.file_size <= MAX_ARCHIVE_BYTES,
                    "IDE056_WHEEL_EXPANSION_DENIED")
            if path.endswith("/uv") or path == "uv" or path.endswith("/LICENSE-MIT") or path == "LICENSE-MIT" or path == "uv-0.9.9.dist-info/METADATA":
                require(not entry.is_dir(), "IDE056_REQUIRED_WHEEL_ENTRY_NOT_FILE")
                payload[path] = zipped.read(entry)
        licenses = [n for n in payload if n.endswith("/LICENSE-MIT") or n == "LICENSE-MIT"]
        require(len(licenses) == 1, "IDE056_LICENSE_SOURCE_AMBIGUOUS")
        license_path = licenses[0]
    natives = [value for path, value in payload.items()
               if (path.endswith("/uv") or path == "uv") and value.startswith(b"\x7fELF")]
    require(len(natives) == 1 and binary.startswith(b"\x7fELF")
            and natives[0] == binary,
            "IDE056_INSTALLED_UV_BYTES_DIFFER_FROM_WHEEL")
    require(payload[license_path] == license_bytes,
            "IDE056_LICENSE_BYTES_DIFFER_FROM_WHEEL")
    metadata_bytes = payload.get("uv-0.9.9.dist-info/METADATA", b"")
    headers = metadata_bytes.decode("utf-8", errors="strict").split("\n\n", 1)[0].splitlines()
    require(headers.count("Name: uv") == 1 and
            headers.count("Version: 0.9.9") == 1,
            "IDE056_WHEEL_INTERNAL_METADATA_DENIED")
    return {
        "schema": "ide056-upstream-uv-wheel-observation-v1",
        "package": PINNED_NAME,
        "version": PINNED_VERSION,
        "wheelFilename": name,
        "wheelSha256": digest,
        "installedExecutableSha256": sha256(binary),
        "installedLicenseSha256": sha256(license_bytes),
        "upstreamRegistryMetadataSha256": sha256(json.dumps(
            metadata, sort_keys=True, separators=(",", ":")
        ).encode("utf-8")),
        "upstreamRegistryMetadataSource": PYPI_URL,
        "registryWheelHashAndInstalledBinaryMatched": True,
        "upstreamPublisherSignatureVerified": False,
        "independentUpstreamAttestationVerified": False,
        "licensesLegallyCleared": False,
        "humanApprovalGiven": False,
        "releaseApproved": False,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--wheel", type=Path, required=True)
    parser.add_argument("--installed-uv", type=Path, required=True)
    parser.add_argument("--installed-license", type=Path, required=True)
    parser.add_argument("--receipt", type=Path, required=True)
    args = parser.parse_args()
    require(not args.receipt.exists(), "IDE056_REFUSE_RECEIPT_OVERWRITE")
    evidence = verify_pypi_uv_wheel(
        public_pypi_record(), args.wheel, args.installed_uv, args.installed_license
    )
    args.receipt.parent.mkdir(parents=True, exist_ok=True)
    with args.receipt.open("x", encoding="utf-8") as file:
        json.dump(evidence, file, sort_keys=True, indent=2)
        file.write("\n")
        file.flush()
        os.fsync(file.fileno())
    print("IDE056_UPSTREAM_UV_REGISTRY_WHEEL_HASH_AND_BINARY=PASS")
    print("IDE056_UPSTREAM_PUBLISHER_SIGNATURE=OPEN")
    print("IDE056_RELEASE_PROVENANCE_AND_HUMAN_APPROVAL=OPEN")


if __name__ == "__main__":
    main()
