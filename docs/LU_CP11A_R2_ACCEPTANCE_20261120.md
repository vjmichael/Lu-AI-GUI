# CP-11A-R2 — Loaded-model benchmarking

Status: scoped installed Debian implementation and acceptance passed; expanded lifecycle and general capability qualification remain open. No publication, merge, new CI or release. Tested product source commit `636f43f054e3cdf05459eddeaf7af28e76dfe6bc`; installed binary SHA-256 `3913bcba10df05058a3c98cbbcc26d1c596796033e33846bbe6647dc47ecf9b3`. The witness script and record can change without replacing the compiled binary.

## Scope and design

Opt-in Check loaded model uses three fixed synthetic text tasks, the currently loaded model only, native approval and a durable check report. Independent expected-answer comparison uses an exact trimmed string. Raw responses and real conversation data are not stored in the report. Report records elapsed and first-output time, output bytes/pieces, sampled process RAM, and Linux engine CPU time. Output pieces are not tokenizer counts. Exclusive idle-queue reservation yields to pending ordinary conversation work, bounded per-case and total deadlines, explicit stop and existing worker supervision. Cancellation can eject the active model; no automatic model reload or benchmark replay. No new download or external tools are required by the benchmark.

## Executed acceptance

- Source lock, native build, frontend build and focused UI tests (7/7) passed. Focused production module tests: 39 passed, 0 failed; one ignored subprocess fixture is invoked by its parent test. New whole-suite UI/native regression not executed; two prior unchanged baseline UI failures and five skips remain recorded separately.
- Installed root-owned nonrelease Debian package, genuine pinned Qwen3-0.6B Q4_K_M CPU inference (model SHA-256 `7af3fdf842f87b24672f8a7f1dd50404043f0bfb71093ff91c31d2b49df4631d`), native accessibility actions in a disposable non-root desktop session.
- Independent installed runs both: refusal passed, approved benchmark completed, 2/3 exact-answer tasks correct (extraction and corrected context pass; arithmetic exact output fails). Failure of the model's answer is a recorded outcome, not failure of the acceptance controller. First run inference elapsed 6.842542 s, second 5.029466 s; sampled peak process RAM 1,261,703,168 and 1,261,748,224 bytes respectively. CPU time is aggregate process CPU, not elapsed wall time. No inference token count or GPU resource claim.
- Active graphical Stop settled in 374.6 ms and 330.7 ms, respectively; both stopped before the first verified result, and both ejected the active engine. Second run verified cold reopen preserves stopped report without replay. Fresh refusal following explicit reload not qualified: AT-SPI could not find the Privacy & Observe navigation witness. Second overall receipt is INCOMPLETE; initial setup/readiness fixture failures are retained separately.

## Remaining

Installed conversation-priority and interruption during in-flight model inference, post-reload fresh approval/refusal, broader repeated model/task comparisons, token-based throughput and GPU resource measurements, Windows and user physical-host acceptance. No comprehensive CP-09/09M claim. CP-11B scheduled user automation remains independent. Keep model checks opted in and engineering details out of ordinary conversation UI.
