#!/usr/bin/env python3
"""Owner-key OpenPGP detached package signatures, never Windows Authenticode.

The signer uses the owner's *local* GnuPG private key. Verification imports only
an independently supplied public key into a disposable keyring and requires an
explicit, separately trusted full fingerprint. CI never receives a private key.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import sys
import tempfile

FPR = re.compile(r"[0-9A-Fa-f]{40}\Z")
PACKAGE_SUFFIXES = {".exe", ".deb"}
FORBIDDEN_STATUS = {"BADSIG", "ERRSIG", "EXPSIG", "EXPKEYSIG", "REVKEYSIG",
                    "KEYEXPIRED", "KEYREVOKED", "SIGEXPIRED", "NO_PUBKEY"}


class Denied(ValueError):
    pass


def fingerprint(value):
    if not isinstance(value, str) or not FPR.fullmatch(value):
        raise Denied("full_40_hex_trusted_fingerprint_required")
    return value.upper()


def regular(path, label):
    path = Path(path)
    if path.is_symlink() or not path.is_file() or not stat.S_ISREG(path.stat().st_mode):
        raise Denied(label + "_missing_or_linked")
    return path.resolve(strict=True)


def file_hash(path):
    h = hashlib.sha256()
    with path.open("rb") as f:
        for block in iter(lambda: f.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def invoke(binary, arguments, env=None):
    try:
        p = subprocess.run([binary, *arguments], capture_output=True, text=True,
                           check=False, timeout=120, env=env)
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise Denied("gpg_executable_or_operation_unavailable") from exc
    if p.returncode != 0:
        raise Denied("gpg_operation_denied")
    return p.stdout


def verified_signature(binary, package, signature, public_key, trusted_fpr):
    """Verify only with a fresh isolated public keyring, never ambient trust."""
    trusted_fpr = fingerprint(trusted_fpr)
    with tempfile.TemporaryDirectory(prefix="lu-ide-gpg-verification-") as tmp:
        env = os.environ.copy()
        env["GNUPGHOME"] = tmp
        invoke(binary, ["--batch", "--no-tty", "--no-options", "--no-auto-key-retrieve",
                        "--import", str(public_key)], env=env)
        keys = invoke(binary, ["--batch", "--no-tty", "--with-colons", "--fingerprint",
                               "--list-keys"], env=env)
        primaries = []
        expecting_primary = False
        for line in keys.splitlines():
            fields = line.split(":")
            if fields[0] == "pub":
                expecting_primary = True
            elif fields[0] == "sub":
                expecting_primary = False
            elif fields[0] == "fpr" and expecting_primary:
                primaries.append(fields[9].upper())
                expecting_primary = False
        if primaries != [trusted_fpr]:
            raise Denied("public_key_primary_fingerprint_not_independently_pinned")
        status = invoke(binary, ["--batch", "--no-tty", "--no-options", "--no-auto-key-retrieve",
                                 "--status-fd=1", "--verify", str(signature), str(package)], env=env)
        valid = []
        good = 0
        for line in status.splitlines():
            if not line.startswith("[GNUPG:] "):
                continue
            fields = line[9:].split()
            if not fields:
                continue
            if fields[0] in FORBIDDEN_STATUS:
                raise Denied("invalid_or_expired_gpg_signature")
            if fields[0] == "GOODSIG":
                good += 1
            if fields[0] == "VALIDSIG":
                # GnuPG records signing subkey fingerprint first and, when
                # present, the primary key fingerprint as its final field.
                valid.append(fields[-1].upper() if len(fields) >= 11 else fields[1].upper())
        if valid != [trusted_fpr] or good != 1:
            raise Denied("signature_not_from_independently_pinned_key")
    return trusted_fpr


def audit(package, signature, public_key, trusted_fpr, gpg="gpg"):
    package = regular(package, "package")
    signature = regular(signature, "signature")
    public_key = regular(public_key, "public_key")
    if package.suffix.lower() not in PACKAGE_SUFFIXES or signature.name != package.name + ".asc":
        raise Denied("package_or_detached_signature_name_invalid")
    before = file_hash(package)
    signature_before = file_hash(signature)
    signer = verified_signature(gpg, package, signature, public_key, trusted_fpr)
    if file_hash(package) != before:
        raise Denied("package_changed_during_verification")
    if file_hash(signature) != signature_before:
        raise Denied("signature_changed_during_verification")
    return {"schema": "lu.ide.gpg-package-provenance.v1", "result": "detached_gpg_verified",
            "package": package.name, "packageSha256": before,
            "signatureSha256": file_hash(signature), "trustedPrimaryFingerprint": signer,
            "authenticode": "not_asserted", "releaseApproved": False,
            "physicalWindowsInstalledAccepted": False}


def sign(package, signature, public_key, trusted_fpr, gpg="gpg"):
    package = regular(package, "package")
    public_key = regular(public_key, "public_key")
    signature = Path(signature).absolute()
    if package.suffix.lower() not in PACKAGE_SUFFIXES or signature.name != package.name + ".asc":
        raise Denied("package_or_detached_signature_name_invalid")
    if signature.exists() or signature.is_symlink() or not signature.parent.is_dir():
        raise Denied("signature_output_must_be_new")
    signer = fingerprint(trusted_fpr)
    before = file_hash(package)
    with tempfile.TemporaryDirectory(prefix="lu-ide-gpg-signing-", dir=signature.parent) as tmp:
        candidate = Path(tmp) / signature.name
        # Preserve local GNUPGHOME and pinentry: only the user's workstation
        # may authorize access to its GPG private key.
        invoke(gpg, ["--armor", "--detach-sign", "--local-user", signer,
                     "--output", str(candidate), str(package)])
        if file_hash(package) != before:
            raise Denied("package_changed_during_signing")
        result = audit(package, candidate, public_key, signer, gpg)
        # Exclusive creation refuses clobbering an existing public signature.
        created = False
        try:
            with candidate.open("rb") as src, signature.open("xb") as dst:
                created = True
                shutil.copyfileobj(src, dst)
                dst.flush()
                os.fsync(dst.fileno())
        except BaseException:
            if created:
                signature.unlink(missing_ok=True)
            raise
    result["signatureSha256"] = file_hash(signature)
    result["result"] = "detached_gpg_signed_and_verified"
    return result


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("action", choices=("sign", "verify"))
    p.add_argument("--package", type=Path, required=True)
    p.add_argument("--signature", type=Path, required=True)
    p.add_argument("--public-key", type=Path, required=True,
                   help="Independently trusted exported PUBLIC GPG key; no private keys")
    p.add_argument("--fingerprint", required=True,
                   help="Full independently confirmed 40-hex owner key fingerprint")
    p.add_argument("--gpg", default="gpg", help="Locally installed GnuPG executable")
    args = p.parse_args(argv)
    try:
        func = sign if args.action == "sign" else audit
        result = func(args.package, args.signature, args.public_key,
                      args.fingerprint, args.gpg)
        print(json.dumps(result, sort_keys=True))
        return 0
    except (Denied, OSError, ValueError) as exc:
        print("DENIED: " + (str(exc) if isinstance(exc, Denied) else "invalid_gpg_package_input"),
              file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())