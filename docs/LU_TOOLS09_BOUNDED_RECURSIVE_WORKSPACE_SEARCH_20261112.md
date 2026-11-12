# Lu TOOLS-09 — Bounded recursive workspace search

**Branch:** `lu/tools09-bounded-workspace-search-20261112`.  
**Base:** P0-SEC06 source `50a6f68ba322b3fed5959af2af49f75388c2905c`.  
**State:** source implementation candidate; native Windows/Linux and installed-app acceptance OPEN.  
**Scope:** Lu Core only. The IDE is not changed. The accepted alpha installer remains untouched.

## Existing capability and engineering additions

The canonical `lu.inspect.search_text` already had a read-only typed
`SearchTextRequest`, opt-in recursion, simple filename suffix filtering,
a `NativeToolTurn` authenticated workspace root and a bounded result.
TOOLS-09 improves **this original tool**, with no second filesystem
search interface, no CLI command, no model-controlled ambient path and no
permission from a returned file fingerprint.

The new implementation:

1. Uses cap-std directory handles and component-by-component validation
   beneath the selected original root. Hidden content, .git internals,
   credentials, private keys, symlinks and reparse points are excluded by
   default. On Linux, both file and directory leaves use O_NOFOLLOW and
   directory leaves additionally use O_DIRECTORY. Relative path and
   opened-file identity are checked before reporting results.
2. Refuses cross-device traversal on Unix and skips files or directories
   whose actual opened identities disagree with the original metadata.
   The source-root is a pinned open directory; later changing its
   ambient pathname cannot redirect the current scan.
3. Keeps opt-in recursion with host-enforced ceilings: 64 directories,
   512 directory entries, 64 opened files, 128 matched lines, 64 KiB
   per file, 4 MiB total source reads, 48 KiB estimated result payload
   and 2 seconds elapsed time. All limits are local and independent of
   model arguments, which can request only smaller file/hit limits.
4. Uses the existing file_suffix filter as the initial safe simple filter.
   No unrestricted glob, regex or model-chosen `ripgrep` executable is
   introduced. Search text is literal UTF-8, case-sensitive and bounded.
5. Treats unreadable, changed, binary/non-UTF-8, oversized, linked or
   excluded-mount files as **PARTIAL** with host-authored limitation
   codes. Truncation also covers time, byte, entry, directory, file,
   match and result limits. A returned zero hits means a complete
   no-match **only when** `status=COMPLETE`.
6. Computes a SHA-256 of the entire successfully read file and attaches
   that digest and byte count to a matching excerpt. No digest is minted
   from partial/mutated bytes. A late-line match gets a bounded excerpt
   containing its actual phrase. Hash is verified *at observation*, not
   an immutable permission or proof the file remains unchanged.
7. Connects dropped native tool futures to an atomic cancellation flag
   seen by the bounded blocking walker. Cancellation returns a typed
   `CANCELLED` error; it never emits a false complete result or retries
   previously approved filesystem access.
8. Preserves field numbers 1–4 of `SearchTextResult` and 1–3 of
   `SearchTextHit`; new trailing Protobuf fields expose limitations,
   read bytes, skipped entries and exact-source fingerprint. Older
   decoders can ignore the additions. Tool ID, scope, registry and
   model-facing JSON input parameters remain unchanged.

## Source-level hostile fixtures

`src-tauri/lu-core/src/tools09_search_adversarial_tests.rs` covers:

- Ordinary and nested UTF-8 matches, literal suffix and no-match.
- Original file SHA-256, long-line excerpt, exact file bytes and stable
  relative paths.
- Binary files, huge files, credential-like and hidden entries.
- Traversal, absolute-path, colon/backslash and invalid filter rejection.
- File/match/entry limits, time-budget and cooperative cancellation.
- Unix external file/directory symlink attempts, pinned original root
  after ambient rename, and excessive relative path depth/length.
- A Windows symlink/reparse fixture where native symlink privileges permit.

`lu-desktop/tests/tools09-bounded-search.test.mjs` adds source contracts
to guard against accidentally registering a parallel, unauthenticated
execution tool or dropping the partial-output semantics.

## Explicit limitations and deferred acceptance

- **No native Cargo compiler or tests were run in the current development
  environment.** Source edits and static review do not constitute runtime
  proof. Run `cargo test --locked --manifest-path
  src-tauri/lu-core/Cargo.toml native_file_extra -- --nocapture` and
  `node --test lu-desktop/tests/tools09-bounded-search.test.mjs` on
  the exact source head, including Windows and Linux native fixtures,
  and inspect original logs before acceptance.
- On Linux, atomic no-follow leaf opens close common symlink swap paths.
  Unix device-number checks **cannot establish** the absence of all
  same-device bind-mount traversal. This stronger mount-boundary proof,
  plus real Windows junction/reparse race tests and opened file-ID
  verification, must be independently accepted or repaired before
  claiming strict hostile-worker filesystem isolation.
- The 2-second wall budget is checked between bounded filesystem
  operations. A kernel-blocked file operation cannot be forcibly
  terminated by an AtomicBool; native OS worker custody and process
  cancellation remain independent acceptance requirements.
- There is no general new model command, filesystem indexer, live watcher
  or persisted search database. Search re-scans the already authorized
  directory and does not inherit the earlier root in a later user turn.
  Additional project-scale indexing should be scheduled through PERF-01.
- Native Tauri/React, real GGUF selection of this tool through an original
  authenticated human turn, source/folder chooser consent and denial,
  cancellation during actual I/O, physical Windows installed app UI,
  exact-head CI and release acceptance remain OPEN.
- A later file modification invalidates the historical digest as current
  correctness. Returned snippets are untrusted document content, not
  instructions, permission, tool arguments or independent task acceptance.

## Task handoff

**TOOLS-09 implementation:** substantially extended and ready for
source-level review, with OS-level acceptance still open.

**Next implementation:** PERF-01 P0/P0D deterministic CPU routing,
measurement and responsive scheduling. Browser packaging and real
normal-chat tools (#204 / TOOLS-10A/B) follow.

**Other remaining groups:** P0-Next/PLAN true task-specific worker
admission, VIS-00B–05 native response presentation and artifacts,
expanded model-library/import progress, DLC-API/IPC after core v1.0,
and consolidated SEC-06/P0-GATE/NSIS/Debian/physical Radeon acceptance.
No PR merge, release or CI dispatch is authorized by this change.
