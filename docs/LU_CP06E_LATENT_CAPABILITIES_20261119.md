# CP-06E — Latent capability reachability and local qualification (2026-11-19 UTC)

Status: **PARTIAL PASS — all 42 original named codecs dispositioned; four of eleven previously latent codecs connected and genuinely qualified within bounded local scope; seven remain open.** Baseline 9928e4ffda8d8168bc257720f76bf616861822af. Ten changed source/test blobs exactly match compiled files. Historical non-runtime docs/workflows were not reconstructed wholesale.

## Four new routes
- XLSX: explicit original request, e.g. `Create an XLSX spreadsheet titled Budget with rows: [["Item","Count","Active"],["=1+1",12,true],["Tea",-2,false]]`. One named sheet (bounded ASCII <=31 bytes), rectangular 1–16 rows, <=8 columns and <=64 cells, bounded text <=256 bytes, booleans, exactly representable bounded integers. Reject unsupported values/formula authoring; formula-looking text stays literal. Genuine inert create_xlsx model selection, native destination selection and exact human review, separate confirmation, existing typed writer and exact policy, verified package size/hash and required OOXML parts plus APPLIED receipt. No overwrite/replay. No arbitrary spreadsheet drafting, charting, formula evaluation or CSV claim. Native spreadsheet recovery receipt recognized.
- TAR: explicit named files e.g. `Create a TAR archive from Alpha.txt and Beta.txt in my folder.` One to four distinct simple ASCII leaves <=100 bytes, <=4 MiB per file and <=8 MiB total, one selected source/destination folder, no symlink/nonregular/traversal. Exact member list/size/hash and deterministic output bound to review and typed payload; inert create_tar selection, separate approval, existing native Linux archive creator. Plain TAR only, no external compressor or extraction exposed. Independently compare actual archive manifest/member bytes, output size/hash and originals; TAR_CREATE_DIRECT_V1 receipt, typed recovery. No overwrite or automatic replay.
- Background status/result: actual native read-only model tool call for original named task question, e.g. `Show status of background task scan-job.` / `Show result of background task scan-job.` Authenticated current foreground turn and owning conversation, singleton catalog and host-bound exact request. Foreign owner, nonterminal result and revoked/cancelled turn rejected. Status detail/checkpoint not model-facing. Ground stored task status and terminal SHA scan receipt; never imply live process checks or independently verified exported file. Seeded persisted test task/receipt, not real scan launched. All four genuine observations exact model reproduction.
- Existing per-turn grants, original request/session, active foreground ticket, separate one-use write approval, receipt verification and cancellation fences remain. Desktop adapters compiled. Fixture-host choices, not installed native dialogs.

## Final local qualification

Offline locked Rust 1.99.1 and restored GUI dependencies. Core 688/0/28 ignored; desktop 415/0/15 ignored; eight selected integrations 40/0/6 ignored. Ordinary 1,143 passes, genuine opt-ins separately. Three new native tests and one desktop recovery-type test included. Checks include unsafe/extra fields, invalid rows, typed literals, stale/missing TAR input, collisions, exact member witness, refusal, ownership, nonterminal result and original authority fences. Existing transfer/custody/Office and ZIP/ISO contract fixtures pass, not proof of newly connected extraction.

| Final model | XLSX | TAR | Status | Result | Two refusals | Time |
|---|---|---|---|---|---|---|
| Qwen3 0.6B Q4_K_M | PASS | PASS | PASS | PASS | no effect | 36.97s |
| Qwen2.5 1.5B Instruct Q4_K_M | PASS | PASS | PASS | PASS | no effect | 72.34s |

Both final exits 0. All 12 cases and 48 read-only reopened SQLite databases independently audited: four actual publications, four stored-task observations, four refusals/no output/effect, zero fallbacks. Verify actual XLSX XML and types including literal =1+1, no formula nodes, TAR bytes/membership and preserved originals, original-turn/selection/preview/confirmation, native effect fingerprints and receipt/observation hashes, terminal stored record and task ownership, exact persisted chat answer. Preliminary four-case runs passed before TAR; logs retained, not counted again. Background record observation does not constitute live worker launch or scan acceptance.

Pinned engine SHA-256 c9816014cab57b18903b365c82251741ad667a5f668573c3824b034aa556bc97; Qwen3 7af3fdf842f87b24672f8a7f1dd50404043f0bfb71093ff91c31d2b49df4631d; Qwen2.5 6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e. Evidence lu-cp06e-latent-evidence.zip SHA-256 44a1569cb4b7c97b10a759d467845e79797ddf7704bf05d8cdf6a0e105aeb955; 211 entries with final source, baseline patch, manifest, logs, evidence databases and audit.

## Full inventory and remaining work

42 originally enumerated concrete names: 13 prior locally checked read codecs (some host fallbacks; not genuinely rerun in E), nine prior scoped CP-06D confirmed effects, four newly connected, eight external unqualified, seven latent remaining and one intentionally withheld. 26/42 bounded prior/new evidence, not simultaneous grants or exhaustive coverage of all project APIs. Companion JSON retains each exact original name and current disposition.

- E-R1: selected-member ZIP, ISO and Linux archive extraction. Existing supervised full-tree route is not equivalent, and does not cover ISO. Bind specifically selected entries to protected workers, exact approval, source/output witnesses. Do not bypass custody or expand to all members.
- E-R2: background start/cancel. Original reviewed bounded job/owned task, OS-owned custody and observed launch/termination. Local cgroup mount is read-only; installed Windows custody not qualified. No arbitrary command/PID or automatic replay.
- E-R3: native web fetch and academic search. Existing typed fixtures and operator research UI not evidence of actual model-directed live network use. Require explicit egress authorization, approved/pinned transport/provider and independent source witnesses.
- F: four reviewed Git names, browser read, media/PDF/image real external worker acceptance; consolidated installed/physical acceptance of earlier/new routes. Browser interact deliberately withheld for lack of ordinary NetworkWrite authority.

CP-06E stays open. No installed GUI consent, live OS-owned background launch, actual network execution, generic semantic verification, physical GPU acceptance, new branch/PR/manual CI/main merge or release asserted.
