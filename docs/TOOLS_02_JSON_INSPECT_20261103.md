# TOOLS-02 — host-approved JSON syntax and shape inspection

**Status:** Development source; not exposed in the accepted installed `0.1.0-alpha.2`. Branch `lu/tools-02-json-validation-20261103` from verified WRITE-01E checkpoint `2a9a1355`. This is a new tool, **not** another WRITE-01E transaction enhancement.

## What is implemented

- `lu.json.inspect` (`lu_json_inspect` for model schemas): genuine `NativeToolRegistry` Core module, typed `InspectJsonFileRequest/Result` Protobuf, a strict `ModelToolCodec`, and deliberately `ReadWorkspace`-classified effect.
- Scope root is supplied **only by the trusted host** at registration and opened using cap-std's capability directory. A model-proposed path must be relative, nonempty, bounded and free of traversal, backslash, drive-letter / colon and control characters. No MCP server or shell.
- Reads at most 512 KiB; refuses oversized files rather than treating truncated JSON as valid. Regular-file check, final symlink refusal, and opened-handle file-type/size checks; additional cross-process alias race/Windows reparse-point adversarial acceptance remains a separate gate, not implicitly solved.
- Strict JSON parse refuses **duplicate object keys at any nesting level**, syntax errors, extra trailing data and deep recursion using serde_json's depth budget. Returns only a valid/invalid result, root kind, top-level key *names* (up to 32 and truncated to 128 Unicode codepoints), root array length, exact observed content SHA-256, and fixed error family/line/column; never raw values or source file bytes.
- No writes, formatting, auto-repair, schema assertion, destructive inspection or model-derived approval. The content digest is evidence of observed bytes, not a write permission. Model-level JSON call structure remains strict, and discovery doesn't grant the tool.
- Added a real `NativeToolTurn` test through the existing registry/catalog/ConversationAuthority flow. The test also verifies revocation stops further model continuations. Host-selected tool availability still requires future desktop UI integration; do not claim the alpha has a visible button.

## Native tests and acceptance ledger

- Module tests: valid JSON, redaction, digest, nested duplicate key classification, truncated JSON, traversal/drive escape, unrecognized argument, size limits and explicit denial.
- Existing supervised/native turn integration: registered native tool schema, scoped, authorized dispatch through `lu_json_inspect`, immediate task revocation.
- Workflow `.github/workflows/lu-tools-02-json.yml` executes both categories on native Linux and Windows and pins the source lock first. **Pending exact-commit CI** at source creation.
- v1.0 GUI acceptance deferred: user directory picker, model opt-in read grant/expiry, nonblocking progress, noisy error diagnostics, resilience and restart. Current tool registration requires host code to invoke it with an already approved root.

## Subsequent tools, not further WRITE-01E expansion

- TOOLS-03 Git status/log/read-only diff with host-selected repository and no unsafe model-supplied shell text; test hostile repository config/hooks and OS behavior.
- TOOLS-04 archive manifest inspection, traversal/bomb checks and distinct authority for extraction/creation.
- Other deterministic tools (source parsers, schema validation, safe process inventory) added via the same typed and host-granted registry, not a parallel plugin runtime.

**Separate v2.0 roadmap only:** `LU_V2_0_DLC_AF_UNIX_RESOURCE_REASONING_DEFERRED_20261103.md` (AF_UNIX Windows/Linux, NVP cooperative model unload/reload and Cognitum optional logic/evidence). No DLC implementation here.
