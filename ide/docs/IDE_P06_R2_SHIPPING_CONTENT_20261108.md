# Lu IDE P06-R2 — Actual shipping-content inventory foundation

**Source stack:** P06-R2 on P06-R1 draft PR #108, after P06 draft #95. No baseline/main merge, no product signing or release approval.

## Goal

The owner explicitly deferred the **final legal redistribution review until the Lu IDE shipping set is finalized**. That is sensible: a lockfile may contain optional target-specific crates, build-only tools, SDK wrappers and binaries never packaged in a particular Windows/Linux release. A legal review should examine **what is actually shipped**, with the source/lock graph as a conservatively maintained superset. Until that review, maintain complete notices and open risk records; never silently delete them based on platform metadata alone.

P06-R2 supplies the read-only source tool `ide/packaging/p06_shipping_content.py`, which inventories an independently designated **unpacked package candidate** or **installed candidate** tree using actual bytes rather than merely source package manifests. It enumerates every regular file, retains zero-byte members, refuses symlink/reparse and special entries, normalizes relative POSIX paths with strict casefold collision detection, hashes each file SHA-256 with pre/post metadata checks, enforces file/depth/total-byte bounds, categorizes likely native binaries and license/notice evidence, and computes a deterministic SHA-256 over the sorted file-record manifest.

The source SHA, OS label and tree-type labels are supplied by the caller and recorded as **claims**, not evidence that the tree is a GitHub-built artifact or the actual installed executable. An independent OS witness must later bind an actual downloaded installer/deb SHA, native extraction/install root, package file closure, source exact head and installed process path to this inventory before it may be relied on for shipment review. The script deliberately sets `treeTrust=unattested_input_tree_native_witness_required`, `packageOriginVerified=false`, `dependencyShipmentVerified=false`, `nativeInstalledVerified=false`, `licenseLegalReviewApproved=false`, `releaseApproved=false`, and `protectedExecutionAccepted=false`. No executable is launched.

## Invocation (after independently capturing the installed or unpacked tree)

```bash
python ide/packaging/p06_shipping_content.py \
  --root /independently-observed/LuIDE/package-tree \
  --source-sha FULL_40_HEX_COMMIT_SHA \
  --platform ubuntu-24.04 \
  --tree-kind unpacked_package_candidate \
  --output /outside-the-tree/p06-ship-tree.json
python ide/packaging/p06_shipping_content.py \
  --root /independently-observed/LuIDE/package-tree \
  --source-sha FULL_40_HEX_COMMIT_SHA \
  --platform ubuntu-24.04 \
  --tree-kind unpacked_package_candidate \
  --output /outside-the-tree/p06-ship-tree.json --verify
```

The verification command freshly hashes the tree and rejects file addition, removal, modification, changed source/OS, fake flags or content hash. Output inside the inspected tree is rejected, preventing the manifest from becoming an unintended payload member.

## Genuine next native acceptance boundary

The dedicated `ide-p06-shipping-content-source.yml` checks the implementation and hostile fixtures on native **Windows 2022 and Ubuntu 24.04**. Those are source/host OS checks only. Later, once P04 native installed Windows and Linux packages are independently passing and final architecture/contents are stable:

1. Bind the exact immutable Git SHA to the actual built and uploaded NSIS/DEB SHA-256. Capture a manifest from that exact installer/deb contents or extracted staged tree, and separately capture a manifest from its **actual installed application directory**. Verify signed/unsigned status factually, without expecting a Windows certificate while signing is deferred.
2. Verify installed executable and bundled notice bytes come from the selected exact installer, and identify intentional installer-only/uninstaller files and platform-specific installation changes. Require a reviewed rule for differences; never equate two different root layouts by silently removing records.
3. Produce a final release-by-release *shipped* binary/notice inventory, then reconcile every shipped binary against source, dependency locks and native runtime/license obligations. Resolve the open risk records or formally document the owner's licensing decisions and disclosures. Legal review and any desired external counsel opinion happen **at this later milestone**, not now.
4. Keep P03 protected debugger/containment, P04 native installs/upgrades, P05 B2/B3 merge, signing identity and P07/WO10 cross-DLC bridge entirely separate. No cleanup of 38 historical workflows or original PRs on the strength of a candidate inventory.

## Verification and limitations

Native source/regression tests include wrong source SHA, OS mismatch, stale manifest/added or deleted file, fake release approval, symlinks to outside tree and symlink root, case collisions, control/traversal names and zero-length payload members. Full secure acquisition, filesystem races under an adversarial local installer, reparse-point/NTFS-specific kernel verification, package archive binary origin and installed GUI/process acceptance require independent native evidence. This inventory is **not** cryptographic attestation by a signing identity, package manager, CI provenance issuer or lawyer.

**Decision:** Final publisher/legal redistribution review is deferred **until the finished shipping set is known**. Keep collecting upstream notices and risk obligations in the meantime. No waiver, premature legal approval or assumption that all locked packages ship is authorized.
