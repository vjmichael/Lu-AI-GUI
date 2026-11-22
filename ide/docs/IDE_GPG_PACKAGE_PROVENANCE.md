# Lu IDE — owner GPG package signatures (separate from Authenticode)

**Purpose:** publish Windows NSIS `.exe` and Debian `.deb` packages with
independently verifiable **OpenPGP detached signatures** using the owner's
existing personal GPG key. This does **not** Authenticode-sign any Windows PE
binary, confer Windows publisher trust, avoid SmartScreen/unknown-publisher
warnings, approve third-party licenses, or qualify physical Windows
SCM/Job/protected worker acceptance. Those are distinct gates.

**Source admission:** this new channel does not modify historical WO7 signed
release workflows or set `releaseApproved=true`. The approved native test SHA
and signed/physical release receipts must remain separately traceable. Use only
fully built packages whose installed behavior and source identity have been
independently checked. Never sign a package just because CI built it.

## One-time local setup (trusted owner's Windows machine)

1. Install and verify [Gpg4win](https://www.gpg4win.org/) or another trusted
   GnuPG distribution on the owner's Windows machine. The `gpg` command must
   be available in PowerShell. **Do not upload or paste your private key into
   GitHub, an Actions secret, ChatGPT, or an artifact.** GitHub's existing GPG
   key registration usually holds the public key only.
2. Confirm the **same owner key has a signing-capable local private key**:

   ```powershell
   gpg --list-secret-keys --fingerprint --keyid-format LONG
   ```

   If this prints no secret key, package signing is **not possible** yet using
   that identity. Retrieve the owner's original protected key on their trusted
   computer; do not manufacture a substitute claiming to be that key.
3. Independently confirm the complete **40-character fingerprint** through the
   owner's key record in GitHub account Settings → SSH and GPG keys. Match it to
   the local secret-key fingerprint. Do **not** trust a fingerprint supplied in
   the same unverified release download.
4. Export **only the PUBLIC key** to a separate file (substitute your verified
   fingerprint; using GPG `--output` avoids Windows PowerShell redirection
   encoding differences):

   ```powershell
   gpg --armor --output owner-public.asc --export YOUR_40_HEX_FINGERPRINT
   ```

## Sign a reviewed package (owner's machine only)

After producing the final NSIS `.exe` package from accepted source, run in the
repository root (PowerShell; use the actual existing package path and **a new**
`.asc` output):

```powershell
python ide/packaging/gpg_package_provenance.py sign `
  --package 'C:\releases\Lu IDE setup.exe' `
  --signature 'C:\releases\Lu IDE setup.exe.asc' `
  --public-key '.\owner-public.asc' `
  --fingerprint YOUR_40_HEX_FINGERPRINT
```

This uses your personal GnuPG signing key and interactive local key agent,
checks the exact package SHA-256 before/after signing, re-verifies against a
fresh public-only keyring, and prints a JSON provenance observation with
`releaseApproved: false`. Do the same for a `.deb` built from identical approved
source, changing only `--package` and matching `.asc` path. Existing signature
files are not overwritten. The script refuses a missing public key, missing
private signer, wrong fingerprint, invalid or changed signature, mismatched
package name, symbolic link or corrupted input.

Publish the exact package file **and its matching `.asc` file**, plus a separate
owner public-key reference and full fingerprint. There is no automatic package
signer on GitHub Actions, and the signer private key is never transferred.

## Verify on another computer (no private key required)

The recipient independently obtains the owner's genuine public key and trusted
full fingerprint (not solely from the same package download):

```powershell
python ide/packaging/gpg_package_provenance.py verify `
  --package '.\Lu IDE setup.exe' `
  --signature '.\Lu IDE setup.exe.asc' `
  --public-key '.\owner-public.asc' `
  --fingerprint YOUR_40_HEX_FINGERPRINT
```

An isolated temporary GnuPG keyring is used: package verification never trusts
random keys already in the recipient's keyring. Verification **fails closed**
on tampering, mismatched fingerprints, absent signatures, or invalid GPG status.
For Windows, completing GPG verification **does not** make an unsigned
installer Authenticode-trusted; the recipient must decide whether to run it.

## Validation boundary

- `python -m unittest discover -s ide/packaging -p test_gpg_package_provenance.py -v`
  runs source/negative tests; where `gpg` exists, one disposable test-only key
  exercises genuine detached sign, verify, and tamper rejection. That key is
  never a release identity.
- Native installed alpha and protected worker acceptance remain required before
  users are told the software is ready. Protected WO7 Authenticode production
  acceptance remains **HOLD**, not substituted with GPG results.
- No new recurring CI, default-branch release workflow, or private-key handling
  is required for these owner-local signing/recipient-verification operations.