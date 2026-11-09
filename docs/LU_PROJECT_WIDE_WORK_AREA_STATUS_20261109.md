# Lu project-wide work area and gate status — 2026-11-09

This is a conservative implementation inventory against the owner's 22-area register. It supplements the [canonical development ledger](LU_DEVELOPMENT_LEDGER.md) and its [machine-readable open PR graph](LU_OPEN_PR_GRAPH.json). An open PR is source evidence, not integration into `main`. The exact SHA and parent for every open PR are in the graph (175 at this refresh, including new IDE #190). The accepted `0.1.0-alpha.2` remains separate.

| Area | Present source / branch family | Remaining work and independent gate |
| --- | --- | --- |
| 00 Repository | `main` inventory script, 175 open PRs at refresh; #188 source lock 803 blobs after npm lock repair | Reconcile branch/lock drift and completed exact-head Windows/Linux jobs; source equivalence to original unpublished snapshot remains unknown. |
| 01 Model lifecycle | Core model manager and WIN-LOAD-01 draft #4 | Real small GGUF load/stop/restart and measured host/resource state; physical 7B+ deferred. |
| 02 ANSF | M2/M3/M4/M5 draft chain #90–101 and later memory stack | Native SQLite migration, original-source/revision isolation, paging, semantic quality and restart evidence. |
| 03 Memory loop | #177 → #178 → #182 → #185 → #187 → #188 | 03B native/GGUF ingress and 03C comprehension bench; 02C installed UI, source freshness and OS witness open. |
| 04 Agent task loop | #152–156, #159, #166, #168, #170, #172–175 | Genuine multi-step fresh approval, safe claim/restart and real small-model reviewed worker outcome. |
| 05 Authority / MBRL | #12, Wave 3 #57, agent/effect stacks | Native hostile boundary, revocation and every normal Send path through one host authority decision. |
| 06 OS worker custody | E1-P03/P08C #105/#133 and AGENT-01L #173/#175 | Real Windows Job and delegated Linux cgroup-v2 positive crash/escaped-child evidence; fail closed when unavailable. |
| 07 Reviewed apps/oracles | #103–151 E1 stack | Actual Git/FFprobe subprocess, independent source revision and method-specific receipt on exact head. |
| 08 Filesystem | #24/#34/#44/#57 and file-effect descendants | Native path/NTFS/race/crash tests and ordinary reviewed desktop flow. |
| 09 Archives | #15/#37/#41/#42/#44/#81 | Real format corpus, split/hostile extraction and transactional cancel/restart. |
| 10 Reader/Office | #27/#39/#44 and utility descendants | Real scanned/multiformat corpus, semantic oracle, page identity, large-file bounds. |
| 11 Browser/research | #26/#59/#61/#77 | Pinned runtime and actual scoped browser/DOI citation/continuation evidence. |
| 12 Observe/image | #61/#66/#72/#83 | Native consent/capture/privacy and genuine multimodal interpretation, not image admission alone. |
| 13 Background | #27/#67/#155/#156 | Native many-chat worker fairness, terminal ACK, crash/restart and UI event order. |
| 14 Chat UX | #157/#158 and later memory UI #185 | React/Tauri edit/reaction/reply/recovery and authenticated new-turn behavior. |
| 15 Measurements | #93/#109/#113 and proposed MEMORY-LOOP-03C | Same pinned small GGUF, independent outcomes and raw K/I/A/F attribution. |
| 16 Packaging | #61 and release/NSIS workflows | Exact artifact hash, SBOM/legal, disposable install/uninstall. Physical owner gate deferred. |
| 17 IDE project | Standalone IDE stack including #143/#145–147 | Verify B1–B11 source and large-project native/installed behavior on IDE ancestry only. |
| 18 IDE Jobs/LSP | #131–141 and #160–162 | Actual pinned LSP/build worker protocols, process custody, streaming and cancel. |
| 19 IDE debugger | #163–176 → #179–181 → #184 → #186 → #189 → #190 | Real protected LLDB stop/PE/PDB and independent LLDB DEBUG_EVENT ownership; no stepping/variables until verified. |
| 20 IDE proposals/package | Independent IDE branch/workflows | No-clobber proposal review and exact package/license/custody; live Core bridge deferred. |
| 21 DLC | #10/#11/#76 documentation/experiments | Design/protocol only; live Core↔IDE/NVP/Cognitum integration deferred. |

## Non-physical acceptance matrix

| Gate | Current evidence | Status / next independent check |
| --- | --- | --- |
| A01 exact source | #188 prior head `cbca41f79a1ede41d66a251196b834d3ed18eb62`: local REPO-01 802/802; repaired head must run anew | OPEN: new exact-head CI and source/branch convergence. |
| A02 native build | No completed #188/#189 exact-head Windows/Linux results observed at this audit | OPEN: Cargo locked tests/check, Node/React, IDE host. |
| A03 original ANSF | M2–M4 and 03B source candidates | OPEN: native migration/tamper/restart and retrieval quality. |
| A04 independent outcome | E1 and #188 method-matching source candidates | OPEN: real Git/FFprobe/oracle contradictory cases on current head. |
| A05 fresh authority | AGENT-01F/G and #188 deny historical grant in source | OPEN: actual OS/model/host denial matrix and restart. |
| A06 OS custody | Windows Job/Linux cgroup source and targeted workflows | OPEN: positive native kernel/forced-host-death witnesses. |
| A07 recovery | ANSF/task/effect/approval journal source candidates | OPEN: cross-journal partial-commit native restart and no replay. |
| A08 tool behavior | Many draft tools | OPEN: real hostile file/archive/Office/browser corpus and independent output. |
| A09 small GGUF | Qwen test workflow and earlier narrow local diagnostic | OPEN: same pinned model real selected tool/feedback benchmark; queued is not passed. |
| A10 UI | #188 local React bundle and 59 static UI checks after npm lock | OPEN: Tauri host interaction/restart/accessibility and controlled installed workflow. |
| A11 IDE native | #189 draft `83b02c4948b0626a1830209a30f9639568fa41da` | OPEN: Windows/Linux exact-head compiler/OS LLDB and real debug-event owner. |
| A12 delivery | CI package candidates | OPEN: exact binary/SBOM/hosted install-uninstall and hashes. |

D01 owner physical installed sign-off, D02 owner physical 7B+ GGUF/AMD/MMProj, and D03 broader provider/DLC remain explicitly deferred, not passed. No present PR is marked complete solely from this matrix. New source commits invalidate ancestor CI acceptance for changed code until new-head jobs pass.
