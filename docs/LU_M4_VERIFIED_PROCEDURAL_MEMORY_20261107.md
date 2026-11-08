# Lu M4: verified procedural and semantic memory — work-order and acceptance ledger

**Opened:** November 7, 2026. **Base:** M3 draft PR #96, commit `4f786086cd520a78ceda8902f93a3fa02202f9f0`. M3 remains open; M4 is a separate stacked development/testing workstream, not a claim M3 passed.

## M4 scope (approved memory-first roadmap)

- **M4-P01** — source-anchored, verified procedural patterns with concrete tool/application versions, prerequisite and failure-boundary metadata. Test independent evidence promotion, idempotence, and adversarial failures.
- **M4-P02** — belief/semantic claim ledger: distinguish provisional claims from observed or separately verified facts; maintain original-source provenance, freshness and contradiction links.
- **M4-P03** — append-only corrections/supersessions and CCM conflict triage. Human correction must prevent stale directions from being retrieved as current.
- **M4-P04** — source-validated, token-bounded selective recall for model context: uncertainty/version/failure caveats, and permissions never inherited. Need actual local-model tests.
- **M4-P05** — durable restart/model-swap and failure-injection acceptance with source-linked independent receipts; benchmark repeated task success, stale advice, unauthorized effects, latency and rediscovery. M5 remains the complete installed-app E2E release milestone.

## First test group — M4-P01 source implementation

`src-tauri/lu-core/src/ansf_verified_procedures.rs` maintains **derived immutable procedure rows inside the original ANSF SQLite book**. It never copies an original transcript or stores executable power. A procedure carries exact project/task/book/source ID, immutable source digest, trusted checkpoint, journal call and receipt digest, tool ID and version, application version, prerequisites, procedural steps, failure conditions, and optionally an older procedure ID that it replaces.

Promotion fails without all of: a current unedited user/assistant ANSF original; the latest exact task checkpoint containing that source; no unresolved checkpoint blockers; an APPLIED native effect linked to that checkpoint with a nonempty receipt that still matches the actual journal; and **a separate trusted host outcome validator**. This means merely marking a journal receipt APPLIED does not independently certify the workflow. Promotion is idempotent only for identical bytes and exact IDs.

Reads are version-exact, bounded to 16, revalidate current revision and source SHA, the latest checkpoint, journal receipt bytes and the independent validator. A new human correction appends an immutable invalidation referencing an actual original user message. A newly independently verified procedure may supersede an earlier record. Old evidence is preserved for forensic navigation rather than mutated. Retrieval never triggers tool execution.

**Boundary:** This is a Lu Core candidate plus native tests, NOT a Tauri/model-facing production feature, installed Windows proof or finished M4 belief revision. The verifier must be implemented by a trusted host with independent native postcondition tests; an arbitrary model Boolean is not sufficient. No model-selected path or historical approval grants tool authorization.

## Required acceptance evidence

1. Exact-head Windows/Linux Rust tests compiling and executing against real ANSF/Continuity/EffectJournal source. No pass from a mocked SQL schema or source grep alone.
2. Deny unverified, absent, FAILED/UNCERTAIN, mismatched or superseded evidence. Allow repeat promotion only when byte-for-byte identical; persist invalidations across reopen. Deny changed version and untrusted model-supplied writes.
3. P02/P03 independent correction/revision, conflict and freshness tests.
4. P04 real GGUF retrieval and provenance/uncertainty output, plus permission-negative tests.
5. P05 genuine installed Windows and restart/model-replacement acceptance, separated from source compile.

**Current status:** P01 source and CI submitted; native job results have not yet been observed. P02–P05 remain open. M3 remains draft and independently gated.
