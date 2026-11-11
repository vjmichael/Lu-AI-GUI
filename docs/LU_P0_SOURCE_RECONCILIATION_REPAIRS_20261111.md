# Lu P0 — Codebase-wide source reconciliation and repair

**Date:** November 11, 2026  
**Branch:** lu/p0-codebase-reconcile-20261111  
**Base:** afcb7ae235de8ae84d3fdc66c127bac6984fa33e (stacked after P0-AUTH)  
**Scope:** source-level consistency and risk repair of the seven stacked P0 implementation commits, NOT compiler/native/CI/PR/regression acceptance. No merge to main.

## What was reconciled

Compared original P0 parent `67f135b8f334eb0b11e8a4358d2a250331bd7184` against the exact P0-AUTH head: **seven commits; 41 changed paths**, including:
- Model-guided multi-document Reader and nested native-workspace worker;
- Model-proposed create/edit draft to the original reviewed write/edit native queues;
- Browser/Reader/OCR/private Observe original ANSF source index;
- PDFInfo/ExifTool specialist adapters, protected background execution, independent resource oracles;
- Exact project task-criterion witness, human decision and original worker/effect evidence;
- Cross-session ANSF corrections, source history, goals and task continuation;
- Conversation authorization/revocation fence, multi-domain native Stop and durable cancellation audit;
- React task board and normal chat UI integration.

Checked the original exact-head Git tree against `source-tree.lock.json`: **941 tracked entries** all matched their Git blob hashes, exactly one additional tree blob was the lock file itself, **no missing/changed/untracked source blobs**. This is a Git source-accounting check, *not* proof of compiler correctness.

Source-contract review covered core/desktop Rust symbol and signature paths including `ConversationAuthority`, reviewed Git/FFprobe/specialist brokers, Reader dispatch, task register and evidence board, original ANSF source/claim/book APIs, P0 MEMORY capsule, native browser/observe and cancellation state. Cross-checked the P0 UI invocation strings against Tauri command registrations (including the directly registered same-module `lu_native_source_history`). No CI or physical workers were invoked.

## Repairs committed here

1. **P0-AUTH Rust test name collision**: a local variable named `result` shadowed the helper function `result(...)`, making the later helper call invalid. Renamed the variable without changing the production cancellation semantics.
2. **Native Stop wrong-chat ambiguity**: the renderer's chat ID was accepted for a native-confirmed stop, but the OS dialog did not identify the actual target. The trusted host now reads the exact original ANSF chat title and opaque chat ID and displays both in the real native approval dialog. The host still revalidates that exact ANSF chat before writing a cancellation intent. No renderer-selected PID, executable, path, global cancellation or inherited model permission is accepted.
3. **ANSF priority source-book deduplication**: when resuming within the same chat, the primary source and current chat IDs were inserted twice into a 24-chat scan. That could silently displace an older original book and understate omissions. Priority IDs are now unique before filling the bounded profile-origin list.
4. **Full original source-window visibility**: a 16-entry source-index fetch can contain only private OCR/Observe rows, hiding older Browser/Reader receipts. The continuation capsule now treats a saturated original source window as potentially incomplete *before* filtering private content. The existing native-source retrieval limit is shared with the read-only capsule instead of copied as a magic number. Private content still never enters model context.
5. **Cross-session human-review inference**: task continuation now requests human reconciliation when native source entries, checkpoint source IDs or independently witnessed effects were omitted; when the exact criterion-review journal is missing or fails to reopen its worker/oracle source; or when the original goal ID/revision is absent. Missing evidence is not presented as a fully reconciled project, and no remembered grant/retry/task-complete authority is restored.
6. **Original ANSF correction and task evidence paths**: reviewed their original journal and source-lease contracts; no new permission or task-state mutation was added.

## What remains open / no overclaims

- The repository is not mounted in this execution environment, and Rust/Cargo is not installed here. GitHub connector operations were source reads, blob writes and source-commit updates, **not a real Rust compiler, JavaScript bundler, or executable test run**.
- The seven feature branches and this reconciliation branch are source candidates; they are not merged. An exact-head Rust/React build may expose additional type, ownership, borrow, JSX or Tauri integration failures. Do not claim the source stack compiles.
- Real original local GGUF model tool selection, Windows NSIS and Linux package startup, native file picker, human approval/refusal, offline OCR, Chromium Browser, physical process-tree custody, Linux delegated cgroup-v2, Windows Job Object, cancellation, uncertain-effect and restart acceptance remain deferred.
- The P0-AUTH launch fence applies to the reviewed Git/media/specialist family. Other existing file, Browser and Observe operations retain their separate native authorization/custody pathways. One generic model-to-OS executor was not built.
- P0-TOOLS-04 PDFInfo/ExifTool have standalone evidence and independent oracles, but do not automatically become preapproved AGENT plan-step evidence. A future explicitly reviewed plan revision is required for that additional feature.
- No automatic task completion, old-approval replay, model-selected external executables or resumed uncertain effects were introduced.

## Recommended later batch sequence (owner deferred)

**Gate 1:** Obtain source-compatible exact-head Rust + frontend compiler diagnostics; fix any demonstrated errors on a new reconciliation commit.

**Gate 2:** Run original native Windows/Linux cross-platform runtime and loaded-GGUF exact-typed-tool selection/denial.

**Gate 3:** Installed NSIS/Debian interaction, restart and independent OS source/custody/effect oracle matrix including confirmed human decisions.

**Gate 4:** Consolidated regression, CI/PR validation and merge/release *only with separate authorization*. Keep development moving until these gates are intentionally started.

**Stop:** source reconciliation/repair complete for this development turn. Acceptance remains explicitly open.