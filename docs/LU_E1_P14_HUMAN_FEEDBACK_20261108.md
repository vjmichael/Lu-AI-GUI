# E1-P14 — Explain verified and uncertain native outcomes to the human

**Source candidate:** stacked on E1-P13 (PR #144) at source SHA `0a9c4cff81ac1bb52779c0ac75eaac874de7965d`.

## Motivation and scope

The E1-P09/P10/P12/P13 pipeline records model turns, OS effect journals, independent factual outcome observations, and approved-source revision custody. Showing machine codes such as `EFFECT_UNCERTAIN_OR_IN_FLIGHT` and `INDEPENDENTLY_VERIFIED_AT_OBSERVATION` without explanation makes it difficult for ordinary people to distinguish an AI answer, an executed operation, a verified external fact, and an operation that might be incomplete.

E1-P14 adds **presentation-only**, human-readable status explanations and suggested *manual* next checks to the already-existing read-only **Native tool activity** panel. The host still supplies the status from its persisted journals. The browser/frontend neither determines the verdict nor changes status, grants, effect records, worker lifetime or source identity. This is a human-facing Feedback (F) improvement in the user-defined K/I/A/F philosophy; it is not a new capability or more authority.

## Contract

1. `OUTCOME_CONFLICT`, missing receipts, rejected actions and uncertain/in-flight effects are explicitly non-successful.
2. An APPLIED receipt means OS execution was recorded; it does **not** independently prove every fact.
3. `INDEPENDENTLY_VERIFIED_AT_OBSERVATION` means the source matched **at that observation**; it does not prove the source remains unchanged or that a worker is still terminated.
4. A normal `COMPLETED` model/tool summary is not equivalent to a verified OS effect.
5. Unknown/new status codes and JavaScript prototype keys fail closed to **Execution status unavailable**; raw unrecognized codes are not echoed as authority.
6. Suggested next steps are read-only text, not buttons or commands; no automatic retry, permission creation, recovery replay or approved-source rescan is triggered.

## Evidence and remaining gates

- Standalone Node tests cover all current reviewed statuses, untrusted keys, historical-provenance limits, and refusal to promote ordinary model completion to OS proof.
- Existing reviewed-app UI source-contract tests assert the panel actually uses the explanations and continues to expose no model-controlled path/argv/executable/permission.
- **OPEN:** exact-head Windows/Linux source-lock, Node UI suite, desktop React build, original E1 native worker acceptance, genuine GGUF Git/FFprobe selection, source custody, and physical installed-Windows/WebView human inspection.
- No E1-P03/P04B/P06B/P06E/P07/P08C/P09/P10/P11/P12/P13 runtime guarantee is elevated by these UI tests.
