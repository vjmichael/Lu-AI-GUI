# TOOLS-02 — internal Lu JSON validation at model/native boundaries

**Owner correction:** 2026-11-03. **Status:** Native development source, not a user-facing tool. **Branch:** `lu/tools-02-json-validation-20261103` (based on the WRITE-01E frozen checkpoint). This supersedes the briefly proposed JSON file inspector; **that inspector, OS picker, chat button and associated Protobuf file messages were removed**. The accepted installed `0.1.0-alpha.2` is unchanged.

## Purpose — guard structured data entering AND leaving Lu

Lu needs an internal JSON boundary service to prevent broken or ambiguous *structured protocol JSON* from being accepted or emitted during model/tool orchestration. It is not a JSON file editor, a document viewer, or a separate model-advertised `lu_json_inspect` capability. Human-readable chat text remains text, and source file contents are out of scope.

The canonical native tool execution path stays: **untrusted model output → strict JSON envelope parsing → schema-specific argument validation → typed Protobuf → exact Lu host authorization → native tool → typed Protobuf result → strict host JSON serialization and result-envelope checks → model continuation.** The verifier cannot approve effects, choose a model tool, grant filesystem permissions, repair a missing field, or perform a tool call.

## Inbound guardrails

1. **Model envelope:** `parse_model_boundary` already enforces a 64 KiB limit and duplicate-key refusal at *any nesting depth*. One narrowly constrained syntax-only cleanup is allowed for an exact Markdown JSON fence and/or trailing punctuation commas. All other malformed input, truncated escapes, arrays where objects are required, extra prose and oversized data fail closed. A repaired envelope is a *candidate*, not a validated tool authorization.
2. **Model tool arguments:** direct `ModelToolCatalog::encode_call` now calls `parse_exact_boundary_object`, not generic `serde_json::from_str<Value>`. Even callers outside `NativeToolTurn` cannot silently overwrite a duplicate `path`, `name`, nested parameter or other argument. No repair is permitted at this second-stage boundary; the exact registered Protobuf codec continues to enforce allowed and required properties, value types and size limits.
3. **No authority from JSON:** only trusted Lu host grants, origin, current session, workspace and user-turn approval may authorize native effects. Parsing cannot grant rights, and an invalid tool call must never reach dispatch.

## Outbound guardrails

1. `serialize_host_boundary` serializes internally produced tool schemas and JSON results, applies the size budget, and immediately reparses with duplicate-key checking. This refuses malformed output from even a deliberately misbehaving custom serializer.
2. `serialize_host_tool_feedback_boundary` requires an exact `{kind,call_id,name,response}` structure, a valid host-native tool name, and a matching typed `response` with **exactly one** of:
   - `{ok:true, tool:<same name>, result:...}`
   - `{ok:false, tool:<same name>, error:{code,message,retryable}}`
   It refuses mismatched tool identity, missing/extra/contradictory keys, incorrect types, oversize payloads and invalid JSON; it does **not** auto-repair host output.
3. `NativeToolTurn::model_feedback` invokes the validated serializer. The supervised generation loop independently checks the outbound envelope before audit/continuation; any invalid feedback stops the controlled generation path. It never fabricates a success receipt or silently retries a possibly applied native effect.
4. **Plain final chat responses are not JSON tool envelopes.** Ordinary prose can be displayed under the existing display-only allowance; it never becomes an executable JSON tool request.

## Test and CI acceptance

- `src-tauri/lu-core/tests/model_json_boundary.rs`: exact repairs, nested duplicate refusal, broken escapes, truncation, arrays/nonobjects, size caps, no-repair arguments, typed outbound success/failure, mismatched/unknown fields and hostile duplicate-key serializations.
- `src-tauri/lu-core/tests/model_native_files.rs`: tests duplicate direct model arguments cannot enter typed Protobuf dispatch (in addition to the existing path/authorization regression).
- Existing `native_agent_tool_turn` / `supervised_native_turn` tests continue to prove schema checks, model tool result feedback, authority and cancellation/revocation.
- GitHub CI runs these tests on native Linux and Windows under the frozen source-tree check; a green source test is not an installed-app release claim.

## Limits and next gates

- These are boundaries for Lu's **structured model/tool JSON paths**. They do not yet audit every JSON interchange with external HTTP model providers or every persisted desktop setting. Each other JSON edge must be explicitly inventoried/adapted before any claim of universal coverage.
- Content inside a JSON **string** remains opaque text; it is not recursively treated as protocol commands. Required or forbidden **semantic fields** are checked by typed codecs or explicit envelope validators, not guessed from generic JSON syntax.
- Only harmless, deterministic syntax cleanup is in scope; no LLM-generated JSON repair loops, invented values, authorization replay, model-visible repair tool or unlimited retry. Deeper repair might be a later separately measured task.
- Next native tools (Git inspection, archive listing, etc.) are separate from this internal boundary infrastructure. No further WRITE-01E edge-case push is required for TOOLS-02.

## Original MBRL completion extension (2026-11-04, owner-directed)

The original Library design `Jan-Mod-Design-Document.md` §§4.6–4.8 calls for **one deterministic normalization → up to two model regenerations → typed rejection/BLOCKED**, and §5.2 distinguishes internal `json_repair` from `validate_schema`. The phrase "advanced JSON repair" came from later reconciled roadmap shorthand, not a separate owner-specified advanced algorithm. The owner explicitly requests original bounded recovery now; semantic reconstruction and guessed values remain forbidden.

Development source updates:
- `supervised_tool_generation.rs`: up to **two corrective model generations per pre-dispatch rejection**, bounded total generations; no retry after an effect, cancellation, revocation, oversized frame or a host-selected routed call. Records an audit `BOUNDARY_BLOCKED` terminal reason after exhaustion.
- Valid parsed tool attempts that cannot be dispatched receive a host-generated, synthetic, typed `lu_boundary` rejection result with a correlation ID, error code and fixed message. This **is not evidence that any native tool executed**. Malformed/unparseable text cannot be falsely turned into a tool-call record.
- The trusted desktop **proposal staging** paths (read/create/edit) now use `parse_exact_boundary_object`; duplicate model-supplied paths/content/limits cannot be silently overwritten by deserialization. They are still inert until exact native user approval.
- External authenticated llama.cpp SSE event JSON now passes strict bounded parsing and nested duplicate-key refusal. Incomplete streaming frames are assembled before parsing; partial/rejected SSE events never become successful engine JSON. Normal user-facing prose is not a tool protocol.
- Dedicated Linux/Windows CI, supervisor regeneration/rejection tests, desktop proposal tests and SSE tests are required before accepting development branch. Installed desktop native tool orchestration (schema advertisement plus exact real UI authority) is a separate host integration gate; no claim of complete installed-v1 product before it is done.
