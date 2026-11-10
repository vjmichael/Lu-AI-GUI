# IDE Group 5 — Package & Toolchain Manager engineering (November 10, 2026)

**Working branch:** `ide/group05-package-review-20261110`, independently stacked on `d01c4b034fc0f9307de12104dc50424e4cface0a`. Group 4 LLVM/native acceptance remains on a different branch and is not an execution dependency for these strictly non-executing source changes.

## Implemented during this Group 5 continuation (source only)

- **5A** `9a8f888`: A requested vcpkg package whose direct declaration has additional features/platform/version metadata is no longer falsely treated as unconditional. Duplicate matching declarations are denied. Native negative regression tests added.
- **5A** `12cf727`: Static vcpkg Sync now parses the current SHA-rechecked manifest rather than accepting malformed or invalid dependency-entry JSON merely because no package name was supplied. Tests cover invalid JSON, unsupported entries and valid no-grant Sync.
- **5C** `8a0e698`: Before reading privately frozen `uv.lock` or installed audit metadata, the private source reader verifies its root is an actual directory, not a symlink/reparse redirect. The existing child-path protections and digest checks remain. Unix symlink-root regression test added.
- No manager process, download, file mutation, approval token or new release entitlement was introduced. There is **no claim** that the new tests have passed a native Windows/Linux runner.

## Remaining exact requirements and acceptance matrix

| Group | Existing foundation | Next concrete engineering gate | Evidence required |
| --- | --- | --- | --- |
| **5A** manifest/preview | Registered-project six-file SHA discovery; typed vcpkg/uv/Conan review and revalidation; new fail-closed parser tests | Consolidated native Rust tests including invalid/ambiguous vcpkg, drift, source-link tests, plus desktop review feedback | One source SHA, both Windows/Linux host tests, no resolver/approval leakage |
| **5B** protected execution | Feature-gated fixed offline uv Sync; Linux cgroup-v2/Bubblewrap; Windows remains denied | Validate true private offline Sync, cancellation/kill/restart, source/manifest tamper, no network/secret access, no child escape | Real kernel/process logs, controlled source and output fingerprints, terminal receipts; no fake approval |
| **5C** provenance | Private installed-byte/PEP 376 RECORD/license verifier; upstream `uv` wheel observation; synthetic CI-only package | Independently pin original runtime and complete transitive wheel closure, prevent root/child substitution, verify installed exact archive-to-binary correspondence | Actual wheel archive bytes/hashes, source and license evidence, bounded hostile audits; no unsupported publisher claim |
| **5D** Windows ACL | Observation-only AppContainer staged-tree DACL/MIC auditor; `uv_sync_offline` fail-closed | Native package SID read-only source/runtime, private write-only output, hostile write/rename/reparse and process escape attempts under real AppContainer/SCM | Independent Windows kernel outcomes and native ACL/Job/owner receipts; **never** replace with unsandboxed launch |
| **5E** recovery | Append-only fsynced lifecycle and private discard-pending states; restart does not restore grants | Fault-inject before/after process exit, audit, discard and fsync; verify actual process tree and bytes on restart | Exact journal-chain replay, no retry, no lost/false terminal receipt, verified source preservation and private cleanup |
| **5F** installed/consent | Feature-gated desktop UX; physical Windows denial fixture; no release-ready installer approval | Real installed Windows and Linux human approve/refuse/timeout/cancel/stale-plan, restart, remove/reinstall/data retention; no CI simulated human | Real OS window interaction, exact installed binary SHA, separate one-use grant/worker/journal receipts |

## Work policy

1. Continue independent development on Group 5; do not hold source work for Group 4 GitHub Ops or alter its running LLVM job/authoritative branch.
2. The initial grouped native tests should include 5A/5C source changes without spawning new per-commit workflow fanout. Test actual 5B/5E protected worker operations only on a reviewed OS runtime. Keep Windows deny-by-default until 5D genuine kernel proof.
3. Do not promote synthetic first-party wheel evidence into third-party release provenance; do not treat a parser, metadata hash, or a CI mock approval as an installed package operation.
4. Defer signing and final legal/publisher approval by owner decision while preserving checks and releaseApproved=false; unsigned development is not signed release acceptance.
5. Lu Core bridge remains deferred. No Docker requirement.
