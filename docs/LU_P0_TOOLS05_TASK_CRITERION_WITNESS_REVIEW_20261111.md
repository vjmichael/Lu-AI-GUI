# Lu P0-TOOLS-05 — Independent execution evidence to human task-criterion review

**Date:** November 11, 2026  
**Branch:** lu/p0-tools05-task-witness-review-20261111  
**Parent:** d7d769d4b0d5b7fa73fc787937109ab673c92e3b  
**Status:** source implementation candidate. Compiler, native installed Windows/Linux, loaded GGUF, CI/PR and regression acceptance are deferred by owner.

## Problem closed

P0-Next gave a human-approved project task an original ANSF goal and agent-plan handoff. The task board already showed historical independent worker outcomes from AGENT-01C. It did not allow a person to bind an original, genuinely checked worker/oracle receipt to the **exact current task acceptance criterion** in a durable reviewed record.

## Source implementation

1. Added a host-only *exact witness* adapter on the existing project-task agent handoff. It reopens the original agent plan, revision, latest source-backed goal and actual native TaskStore/EffectJournal/independent-oracle receipt. The witness must come from the exact current STAGED human-reviewed Git/media plan, exact task, current board/register lease and original receipt SHA. VERIFIED_AT_OBSERVATION is **historical**; unsupported, conflicting, stale, substituted-tool/method and wrong-plan results are ineligible.
2. Added a new append-only, individually SHA-256-checked and chain-linked criterion-evidence journal inside the existing ANSF Library SQLite database. No other memory DB, execution process or task register is created. Each event pins the exact register/task/version, board/source lease, original requirement and criterion bytes+hash, source evidence graph/status, independent tool/method, original effect receipt, independent observed bytes hash, human decision and written reason. Every audit row has no_execution_authority=1.
3. On the native Project Tasks window the user chooses an eligible original witness and one decision: ACCEPTED_AS_SUPPORTING_EVIDENCE, REJECTED_AS_IRRELEVANT, or NEEDS_MORE_INDEPENDENT_EVIDENCE. A reason of 10–500 characters is mandatory. A fresh OS folder picker chooses the actual project root; native Lu re-scans its evidence graph/status and compares its source against the original reviewed register. The host reopens the original worker journals before a native human confirmation and **again afterwards**, then appends exactly one version-bound decision under an immediate SQLite transaction.
4. The UI can reopen original reviewed decisions, including accepted/rejected relevance, exact original criterion, independent method/receipt and whether the task register/board lease is now stale. A changed task/version/source requires reconciliation; the old human decision remains historical rather than being upgraded to a current fact. A second contradictory decision for the same original witness and task version is refused, not overwritten.
5. Original PLAN-05 task status, independently_verified_requirement, and all acceptance flags remain untouched. **A human acceptance of one supporting receipt does not establish full feature completion.** No task status, code change, tool grant, next agent step or subprocess can be authorized by this journal or review UI.
6. Current independent method eligibility is deliberately narrow: original AGENT scheduled Git HEAD/status/branch/history and FFprobe WAV/MP4 methods that pass the existing independent source oracle. Generic APPLIED process receipts, unplanned operations, model replies, copied source locators and older/superseded plan revisions are not evidence for human criterion admission.

## Boundaries and gaps

- This review is about criterion **evidence relevance**, not task completion. Final exact requirement acceptance remains with the independently authorized PLAN-07 source revalidation and human status workflow. A witness for Git HEAD only proves the witnessed Git source fact; it does not prove a complex feature implemented and verified.
- P0-TOOLS-04 PDFInfo and ExifTool are currently standalone reviewed worker operations. They produce original independent oracle records, but *do not have approved AGENT plan-step types or task-specific acceptance criteria.* This group does **not** launder their receipts into the existing Git/media witness vocabulary. A future typed plan revision plus separate source-oracle method-to-criterion mapping is required.
- Source rechecks are repeated before and after the native operator dialog, and the final register/board audit is transactionally leased; this is not a complete atomic filesystem snapshot. An external file changed immediately after a scan cannot be guaranteed against without stronger OS snapshot/open-handle protections. That belongs to P0-AUTH and later installed acceptance.
- Original SHA-256 is tamper-evidence, not proof of authenticity against an adversary able to replace SQLite database and recompute all hashes. Original native worker/oracle journals are always reopened for admission.
- No compiler tests, genuine GGUF selection, physical device, installed restart UX, CI, PR or regression runs were initiated, by owner development-first instruction. Source-level tests were added for exact independently admitted method mapping and for denial of forged task completion/execution flags.

## Next work

**P0-MEMORY:** ANSF task, corrected-belief and source continuity across sessions, protecting source staleness and authority.

**P0-AUTH:** Unified per-tool authorization, process custody, cancellation and uncertain-effect recovery, including stronger source identity pinning.

**Deferred consolidated acceptance:** compiler, native Windows/Linux, genuine GGUF tool selection, installed app human interactions, restart recovery, CI/PR verification and regression testing when explicitly authorized.

**Stop point:** P0-TOOLS-05 source handoff completed without CI or PR submission.

## P0-NEXT-CM1 — method-to-criterion review admission (source candidate)

New positive human supporting-evidence decisions require the EXACT current
human-reviewed task criterion to start with a method binding, for example:

    LU_CRITERION_METHOD_V1:lu.reviewed.git.head|GIT_HEAD_DIRECT_V1
    Observe the original Git HEAD for a separate human source review.

The original independently audited worker tool and oracle method must equal
the header; generic or substituted Git/media receipts and unsupported PDF/Image
methods cannot be accepted as supporting evidence. The host validates before
the human dialog, and repeats the gate after OS-selected source rescan and
original receipt reopening. The task UI disables positive review for unmatched
criteria while retaining reject and needs-more-evidence. Original v1 history
stays readable, all task completion and execution authority flags remain
false, and no original SEC-06 acceptance or installed tests are waived.
Rust unit tests cover exact and hostile ambiguous bindings. This separate
source branch remains unmerged pending batched native acceptance.
