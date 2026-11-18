# Lu PLAN-03 — Evidence-based project status classifier

**Scope:** Lu Core and the existing desktop Settings window in `vjmichael/Lu-AI-GUI`; stacked after PLAN-02, no IDE work, no PLAN-04 GGUF planning.

## Trusted facts and conservative vocabulary

The classifier consumes an **audited immutable PLAN-02 evidence graph** and native observations derived solely from the human-selected project directory. Status dimensions are `MISSING`, `PARTIAL`, `NOT_INTEGRATED`, `BROKEN`, `UNVERIFIED`, `ACCEPTED`, `BLOCKED`, `UNKNOWN`; multiple dimensions can coexist. The machine-readable result retains graph SHA-256, selected exact Git HEAD, project scope SHA-256, original requirement source and line, linked node IDs, observation coverage, provenance/claim conflicts, evidence freshness, next evidence, and an independently recomputed status digest.

- `MISSING`: only when a complete bounded scan, current original requirement provenance, and native `NotFound` metadata establish absence of every explicitly named file path. Never generalize to missing semantic features.
- `PARTIAL`: mixed independently present and absent file paths for one explicit multi-path requirement; behavior still unknown.
- `NOT_INTEGRATED`: an explicitly cited local `refs/heads/...` branch tip was **independently proven not to be an ancestor** of selected HEAD using the existing reviewed fixed-argv Git process. Git errors fail closed (not inferred non-containment).
- `BROKEN`: a narrowly scoped operator-selected required JSON syntax gate fails original cap-std file read and actual `serde_json` parser. Untrusted report `failed` is *not* BROKEN.
- `ACCEPTED`: only for explicit narrow presence-only, JSON-syntax-only, or branch-in-HEAD-only requirements that native OS metadata, original JSON parser, or Git ancestry independently verify. These statuses **never certify installed behavior, runtime integration, model quality or human acceptance**.
- `BLOCKED`: native permission-denied error on a cited required file path; the unblock condition is to grant reviewable access and retry. Unsupported/symlink paths remain unknown, not blocked.
- `UNVERIFIED`: related source/test files have been inspected, but current native outcome and required installed/human acceptance have not.
- `UNKNOWN`: insufficient source provenance, uninspected targets, conflicting self-reported current-head results, or partially scanned projects. `stale` flags a claimed older/unknown-SHA run; historical PASS does not become current acceptance.

## Original authorization, prevention of false claims, and persistence

The host uses **a fresh Tauri Settings folder picker**, reuses the PLAN-02 graph scanner, performs bounded capability-scoped native metadata and JSON checks, and checks the local exact Git HEAD once more before saving. The model, project files, old task journal, frontend, and remote reports cannot create native evidence facts. An untrusted `lu-project-native-run-v1` JSON file can claim pass/fail, but has no accepted authority. Search misses are not absence proofs. No network or model inference, no execution, Git mutation or tool grant.

Reports are atomically appended to the existing ANSF `ansf/library.db` in `ansf_project_status_snapshots` with original graph foreign identity and re-read durable status digest, while retaining immutable historical snapshots; no second database.

## Exact-head acceptance

The PLAN-03 native acceptance group must cover both Windows and Linux: frozen source lock, React/Node Settings contract, genuine cap-std project fixture, scoped JSON failure-to-repair fixture, original branch divergence and merge witness, stale and contradictory test-report claims, SQLite append/reopen, tampered digest rejection, and ordinary Tauri compilation. Physical installed WebView interaction remains separately open.

## Limitations

Comprehensive behavioral BROKEN and ACCEPTED findings require purpose-built independent native runner receipts, source/artifact pins, installed app and human acceptance, none of which a local project report can self-issue. PLAN-03 does **not** run a test suite, synthesize tasks, activate a GGUF or create work orders. PLAN-04 is the next separate group.
