# DEV-04.2 — Protected completed build to independently witnessed executable

## Source implementation

Stacked on DEV-04.1 draft PR #163. The existing restricted offline
`rust_build` Cargo recipe now reaches the SAME supervisor
`review_build_test_evidence` path as other fixed WO6 recipes. This
repairs an actual missing recipe in the reader's allowlist; no shell
command, arbitrary project binary or runner is introduced.

A native `BuildRunView` produced after actual worker completion adds
the exact supervisor `terminalReceiptSha256` and optional
`debugBinaryCandidate`. Candidate creation is restricted to:
1. Native JobPhase Completed, zero process exit and no reconciliation.
2. Exact project, JobID, recipe, plan/source/snapshot/toolchain hashes
   already bound to the supervisor's hash-linked terminal receipt.
3. Independent post-termination native output witness. An output
   manifest is advisory; it is not itself a signed binary.
4. Exactly ONE regular top-level `target/debug/<name>` ELF candidate
   on Linux or `target/debug/<name>.exe` MZ/PE candidate on Windows.
   Libraries, deps, nested generated targets, ambiguous multi-binary
   builds and truncated manifests are denied.
5. Reopen the host-owned output using no-follow flags, recheck
   metadata/size (and Unix inode/hard-link/executable-bit), hash the
   exact file bytes a second time and require equality to the
   independently audited output witness SHA-256.

The result contains relative path, size, binary SHA, output tree SHA,
binary format header observation, and `debuggerExecutionAuthorized=false`.
No worker handle, OS path, executable bytes, approval, or permission is
transmitted to WebView. The UI validates exact job/plan/receipt, output
tree and matching manifest entry before showing it. Binary header
observation is *not* full PE/ELF parser, debug symbol attestation,
signing or a safe launch contract.

## Genuine native process acceptance

The existing kernel-contained fixtures now run **real** `rust_build`:
- Linux: `wo6_linux_native_build_acceptance --build` is launched only
  through systemd's genuinely delegated cgroup + Bubblewrap fixed host,
  reopens the real published build tree and independently witnesses its
  sole executable and journal receipt. The same WO6 workflow retains
  check/test/cancel and kernel cleanup/restart gates.
- Windows: `d7_windows_acceptance --build-native-worker` performs
  actual pinned-offline Cargo inside the preassigned AppContainer/Job,
  requiring an independently authenticated broker/SCM and final
  verified tree cleanup; it rehashes the protected published exe,
  compares the completed source/plan/receipt and never launches it.
  Both D7B and protected-service D7D workflows invoke the mode.
- An extra Windows/Linux source test matrix checks Rust supervisor
  bytes and Tauri integration plus hostile frontend evidence shape.
  Synthetic ELF/PE tests are not a substitute for those real OS runs.

## Open gates

Exact-head CI, genuine delegated Linux build and Windows independent
SCM-protected build, binary hash recheck, cross-process output tamper,
installed signed/pinned desktop workflow, deep PE/ELF validation,
debug information, source-to-address mapping, and project debugger
approval/launch remain open until independently witnessed.

DEV-04.3 must establish executable format and symbol closure before
DEV-04.4 may offer a protected launch. Never label this advisory
hash witness as a runnable debugger capability or signed artifact.
