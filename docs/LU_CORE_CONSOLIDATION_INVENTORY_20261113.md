# Lu Core source convergence — exhaustive branch and PR inventory

**Snapshot UTC:** 2026-11-14T05:37:49.764Z
**Canonical tracking index:** [Lu Development Ledger](LU_DEVELOPMENT_LEDGER.md) on `main`.
**Source comparison target:** CORE-01E PR #226, exact SHA `f4beaba3f6e77693f9048cd7e67764142d6c41d2`.
**Snapshot main:** `2e31b22a597c8a92aec979d9ff057401c518088c`. This is an audit of committed source ancestry, never native/installed acceptance.

## Counts and rules

- Open PRs total: 215; Core: 121; IDE excluded: 94.
- Live branches total: 286; Core exact-head comparisons: 158; Core with no open PR: 37.
- Exact Core branch statuses: ahead=103, identical=1, behind=1, diverged=53.
- Compared each branch exact SHA as base to frozen CORE-01E as head. INHERITED means branch head is an ancestor of CORE-01E, **not** merged to main. DIVERGED means source overlaps require selective diff; do not merge its full historical stack. NEW DESCENDANT means unique commits after CORE-01E.
- Exclude `ide/*` and P7L/KoboldCpp alternative runtime. Continue llama.cpp. Optional PERF P6 multi-GPU is deferred. Interop/DLC contracts require scope review, not specialist runtime import.

## All open Core PR heads (121)

| PR and title | Exact SHA | PR base | Relative to CORE-01E | Disposition |
| --- | --- | --- | --- | --- |
| [#235 P0-NEXT-CM1 (draft): bind original independent receipts to exact task criterion methods](https://github.com/vjmichael/Lu-AI-GUI/pull/235) | `96ef9e5ce37c321bc5b59fb2656464f804366b4e` | `lu/core01-source-convergence-no-ide-no-kobold-20261112` | NEW DESCENDANT +2 | INTEGRATE NEW DELTA |
| [#229 CORE-01F-03/04 (draft): preserve main ledger and reconcile native/installed blockers](https://github.com/vjmichael/Lu-AI-GUI/pull/229) | `6e9c4e4a7d30677d167119606dceb43800db6659` | `lu/core01-source-convergence-no-ide-no-kobold-20261112` | DIVERGED (Core +125, source +10) | DIFF REVIEW |
| [#228 CORE-01F (draft): read-only Lu Core merge graph, source preflight and staged integration ledger](https://github.com/vjmichael/Lu-AI-GUI/pull/228) | `af56b2c09c9b5a1f0e59b290d58fcdc94d936855` | `lu/core01-source-convergence-no-ide-no-kobold-20261112` | DIVERGED (Core +127, source +14) | DIFF REVIEW |
| [#226 CORE-01E (draft): integrated Lu Core native Windows/Linux, installed and real GGUF acceptance](https://github.com/vjmichael/Lu-AI-GUI/pull/226) | `f4beaba3f6e77693f9048cd7e67764142d6c41d2` | `lu/p0-win-chat-204-20261111` | EXACT CORE01E | DO NOT REPLAY |
| [#224 P0-WIN-CHAT #204 (draft): measured Windows clock and authorized normal-chat browser routing](https://github.com/vjmichael/Lu-AI-GUI/pull/224) | `9b14dde7a01ad36208642be7536159ccb1f42e93` | `lu/p0-gate-consolidated-20261111` | INHERITED (Core +145) | DO NOT REPLAY |
| [#223 P0-GATE (draft): exact-head compiler diagnostics, native and installed acceptance](https://github.com/vjmichael/Lu-AI-GUI/pull/223) | `e422d18cef10fdb1092578631b5fec692b3d7a4f` | `lu/p0-codebase-reconcile-20261111` | INHERITED (Core +149) | DO NOT REPLAY |
| [#219 PLAN-07A/B/C/D (DRAFT): source audit, genuine GGUF suggestions and installed acceptance gates](https://github.com/vjmichael/Lu-AI-GUI/pull/219) | `fb336d3868a7dccb751ca93027956219e8d8147e` | `lu/plan06-native-task-list-20261110` | INHERITED (Core +167) | DO NOT REPLAY |
| [#216 PLAN-06 (DRAFT): native project task list, evidence review, ordering and export](https://github.com/vjmichael/Lu-AI-GUI/pull/216) | `67a7c8938427aff4b03854c53a5137bebe858ef5` | `lu/plan05-durable-project-task-register-20261110` | INHERITED (Core +253) | DO NOT REPLAY |
| [#213 PLAN-05 (DRAFT): human-owned durable task register, revisions and evidence acceptance](https://github.com/vjmichael/Lu-AI-GUI/pull/213) | `223d35e65ad58f7aeaabac5c9d1f1d97d87c714f` | `lu/plan04-genuine-gguf-task-synthesis-20261110` | INHERITED (Core +295) | DO NOT REPLAY |
| [#211 G02 (draft): generic Lu Workspace-authorized shared inference gate](https://github.com/vjmichael/Lu-AI-GUI/pull/211) | `bf9aa9ec674d62ff04c02924ed2d63866b767022` | `lu/plan04-genuine-gguf-task-synthesis-20261110` | DIVERGED (Core +338, source +6) | DIFF REVIEW |
| [#210 PLAN-04 (DRAFT): genuine local GGUF evidence-backed project task proposals](https://github.com/vjmichael/Lu-AI-GUI/pull/210) | `ea8ea8ecde2bfbb9ccb5429ac0543d9a5f2a91dd` | `lu/plan03-evidence-status-classifier-20261110` | INHERITED (Core +337) | DO NOT REPLAY |
| [#209 PLAN-03 (DRAFT): conservative project status classifier with native evidence gates](https://github.com/vjmichael/Lu-AI-GUI/pull/209) | `8070b69ca98163c3d00843252d307c38b0157e0e` | `lu/plan02-project-evidence-graph-20261110` | INHERITED (Core +341) | DO NOT REPLAY |
| [#208 PLAN-02 (DRAFT): source-pinned project evidence graph, Git ancestry and ANSF snapshots](https://github.com/vjmichael/Lu-AI-GUI/pull/208) | `7e9fa26d20147e3a7bbc6ffa3c2fec8e7808ab83` | `lu/plan01-project-baseline-20261110` | INHERITED (Core +347) | DO NOT REPLAY |
| [#207 PLAN-01 (DRAFT): selected local project discovery & source-grounded requirement baseline](https://github.com/vjmichael/Lu-AI-GUI/pull/207) | `e18d92c62baeb5d5d49d6c1744fc25bbd68bfd50` | `lu/p0-installed-ux-agent-acceptance-20261110` | INHERITED (Core +351) | DO NOT REPLAY |
| [#205 P0 Lu installed desktop UX: chat-first settings, model library, contextual reactions and host clock ](https://github.com/vjmichael/Lu-AI-GUI/pull/205) | `378ee5273201a6fa451ff511c0051d76bd1ea70c` | `lu/r00-core-convergence-20261109` | INHERITED (Core +353) | DO NOT REPLAY |
| [#200 R00 Lu Core convergence: auditable source branch, full PR graph and reconciliation register (DRAFT)](https://github.com/vjmichael/Lu-AI-GUI/pull/200) | `e61d4bf998bccf51074b01c864df208c233eb2f8` | `main` | INHERITED (Core +413) | DO NOT REPLAY |
| [#198 MEMORY-LOOP-03E: crash-consistent reviewed memory publication and checkpoint recheck](https://github.com/vjmichael/Lu-AI-GUI/pull/198) | `74dbdfe1f8457631f1992efed2c79b37629715aa` | `lu/memory-loop03d-reviewed-journal-index-20261109` | INHERITED (Core +471) | DO NOT REPLAY |
| [#197 MEMORY-LOOP-03D: original-turn reviewed worker evidence index for normal Lu GGUF chat](https://github.com/vjmichael/Lu-AI-GUI/pull/197) | `68d0e242550c52968e23b9f508b4bee1cf347aa3` | `lu/memory-loop03c-reviewed-os-worker-20261109` | INHERITED (Core +495) | DO NOT REPLAY |
| [#196 MEMORY-LOOP-03C-G5: protected reviewed Git/FFprobe workers → ANSF evidence, source change, restart](https://github.com/vjmichael/Lu-AI-GUI/pull/196) | `76a7a5f88672dbe02cb6651c9cf5ceef1d6c57b8` | `lu/memory-loop03c-positive-git-witness-20261109` | INHERITED (Core +496) | DO NOT REPLAY |
| [#195 MEMORY-LOOP-03C-G4: native Git/ANSF positive evidence, corrections and real-Qwen paired corpus](https://github.com/vjmichael/Lu-AI-GUI/pull/195) | `9be77920d548264544956b5e961b96526d5da449` | `lu/memory-loop03c-native-fixture-benchmark-20261109` | INHERITED (Core +497) | DO NOT REPLAY |
| [#194 MEMORY-LOOP-03C: native ANSF host corpus and genuine-Qwen two-arm error-attribution gate](https://github.com/vjmichael/Lu-AI-GUI/pull/194) | `7f2b676f7a99d70b50dfbd98c6656b838f80a739` | `lu/r00-ci-task-group-gating-20261109` | INHERITED (Core +498) | DO NOT REPLAY |
| [#193 R00 CI task-group gating: one automatic source check, 67 explicitly dispatched acceptance workflows](https://github.com/vjmichael/Lu-AI-GUI/pull/193) | `f2d86b864f92a23c975b9b0d5c2d6702a8355b53` | `lu/memory-loop03b-procedural-belief-ccm-20261109` | INHERITED (Core +499) | DO NOT REPLAY |
| [#191 R00: paginated PR stack graph and project-wide gate matrix](https://github.com/vjmichael/Lu-AI-GUI/pull/191) | `7c434b27caeac72eece67c4b6b55ed786ff2b6c0` | `main` | DIVERGED (Core +2920, source +4) | DIFF REVIEW |
| [#188 MEMORY-LOOP-03B (DRAFT): source-verified procedures, belief revision and CCM in Lu's GGUF reasoning ](https://github.com/vjmichael/Lu-AI-GUI/pull/188) | `4c63e365122b52f8bac55c59964a400788f2642e` | `lu/memory-loop03-goal-recovery-preflight-20261109` | INHERITED (Core +500) | DO NOT REPLAY |
| [#187 MEMORY-LOOP-03A (DRAFT): host goal/clarification and original recovery preflight for real Lu GGUF tu](https://github.com/vjmichael/Lu-AI-GUI/pull/187) | `a1a8700b1d467fead5b4be8664656f33e37777d2` | `lu/memory-loop02c-operator-provenance-20261109` | INHERITED (Core +523) | DO NOT REPLAY |
| [#185 MEMORY-LOOP-02C (DRAFT): host-only original-source recovery provenance in Lu chat](https://github.com/vjmichael/Lu-AI-GUI/pull/185) | `072770de5de0df291ba8f1cb815948002f9702d3` | `lu/memory-loop02b-host-safe-decision-20261108` | INHERITED (Core +532) | DO NOT REPLAY |
| [#182 MEMORY-LOOP-02B (DRAFT): native host safe-recovery decisions independent of Qwen comprehension](https://github.com/vjmichael/Lu-AI-GUI/pull/182) | `13d1abe5d917f2909a308f90963d0766fd6b7db8` | `lu/memory-loop02-verified-feedback-20261108` | INHERITED (Core +545) | DO NOT REPLAY |
| [#178 MEMORY-LOOP-02 (DRAFT): independently verified outcome feedback for next Lu GGUF turn, no authority ](https://github.com/vjmichael/Lu-AI-GUI/pull/178) | `08df965f00453e0cf383f78f6df14d2e1bcd6dbd` | `lu/memory-loop01-agent-handoff-20261108` | INHERITED (Core +547) | DO NOT REPLAY |
| [#177 MEMORY-LOOP-01 (DRAFT): ANSF verified agent-plan handoff to GGUF context, zero inherited authority](https://github.com/vjmichael/Lu-AI-GUI/pull/177) | `783e8953f669c93ef6e2c6257a1b2611d0f02b6e` | `lu/agent01l-linux-cgroup-v2-20261108` | INHERITED (Core +559) | DO NOT REPLAY |
| [#175 AGENT-01L Linux (DRAFT): crash-surviving cgroup-v2 custodian, escaped grandchild OS gate, Windows ph](https://github.com/vjmichael/Lu-AI-GUI/pull/175) | `b9da21178f6a59df21ea599f7fc336fc3915ba96` | `lu/agent01l-native-custody-20261108` | INHERITED (Core +562) | DO NOT REPLAY |
| [#173 AGENT-01L (DRAFT): Windows kernel Job custody witness and strict child admission; Linux fail-closed](https://github.com/vjmichael/Lu-AI-GUI/pull/173) | `42017b8732356a160a62404f84d64ce86d2ecd52` | `lu/agent01k-recovery-decision-20261108` | INHERITED (Core +590) | DO NOT REPLAY |
| [#172 AGENT-01K (DRAFT): independently verified recovery decisions, ancestor oracle and OS custody boundar](https://github.com/vjmichael/Lu-AI-GUI/pull/172) | `6446e6ca34948ebd53d1c6b711b8b7e72a8513da` | `lu/agent01j-lineage-recovery-20261108` | INHERITED (Core +618) | DO NOT REPLAY |
| [#170 AGENT-01J (DRAFT): restart-safe child-plan lineage and conversation-visible reconciliation](https://github.com/vjmichael/Lu-AI-GUI/pull/170) | `b394c2e57938aa8a375170c8c2927c7507386ba3` | `lu/agent01i-fresh-continuation-20261108` | INHERITED (Core +642) | DO NOT REPLAY |
| [#168 AGENT-01I (DRAFT): fresh authorized child-plan execution and real two-Qwen native acceptance gate](https://github.com/vjmichael/Lu-AI-GUI/pull/168) | `31c12bfb7b4479ee8a4b32ac4b2d795d4d3ec8ed` | `lu/agent01h-next-step-preview-20261108` | INHERITED (Core +651) | DO NOT REPLAY |
| [#166 AGENT-01H — read-only next-step readiness and safe recovery preview](https://github.com/vjmichael/Lu-AI-GUI/pull/166) | `7f1bbbc10d483b4139296fc59361639daedd2307` | `lu/kiaf-execution-agent01f-20261108` | INHERITED (Core +676) | DO NOT REPLAY |
| [#159 K/I/A/F active execution roadmap + AGENT-01F original-goal draft admission (DRAFT)](https://github.com/vjmichael/Lu-AI-GUI/pull/159) | `7ac144c0d921f9d5143172f8f4d0a5cddf60dbcb` | `lu/ux-chat02-revisions-replies-provenance-20261108` | INHERITED (Core +678) | DO NOT REPLAY |
| [#158 UX-CHAT02 (DRAFT): immutable message edits, pinned human replies and version provenance](https://github.com/vjmichael/Lu-AI-GUI/pull/158) | `4e6ac21f915095e2d033cd297a1169c1a7a0240e` | `lu/ux-chat01-message-interactions-20261108` | INHERITED (Core +706) | DO NOT REPLAY |
| [#157 UX-CHAT01 (DRAFT): emoji reactions, copy/safe replay and per-chat background/bubble colors](https://github.com/vjmichael/Lu-AI-GUI/pull/157) | `da9288cf8dac92f22ae89d9c0a62f6e991eb2f1d` | `lu/agent01e-broker-progress-integration-20261108` | INHERITED (Core +733) | DO NOT REPLAY |
| [#156 AGENT-01E (DRAFT): reviewed broker claim binding and original-chat approval/recovery UI](https://github.com/vjmichael/Lu-AI-GUI/pull/156) | `854a18ab4140dc4889fa7484d9f9bd13145edd0c` | `lu/agent01d-fenced-host-scheduler-20261108` | INHERITED (Core +753) | DO NOT REPLAY |
| [#155 AGENT-01D (DRAFT): bounded host-fenced step scheduler, fresh human consent and fail-closed recovery](https://github.com/vjmichael/Lu-AI-GUI/pull/155) | `e9f8aadfc0d1e814fa75f279977cf8cead7141b5` | `lu/agent01c-verified-deliverable-receipts-20261108` | INHERITED (Core +801) | DO NOT REPLAY |
| [#154 AGENT-01C (DRAFT): immutable independently checked deliverable and execution receipts](https://github.com/vjmichael/Lu-AI-GUI/pull/154) | `191695389b47f47ff8912e5e9cc0c6b8cb926829` | `lu/agent01b-immutable-goal-plan-revisions-20261108` | INHERITED (Core +823) | DO NOT REPLAY |
| [#153 AGENT-01B (DRAFT): immutable goal, assumption and deliverable plan revisions](https://github.com/vjmichael/Lu-AI-GUI/pull/153) | `bf69c9328e572225773fe2452871d602b575ce5f` | `lu/agent01a-durable-plan-ledger-20261108` | INHERITED (Core +835) | DO NOT REPLAY |
| [#152 AGENT-01A (DRAFT): durable human-scoped, non-executing multi-step plan ledger](https://github.com/vjmichael/Lu-AI-GUI/pull/152) | `58ccfe2d9c17041749f4f49a442eedfa50e03216` | `lu/e1-p14-human-readable-effect-feedback-20261108` | INHERITED (Core +854) | DO NOT REPLAY |
| [#151 E1-P15 (DRAFT): durable original-turn task recovery and read-only continuation evidence](https://github.com/vjmichael/Lu-AI-GUI/pull/151) | `2e4f15a56e036118785c47a25e95287fa55f3923` | `lu/e1-p14-human-readable-effect-feedback-20261108` | INHERITED (Core +844) | DO NOT REPLAY |
| [#148 E1-P14: explain verified, uncertain and rejected native tool outcomes to humans](https://github.com/vjmichael/Lu-AI-GUI/pull/148) | `07abadabf8ed806b4b26caafd173fb7761140b98` | `lu/e1-p13-revision-custody-20261108` | INHERITED (Core +863) | DO NOT REPLAY |
| [#144 E1-P13: pin approved source revisions across reviewed Git and FFprobe worker execution](https://github.com/vjmichael/Lu-AI-GUI/pull/144) | `0a9c4cff81ac1bb52779c0ac75eaac874de7965d` | `lu/e1-p12-git-status-pack-mp4-20261108` | INHERITED (Core +873) | DO NOT REPLAY |
| [#142 E1-P12: independently verify Git tracked status, packed history and MP4 metadata](https://github.com/vjmichael/Lu-AI-GUI/pull/142) | `de330493a434e7e64175b3d489ac152c3c50c27e` | `lu/e1-p11-independent-git-history-20261108` | INHERITED (Core +904) | DO NOT REPLAY |
| [#139 E1-P11: independently verify Git reachable commit count through native loose-object DAG traversal](https://github.com/vjmichael/Lu-AI-GUI/pull/139) | `5a869a77c2ee9c5683db35bb67efbd15bb22043d` | `lu/e1-p10-independent-operation-oracles-20261108` | INHERITED (Core +948) | DO NOT REPLAY |
| [#136 E1-P10: independently verify reviewed Git/WAV outcomes before model tool feedback](https://github.com/vjmichael/Lu-AI-GUI/pull/136) | `4ded8d6ddb418d4d07e874bc64f1d6defeac15e9` | `lu/e1-p09-reviewed-app-evidence-panel-20261108` | INHERITED (Core +975) | DO NOT REPLAY |
| [#135 E1-P09: show authenticated reviewed-app journal evidence in Lu Tool activity](https://github.com/vjmichael/Lu-AI-GUI/pull/135) | `04221078523d7d4f593684659ffd1f66367fb419` | `lu/e1-p08-reviewed-turn-recovery-20261107` | INHERITED (Core +1005) | DO NOT REPLAY |
| [#133 E1-P08C: delegated Linux cgroup-v2 custody and native SIGKILL/setsid proof](https://github.com/vjmichael/Lu-AI-GUI/pull/133) | `ec88d4dba411b9c84091d7c77bd50eb52701858e` | `lu/e1-p08-reviewed-turn-recovery-20261107` | DIVERGED (Core +1035, source +29) | DIFF REVIEW |
| [#130 E1-P08: recover reviewed application task/effect journals after Lu restart](https://github.com/vjmichael/Lu-AI-GUI/pull/130) | `c5ae97fbb9720ef0140c696df3cf58bf7b9a47d0` | `lu/e1-p07-reviewed-app-model-tool-20261107` | INHERITED (Core +1035) | DO NOT REPLAY |
| [#127 E1-P07: expose host-reviewed Git and FFprobe tools to live Lu model conversations and desktop](https://github.com/vjmichael/Lu-AI-GUI/pull/127) | `faa1cb9f3bc809d037843b794ef704a51b9c1f35` | `lu/e1-p06e-ffprobe-typed-media-20261107` | INHERITED (Core +1096) | DO NOT REPLAY |
| [#122 E1-P06E: genuine FFprobe media facts via native reviewed application adapter](https://github.com/vjmichael/Lu-AI-GUI/pull/122) | `76fc4a90a1809bc69351dd8bc283099bea6a3485` | `lu/e1-native-evidence-integrity-20261107` | INHERITED (Core +1131) | DO NOT REPLAY |
| [#120 E1 acceptance: pin native CI to exact PR head, require real tests, repair P05 p95 and raw receipts](https://github.com/vjmichael/Lu-AI-GUI/pull/120) | `a6b48c93b06b13fa4749e22b8c21b6614b85edb0` | `lu/e1-p06d-typed-git-context-20261107` | DIVERGED (Core +1138, source +4) | DIFF REVIEW |
| [#118 E1-P06D: actual Git branch and history context through reviewed native app adapter](https://github.com/vjmichael/Lu-AI-GUI/pull/118) | `1e86fbbaf7c2af0beeaf6acdc2c4016c3403e886` | `lu/e1-p06c-typed-git-ops-20261107` | DIVERGED (Core +1139, source +4) | DIFF REVIEW |
| [#117 E1-P06C: typed external Git head/status operations; preserve unsandboxed Lu native host](https://github.com/vjmichael/Lu-AI-GUI/pull/117) | `7f28bb6fc0ff128f5d910b4885466e033bcc0210` | `lu/e1-p06b-genuine-thirdparty-supervision-20261107` | DIVERGED (Core +1140, source +4) | DIFF REVIEW |
| [#116 E1-P06B: real reviewed Git/Python process execution, bounded stdout and journal recovery](https://github.com/vjmichael/Lu-AI-GUI/pull/116) | `0ffa5c1ce6fc4c44ce8afac41fbad23b6024ad69` | `lu/e1-p06-typed-thirdparty-adapters-20261107` | DIVERGED (Core +1142, source +4) | DIFF REVIEW |
| [#115 E1-P06A: host-reviewed typed third-party version adapter admission](https://github.com/vjmichael/Lu-AI-GUI/pull/115) | `7910351c6c4b06b85dd407d59b53d231652a0cc6` | `lu/e1-p05c-native-docx-parity-20261107` | DIVERGED (Core +1143, source +4) | DIFF REVIEW |
| [#113 E1-P05C: fix unfair DOCX native baseline and require two-format artifact parity](https://github.com/vjmichael/Lu-AI-GUI/pull/113) | `a94622ea388b43791bd82ccf9ce28eb30ac0291c` | `lu/e1-p05b-independent-semantic-witness-20261107` | DIVERGED (Core +1144, source +4) | DIFF REVIEW |
| [#112 CI-QUEUE-01: safe superseded-run cleanup and selective PR workflow fanout](https://github.com/vjmichael/Lu-AI-GUI/pull/112) | `ab19729eadd795ea78863ad4601cd38d386dc988` | `lu/e1-p05b-independent-semantic-witness-20261107` | DIVERGED (Core +1145, source +1) | DIFF REVIEW |
| [#110 E1-P05B: independent OOXML semantic benchmark witness and hostile fixtures](https://github.com/vjmichael/Lu-AI-GUI/pull/110) | `3bf173ae56c02db33e5f7d289991423380b57c5d` | `lu/e1-p05-three-arm-benchmark-20261107` | INHERITED (Core +1145) | DO NOT REPLAY |
| [#109 E1-P05: Genuine Qwen, native utility and reviewed app three-arm benchmark](https://github.com/vjmichael/Lu-AI-GUI/pull/109) | `7b86e3f514fb005f92ec2f55ed59a5c1f6489a5b` | `lu/e1-p04-artifact-custody-20261107` | DIVERGED (Core +1147, source +4) | DIFF REVIEW |
| [#107 E1-P04B: Native worker-to-Office artifact custody and semantic publication](https://github.com/vjmichael/Lu-AI-GUI/pull/107) | `2468bd5000e65c6c7f4d1452ba771581ca0c443f` | `lu/e1-p04-independent-outcome-oracles-20261107` | DIVERGED (Core +1148, source +4) | DIFF REVIEW |
| [#106 E1-P04: Independent typed output oracle and durable semantic receipts](https://github.com/vjmichael/Lu-AI-GUI/pull/106) | `08c1584fec025f79529bc70ce60b0f03c4210036` | `lu/e1-p03-contained-app-worker-20261107` | DIVERGED (Core +1149, source +2) | DIFF REVIEW |
| [#105 E1-P03: Contained, durable execution of reviewed one-shot application worker](https://github.com/vjmichael/Lu-AI-GUI/pull/105) | `d4130340a403239049c0003bee5945d443a50ae3` | `lu/e1-reviewed-app-admission-20261107` | DIVERGED (Core +1150, source +2) | DIFF REVIEW |
| [#103 E1-P01: Host-reviewed app executable preflight with per-turn authority](https://github.com/vjmichael/Lu-AI-GUI/pull/103) | `df57e46328e4f4b54f12b91773530be83748426c` | `lu/m5-local-memory-e2e-20261107` | INHERITED (Core +1152) | DO NOT REPLAY |
| [#101 M5: Real local-memory repeated-task replay and GGUF acceptance](https://github.com/vjmichael/Lu-AI-GUI/pull/101) | `96688241abc751cf5a31dd6664a59bc8d6a303e3` | `lu/m4-verified-procedural-memory-20261107` | INHERITED (Core +1155) | DO NOT REPLAY |
| [#98 Lu M4-P01 — verified procedural memory, correction safety and native tests](https://github.com/vjmichael/Lu-AI-GUI/pull/98) | `dcf43f23b357bcd7da16f2313745af0848825895` | `lu/m3-cross-chat-recall-source-ids-20261107` | INHERITED (Core +1157) | DO NOT REPLAY |
| [#96 M3 — ANSF original history, hot scrolling, operational continuity and local semantic recall](https://github.com/vjmichael/Lu-AI-GUI/pull/96) | `4f786086cd520a78ceda8902f93a3fa02202f9f0` | `lu/m2-ansf-canonical-conversation-20261107` | INHERITED (Core +1167) | DO NOT REPLAY |
| [#94 M2 — Canonical ANSF SQLite conversation cutover, native goals and legacy migration](https://github.com/vjmichael/Lu-AI-GUI/pull/94) | `4cfc7ddb0344bff594b1decbfaa88d62e7c52441` | `lu/ansf-three-arm-real-model-baselines-20261107` | INHERITED (Core +1197) | DO NOT REPLAY |
| [#93 BENCH — Three-arm real GGUF baselines: none vs recent chat vs ANSF source IDs](https://github.com/vjmichael/Lu-AI-GUI/pull/93) | `b4c98aa795bc50f26b2e1f17c90545e6db4b97fd` | `lu/ansf-direct-source-ids-artifact-references-20261107` | INHERITED (Core +1265) | DO NOT REPLAY |
| [#92 ANSF — Direct source IDs and external-only multimodal artifact references](https://github.com/vjmichael/Lu-AI-GUI/pull/92) | `cc75bb45aba45d58f49c69744dc86d7518b0e4dc` | `lu/m2-ansf-goals-clarifications-20261107` | INHERITED (Core +1275) | DO NOT REPLAY |
| [#90 M2 — ANSF/SQLite goal memory and user clarification continuity](https://github.com/vjmichael/Lu-AI-GUI/pull/90) | `667eb98a302752b4a9617461ecb6bfdfdad376d1` | `lu/m1-memory-evidence-authority-audit-20261107` | INHERITED (Core +1294) | DO NOT REPLAY |
| [#89 M1 — Audit live memory storage, prompt wiring, authority and recovery boundaries](https://github.com/vjmichael/Lu-AI-GUI/pull/89) | `40463c6a8a2439e392b6c5edb8fb9cfb5c1a4400` | `lu/human-centered-memory-first-philosophy-20261107` | INHERITED (Core +1328) | DO NOT REPLAY |
| [#88 Docs — Human-centered Lu, clarification loop, and memory-first execution order](https://github.com/vjmichael/Lu-AI-GUI/pull/88) | `fddc7b9510ac56f80bdb1001040e31e5a3216c3a` | `lu/ai-execution-p01-p12-recommendations-20261107` | INHERITED (Core +1333) | DO NOT REPLAY |
| [#87 AI Execution K/I/A/F — P01–P12 scoped recommendations and verified artifact interface](https://github.com/vjmichael/Lu-AI-GUI/pull/87) | `1615f80bdeb505ef6d02560f87abed50ec51bb02` | `lu/ai-execution-philosophy-audit-20261107` | INHERITED (Core +1341) | DO NOT REPLAY |
| [#86 Docs — Canonical AI Execution Layer f(K,I,A,F) philosophy and tool audit](https://github.com/vjmichael/Lu-AI-GUI/pull/86) | `58ab52dc8e52e0ebad89392dae7d1e0db1c0b6a8` | `lu/p11-git-core-boundary-20261107` | INHERITED (Core +1371) | DO NOT REPLAY |
| [#85 P11 GIT-CORE-01 — bounded Git read/execute boundary and hostile-config regression](https://github.com/vjmichael/Lu-AI-GUI/pull/85) | `7f2476d82a05be745ae4a514449072c3d5f15971` | `lu/p10-image-boundary-20261107` | INHERITED (Core +1379) | DO NOT REPLAY |
| [#83 P10 IMAGE-BASICS-01 — AI-first tool policy and bounded image correctness](https://github.com/vjmichael/Lu-AI-GUI/pull/83) | `9ccb7e5ba893cd7c0845978775ae5ad3b3d7eaac` | `lu/p09-office-boundary-20261107` | INHERITED (Core +1388) | DO NOT REPLAY |
| [#82 P09 OFFICE-BOUNDARY-01 — bounded Office fidelity and safe read/edit disclosure](https://github.com/vjmichael/Lu-AI-GUI/pull/82) | `d8bc94b2592555ddb31239b7dac60da77c2f21b5` | `lu/p08-archive-split-20261107` | INHERITED (Core +1398) | DO NOT REPLAY |
| [#81 P08 ARCHIVE-SPLIT-01 — native multipart split, staged resume, verified reassembly](https://github.com/vjmichael/Lu-AI-GUI/pull/81) | `d5be5c38b5d1bb89023c66d9da823ace53526066` | `lu/p07-research-citation-styles-20261106` | INHERITED (Core +1405) | DO NOT REPLAY |
| [#77 Lu P07 RESEARCH-STYLE-01: evidence-corroborated scholarly citation profiles](https://github.com/vjmichael/Lu-AI-GUI/pull/77) | `e018b1fba93d69b44243e0d3ff7f1e7b847b0ed6` | `lu/p06-observe-privacy-20261106` | INHERITED (Core +1445) | DO NOT REPLAY |
| [#72 Lu P06 OBSERVE-PRIVACY-01: native provenance migration, bounded retention and reviewed forget](https://github.com/vjmichael/Lu-AI-GUI/pull/72) | `71a43284ca58237f3098bdde5570ece6567931e4` | `lu/p03-ai-ops-model-manager-20261106` | INHERITED (Core +1526) | DO NOT REPLAY |
| [#71 Lu P03 AI-OPS-01: scoped tool ranking, verified model inspection and provider metadata guard](https://github.com/vjmichael/Lu-AI-GUI/pull/71) | `f54fd82cccd5803b2912efe7d8be513b19b9b559` | `lu/reader-ocr-image-closeout-20261106` | INHERITED (Core +1645) | DO NOT REPLAY |
| [#67 Lu tools: P01 indexed discovery + P02 guarded task scheduler/host recovery closeout](https://github.com/vjmichael/Lu-AI-GUI/pull/67) | `b69e19658b7cb1761180cdeb94aac7d9e7c3dc9d` | `lu/tool-completion-preview-20261106` | INHERITED (Core +1698) | DO NOT REPLAY |
| [#66 IMAGE-WS-PREVIEW-01: native volatile staged image preview (source only)](https://github.com/vjmichael/Lu-AI-GUI/pull/66) | `6bf64976e992a69fbbcfbd3d1e22e23f60c7e375` | `lu/tool-completion-waves-20261106` | INHERITED (Core +1751) | DO NOT REPLAY |
| [#61 Lu v1.0 Observe & Learn + Wave 4 tool integration (draft; native gates pending)](https://github.com/vjmichael/Lu-AI-GUI/pull/61) | `9b6a886caddaa221e13933446fe0100afe77cf90` | `lu/wave4-contextual-tools-20261106` | DIVERGED (Core +1835, source +15) | DIFF REVIEW |
| [#59 Wave 4 contextual tools: Git, Background, Environment, Vision, Image Workspace, Browser/Research (dr](https://github.com/vjmichael/Lu-AI-GUI/pull/59) | `575149d35d7b287955dcb601695c537ad0142471` | `lu/wave3-conversation-tools-20261105` | INHERITED (Core +2020) | DO NOT REPLAY |
| [#57 Wave 3: First normal-chat native tool turn (opt-in read-only workspace)](https://github.com/vjmichael/Lu-AI-GUI/pull/57) | `da04f89b5379b0f810f903a62f69f1b9ce5cb349` | `lu/wave2-file-archive-operations-20261105` | INHERITED (Core +2027) | DO NOT REPLAY |
| [#44 WAVE2-01: format-aware CSV edit and exact-approved ZIP extraction](https://github.com/vjmichael/Lu-AI-GUI/pull/44) | `d46948d68ef6fdef5827cad5799c82ff36ee1397` | `lu/tools-linux-archives-20261105` | DIVERGED (Core +2044, source +6) | DIFF REVIEW |
| [#42 TOOLS-LINUX-01: consolidate TAR/gzip/xz/bzip2/zstd source and native acceptance](https://github.com/vjmichael/Lu-AI-GUI/pull/42) | `7a8d03f3e579559b6f42ae62e77c8d4eef06d4a6` | `lu/tools-iso9660-inspect-extract-20261105` | INHERITED (Core +2273) | DO NOT REPLAY |
| [#41 TOOLS-ISO-01: native ISO9660 inspect and exact-approved single-file extraction](https://github.com/vjmichael/Lu-AI-GUI/pull/41) | `18d07296e5dc416e8b1ba98fdd6241778c8967ff` | `lu/tools-all-source-consolidation-20261105` | INHERITED (Core +2289) | DO NOT REPLAY |
| [#40 TOOLS-CONSOLIDATE-01: unified native filesystem, ZIP and spreadsheet implementation source](https://github.com/vjmichael/Lu-AI-GUI/pull/40) | `d3f79a549c480bbb30fb427888f20cde1e1c1ed7` | `lu/tools-spreadsheet-create-20261105` | INHERITED (Core +2309) | DO NOT REPLAY |
| [#39 TOOLS-WRITE-SPREADSHEET-01: native structured XLSX/CSV creation engine](https://github.com/vjmichael/Lu-AI-GUI/pull/39) | `351326af6308645ca3912f0df82eff7e8772214b` | `lu/tools-read-workspace-turn-20261105` | INHERITED (Core +2331) | DO NOT REPLAY |
| [#38 TOOLS-INTEGRATION-READ-01: authenticated multi-tool workspace model turn](https://github.com/vjmichael/Lu-AI-GUI/pull/38) | `5dc2d68b7b7b5ca9b507b85c5b2257b632edaba1` | `lu/tools-01-05-unified-integration-20261105` | INHERITED (Core +2341) | DO NOT REPLAY |
| [#37 TOOLS-04F: bounded host-owned ZIP creation with native regression tests](https://github.com/vjmichael/Lu-AI-GUI/pull/37) | `2ed6cbe35ed9c5964c1116c4919c9d3af2d580b9` | `lu/tools-01-05-unified-integration-20261105` | INHERITED (Core +2344) | DO NOT REPLAY |
| [#36 TOOLS-01F: host-owned no-clobber file copy/move and native regressions](https://github.com/vjmichael/Lu-AI-GUI/pull/36) | `e8971cc01bd1ccb5a2d144a5889404c77b2881d1` | `lu/tools-01-05-unified-integration-20261105` | INHERITED (Core +2343) | DO NOT REPLAY |
| [#35 LU-IMAGE-WS-01: shared human/AI image workspace contract and edit intent guard](https://github.com/vjmichael/Lu-AI-GUI/pull/35) | `d26b7175012bebde97f7fb715e918b5386e08f84` | `main` | DIVERGED (Core +2923, source +3) | DIFF REVIEW |
| [#34 LU-TOOLS-01–05: unify filesystem, ZIP archive, controlled execution, background and Reader in deskto](https://github.com/vjmichael/Lu-AI-GUI/pull/34) | `938c620a6b8b6469a47ddb78899d3e422533dd3b` | `main` | INHERITED (Core +2352) | DO NOT REPLAY |
| [#29 LU-AIW-AUDIT-01: foundational AI-usability audit across Lu Core, desktop and draft PRs](https://github.com/vjmichael/Lu-AI-GUI/pull/29) | `7d1f965d10b407851fa5ddec2f059b806a9d10df` | `main` | DIVERGED (Core +2943, source +4) | DIFF REVIEW |
| [#28 LU-OPMEM-01A: durable operational resource memory and AI repo orientation](https://github.com/vjmichael/Lu-AI-GUI/pull/28) | `c67d5f7abdaa1427102d0c9ad8ae98f19cf0bc5a` | `main` | DIVERGED (Core +2943, source +13) | DIFF REVIEW |
| [#27 TOOLS-11 + universal Reader: DOCX, XLSX/ODS, PPTX/ODP, basic PDF, paged selections](https://github.com/vjmichael/Lu-AI-GUI/pull/27) | `5fef956d6a81fb2c9843e116465847fa1251f690` | `main` | DIVERGED (Core +2944, source +202) | DIFF REVIEW |
| [#26 TOOLS-10A–E source implementation: verifiable research, browser worker and durable recovery](https://github.com/vjmichael/Lu-AI-GUI/pull/26) | `19af8fe3e411e4a4175e285ce1c6c2130c36fe4d` | `lu/tools-03-git-inspection-20261104` | DIVERGED (Core +2378, source +303) | DIFF REVIEW |
| [#24 TOOLS-09: extend existing scoped search with bounded recursion and suffix filters](https://github.com/vjmichael/Lu-AI-GUI/pull/24) | `3ceb00c7c60f29a56a9af4a709fb90892fb6ce3d` | `main` | INHERITED (Core +2370) | DO NOT REPLAY |
| [#22 CI-only replay: TOOLS-04 archive exact-head Windows/Linux acceptance](https://github.com/vjmichael/Lu-AI-GUI/pull/22) | `cb04fe12b75fd75334049cd78f41bfbb135032cf` | `lu/tools-03-git-inspection-20261104` | DIVERGED (Core +2388, source +27) | DIFF REVIEW |
| [#21 Lu ANSF: host-authorized Cognitum C06 retrieval via existing Rust FTS5](https://github.com/vjmichael/Lu-AI-GUI/pull/21) | `5afe669aa9a43dfe30cba11c583f1649f0a39ae3` | `main` | DIVERGED (Core +2944, source +5) | DIFF REVIEW |
| [#20 BACKEND-CTRL-01: llama.cpp finish semantics and llama/vLLM control design](https://github.com/vjmichael/Lu-AI-GUI/pull/20) | `ed24da5d00047f846989d9fc54fd0c4939e980bd` | `lu/tools-08-loop-guard-20261104` | DIVERGED (Core +2411, source +92) | DIFF REVIEW |
| [#19 TOOLS-08: verified progress guard and streaming token-loop stop (no 8-call ceiling)](https://github.com/vjmichael/Lu-AI-GUI/pull/19) | `62992c9ac20bed96e815ba732c94ebf186899a1e` | `lu/tools-02-json-validation-20261103` | DIVERGED (Core +2411, source +38) | DIFF REVIEW |
| [#18 TOOLS-07: original vision-first image read with internal Tesseract OCR fallback](https://github.com/vjmichael/Lu-AI-GUI/pull/18) | `ad7bf9f483713de1e94d51205041a9cd500ba606` | `lu/tools-02-json-validation-20261103` | DIVERGED (Core +2411, source +20) | DIFF REVIEW |
| [#17 TOOLS-06: Jan §6A ambient environment discovery and scoped native check_env](https://github.com/vjmichael/Lu-AI-GUI/pull/17) | `a3dca4b968cf72a4e699da3311d47035de5145d2` | `lu/tools-02-json-validation-20261103` | DIVERGED (Core +2411, source +12) | DIFF REVIEW |
| [#16 TOOLS-05: native offline JSON Schema validation (original Jan validate domain)](https://github.com/vjmichael/Lu-AI-GUI/pull/16) | `7a77bbdb8aed3bf46f4ed68f87e940761c240d67` | `lu/tools-02-json-validation-20261103` | DIVERGED (Core +2411, source +16) | DIFF REVIEW |
| [#15 TOOLS-04: original-first native ZIP manifest inspection (read-only)](https://github.com/vjmichael/Lu-AI-GUI/pull/15) | `6e17793c52fd81dbaf8fbbbcd2d2f50cfe6e6ff5` | `lu/tools-03-git-inspection-20261104` | DIVERGED (Core +2380, source +51) | DIFF REVIEW |
| [#14 TOOLS-03: basic read-only Git Core + original tool design; advanced gh reserved for IDE](https://github.com/vjmichael/Lu-AI-GUI/pull/14) | `b2c5e10ba7a3fbad095d9c86194bdf87ae298433` | `lu/tools-02-json-validation-20261103` | INHERITED (Core +2378) | DO NOT REPLAY |
| [#12 TOOLS-02: full original bounded MBRL JSON guard, recovery and desktop boundary checks](https://github.com/vjmichael/Lu-AI-GUI/pull/12) | `383a30fae49092ebaaa150118e24885c7681a692` | `lu/write-01e-approved-edit-gate-20261103` | INHERITED (Core +2411) | DO NOT REPLAY |
| [#11 DLC-API-00: generic first-party Lu Host Services RFC and typed bidirectional Protobuf contract](https://github.com/vjmichael/Lu-AI-GUI/pull/11) | `5eca23b61f41a34b813d854577078b4fbe4d41c6` | `main` | DIVERGED (Core +2944, source +6) | DIFF REVIEW |
| [#10 DLC-IPC-01: common Windows/Linux Workspace inference contract (source-only)](https://github.com/vjmichael/Lu-AI-GUI/pull/10) | `9cc0904904870c3860057f8130dda3872c0403d9` | `lu/write-01e-approved-edit-gate-20261103` | DIVERGED (Core +2485, source +19) | DIFF REVIEW |
| [#6 Lu/NVP source-only inference interop contract and Windows containment source blocker](https://github.com/vjmichael/Lu-AI-GUI/pull/6) | `80c1a5aa33dac5839985f7e297ea860d6d3fa922` | `main` | DIVERGED (Core +2944, source +6) | DIFF REVIEW |
| [#5 WRITE-01E: journaled existing-file edit, recovery and explicit rollback](https://github.com/vjmichael/Lu-AI-GUI/pull/5) | `2a9a135500b6e142564b7dec753f52f5184fd9bb` | `lu/win-load-01-host-trace-20261103` | INHERITED (Core +2485) | DO NOT REPLAY |
| [#4 WIN-LOAD-01: host-side bounded model-load lifecycle diagnostics](https://github.com/vjmichael/Lu-AI-GUI/pull/4) | `e794d9bb8dc2dc7180e6003753343c3697d8f3ab` | `main` | INHERITED (Core +2668) | DO NOT REPLAY |
| [#3 REPO-01: fail-closed source identity verifier and unit CI](https://github.com/vjmichael/Lu-AI-GUI/pull/3) | `45f4db62d1f47c1d070fddd6565612d61c6d2e56` | `main` | DIVERGED (Core +2948, source +3) | DIFF REVIEW |

## Live Core branches without open PRs (37)

Do not delete historical branches until their unique code and receipts are reconciled.

| Branch | Exact SHA | Relative to CORE-01E | Disposition |
| --- | --- | --- | --- |
| `lu/e1-p08c-parent-reconciled-20261108` | `b11a077bbdc02b07f5b427c9da5425d25d5b4e49` | DIVERGED (Core +1067, source +10) | DIFF REVIEW |
| `lu/e1-p10-rebased-on-p09-ci-fix-20261108` | `44a774772689cd66870325fdda49d1200533f6b4` | INHERITED (Core +1039) | DO NOT REPLAY |
| `lu/e1-p11-rebased-on-p09fix-20261108` | `e6e62c21ae72ce4a4d482e956326bdf6a30d7159` | INHERITED (Core +1029) | DO NOT REPLAY |
| `lu/p0-auth-unified-custody-20261111` | `afcb7ae235de8ae84d3fdc66c127bac6984fa33e` | INHERITED (Core +157) | DO NOT REPLAY |
| `lu/p0-codebase-reconcile-20261111` | `cc312e281b5989a0a9fb972eb04b9e34215a77da` | INHERITED (Core +156) | DO NOT REPLAY |
| `lu/p0-memory-original-continuity-20261111` | `c5f644bc96974e6d8379fc3c6b3154c970960798` | INHERITED (Core +158) | DO NOT REPLAY |
| `lu/p0-next-project-agent-handoff-20261111` | `67f135b8f334eb0b11e8a4358d2a250331bd7184` | INHERITED (Core +164) | DO NOT REPLAY |
| `lu/p0-sec-guardian-hardening-20261111` | `50a6f68ba322b3fed5959af2af49f75388c2905c` | DIVERGED (Core +145, source +100) | DIFF REVIEW |
| `lu/p0-tools01-multidoc-workspace-20261111` | `d18c46637e2de9db88ef1d7cb2324a919628fe66` | INHERITED (Core +163) | DO NOT REPLAY |
| `lu/p0-tools02-reviewed-file-effects-20261111` | `314e88e3a989eba02f9ec2620fbc607a026cb293` | INHERITED (Core +162) | DO NOT REPLAY |
| `lu/p0-tools03-ansf-source-evidence-20261111` | `2de2845eb4fac4382642b864971118379c56387c` | INHERITED (Core +161) | DO NOT REPLAY |
| `lu/p0-tools04-specialist-adapters-20261111` | `d7d769d4b0d5b7fa73fc787937109ab673c92e3b` | INHERITED (Core +160) | DO NOT REPLAY |
| `lu/p0-tools05-task-witness-review-20261111` | `ca6c3bc07c716cb8af43d28e48229f9b47b0de3a` | INHERITED (Core +159) | DO NOT REPLAY |
| `lu/perf01-p0-p0d-closeout-20261112` | `7072e14047bf84b46874d21d34d48b71d7586f9d` | DIVERGED (Core +145, source +176) | DIFF REVIEW |
| `lu/perf01-p0-p0d-cpu-first-20261112` | `ae0acfd8f6ed0e07dee949d897197def616ec87c` | DIVERGED (Core +145, source +149) | DIFF REVIEW |
| `lu/perf01-p1-warm-model-20261112` | `af907c0156c181bf71e2005b0fdaa86774112237` | DIVERGED (Core +145, source +181) | DIFF REVIEW |
| `lu/perf01-p2-inference-latency-20261112` | `faa367a73636a45e51ab911935482815a221ce27` | DIVERGED (Core +145, source +192) | DIFF REVIEW |
| `lu/perf01-p2-kv-cache-20261112` | `4cf7f59184aff62696b5e1f8c0441477acbce472` | DIVERGED (Core +145, source +208) | DIFF REVIEW |
| `lu/perf01-p3-direct-streaming-20261112` | `4f95d9d19bd06db33741ba57389c75604de376f6` | DIVERGED (Core +145, source +232) | DIFF REVIEW |
| `lu/perf01-p4-interactive-priority-20261112` | `ae3739d7485ee4205f75d1f85456c1ba35949d45` | DIVERGED (Core +145, source +247) | DIFF REVIEW |
| `lu/perf01-p5-inference-tuning-20261112` | `fbc530bc9eed614b3462bcaa2a4887fe7e889729` | DIVERGED (Core +145, source +267) | DIFF REVIEW |
| `lu/r17-chat-continuity-attachments-20261101` | `523189b9c77014236bdbf352ee8a560b7d483f49` | INHERITED (Core +2959) | DO NOT REPLAY |
| `lu/r18-cross-chat-queue-20261101` | `bd431cc25b1523c815096d64e47b3485838c44f8` | INHERITED (Core +2958) | DO NOT REPLAY |
| `lu/r19-reference-first-desktop-ux-20261101` | `1a78387ab0ab69f8fc95f1e9ce009404e1114b94` | INHERITED (Core +2956) | DO NOT REPLAY |
| `lu/r20-markdown-attachment-ux-20261101` | `4dc6227cc122f8789cf5180e7d3c10d348e62173` | INHERITED (Core +2955) | DO NOT REPLAY |
| `lu/r21-image-admission-dnd-20261101` | `7f1f8ae9ea6aeddce3c5b3f2a4627f0601e14925` | INHERITED (Core +2954) | DO NOT REPLAY |
| `lu/r22-typed-chat-mmproj-diagnostics-20261101` | `0d34d70595f93979c8d4370627be217cda5f8041` | INHERITED (Core +2952) | DO NOT REPLAY |
| `lu/r22-typed-chat-vision-boundary-20261101` | `94d3da920c60552688c722a0815e86754a744de1` | DIVERGED (Core +2954, source +1) | DIFF REVIEW |
| `lu/r23-per-model-inference-profiles-20261102` | `b3360ac2511375981e1b846c9f685dd06a0e1c31` | DIVERGED (Core +2951, source +2) | DIFF REVIEW |
| `lu/reader-nested-page-tree-20261105` | `9b4269badd49a66a5229ee6b2501c96150faa002` | DIVERGED (Core +2944, source +197) | DIFF REVIEW |
| `lu/tool-completion-waves-20261106` | `1759a872c53019ee5fc8edaf913aaddfdf128583` | INHERITED (Core +1755) | DO NOT REPLAY |
| `lu/tools09-bounded-workspace-search-20261112` | `d27c31ad5592999fdbb0f7056009988cedcdc836` | DIVERGED (Core +145, source +120) | DIFF REVIEW |
| `lu/tools-04-desktop-lock-repair-20261105` | `7efa41870daf1bb191e30f9196a2bd4a8f99aeb7` | DIVERGED (Core +2380, source +50) | DIFF REVIEW |
| `lu/tools-08-durable-progress-design-20261104` | `55f03b367e627199f4a4ea8307d2fb6a333bc404` | DIVERGED (Core +2411, source +39) | DIFF REVIEW |
| `lu/tools-09-search-budget-regression-20261105` | `69360585f07d326c300260290cd1d2f05143e5fe` | INHERITED (Core +2371) | DO NOT REPLAY |
| `lu/tools-11-model-observation-20261105` | `d16d681890e309531bc508a260cf4b76b760cf24` | DIVERGED (Core +2944, source +201) | DIFF REVIEW |
| `lu/v1-01-windows-icon-import-20261103` | `49e4435b91342b2c0d5d50844d64ce184ec97d21` | DIVERGED (Core +2948, source +22) | DIFF REVIEW |

## Excluded or deferred

- `lu/perf01-p6-multigpu-nodes-20261112` at `65854e431fc98e63bc4e9fa3b2c20ff3fe578ae2` — P6.
- `lu/perf01-p7-backend-evaluation-20261112` at `41c05afff001de9a737149d0a51ee5ed1f6c72c9` — P7L.
- `lu/perf01-p7l1-native-repair-20261112` at `be0f431a74f11fdb7c41cc0aa3e16ff5bec44c62` — P7L.
- `lu/perf01-p7l-koboldcpp-supervised-20261112` at `f1c1ddea3704267a6350f8b32a90e78a64b69a73` — P7L.
- IDE: 123 branches and 94 open PRs tracked separately. No IDE source merge through Lu Core.

## Closed historical repair PRs

- [#33 TOOLS-04 closeout: reconcile desktop Cargo.lock for ZIP native dependency](https://github.com/vjmichael/Lu-AI-GUI/pull/33) — merged into development source.
- [#32 TOOLS-09 closeout: repair native search entry-budget regression and source lock](https://github.com/vjmichael/Lu-AI-GUI/pull/32) — merged into development source.
- [#31 Reader RDR-03: safely traverse bounded nested PDF page trees](https://github.com/vjmichael/Lu-AI-GUI/pull/31) — merged into development source.
- [#30 TOOLS-11: scoped task status/results plus host-bound one-shot start/cancel](https://github.com/vjmichael/Lu-AI-GUI/pull/30) — closed without GitHub merge.

## Consolidation execution queue

1. C0: preserve current main ledger and one source-locked Core convergence head; do not grant code merge/release from ancestry.
2. C1: inspect 53 divergent branch heads (35 open PRs and 18 orphan branches); group unique functions by exact blobs and overlapping source contracts, not PR count.
3. C2: inspect newer PR #235 (only descendant) for selective integration after source/native review.
4. C3: integrate genuinely missing Core tools/Reader/browser, ANSF/PLAN/agent, native authority, model UX and performance into one reviewed source graph. Keep DLC and excluded runtimes separate.
5. C4: original SEC-06C/06F and CORE-01F-04 Windows/Linux/native model/receipt reconciliation at ONE unchanged source SHA; no security gate bypass.
6. C5: owner-reviewed final code merge to main only after required acceptance; preserve main ledger/workflow and reconcile overlap files.
7. C6: hosted NSIS/UIA, Debian/AT-SPI, real GGUF, physical Radeon/MMProj and browser packaging remain separate release acceptance.

**No source code merge, branch deletion, PR closure, installed test or release authorization is performed by this inventory.**
