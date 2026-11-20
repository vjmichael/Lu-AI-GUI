# CP-11A — Opt-in local checks and measured diagnostic reporting

## Product behavior

Settings → Privacy & Observe → Check Lu is an explicitly requested, one-shot local check; nothing runs on app startup or when the panel mounts. The fixed native consent dialog describes the scope, no model load, no personal files, no network, and no automatic repair. The native host reserves at most one run, rechecks current foreground work and available RAM before work and between units, and supports cooperative stop and shutdown fencing. The ordinary model registry receives no new tool. The panel receives only plain-language check outcomes and measured sample-workload duration; it does not reveal tool names, runtime IDs, paths, or protocol details.

The current suite uses only generated sample data:

* Reading: exact UTF-8 text, Unicode window and invalid encoding refusal through the production Reader.
* Saved history: actual fresh SQLite ANSF store, original source preservation, correction head, omission of superseded implicit source, cross-conversation read refusal, and close/reopen.
* Sample history search: 128 generated messages and 16 exact search/readback comparisons through production FTS. Timing includes fixture preparation and all queries; it is not a model benchmark, general scalability measurement, or an estimated speedup.

The one latest typed report is kept in the existing diagnostic archive. The host verifies the ordered result prefix and exact run identity on writes, reopens the record at completion, and rejects malformed or impossible complete reports. The existing explicit local troubleshooting-report confirmation includes these fixed categories, measured durations and counts, not the sample contents or any user conversation. The report is not a signed OS attestation and cannot authorize work or claim any cause that was not observed.

Cancellation is cooperative at check boundaries and between individual sample inserts/queries, with a 30-second budget checked at those boundaries (not a hard real-time interrupt of a blocked SQLite/OS call). Shutdown requests stop and allows existing bounded exit grace. On reopening a live journal it is marked interrupted; there is no automatic retry. The disposable workspace is exclusively created, cleaned by fixed filenames only on normal completion, and never recursively deleted by the product. An abandoned or pre-existing workspace is deliberately refused and not automatically removed. Such recovery needs an explicit reviewed solution; this slice does not modify real chats, run shell commands, perform general repair, or launch scheduled work.

## Acceptance disposition

Local Debian native verification: 7 focused tests passed (one subprocess-only ignored entry was actually invoked by its parent); includes actual sample operations, consent refusal, duplicate exclusion, stop, startup recovery, an abruptly exiting child process, preservation of an existing workspace, corrupt report refusal and troubleshooting report projection. Full desktop 429 passed / 0 failed / 18 ignored; Core 702 passed / 0 failed / 32 ignored. React build and 59/59 source/bundle checks passed, 3/3 new presentation/authority contracts. These are native app-library fixtures, not an operator-driven installed graphical run. See preserved receipt for actual measured timings.

CP-11A remains open for installed native consent/stop and restart, safe abandoned sample-workspace reconciliation, and broader separately approved functional/stress/recovery and real-model capability benchmarks. CP-09 remains the matched-model/paired empirical benchmark; CP-09M remains the memory ablation. CP-12A remains original correlated diagnostic tracing; no other diagnosis or self-repair authority implied. CP-11B remains separate scheduled user automation. No new dependency, remote source publication or CI dispatch.
