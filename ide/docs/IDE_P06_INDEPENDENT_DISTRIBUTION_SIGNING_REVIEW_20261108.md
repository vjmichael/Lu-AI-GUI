# Lu IDE P06 — exact-artifact redistribution and trusted publisher review

**Status: TECHNICAL REVIEW PACKET, not a legal opinion, license sign-off, publisher attestation, or authorization to distribute.** All remaining legal and signing gates remain default deny.

## 1. Real unsigned Windows CI artifact inspected

- Original exact source: `ab7fba4097a746230545e33fd0830d646be1acce`, draft PR [#97](https://github.com/vjmichael/Lu-AI-GUI/pull/97); native [WO9 run 42430581152](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42430581152). That run's installed GUI review/consent phase **FAILED**; the license/NSIS artifact upload itself was produced and inspected separately. Do not claim installed Git acceptance from it.
- CI artifact ZIP: `lu-ide-wo9-windows-unsigned-and-licenses`; SHA-256 `d4d095714ac151dd6b121d0202b29a0e6e6cfd5d0efd645cd1f50433d233636b`.
- Actual NSIS installer inside archive: `Lu IDE_0.1.0-alpha.1_x64-setup.exe`; 4287732 bytes; SHA-256 `4fbc767efd0734d00d0df76bd786a6724bd82504435ddc65b3f0b15fc9fc7831`; **unsigned**.
- Resolved npm/Cargo dependency inventory: **521** package records; **zero** unresolved package notices; original inventory SHA-256 `f08302833ca6520e15e45e1832bf140efcb06b9a4d8b848fd5ec7e4f38729566`; upstream-fetch receipt SHA-256 `389c596df3ba58bcc3e0be4a991e243cc5b888ddc1cb6106bc875988e68611ba`, six original upstream license sources retrieved and hashed. These hashes bind this packet to one previous **unsigned** build, not to any future native release build.
- Attached JSON: [p06-exact-windows-unsigned-receipt.json](../packaging/p06-exact-windows-unsigned-receipt.json). No compiled binary shipment closure was independently performed on the NSIS internal payload, and its full installed tree has not been certified. `independentLegalReviewApproved=false`, `releaseApproved=false`.

## 2. Review decisions, by actual license obligation

| Group | Actual notice evidence | Independent review still needed | Current disposition |
| --- | --- | --- | --- |
| Five MPL-2.0 crates (cssparser, cssparser-macros, dtoa-short, option-ext, selectors) | Exact versions and source notices in collected inventory | Review compiled Windows/Linux shipment, covered-file source availability, modifications and recipients' access; MPL is not an automatic prohibition of commercial use | OPEN |
| r-efi 5.3.0 and 6.0.0 | Declared `MIT OR Apache-2.0 OR LGPL-2.1-or-later`; original notices present | Choose and document the actual permitted MIT/Apache option for distributed code, notices and linkage; do not falsely infer mandatory LGPL from an OR expression | OPEN |
| Nine `protoc-bin-vendored` 3.3.0 packages | Rust wrappers declare MIT; original wrapper notices included | Determine whether **Google-built protoc executables** actually occur in final binary/installer/installed output versus being build-time-only; if distributed, verify exact compiler license/copyright and binary redistribution obligations separately | OPEN |
| 19 Apple SDK / objc2 / dispatch2 / block2 derivatives | Present in lock-resolution inventory | Prove target selection and actual Windows/Linux binary shipment or absence; do not infer shipment from being in Cargo.lock | OPEN |
| Seven external native SDK / libraries | Source notices present for listed crates | Identify actual bundled binary dependencies and imported/runtime native libraries, per architecture, including any LGPL or separate SDK licenses | OPEN |
| Three Rust WebView2 bindings | Exact MIT binding sources and notices present | Separately establish Microsoft **WebView2 Runtime** delivery method, terms, exact included components and installer prerequisites; Rust binding MIT does not license all Microsoft runtime binaries | OPEN |
| Tauri / CodeMirror / esbuild / LLVM / Python runtime components | Six original upstream exact-revision source texts, package-specific resolved notices | Confirm whether LLVM/Python runtime binaries are **actually shipped**, compiled target closure, copyright attribution, modified sources and redistribution conditions on each native installer | OPEN |

The numeric categories overlap: they are review buckets, **not** a distinct count of unresolved packages. The zero unresolved-notice count means *inventory coverage*, not independent legal clearance.

Official reference for executable form and covered source availability: [Mozilla Public License 2.0](https://www.mozilla.org/en-US/MPL/2.0/). Official Microsoft delivery mechanisms: [Distribute the WebView2 Runtime](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution) and [Evergreen versus Fixed Version](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/evergreen-vs-fixed-version). Offline standalone Evergreen redistributables are documented by Microsoft, but the exact installer payload, actual EULA/terms and license review must be verified.

## 3. Release-specific independent sign-off receipt requirements

Before **any** legal approval, the independent reviewer must inspect the exact produced installer **and installed binaries** for Windows and Linux; enumerate actual native executables, DLLs/shared libraries and on-disk notices with SHA-256 digests; prove actual build-time-only exclusions (including protoc) rather than infer from lockfile target graphs; record the license option and source/notice obligations for every license-sensitive shipped dependency; document any third-party publisher/runtime distribution rights; and bind the legal decision to the exact artifact digest, source SHA, reviewer identity, date, and review scope.

No generic `licensesReviewed=true` flag in a JSON file substitutes for that independent review. Do not alter the fail-closed P06 source triage report solely to advance the release gate.

## 4. Signing identity and evidence

The source signing script [Sign-Wo7ReviewedArtifact.ps1](../packaging/Sign-Wo7ReviewedArtifact.ps1) now requires a real publisher certificate selected by **exact** SHA-1 thumbprint, a private key accessible to Windows CAPI/KSP, validity dates, Code Signing EKU OID `1.3.6.1.5.5.7.3.3`, and successful Windows certificate-chain verification with online revocation. The exact SignTool.exe SHA-256 and RFC3161 timestamp authority are pinned, and the final signed installer, signed desktop and signed service are each independently Authenticode-verified. A local self-signed test certificate is **not** an acceptable production issuer.

**Owner-controlled external items not yet available:**
1. Official publisher identity to place on released binaries (person or organization) and a certificate from a trusted code-signing provider, or an approved Microsoft-hosted Artifact Signing integration with an independently audited access policy. Do not fabricate a publisher, generate a release key in CI, or commit any private key.
2. Install/provision the trusted certificate/private-key pair in the authorized Windows signer store or HSM/KSP, record **only the public certificate thumbprint and its expected policy**; keep signing permissions outside normal draft CI. SignTool must match the independent tool digest pin.
3. For Linux Debian redistribution, determine the signed APT repository / Release metadata (or other deliberately chosen distribution signature) and independently provision its trusted GPG identity; a disposable unsigned test `.deb` is not a signed release.
4. Run two **genuine** signed old/new version NSIS and Debian upgrades, rollback failure injection, service lifecycle, protected debugger/run/test, source tamper denial, restart recovery and full uninstall. Record successful exact-head native Windows/Linux receipts. Signing alone cannot substitute for installed acceptance.

Official Microsoft [code signing for Smart App Control](https://learn.microsoft.com/en-us/windows/apps/develop/smart-app-control/code-signing-for-smart-app-control) describes trusted CA certificates, Artifact Signing and SignTool.

## 5. P05 preserved historical work and remaining approval gates

P05's exact native workflow inventory still holds **38 original workflows** and no release/closure authority. The manually audited older source PRs include **nine unique unported B2/B3 blobs** (buffer identity, installed buffer tests and project search source/test), plus separate deferred **P07/WO10** bridge files. All thirty original indexed PR heads remain under retain-open policy. These nine source blobs require a **separate reviewed port**, duplicate-functionality conflict checks and new native installed-editor tests before any associated PR is closed. Do not import the deferred WO10 Lu Core bridge before Lu Core v1.0 interface decisions.

The existing P05 Windows source/audit test passed on [run 42431665046](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42431665046) at prior exact head `a47dba0`; Ubuntu's companion job and all signed/installed/native parity gates still awaited independent runs when inspected. No PRs or workflows were retired.

## 6. Gate result

**`independentLegalReviewApproved=false`; `trustedPublisherIdentityVerified=false`; `releaseApproved=false`.** A complete copyright notice inventory and source compilation are useful progress but do not grant release authorization. This document is intended for the external reviewer and publisher to complete these evidence-specific decisions.
