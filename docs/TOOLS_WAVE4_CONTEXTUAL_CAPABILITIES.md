# Lu Wave 4 — contextual tools for ordinary conversations

Status: Wave 4 started; **WAVE4-GIT-01 source-implemented**, native and installed acceptance pending.
Branch: `lu/wave4-contextual-tools-20261106`, stacked on Wave 3 PR #57.
This wave **does not supersede** the owner-approved original Jan tool design or current Lu v1.0 constraints.

## Wave boundaries

- Wave 3: native Reader/Workspace, read-only discovery, user-reviewed file effects, progress, cancellation, audit, tool-loop guards — source integrated, exact-head CI and installed Windows GGUF acceptance remain separate.
- Wave 4: complete **Browser, Research, Background, Environment, Git, Vision and Image Workspace** integration into Lu's ordinary human-first conversation. Reuse existing host protocols/modules wherever possible; never create a parallel agent engine.
- Wave 5: native acceptance and hardening (Windows/Linux CI, actual GGUF+MMProj, permission/cancellation/recovery, installed desktop/NSIS). Earlier verification is welcome and required for merging; Wave 5 is not permission to skip present native regressions.

## WAVE4-GIT-01 — scoped local Git inspection from normal Send

- Human asks directly to see `git status`, `git log`, `git diff` or local repository history. The trusted host uses a conservative *current-human-message* classifier; creation, mutation, fetch/push, general shell, ambiguous explanation, negation, pasted attachments and model-control tokens do not route to Git.
- The native OS folder dialog selects one real local repository; Lu rejects symbolic-link/reparse folders and repositories with unsafe Git metadata. A *host-selected fixed-location* installed Git executable is required: Windows Git for Windows in Program Files; Linux `/usr/bin/git` or `/bin/git`. No PATH, model-provided command/executable, repo-config executable or network fetch is admitted.
- `WorkspaceReadTurnSession::new_git` **reuses the existing authenticated per-turn authority, supervisor, typed Protobuf catalog, tool-loop guard, lifecycle revocation and cancellation**. Its registry includes exactly `lu_git_status`, `lu_git_log`, `lu_git_diff` with `ReadWorkspace` effect and bounds. The ordinary Workspace mode retains its previous file-oriented catalog; Git is not globally advertised.
- The Git adapter reuses `native_git_inspect`'s allowlisted arguments, fixed Git process environment, local repo hardening, output caps and process timeout. Git commit messages and diffs are untrusted content. No model write grant, credential exposure or GitHub CLI access.
- The host tracks per-generation progress, writes a separate `lu-git-read-audit.sqlite` model/tool audit and includes content-free Git task summaries in ordinary chat's **Tool activity** pane. A selected repository is not persisted as future authority. Cancel and shutdown revoke task access and no remote side effect is implied.
- Tests: real local Git fixture, schema whitelist, wrong-chat/user-turn denial, and deterministic intent rejection. Exact-head Linux/Windows native and installed Windows+GGUF acceptance **not yet passed**.

## Next Wave 4 integrations

Background: reuse existing `TaskStore`, `BackgroundStatusTool`, terminal notices and scoped dashboard; do not permit model-selected processes or unverified artifact claims.

Environment: host-attested tool locations/versions/fingerprints and usable resource inventory. Separate recalled state from freshly verified facts.

Vision/Image Workspace: route to genuinely **verified** native vision capability and user-approved image scope; never substitute OCR or hallucinated descriptions for unsupported native vision.

Browser/Research: preserve human-visible controls, explicit navigation/authorization, untrusted page content, and structured evidence; the current repository does **not** contain a complete native browser engine, so do not mark these implemented or invent one.

Advanced GitHub operations belong to Lu IDE DLC, not the Core Git domain.

## WAVE4-BACKGROUND-01 — source integration (native acceptance pending)

Explicit human status questions in normal Send reuse the existing BackgroundProcess reconciliation and durable TaskStore. A new chat-scoped SQL query filters before its limit; only host-owned task IDs, state enums and recorded percentages enter a marked GGUF context snapshot. No worker-authored text, private paths, other chat data, scheduler, new execution tool, process cancellation or authority is introduced. Native and desktop tests were added. Installed Windows GGUF and Linux/Windows CI remain pending.

Browser, Research, Environment, Vision and Image Workspace remain distinct open Wave 4 domains.


## WAVE4-ENV-01 — local environment observation (source implemented)

A current human question such as "Show my system specs" now enriches an ordinary GGUF chat turn with a freshly collected, bounded and host-owned environment snapshot. Reuses the existing compute_inventory CPU, instruction and platform GPU-hint collectors. Reports OS family, architecture, logical CPU availability and vetted CPU feature hints. Windows GPU probing remains unverified; Linux node counts remain hints, never backend initialization or successful inference. No shell, package manager, environment-variable dump, model-selected executable or permission is introduced. Dedicated Windows native classifier/snapshot tests added. Software version and executable fingerprint attestation remain open for later ENV slices; release and installed acceptance pending.


## WAVE4-IMAGE-WS-01 — draft attachment viewer and edit intent (source implemented)

Reused LU-IMAGE-WS-01 draft's typed image-edit intent contract and regression fixtures in the active Wave 4 UI source; corrected string C0 control-character validation. The existing image attachment strip now opens an opt-in, zoomable read-only preview of the already admitted original draft image. It never overwrites the original, writes files, auto-edits pixels or advertises unavailable model image-editing capabilities. Closing, removing the image or submitting a turn closes the viewer; existing image inference and attachment handling stay unchanged. This is a first UI/contract integration, NOT the promised persisted-image revision editor. Host-side image revision identity/authorization, deterministic native crop/resize, immutable output history, undo/export and installed acceptance all remain open.


## WAVE4-VISION-01 — host-only capability diagnosis and early image admission (source implemented)

Existing admitted image bytes and MMProj model binding are retained. The normal Send host now checks the loaded model's admitted projector before queueing/persisting a new image request; a model without MMProj produces a direct error instead of an abandoned queued generation. A separate read-only "Check vision" UI invokes native host status for that **current session** and accurately labels no model/no projector as unavailable, and admitted MMProj without live image probe as unverified. Reuses `vision_routing::VisionEvidence`, and never implies OCR can describe arbitrary photos. This is not a genuine image-probe implementation; backend-specific image grounding tests, probe revocation on unload/reload, and real Windows MMProj confirmation remain OPEN.


## WAVE4-BROWSER-01 — persistent operator visibility preference (source integrated)

Reused tested TOOLS-10 browser visibility-state and SQLite preference modules on the active Wave 4 branch. The Tauri host persists human-selected `automatic_hidden` (default headed browser hidden until user interaction) or `always_visible` and shows a Settings selector. Changing a preference never launches Chromium or grants model/network permission. The packaged Chromium/Node/Playwright worker, origin authorization, same-window CAPTCHA handoff and Windows/Linux installed browser acceptance remain OPEN in draft PR #26.

## WAVE4-RESEARCH-01 — citation integrity and research analysis core (source integrated)

Reused the existing TOOLS-10 verified-evidence ledger, query planner, thesis evidence analysis, premise/cross-domain revision, sufficiency and maturity modules. Discovery is not a verified citation; interpretation is not a verified passage; early missing evidence does not prove scientific absence. The native Linux and Windows test matrix includes each module. Browser/HTTPS provider ingestion, full-text extraction, durable orchestration and one-turn normal-chat Research network dispatch remain OPEN. No live web search or model browser authority was added by staging these offline modules.


### Browser/Research import closeout step — functional host wiring OPEN

Copied the existing TOOLS-10 PR #26 native browser supervisor, native browser tools, Chromium custody and visibility bridge, verified HTTPS/Crossref/OpenAlex research modules, worker script and native acceptance fixtures **without redesign**. These modules preserve explicit NetworkRead/NetworkWrite grants, user-selected origins, host-owned binaries and source verification. Merely exporting core modules does not start the browser or grant model network access. Windows/host packaging and actual one-turn desktop Browser/Research integration remain OPEN pending native acceptance.


### WAVE4-BROWSER-02 — owned desktop lifecycle (source implementation)

The active Lu desktop now integrates the original TOOLS-10 headed Chromium/Playwright runtime manifest verifier (default `not-packaged`), OS-owned browser-window custody and native task start/status/show/hide/stop commands. A host-scoped browser task requires an authenticated persisted human conversation turn, separate native approval for each bounded HTTPS origin list, and a NetworkRead-only conversation authority grant. Startup/reservation races and browser crashes revoke the original task. The React Settings panel reports measured `launchReady`, collects exact HTTPS origins, and exposes operator-only start/watch/hide/human-verification/stop controls. No model-facing general-purpose browser execute API was added. A native runtime cannot launch until its fully reviewed Chromium/Node/Playwright package is staged with an exact embedded manifest; ordinary builds fail closed.

The original read-only model Browser tool and structured citation-safe Research modules exist in Lu Core, but the primary GGUF model queue is not yet connected to the Browser dispatch path. Genuine verified Browser-native Windows/Linux installed acceptance and normal-GGUF tool invocation remain OPEN.


### WAVE4-RESEARCH-02 — human-approved OpenAlex discovery (source implementation)

The Tauri host now exposes a `lu_research_openalex` command that requires a current, existing chat and a separate native approval dialog before an OpenAlex search. Only a **bundled, SHA-256-pinned** HTTPS client may execute; the default build does not include one and reports `lu_research_available=false`. The request uses TOOLS-10's SSRF-resistant public HTTPS adapter (now clears inherited environment variables) and records each source as `DISCOVERED` in a per-chat WAL-backed EvidenceLedger. UI results are unverified leads, not citations. Each search uses the existing durable TaskStore lifecycle; the host does not silently invoke a model. The pinned network runtime, live Windows/Linux search, Crossref corroboration and full-text passage verification remain OPEN for installed acceptance.


### WAVE4-BACKGROUND-02 — chat-scoped desktop status (source)
Normal Background Tasks now require an actual existing conversation ID and query the task ledger using SQL workspace filtering *before* the LIMIT. The UI passes only the selected chat. Model context already admits only task ID, enum and progress. Global operator task inventory no longer leaks unrelated chat task text into a regular conversation's panel. Native/React CI remains pending.

### WAVE4-ENV-02 — supervised runtime evidence (source)
Native `lu_environment_status` returns Lu desktop version/OS/architecture and **actual** current engine phase plus MMProj catalog admission, but intentionally marks GPU execution and visual reasoning UNVERIFIED until a real host-owned execution probe demonstrates them. No arbitrary env vars, subprocesses, package installs or permission changes; the native API is read-only.

### WAVE4-IMAGE-WS-02 — deterministic immutable new-version crop/resize (source)
Image Workspace now has bounded deterministic PNG crop and resize from Lu's existing host-admitted PNG/JPEG bytes, **native operator approval and OS save dialog** and a `create_new` filesystem write that cannot overwrite the original or another existing file. An output digest and dimensions are reported after reopening/rechecking the new PNG. User-provided geometry is revalidated in Lu Core; arbitrary image/model edits and paths are rejected. React's preview offers numeric crop/resize and shows the saved version receipt. This is not the full image editor: pixel masks, annotations, true semantic edit model, persistent revision graph/undo, crash-recovery journal and installed Windows acceptance remain OPEN.
