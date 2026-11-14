# B5 — Large-project editor responsiveness and bounded working set

**Implementation branch:** `ide/b5-large-project-editor-working-set-20261108`,
stacked on B4 draft PR #146, exact source parent
`93e8fad1fa48478e31ffebb62c89d6f1d0c3a2ed`.

**Status:** Source implementation + native Windows/Linux test workflow committed.
Exact-head native tests, actual installed two-pane OS UI, and physical editor
memory/latency benchmarks are **OPEN** until genuine completed evidence exists.

## Problem established from the active code

The native source reader had preexisting capability-scoped safeguards:
`MAX_TEXT_BYTES = 512 KiB`, `MAX_ENTRY_COUNT = 400`, bounded path
validation, and a blocking worker for file preview. However, the desktop
previously rendered every open tab and all 400 directory rows in the WebView,
retained a CodeMirror state and its full undo stack indefinitely for every
opened file, and eagerly constructed highlighting and line wrapping for any
accepted source length. React also rebuilt secondary-pane diagnostic marks on
unrelated primary edits. These are implementation observations, not
benchmarked claims of any particular performance gain.

## Bounded implementation

- **At most 48 tabs per registered project and 96 tabs globally across projects.** New opens, saved project-search
  results, and multi-file proposal staging use the same pure admission
  function. A request that would exceed capacity is rejected visibly before
  replacing any editor state. Opening an existing tab does not consume another
  slot. B2 private draft identity, exact Save revision, independently
  recoverable private checkpoints, and every existing Undo history are
  preserved. There is *no* automatic eviction of dirty, clean, or recently
  edited buffers.
- **12-tab visible strip.** Previous/next page controls and a jump-to-open
  file selector let the user reach all 48 permitted tabs without rendering
  buttons for hidden tabs. External source hits and diagnostics recenter a
  selected tab; manual paging remains stable during normal typing.
- **120-entry incremental directory presentation.** The preexisting native
  listing still returns at most 400 entries per invocation; the WebView
  initially mounts 120 rows and shows more on explicit request. No background
  directory crawl, recursive model inference or foreign filesystem access.
- **Direct native relative-file open.** When a directory contains more than
  400 entries, a user can type a known relative path such as
  `src/subtree/file.rs`. The local UI rejects traversal/absolute/control
  paths and uses the *existing* native cap-std registered-project file
  preview, which repeats authoritative validation. It does not take an
  ambient disk path, execute project code or bypass the 512 KiB source bound.
  This is not a claim that folder pagination or full 400+ discovery exists. Admission uses the current tab ref after asynchronous preview, so a late response cannot knowingly exceed the working-set ceiling.
- **Lightweight 128 KiB text mode.** For loaded source exceeding 128 KiB of
  UTF-16 text, CodeMirror no longer eagerly attaches a language parse tree or
  wraps every long line. Text editing, cursor, Undo, B2 buffer reference,
  explicit Save and existing LSP permission boundaries remain intact.
  The mode is displayed to the user.
- **Stable derived state.** Active tab and project tab set use React memo
  boundaries; secondary-pane diagnostics are memoized so typing/caret events
  in the other pane do not issue redundant decoration transactions.
  Approved LSP completion compares serialized `sliceDoc()` draft content,
  repairing an existing CRLF mismatch without weakening identity checks.

## Resource ceiling interpretation

With a single project, 48 tabs × the native per-file read maximum (512 KiB) gives **24 MiB of
possible original UTF-8 file bytes**, *not* an operating-system memory cap.
JavaScript strings, immutable editor trees, syntax state, undo histories,
diff caches, Windows WebView2 and GTK renderer allocations can consume
considerably more memory. No user-visible RAM ceiling or speedup is claimed.

## Tests and acceptance

Pure JavaScript `editor_working_set.test.mjs` checks concurrent proposal
batch admission, same-project and cross-project tab budgets, tab-strip
windowing, directory progressive display, hostile relative paths, and a
reproducible 1,200-window synthetic operation stress run against 10,000
*mock* tabs. It emits actual measured Node elapsed time and explicitly says
it is not an installed-GUI memory/latency result.

The standalone source regressions enforce actual native file read bounds,
host capability checks, all three tab admission paths, CodeMirror's large
file mode, no silent Undo eviction and no changed worker authorization.

`.github/workflows/ide-b5-large-project-native.yml` runs full real Windows
2022/Ubuntu 24.04 npm build+Node suite, host Rust tests, locked native Tauri
compilation and a source-authority diff against the exact B4 parent. It
does not declare an unreviewed LSP redistributable or signed release.

**Remaining genuine installed acceptance:** exact-head B4
`ide-b4-installed-editor-windows.yml` and
`ide-b4-installed-editor-linux.yml` must finish successfully, demonstrating
two independent OS-accessible CodeMirror panes with disk SHA isolation,
targeted Save and uninstall. Afterward an independent installed B5 workload
should physically open dozens of registered source files, switch between
tabs, type/Undo/Save while monitoring renderer RSS, time-to-open and
switch latency, and test 128–512 KiB source files on both OSs.
A synthetic Node result or pending GitHub job cannot satisfy this gate.

## Existing failures and stack provenance

The actual prior B2 Windows native frontend run reported four failures:
a 41-vs-46 capability count, a stale fixed command allowlist, and two
Find/Replace assertions that had not been updated for CRLF-safe CodeMirror
tree offsets. These were repaired on B2 PR #145 exact source
`97940239e1af4a6c8edbdfa3a176579d01ae5f18`. The B2 and B4
**current exact-head** native/installed OS jobs were still queued/pending
at B5 start; their real results must be checked and any new failures
repaired. All original independent B2/B3 and protected LSP PRs are retained
until equivalent native acceptance. No signing/legal approval is inferred.


## B5 physically installed editor measurements — native harness added

The B5 stack now extends the **existing** B4 installed unsigned NSIS/Debian
workflows; it does not launch additional independent OS build matrices for the
same changes. On B5 exact-head workflow execution, the existing B4 two-pane
acceptance must pass first, then the **same installed package** is used for
a separate B5 OS-level workload before real package removal:

- A disposable native project contains 48 distinct UTF-8 source files,
  including a >130 KiB (within native 512 KiB) text source, and an additional
  49th file to confirm fail-closed source-tab capacity. Some files are over
  the 128 KiB CodeMirror lightweight threshold.
- Files are opened through **real Windows UI Automation / native GTK AT-SPI**
  and the visible project-relative Open file control. Each open is verified
  using genuine OS input and OS clipboard copying the **selected CodeMirror
  source's unique marker**. The time from input to verified clipboard is
  recorded as **UI open round-trip**, including keyboard/accessibility/
  clipboard overhead. It is *not* raw disk I/O time or frame latency.
- Alternating first/last tab via the native accessible jump selector yields
  12 independently timed **tab-switch-and-clipboard-verification round trips**.
  The interaction pass includes genuine typing, Undo, an unchanged original
  file hash until explicit Save, explicit Save with changed saved file bytes,
  and verification that other project files were not written.
- The OS process tree is sampled at initial source load, 12, 24, 36, 48
  open tabs and after Save. Windows uses installed-parent PID plus descendants
  from Win32_Process and Get-Process to sum **WorkingSet64/PrivateMemorySize64**.
  Linux uses the installed-parent PID plus live /proc descendants to sum
  **VmRSS/VmSize**. These scopes are independently labeled and may exclude
  OS-reparented renderer processes; the numbers cannot be represented as a
  total machine RAM bound. A 49th tab must be visibly denied without evicting
  retained drafts.
- JSON receipts are written only after completing real installed OS actions:
  `ide-b5-windows-installed-performance.json` and
  `ide-b5-linux-installed-performance.json` in the disposable runner temp.
  B4 installed workflows upload these receipts, including on failure when a
  partial file exists. Missing controls, wrong source, missing OS clipboard,
  absent memory observations, wrong Save target or missing receipt fail the
  acceptance steps. No programmatic WebView JS, in-process mock or Tauri IPC
  shortcut is accepted.
- Both OSs report measured open P95, switch P95, kernel-observed RSS peak,
  real typing/Undo/Save round trips, and exact source head. Measured values
  and observed action counts must be reviewed before setting user-visible
  responsiveness thresholds or accepting any performance claim. A successful
  source compilation or synthetic Node benchmark still cannot close the
  installed memory/interaction gate. Any gate closure requires the evidence
  for the exact PR head SHA and an actual native result on **both** OSs.

### GitHub scheduling blocker (observed, not a code failure)

At the latest review, original B4/B5 exact-head jobs
`42510890637`, `42510890663`, `42512903437`,
`42512903465`, and `42512903345` were **queued or pending**.
No failed B4/B5 runtime test logs were available. The active repository has
a large concurrent workflow backlog; posting another successful-looking
source artifact would not substitute for the missing OS execution.
The old completed B3 source run `42504529949` reported two stale frontend
contract expectations (both OSs), repaired in the later B2 stack but
not yet proven at B4/B5 exact-head. Neither B4 nor B5 is accepted.
