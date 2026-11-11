# Lu P0-MEMORY — Original ANSF context and cross-session reasoning

**Date:** November 11, 2026  
**Branch:** lu/p0-memory-original-continuity-20261111  
**Source parent:** ca6c3bc07c716cb8af43d28e48229f9b47b0de3a  
**Status:** implementation committed as source candidate. Compiler, local GGUF, installed native Windows/Linux, CI/PR and regression acceptance deferred by owner.

## Gap addressed

Lu already had immutable ANSF SQLite books and a profile Library, per-chat goals and human clarifications, original message and semantic retrieval, source-hash receipts for native Browser/Reader/OCR/Observe, independently checked claim/retraction/procedure records, and source-backed task checkpoints. However, a later explicit project/task continuation still used a partial checkpoint-only context, separate from source provenance, correction/contradiction state and human task-criterion evidence. It could not explain whether another conversation had a conflicting claim or whether the original correction still existed after restart.

## Implemented

1. **One original-source continuity capsule:** Normal GGUF Send now recognizes an exact current human instruction such as "Continue project lu task review_git_head" or the existing "Resume project lu task review_git_head". The host verifies the newly appended original ANSF user turn and original active conversation; a recalled document/model message can never choose a project or task. It reads from the currently opened native Lu profile, not an arbitrary model-selected pathname, and never starts an effect.
2. **Cross-session recovery:** The host reopens the latest source-backed checkpoint for the exact project/task across conversations in the same profile, revalidates every original ANSF source ID, uses the original CCM comparison, and reopens the checkpoint's verified-effect references. Prior work and suggested next actions are explicitly *reported historical task text*, not independently established completion. A checkpoint without original sources is not invented.
3. **Goals/clarifications:** From the checkpoint's originating conversation, Lu reconstructs the original ANSF goal revision and human answered/pending/deferred questions. Deferred responses are not permissions and remain open. Only bounded excerpts are passed into the local model's current context.
4. **Real source history:** The capsule includes a bounded, integrity-checked view of historical, same-original-conversation Browser/Reader source IDs and SHA-256 fingerprints. Source bytes remain absent and freshness is NOT RECHECKED; these other source observations are NOT automatically tied to the task's acceptance criterion. OCR transcriptions and private Observe metadata/pixels/notes are excluded.
5. **Source-based belief revision across conversations:** Added a task-wide original claim-assessment API in the existing ANSF book that reuses the real original source and independent outcome verifier. The desktop scans at most 24 same-profile original books plus the priority checkpoint/current sources. It reports verified-at-observation, unverified, stale, corrected/retracted and superseded claim statuses. Contradictions are keyed by exact subject+predicate, not by unrelated task facts; conflicting values across old and new conversations require human review rather than selecting a winner. Claims and corrections remain original immutable book records; there is **no second memory database**.
6. **Correction origin integrity:** Every retraction or supersession must still point to a later, current genuine human ANSF message. If that source is edited, removed, inaccessible or ambiguous after restart, the older claim is marked STALE and referred for human reconciliation. It is never resurrected as verified. The historical claim assessment and exact targeted belief recall share this stronger rule.
7. **Review-decision continuity:** For an exact PLAN05 register/task selector, Lu reads the existing P0-TOOLS-05 human criterion-evidence chain, checks original plan/worker/oracle witnesses where the task's register lease still matches, and labels decisions historical. Neither human evidence acceptance nor a prior worker receipt constitutes fresh filesystem source verification or task completion.
8. **Bounded and fail-closed:** Every capsule is capped, samples are explicitly marked incomplete when older chats, claims, original IDs, source observations or worker receipts were omitted. More than 24 candidate chats or 8 claims requires human review, not a false "no conflicts" assertion. Unreadable original journals/claims produce an unavailable/reconciliation status, never an invented success.
9. **Single model context:** Removed the old checkpoint-only resume envelope from normal Send and skips the older overlapping 03B capsule for exact continuation requests. Regular non-resume lexical/semantic recall and explicit M4 single-claim/procedure retrieval continue to work. There is no new system role, model tool, authority grant, worker scheduling, auto-retry or write permission.

## Trust and status contract

- Historical **VERIFIED_AT_OBSERVATION** does not mean current-source freshness, surviving OS descendant custody, final task acceptance or access to the old file.
- Source IDs are immutable locators, never executable commands.
- Reported/unverified claims never become facts by frequency; contradictions and edited human corrections block automatic confidence.
- Private Observe and OCR data never enter model context through this retrieval.
- A model's suggestion cannot write a human correction, close a clarification, mark a task done or authorize the next effect.
- Current operator source selection, permissions and independent execution verification belong to the **future** native operation, not memory.

## Source-level test cases written (NOT run)

Exact continuation-selector vs. negation/attachments/extra directives; fallback without task completion or authorization; task-wide unrelated claim keys vs. exact conflicts; original human correction surviving ANSF SQLite restart; corrected human message edited later causing STALE instead of resurrecting previous truth. Existing targeted belief, continuity, source and private-observe tests remain in repository.

## Remaining groups

**P0-AUTH:** Unify OS process custody, effect authorization, cancellation, uncertain interrupted work and source revision checks across native effects, Browser/Reader/Observe and specialist workers. Automatic recovery must remain conservative.

**P0-GATE:** One consolidated codebase-wide compiler, installed Windows/Linux, genuine loaded GGUF, native consent/denial/restart, CI/PR and regression acceptance after remaining implementation. This group deliberately did not run those tasks or authorize any merge/release.

**Stop at P0-MEMORY.**