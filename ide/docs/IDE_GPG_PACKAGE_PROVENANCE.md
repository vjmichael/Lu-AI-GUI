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

## Owner-selected public signing identity and tested-build handoff

The owner selected the first locally held GnuPG signing primary: `47F3D98F8BDC1CDF908BEBB5D04103399ECFFFB9`
(GitHub account key ID `D04103399ECFFFB9`). The owner has demonstrated a
successful *public-only* export to `$HOME\lu-ide-public.asc` and
`gpg --show-keys --fingerprint` reporting that exact primary fingerprint.
This is a public identity pin, **not** evidence of a signed release package.
Do not trust public-key identity solely because it was bundled with an installer;
verify it independently against the owner's GitHub-account key record.

After a **successful full six-job source-locked Actions run**, the two installed
regression jobs upload their *actually tested unsigned* package bytes:

- `ide-gpg-unsigned-windows-<full-exact-head-sha>`: real Windows NSIS setup
  `.exe` plus its source-bound package verification receipt;
- `ide-gpg-unsigned-debian-<full-exact-head-sha>`: real Linux `.deb` plus its
  source-bound package verification receipt.

Both artifacts are retained for 30 days and are downloadable from the exact
Actions run; their presence is not a release approval. Match source SHA, check
the receipt and compute an independent SHA-256 over the extracted *package*
before owner-local GPG signing. Never sign a stale/different file; never upload
the GPG private key to GitHub Actions. The owner must sign and test the exact
bits outside CI. These hosted UI regression builds are not physical Windows
SCM/Job or protected Run/Test/Debug evidence and must not be advertised as
release-accepted software.
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