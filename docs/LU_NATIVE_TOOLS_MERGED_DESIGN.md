> **2026-11-07 explicit owner philosophy overlay:** The [Canonical AI Execution Layer philosophy](LU_AI_EXECUTION_LAYER_PHILOSOPHY_20261107.md) and [P01–P12 tool review](LU_AI_EXECUTION_TOOL_REVIEW_20261107.md) are authoritative **for AI Execution Layer philosophy and classification**: Practical AI Capability = f(K, I, A, F); **Foundational, Utility, Specialist**. Latest explicit owner direction controls conflicts over building unnecessary domain micro-apps. The preserved Jan-Mod original and this merged specification still govern native Protobuf/model boundary, authority, supervisor, loop protection and durable-effects semantics; do not rewrite historical decisions. The audit does **not** assert a general model-facing code/CLI/desktop worker already exists and authorizes **no implementation or permission expansion**.

# Lu native tool system — merged authoritative design

**Decision:** 2026-11-04. **Owner's design precedence:** preserve the original
`Jan-Mod-Design-Document.md` architecture and tool intentions as the **primary
tool specification**, then merge in Lu-specific source and explicit owner
decisions. Earlier assistant-authored tool notes are implementation drafts and
**cannot silently supersede the original**.

**Source of truth:** the preserved original [Jan-Mod-Design-Document.md](reference/Jan-Mod-Design-Document.md), copied into this repository from the user's Library without editing,
especially §§3.9 (workspace tools), 4.1–4.9 (typed IPC/model boundary/MBRL/
effect journal), 5.1–5.3 (consolidated toolkit and two-layer schemas), 6A
(ambient availability/context) and 7 (loop guard). This consolidated document
is a Lu-native, implementation-oriented **merge of those tool requirements**,
not a claim to replace/reproduce every other section of the full historical
Jan design.

**Additional binding sources:** `docs/LU_CENTRAL_DESIGN.md`,
`docs/LU_V1_0_DESIGN_RECONCILIATION_20261103.md`,
`docs/LU_V1_0_TASKS.md`, and documented direct owner decisions,
particularly v1.0 general-purpose Lu, trusted per-task permission, no
mandatory Lu-wide sandbox, and DLC platform/runtime work postponed to v2.0.
Changes to owner-specific decisions require an actual owner instruction,
not an assistant-authored design appendix.

## 0. Latest owner-specific scope decision and ordered implementation priority (2026-11-06)

**Binding owner direction, not assistant-proposed source completion.** Lu Core's tools should make an AI effective at locating resources, choosing/using authorized capabilities, coordinating work and preserving continuity; avoid replicating entire specialist applications. See [priority and integration design](LU_CORE_TOOLS_SCOPE_AND_PRIORITY_20261106.md) and [ordered implementation ledger](LU_NATIVE_TOOLS_IMPLEMENTATION_LEDGER.md).

**Future work order:** P01 Filesystem Discovery; P02 Background Tasks + Resource Guardian; P03 AI Operations Layer + model/provider management; P04 unified Reader/OCR including EPUB, streaming and selective visual reading; P05 Browser and shared Transfer Manager; P06 Observe & Learn; P07 Research and MLA/APA/Chicago/Turabian/SBL styles; P08 split archives/optional ISO-UDF; P09 bounded Office/spreadsheet reading and editing; P10 basic image editing/interface only; P11 simple Lu Core Git (advanced operations in Lu IDE); P12 internal JSON/Protobuf validation and boundary repair.

**Architectural reconciliation:** Keep the original first-party unified native toolkit, narrow model-facing schemas, Protobuf boundary, task-scoped permissions and durable effects. 'Unified Reader/OCR' means **one public reader capability with separate internal parsers/OCR/vision**, not a rewritten monolith. AI Operations Layer, indexing and the shared Task/Guardian/Transfer infrastructure should reuse existing registries and supervision rather than bypass authorization or create competing services. Priority does not automatically imply all features are v1.0 release blockers. This documentation request is explicitly **DOCUMENT AND STOP** until the owner instructs development.

## 1. Conflict handling — original design is the default, not a footnote

For every native tool, use the following order:

1. **Latest explicit owner decision** on the precise conflict, including
   the 2026-11-04 instruction to prefer the original tool design.
2. **Original `Jan-Mod-Design-Document.md` tool architecture and intended
   behavior**, even where a later Lu note simplified/reworded it.
3. **Later explicitly owner-confirmed Lu-specific requirements**, to the
   extent they can coexist with #2 (e.g. Windows native host, Protobuf IPC,
   security without a mandatory sandbox, task-specific Git support).
4. **Current working implementation and CI** as factual state only.
   A working Rust module does not create a product requirement.
5. Assistant-authored tool sketches and tentative TODOs are proposals.

If #2 and #3 appear incompatible, **document the conflict and choose the
original until the owner specifically resolves it**. Preserve the original
source text in the Library; do not silently rewrite its intent in a new file.
The purpose of this document is to merge implementation guidance and show
deviations, not to retroactively edit history.

## 2. Architecture — original combined toolkit with two interfaces

The original §5.2 deliberately **consolidates more than 52 individual
functions into approximately 15 unified internal domains**. This remains
the intended architecture. A small model (e.g. the user's Qwen 27B) must
**not** be given one sprawling multi-operation JSON schema merely because
internal code is unified.

- **Model-facing JSON:** task-specific, narrow names and few arguments, e.g.
  `file_read(path)`, `file_edit(path,search,replace)`, `file_write(path,content)`.
  Limit advertised schemas to the presently relevant, truly authorized
  capabilities; identify the active catalog in the model context. Do not
  advertise a native tool that the installed host cannot dispatch.
- **Boundary adapter:** assemble complete model JSON, strictly validate
  its envelope, schema and size, apply only the original bounded syntax
  normalization, convert **only an authorized tool** to typed Protobuf.
  Never treat model text as an authority grant.
- **Unified native implementation:** Protobuf requests/actions in shared
  Lu Core domain handlers, dispatched through a Rust native tool registry.
  Host-owned task authorization and registry policy gates **precede each
  effect**. A normalized JSON object is not a filesystem or process grant.
- **Return path:** native typed Protobuf result → exactly validated and
  correlated JSON tool response to the model. A rejection also gets a
  structured result when a tool call was presented, not a dangling tool
  call. Plain chat text is not an executable command.
- **Transport:** Protobuf for Lu's native IPC, including framed workers
  and raw binary channels where appropriate; Tauri JSON convenience
  interfaces may exist only as bounded adapters. **No mandatory MCP**.
  A first-party tool or local executable need not be an MCP server.

The original consolidated domains and their intended roles:

| Native domain | Core purpose / model-facing examples | Status / timing |
| --- | --- | --- |
| `file` | Create, read, edit, patch, append, copy, rename, metadata, symlink | Core v1.0; edits use owner-approved atomic journal, no extra WRITE-01E research here |
| `read` | Several bounded files/lines/streams | Core; trust exact selected targets and resource limits |
| `search` | Directory/list/content/symbol/references | Basic Core; IDE symbol graph later |
| `exec` | Language-specific code and sandboxed/host execution | Explicit host authority; no arbitrary shell grant from tool catalog |
| `web` | Search/fetch/render/research | Core can stage; optional external providers |
| `memory` | Persisted get/set/list/delete/history with provenance | Core ANSF/SQLite |
| `work` | Save/load/clear scoped work state | Core durable task state |
| `scratchpad` | Validate/change/commit staging area | Staged domain; never substitute for durable effect journal |
| `background` | Start/status/stop supervised tasks | Core lifecycle and resource guardian |
| `media` | Image/audio/video transformations | Stage according to verified model/native capability |
| `validate` | Schema and semantic validation | **Separate** from JSON syntax repair |
| `json` | **Internal JSON model boundary repair and validation** | Original bounded MBRL; **not** a user file inspector |
| `loop_guard` | Call repetition, oscillation and runaway-turn prevention | Core agent dispatcher |
| `transcribe` | Speech/audio transcription | Capability-driven |
| `video` | Video analysis and processing | Optional capability, NVP workspace remains DLC |
| `symlink`, `env` | Operations absorbed into `file`, `exec` | Not duplicated standalone default tools |

This table is a **domain design map**, not a claim that every operation above
has shipped. If existing Lu Core Rust exposes multiple small handlers, they
are compatible with this design as **facades/adapters over a unified domain**;
they are not permission to fragment the internal architecture indefinitely.

## 3. Original MBRL — internal JSON tool and error recovery

The original §§4.6 and 4.8 require three layers:
A. internal typed Protobuf, B. model-boundary reliability, and C. the durable
effect journal. Lu must not confuse them.

1. Collect complete model/engine frames; partial streams are
   `INCOMPLETE_STREAM`, **not executable errors or partial tool arguments**.
2. Strictly parse the full JSON and validate the schema; reject duplicates,
   unknown/missing required fields, excessive size and ambiguous decoding.
3. Apply at most **one deterministic syntax normalization** (e.g. exact
   Markdown JSON fence or trailing comma), not guessed parameters.
4. Allow at most **two model regenerations** on pre-dispatch syntax/schema
   errors; stop as `BLOCKED`/`NEEDS_USER` on exhaustion. Revalidate each
   candidate against the current host catalog and human task authority.
5. Return typed error/status such as `JSON_SYNTAX_INVALID`,
   `TOOL_SCHEMA_INVALID`, `UNKNOWN_TOOL`, `POLICY_DENIED`,
   `PAYLOAD_TOO_LARGE`, `PROTOBUF_DECODE_INVALID`,
   `TOOL_EXECUTION_FAILED`, or `OPERATION_OUTCOME_UNKNOWN`.
   Do not automatically retry denied, cancelled or possibly applied effects.
6. Preserve transcript validity: paired typed tool-result errors when a
   presented tool call is rejected; never synthesize a false execution.
7. Plain ordinary text and explanatory code blocks cannot be converted
   into privileged native tool invocations merely because they mention JSON.

`docs/TOOLS_02_INTERNAL_JSON_BOUNDARY_20261103.md` is a **source status and
test note under this specification**, not a competing source of requirements.

## 4. Original durable-effect and loop-protection rules

- Status: `PREPARED → EXECUTING → APPLIED | FAILED | UNCERTAIN`,
  with immutable call/task IDs and typed effects. Only retry a `FAILED`
  operation when its registered idempotency class permits. `APPLIED`
  cannot replay. `UNCERTAIN` requires authoritative observation/reconcile.
- Registration-time idempotency classes from original §4.9:
  `natural` for reads/searches, `keyed` for supported idempotency-key APIs,
  `reconcile` for observable file writes, and `ask_user` for opaque
  external commands/deletions/network sends. The model cannot self-label
  an effect harmless at runtime.
- A batch operation should be represented as **one validated manifest**
  executed with per-item journaling, not hundreds of isolated hallucinated
  LLM commands.
- Original §7 loop guard watches recent call names + canonical argument
  hashes, repeated identical calls, same-tool overuse, ping-pong cycles,
  total budget, with graduated **allow → warn → block → kill**.
  Must not block legitimate progress merely because the same operation
  is used with meaningfully different inputs. The guard acts **before**
  native dispatch and preserves typed error feedback.

## 5. Native execution and original workspace placement

The original §3.9 places **Git, linter, debugger and test runner** as
additional **Code/IDE workspace tools**. Core has a domain registry that
can **implement** a minimal Git primitive before the IDE exists, but
the original intent is **not** a Git-centric universal default AI prompt.
### 5.1 Owner confirmation — Core Git versus IDE GitHub CLI (2026-11-04)

The owner reconfirmed the intended division:
- **Lu Core** provides basic local Git repository tools (status, bounded history,
  review diffs, and similarly narrow, read-oriented primitives), without a
  GitHub account connection or automatic branch/commit/push capability.
- **Lu IDE DLC** owns development workflows and advanced Git operations
  (creating/switching branches, staging and committing code, code review,
  pull requests and repository publishing). Its **optional GitHub CLI
  `gh` adapter** can authenticate to GitHub and operate on pull requests,
  issues, repositories, GitHub Actions and releases. See official
  https://cli.github.com/manual/ and https://cli.github.com/manual/gh_auth_login.
- **Git and GitHub are distinct:** `git` works with local repositories;
  `gh` is the GitHub service client. The future IDE must use the **trusted
  host's GitHub account/credential broker**, never expose OAuth tokens to
  model output, require explicit authorization for network/API changes and
  preserve audit/recovery of remote effects. No `gh` model tool,
  authentication, API request or GitHub permission is in TOOLS-03 Core.
- Keep the original unified-domain/narrow-facade rule: Core has a local
  Git domain, while the IDE composes that domain with separately authorized
  development and GitHub platform workflows.

The later Lu v1.0 mandate for baseline Git support is satisfied by building
the native primitive in the core; **activation and advertisement must be
scoped to an explicitly delegated repository/code task or a future IDE
workspace**. No globally advertised Git tool merely because Git is installed.

- Current TOOLS-03 minimal operation set: `git_status`, `git_log`,
  `git_diff`, each a narrow schema, typed Protobuf, host-selected
  repository/executable, `ReadWorkspace` per-turn authorization.
  **No `git reset`, checkout, fetch, push, arbitrary flags, hooks, editable
  config or general-purpose shell entrypoint.**
- Git native subprocess is an **OS program with process side effects**
  (possible locks, hooks, helpers and metadata) and therefore demands
  no-write tests, environment control, lifecycle cleanup and Windows/Linux
  acceptance. The current draft's read-only intent is **not enough**
  to claim malicious-repository containment; tests and implementation
  must explicitly examine hostile repo config and attributes.
- Workspace tool capability is host selected, not a command hidden in
  model text, a retrieved file, a diff hunk or an ANSF memory entry.
  Source code in Git diffs can contain prompt injections; it is data only.
- The original §8 proposed AppContainer/CLM for potentially unsafe code
  execution. The **explicit later owner directive** rejected mandatory
  Lu-wide sandboxing/restricted tokens. Preserve the original aim of safe
  execution but satisfy it with **human-task authorization, supervised
  processes, optional containment, and effect journals**, without falsely
  claiming that a read-only binary automatically cannot mutate metadata.
- Windows and Linux AF_UNIX IPC for future DLCs is documented separately;
  that decision does not authorize expanding v1.0 tools into IDE/NVP
  interfaces.

## 6. Required review method for all remaining tools

Before implementing or modifying **every** TOOLS-* module:

1. **Locate the corresponding original source paragraph**, quote section
   number and intended role (internal boundary? model-facing? workspace?).
2. Check direct later owner approvals; avoid confusing a later assistant
   implementation idea with a changed product requirement.
3. Prepare the **domain / narrow model facade / native Protobuf /
   trusted authority / UI exposure / durable effect** map.
4. Identify what a successful implementation must not do (unapproved
   writes, arbitrary shell, unsafe overreach, live DLC integration).
5. Write tests on the actual executable or parser, including denial,
   truncated/invalid data, prompt-injection data and OS differences.
6. Record the exact committed source SHA and CI evidence. Do not mark a
   source-only tool as installed, user approved or released until an
   end-to-end GUI test proves it.

**Review applies immediately to TOOLS-03 Git and next TOOLS-04 archives**.
It also corrects earlier TOOLS-02 confusion: the JSON tool was an internal
boundary service; only separate `validate` schema operations may be
model-visible in a task-specific manner.

## 7. Reconciliation and unresolved decisions

| Historical original vs newer Lu note | Merged authoritative interpretation |
| --- | --- |
| Git is Code workspace addition (§3.9) vs Lu v1.0 Git in Core | Implement Git **primitives** in Core, **activate** for relevant authorized code/repository tasks; full IDE workspace/DLC remains later |
| ~15 unified native domains (§5.2) vs current per-operation Rust modules | Keep the unified **domain implementation**; present narrow per-task facades to smaller LLMs; no proliferation of unscoped APIs |
| JSON "tool" listed (§5.2) vs mistaken user-facing JSON inspector | Original MBRL controls: **internal** JSON repair/validation; separate `validate` domain for schema operations; no JSON file inspection GUI |
| 1 normalization + 2 regenerations (§4.8) vs later "advanced repair deferred" note | Original bounded MBRL is v1.0; semantic guessing/reconstruction is not required |
| Older system prompt says no permission prompts for ordinary file/web calls (§6A) vs newer host authority | Current explicit owner task-scoped delegation applies; **avoid repeated prompts within a grant** but never infer permission from availability |
| Original AppContainer/CLM for code (§8) vs newer no mandatory sandbox | Later explicit owner decision overrides **mandatory** sandboxing; preserve resource/authority controls and offer optional isolation |
| Original mentions raw Protobuf Tauri IPC (§4.7) vs desktop JSON commands | Model/GUI JSON may be adapters; **native tool/worker messages are Protobuf**, and binary transport is preferred when useful; no unsupported claim of fully wired GUI |
| Original desktop application was Jan fork vs Lu independent shell | Original **tool behavior and architecture** preferred; subsequent owner-approved independent Tauri/Rust Lu platform remains binding |

**Change control:** new tool design notes may enumerate evidence and test gaps,
but they cannot silently edit the original architecture. Any future proposal
to change the unified-domain plan or workspace-facing tool visibility must
name the source conflict explicitly and await a direct owner decision.
