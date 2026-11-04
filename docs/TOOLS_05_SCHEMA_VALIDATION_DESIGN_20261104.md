# TOOLS-05 — Schema validation, original-first design review

**Date:** 2026-11-04
**Scope:** Lu Core, read-only **caller-supplied JSON Schema validation**. No file access, network, JSON repair, model authority, IDE workflow or UI.
**Source precedence:** [original Jan design](reference/Jan-Mod-Design-Document.md), §5.2 `validate_schema`; [merged authoritative native tool design](LU_NATIVE_TOOLS_MERGED_DESIGN.md), §2 and §3. This is a subordinate implementation/test record.

## Source requirement mapping

1. **Original §5.2** lists the `validate` domain separately, with `validate_schema` retained for actual schema checking; the `json` domain is explicitly for model-boundary repair. They are **not interchangeable**.
2. **Original §§4.6–4.8:** narrow model-visible JSON arguments → validated, typed Protobuf native tool → bounded, structured host result; reject duplicate/invalid JSON and unknown fields **before** dispatch. Input validation must not silently repair or coerce data.
3. **Original §4.9 and §7:** effect admission, no ambiguous retries, and loop/repetition guard apply to all model-visible tools. A result `valid: false` is a **successful validation operation finding nonconforming data**, not authorization to rewrite it.
4. **Owner-confirmed Lu v1:** keep consolidated native domain handler, no MCP requirement, no arbitrary tool permissions from model or catalog. File validation can later be composed with a separately authorized file-read and reviewed write/edit pipeline. **This initial tool accepts only JSON values present in its request**; no paths or filesystem side effects.
5. **Established standard library:** `jsonschema` Rust crate (pinned 0.55.0; native Draft 2020-12 engine, no custom schema compiler); `regex` Rust crate for linear-time regex preflight. The `jsonschema` crate has all remote/file reference-fetching features **disabled**. It is invoked entirely in memory. See https://docs.rs/crate/jsonschema/0.55.0 and https://docs.rs/jsonschema/0.55.0/jsonschema/draft202012/.

## Native and model interfaces

| Layer | Contract |
| --- | --- |
| Internal module / tool IDs | `lu.validate` / `lu.validate.schema` |
| Narrow model facade | `lu_validate_schema(schema,data)` — two required JSON values, no optional action text or file paths |
| Host Protobuf | `SchemaValidateRequest{schema_json,instance_json}` → `SchemaValidateResult{valid,issues,truncated}` |
| Parsing | Existing TOOLS-02 duplicate-safe model argument boundary; canonicalized/length-checked Protobuf JSON bytes at native dispatch. No syntax repair. |
| Actual validation | Draft 2020-12 schema meta-validation and runtime validator, with enabled formats |
| Offline | `resolve-http` / `resolve-file` not enabled, nonlocal `$ref` refused, schema ids/dynamic refs restricted |
| Bounded budgets | 16 KiB schema, 32 KiB instance, schema 1024 nodes, instance 4096 nodes, tree depth 32, regex <=128 chars and 32 patterns, <=32 bounded local refs, at most 8 returned diagnostics |
| Authority | Original `ToolEffect` has no **Pure** variant; use conservative `ReadWorkspace` task scope pending a separate cross-cutting effect taxonomy migration; this tool still performs zero workspace I/O |
| Result meaning | `ok:true,valid:true` conforming; `ok:true,valid:false` nonconforming; `ok:false` malformed/unsafe schema, invalid Protobuf, or policy denial |

A validated JSON instance may be an object, array, scalar or `null`; schema may be an object or boolean. This tool **does not** validate files by path, execute arbitrary scripts, follow URI references, download schema documents, automatically repair a document, or run external linters. Those are separate capabilities and permissions.

## Branch and release gating

**Independent development branch:** `lu/tools-05-schema-validation-20261104`, based on verified TOOLS-02 JSON boundary rather than the Git (TOOLS-03) or ZIP (TOOLS-04) unfinished CI/lockfile branches. This avoids mistaking inherited Git/ZIP failures for schema-validator defects; all three native tool branches require later deliberate integration.

**Tests:** genuine Draft 2020-12 constraints including required/properties/minimum, accepted/rejected data, safe local `$defs` references, bounded errors, invalid schemas, malicious remote ref attempts, unsupported regex, canonical Protobuf and duplicate-key refusal, permission-denied/revoked user turns, OS Linux/Windows CI.

**Unfinished:** Cargo-generated lock updates for new established crates, exact frozen-source CI, both platform tests, native desktop model tool/approval dispatch and eventual installed Windows acceptance. No claim of integrated GUI feature or accepted v1.0. User accepted baseline `0.1.0-alpha.2` untouched.
