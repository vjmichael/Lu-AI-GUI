"""Hostile GPG package-provenance fixtures; never production release evidence."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest import mock

import gpg_package_provenance as gate


class GpgPackageTests(unittest.TestCase):
    def setUp(self):
        context = tempfile.TemporaryDirectory()
        self.addCleanup(context.cleanup)
        self.root = Path(context.name)
        self.package = self.root / "Lu IDE setup.exe"
        self.package.write_bytes(b"not a real executable; test-only fixture")
        self.signature = self.root / (self.package.name + ".asc")
        self.public = self.root / "owner-public.asc"
        self.public.write_text("fixture not a real key")

    def test_refuse_weak_or_unapproved_fingerprint(self):
        for value in ("", "abcd1234", "z" * 40, "a" * 64):
            with self.subTest(value=value), self.assertRaises(gate.Denied):
                gate.fingerprint(value)

    def test_refuse_unsigned_or_wrongly_named_package(self):
        with self.assertRaisesRegex(gate.Denied, "signature_missing_or_linked"):
            gate.audit(self.package, self.signature, self.public, "a" * 40)
        self.signature.write_text("test fixture")
        wrong = self.root / "different.asc"
        wrong.write_text("wrong named test fixture")
        with self.assertRaisesRegex(gate.Denied, "signature_name_invalid"):
            gate.audit(self.package, wrong, self.public, "a" * 40)

    def test_refuse_symbolic_link_package(self):
        target = self.root / "link.exe"
        try:
            target.symlink_to(self.package)
        except (OSError, NotImplementedError):
            self.skipTest("No symlink privileges")
        with self.assertRaisesRegex(gate.Denied, "package_missing_or_linked"):
            gate.sign(target, self.root / "link.exe.asc", self.public, "a" * 40)

    def test_never_overwrite_signature(self):
        self.signature.write_bytes(b"existing signature")
        with self.assertRaisesRegex(gate.Denied, "signature_output_must_be_new"):
            gate.sign(self.package, self.signature, self.public, "a" * 40)

    def test_refuse_mismatched_public_fingerprint_before_verify(self):
        calls = []
        def invoke(_binary, args, env=None):
            calls.append(args)
            if "--list-keys" in args:
                return "pub:-:255:22:0000000000000000:0:0:::::::scSC:\nfpr:::::::::BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB:\n"
            return ""
        self.signature.write_bytes(b"fixture")
        with mock.patch.object(gate, "invoke", side_effect=invoke):
            with self.assertRaisesRegex(gate.Denied, "primary_fingerprint_not_independently_pinned"):
                gate.audit(self.package, self.signature, self.public, "a" * 40)
        self.assertFalse(any("--verify" in c for c in calls))

    @unittest.skipUnless(shutil.which("gpg") and os.name != "nt",
                         "Disposable real GPG key fixture runs on Linux; Windows source tests are fail-closed")
    def test_real_disposable_key_sign_verify_and_mutation_refusal(self):
        # This synthetic ephemeral test key is NOT the owner's key or a release signer.
        signing_home = self.root / "fixture-home"
        signing_home.mkdir(mode=0o700)
        env = os.environ.copy()
        env["GNUPGHOME"] = str(signing_home)
        def gpg(*args):
            return subprocess.run(["gpg", "--batch", "--pinentry-mode", "loopback",
                "--passphrase", "", *args], capture_output=True, text=True,
                env=env, check=True).stdout
        gpg("--quick-generate-key", "Lu IDE Test Fixture <fixture@example.invalid>",
            "ed25519", "sign", "0")
        data = gpg("--with-colons", "--fingerprint", "--list-secret-keys")
        key = next(x.split(":")[9] for x in data.splitlines() if x.startswith("fpr:"))
        self.public.write_text(gpg("--armor", "--export", key))
        with mock.patch.dict(os.environ, {"GNUPGHOME": str(signing_home)}):
            record = gate.sign(self.package, self.signature, self.public, key)
        self.assertEqual(record["result"], "detached_gpg_signed_and_verified")
        self.assertFalse(record["releaseApproved"])
        self.assertEqual(gate.audit(self.package, self.signature, self.public, key)["trustedPrimaryFingerprint"], key)
        with self.assertRaisesRegex(gate.Denied, "signature_output_must_be_new"):
            with mock.patch.dict(os.environ, {"GNUPGHOME": str(signing_home)}):
                gate.sign(self.package, self.signature, self.public, key)
        self.package.write_bytes(b"tampered after detached signing")
        with self.assertRaises(gate.Denied):
            gate.audit(self.package, self.signature, self.public, key)


if __name__ == "__main__":
    unittest.main()