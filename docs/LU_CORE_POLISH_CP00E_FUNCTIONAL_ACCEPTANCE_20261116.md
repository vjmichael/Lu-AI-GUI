# CP-00E — Functional acceptance coverage and residual integration audit

**Audit frozen source** [93b63de983703bad2fb768e9d104623dcfd6c4d5](https://github.com/vjmichael/Lu-AI-GUI/commit/93b63de983703bad2fb768e9d104623dcfd6c4d5), executable baseline [25618e38](https://github.com/vjmichael/Lu-AI-GUI/commit/25618e383dda49f0cc85a92ef6f6f0b745c378ea). Core only; IDE/Kobolt/P7L excluded. Source/acceptance evidence reconciliation, not fresh execution. This is the final CP-00 audit checkpoint; repair phase may start after publishing. No runtime modification, new branch/PR, installer, heavy CI dispatch or fabricated acceptance.

## 1. Evidence taxonomy — what counts

A **source assertion** checks strings, syntax, registration, manifests or policy shape; a **native/component test** executes real code/fixtures (including genuine OS worker or file output) but can bypass production ingress; a **scripted-model** test supplies prewritten tool frames and cannot prove model choice; a **genuine-GGUF scoped harness** runs verified real inference, model-selected tool calls and actual worker/receipts with controlled resource/authority; an **installed OS test** proves actual packaged application behavior specified by its script, not a model or real human merely by installation; **production end-to-end actual-model and physical owner** requires ordinary Chat ingress, real model, actual native tools, human authority, independently observed outcome, model feedback, correct user answer, and matching source/package/environment identity. These dimensions may overlap, not substitute for each other. A robust security negative is still useful evidence, but not a positive task completion.

Source/test inventory: 5 paths under tests/, 118 under lu-desktop/tests/, 123 under src-tauri/lu-core/tests/ = **246 files** including fixtures, Rust harness manifests and documentation; 45 benchmark files, 78 workflows, 86 scripts. Inventory in appendix; filenames alone not test results. Targeted actual body review of representative positive/negative tests, workflow callsites, production source and historical run/job/step metadata. Not a line-by-line review of every test body, fresh compilation, blind execution or native logs for every older branch. Same exact code blobs provide useful continuity, but not current installed acceptance.

## 2. Verified historical CI and physical evidence

| Original run / exact SHA | Verified actual job/step results | Does NOT establish |
|---|---|---|
| [#43682150507](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/43682150507) a4521cecc6d784ed76cd1ad29c4b9ff4d91ef114 | Actual Windows/Linux native planner/security/recovery, root-owned Git, real pinned Qwen/Git/FFprobe PASS; real delegated-kernel job FAILED (original no delegation exit 78) and all-or-nothing FAILED. | Full Linux cgroup / physical / release. |
| [#43905518735](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/43905518735) b634653cc39b8f1192f77d55850d5b3a95aabfa4 | Native Windows/Linux, actual native Git/Python version and Reader/Browser/Observe source authority, Linux real scanned PDF Poppler/Tesseract, native Win32 Job/security and root-owned Git; **genuine-local-qwen** PASS; genuine installed Windows WebView2 task board scripted refusal/cancel/replan/restart/uninstall PASS. Consolidated all-or-nothing job **FAILURE**, not group-wide pass: qualified Linux kernel positive not run. | 27B/R9700/MMProj, every ordinary Send route, live human positive, full Linux kernel. |
| [#43921265557](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/43921265557) b634653cc39b8f1192f77d55850d5b3a95aabfa4 | REPO-01 frozen source PASS, no other non-skipped jobs. | Re-execution of native/model tests. |
| [#43955995665](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/43955995665) 279123e88b9b18f17a6d80ab25b07f2d3180f8c4 | Windows+Ubuntu C1 narrow native actual C06, TSV confidence, useful 25-read/24-edit progress and restart/cancellation, source queue checks PASS; actual desktop compile (non-executable engine resource placeholder). | Real installed OCR/MMProj, full model route, physical. |
| [#43998629830](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/43998629830) 48250a2e3280b3292e5ef1596c3d38c41a142961 | Genuine reviewed Windows native Chromium/Node package, 482-file allowlist, CPU/Vulkan build, real installed NSIS/UIA **old separate windows**, uninstall/AppData PASS. | Real model browser search or new tabs/physical. |
| [#44018707690](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/44018707690) 25618e383dda49f0cc85a92ef6f6f0b745c378ea | **Current executable baseline** actual compiled reviewed browser package, native Rust independently recomputed embedded manifest and every file; genuine hosted Windows install / browser bytes / **unified tabs** / uninstall / AppData preservation PASS. REPO-01 PASS. | New real local model invocation, attachment or native tool choice in installed chat; physical acceptance still needed. |
| [#44028605914](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/44028605914) 93b63de983703bad2fb768e9d104623dcfd6c4d5 | REPO-01 exact head assertion PASS, **tracked-source snapshot FAILED**; subsequent source checks skipped. Source lock was stale after prior documentation updates. | Any current-source acceptance. |
| C6.3 owner physical observation (ledger and owner's reports) | Actual Qwen3.8-27B text generation on R9700/Vulkan selected; MMProj HTTP 400/missing projector, Google search not routed, workspace InferenceFailed, DOCX rejection, missing capture worker, catalog failure and goal UI error. | Full physical acceptance or exact measured tensor residency. |

The stale source-lock defect is concrete: audited current tree contains 1,073 non-lock blobs vs 1,069 in committed manifest; four added audit docs and changed canonical ledger, no removed blobs. This checkpoint regenerates complete lock from actual Git tree including the new CP-00E document and ledger; verifier script unchanged. Publish a source commit, then lock-only descendant as one non-force ref update. Verify final manifest against final tree, excluding only itself. Hosted REPO-01 automatic source check, if subsequently run, remains separate, not preclaimed. No need to re-run earlier heavy jobs merely for audit documentation.

## 3. Representative test body and actual evidence distinction

- **Genuine model + real tools:** [e1_reviewed_model_tools.rs:848](https://github.com/vjmichael/Lu-AI-GUI/blob/93b63de983703bad2fb768e9d104623dcfd6c4d5/src-tauri/lu-core/tests/e1_reviewed_model_tools.rs#L848) implements actual GGUF step generator; :1013 genuine model selects reviewed Git/media tools and :1063 runs journaled observed loop; :1205 and :1496 fresh ANSF/current turn approvals (harness-controlled), native oracle and restart; :1506 multi-step Git HEAD then WAV PCM. Historical C5 workflow at [original SHA](https://github.com/vjmichael/Lu-AI-GUI/blob/b634653cc39b8f1192f77d55850d5b3a95aabfa4/.github/workflows/lu-core-merge-c4-grouped.yml#L842-L864) actually invokes these four ignored genuine test entrypoints; C5 genuine model step passed. The model is downloaded pinned Qwen3-0.6B and executes local CPU, not target 27B physical, not renderer Send. Other test with misleading `actual_model_selected_reviewed_git_and_media_tools_return_verified_receipts` at :342 is **not** interchangeable with :1013; actual native worker and caller-supplied frames are not proof of GGUF model choice.
- **Genuine model file reading exists:** [real_gguf_tool_json.rs:107](https://github.com/vjmichael/Lu-AI-GUI/blob/93b63de983703bad2fb768e9d104623dcfd6c4d5/src-tauri/lu-core/tests/real_gguf_tool_json.rs#L107) ignored explicit real engine/model env, rejects synthetic GGUF; :139–147 fresh file nonce; :223 live PID; :303–320 model JSON, native Protobuf dispatch, real content and final output validation; :338 journal evidence. Need actual original passing run/explicit ignored selection before claiming PASS; no current installed evidence identified.
- **Scripted model + actual executor:** [native_workspace_read_turn.rs:16](https://github.com/vjmichael/Lu-AI-GUI/blob/93b63de983703bad2fb768e9d104623dcfd6c4d5/src-tauri/lu-core/tests/native_workspace_read_turn.rs#L16) Scripted vector, :37–64 real file/archive calls, :70 traversal negative. [supervised_native_turn.rs:60](https://github.com/vjmichael/Lu-AI-GUI/blob/93b63de983703bad2fb768e9d104623dcfd6c4d5/src-tauri/lu-core/tests/supervised_native_turn.rs#L60) test generator, :183 actual typed dispatch with subsequent model context, :298 audit unavailable, :360 cancel, :410 forged host ID, :476 invented tool, :552 repeated invented, :677 invalid args, :718 denied result no retry. Good native coordinator proof, not actual model intelligence or Send.
- **Real MMProj but bypasses application:** [p0_win_chat_real_mmproj.rs:25](https://github.com/vjmichael/Lu-AI-GUI/blob/93b63de983703bad2fb768e9d104623dcfd6c4d5/src-tauri/lu-core/tests/p0_win_chat_real_mmproj.rs#L25) ignored requires externally running pinned llama CPU server, one red and one blue image in memory, calls generate_image_chat_stream :40 directly. [lu-repo01-full-source.yml:3596](https://github.com/vjmichael/Lu-AI-GUI/blob/93b63de983703bad2fb768e9d104623dcfd6c4d5/.github/workflows/lu-repo01-full-source.yml#L3596) compile-only is not this test execution; :3702–3727 explicitly runs real SmolVLM 256M with compatible projector. Workflow defined, not evidence of a pass at current head. Even real positive only transport/model-pixel pair, not image ingestion, host session/projector selection, save/reopen or actual four-image Qwen. Need CP-01 integrated test.
- **Reader/OCR real native:** [native_reader_ocr_acceptance.rs:20](https://github.com/vjmichael/Lu-AI-GUI/blob/93b63de983703bad2fb768e9d104623dcfd6c4d5/src-tauri/lu-core/tests/native_reader_ocr_acceptance.rs#L20) ignored requires real reviewed Poppler, Tesseract, tessdata and image-only fixture. C5 Linux executed; C1 genuine word confidence parser tested. [reader-desktop.test.mjs:10](https://github.com/vjmichael/Lu-AI-GUI/blob/93b63de983703bad2fb768e9d104623dcfd6c4d5/lu-desktop/tests/reader-desktop.test.mjs#L10) checks source includes, not file dialog or drag behavior.
- **Browser actual vs regex:** [lu-browser-real.test.mjs:8](https://github.com/vjmichael/Lu-AI-GUI/blob/93b63de983703bad2fb768e9d104623dcfd6c4d5/tests/lu-browser-real.test.mjs#L8) real visible Chromium worker, navigate/read/screenshot/stop; actual package and native fixture passed at #44018707690. [browser-model-read.test.mjs:13](https://github.com/vjmichael/Lu-AI-GUI/blob/93b63de983703bad2fb768e9d104623dcfd6c4d5/lu-desktop/tests/browser-model-read.test.mjs#L13) strings assert call flow, not an actual model turn. Actual worker test navigates known fixture, not general provider query or live user consent. Preserve separate read/write authority and independent observed citation scope.
- **Frontend helpers/regex:** [r20-markdown-attachments.test.mjs:49](https://github.com/vjmichael/Lu-AI-GUI/blob/93b63de983703bad2fb768e9d104623dcfd6c4d5/lu-desktop/tests/r20-markdown-attachments.test.mjs#L49) actual byte budget helper, :58 source assertions for draft retention; [r21-image-ux.test.mjs:15](https://github.com/vjmichael/Lu-AI-GUI/blob/93b63de983703bad2fb768e9d104623dcfd6c4d5/lu-desktop/tests/r21-image-ux.test.mjs#L15) real image normalization, :35 source assertions; [c64-unified-tabs.test.mjs:9](https://github.com/vjmichael/Lu-AI-GUI/blob/93b63de983703bad2fb768e9d104623dcfd6c4d5/lu-desktop/tests/c64-unified-tabs.test.mjs#L9) regex/assertions, not WebView; [core01d-normal-chat-boundary.test.mjs:27](https://github.com/vjmichael/Lu-AI-GUI/blob/93b63de983703bad2fb768e9d104623dcfd6c4d5/lu-desktop/tests/core01d-normal-chat-boundary.test.mjs#L27) source routing checks; [perf01-cpu-first.test.mjs:28](https://github.com/vjmichael/Lu-AI-GUI/blob/93b63de983703bad2fb768e9d104623dcfd6c4d5/lu-desktop/tests/perf01-cpu-first.test.mjs#L28) source checks, not natural prompt model access. [background-worker-e2e.test.mjs:20](https://github.com/vjmichael/Lu-AI-GUI/blob/93b63de983703bad2fb768e9d104623dcfd6c4d5/lu-desktop/tests/background-worker-e2e.test.mjs#L20) spawns compiled scanner harness, reads actual media-sized bytes/digest; does **not** start model-directed task in installed Lu.
- **Installed physical distinction:** [p0-windows-installed-gui-smoke.ps1:1](https://github.com/vjmichael/Lu-AI-GUI/blob/93b63de983703bad2fb768e9d104623dcfd6c4d5/scripts/p0-windows-installed-gui-smoke.ps1#L1) explicit NO GGUF/tool effects; UIAutomation actual process identity and same-window controls :226–261. Historical C5 installed task board uses scripted operator, not real separate live human positive. [p0-qwen38-radeon-physical.ps1:16](https://github.com/vjmichael/Lu-AI-GUI/blob/93b63de983703bad2fb768e9d104623dcfd6c4d5/scripts/p0-qwen38-radeon-physical.ps1#L16) physical flag, adapter check :37, foreground sampler :110, checklist :138 is owner observation not an independent task oracle. Update CP-10 physical cases from old picker/window design to new exact-path authorization and five tabs. Never upload private files or mark self-report as proof.
- **Benchmarks:** [e1_p05_real_three_arm.py:49](https://github.com/vjmichael/Lu-AI-GUI/blob/93b63de983703bad2fb768e9d104623dcfd6c4d5/benchmarks/e1/e1_p05_real_three_arm.py#L49) actual native output runner, :113 genuine direct model, :161 aggregates including missing arms and nearest-rank p95 :182; compares direct model lacking file access against native utility and reviewed first-party worker, not full Lu model orchestrator. [r00_genuine_kiaf_comparison.py:24](https://github.com/vjmichael/Lu-AI-GUI/blob/93b63de983703bad2fb768e9d104623dcfd6c4d5/benchmarks/e1/r00_genuine_kiaf_comparison.py#L24) requires pinned actual loaded GGUF and explicitly installedLu=false/realOsCustodyProven=false :31; paired two arms source_only/host_classified, syntax protocol comparison, not model-to-worker acceptance. [lu-memory-loop02-real-qwen.yml:123](https://github.com/vjmichael/Lu-AI-GUI/blob/93b63de983703bad2fb768e9d104623dcfd6c4d5/.github/workflows/lu-memory-loop02-real-qwen.yml#L123) actual model interpreting native preproduced evidence is not necessarily choosing/executing tools. [lu-kiaf-execution.yml:25](https://github.com/vjmichael/Lu-AI-GUI/blob/93b63de983703bad2fb768e9d104623dcfd6c4d5/.github/workflows/lu-kiaf-execution.yml#L25) native Rust tests, no actual loaded GGUF declared. Do not claim generic practical capability uplift from existing benchmark names.

## 4. Functional acceptance matrix by repair group

All rows require exact-source test attribution. No PASS at new post-repair SHA claimed. Preserve prior successes scoped to their original evidence.

| Task | Actual behavior | Existing evidence | Remaining status | Required positive AND negative acceptance |
|---|---|---|---|---|
| CP-01 | Image admission, save, multi-image MMProj, cancellation | R21 signature and conversion helpers + source assertions; multimodal_saved_turn/HTTP Rust; actual red/blue test separate local engine | C6 physical HTTP 400/missing loaded MMProj and lost attachment. Genuine SmolVLM test defined, no matching passing run established here; C5/installed UI does not cover actual attachment use. | Installed ordinary drop/paste/chooser → atomic host admission/queue → 1 and 4 independent unknown pixel images → real Qwen27B+MMProj → correct answer → persistent attachment chips/reopen. Malformed, wrong projector, race Send, switch/new chat, queue cancel/retry, crash, eject, no cross-chat/egress; original bytes and native model request receipt. |
| CP-02 | Reader/document attachments, OCR | Reader source contract + native parser tests; historically real Poppler/Tesseract scanned PDF C5 Linux; C1 TSV confidence native pass | PDF/DOCX rejected by drop. Native file chooser/Reader differs from ordinary composer. OCR confidence not truth. No general genuine model document attachment acceptance. | Ordinary drop/chooser/paste/pick of PDF/DOCX/XLSX/PPTX/ODF/TXT/CSV; bounded typed asset → Reader page/sheet/slide → source digest → correct model extraction with source references; scanned PDF real OCR/identity/confidence, corrupt/encrypted/oversize, injection/permissions. No renderer parsing or authority from document. |
| CP-03 | Workspace explicit original-human path and read-only traversal | Native workspace actual files/archive with Scripted model; native discovery/search/authorization tests; genuine GGUF file read test defined | Physical InferenceFailed and unwanted picker. Normal typed path route missing; file-read fixture is not Windows Send. | Original human absolute path incl Unicode/spaces, already-approved exact root re-use without picker, native confirmation for new scope, clarify ambiguous only; real model lists/finds/reads and synthesizes independently known nonce. Deny nonexistent, relative ambiguity, junction/symlink/race, revoked scope, attachment prompt path, unexpected write. |
| CP-04 | General public web search and browser | Real headed Chromium worker and 482-file package installed; source contracts browser-model-read, actual origin/citation native tests; separate academic fetch | C6 physical Google search never dispatched; no proof natural-query real model search. Browser only explicit HTTPS origin. Separate NetworkWrite intentionally excluded. Candidate CAPTCHA duplicate retry risk. | Plain Google/search request → real provider choice and native approval → reviewed actual browser read → fresh excerpt/digest/citation → model incorporates independent source facts. Search controlled fixture and live public provider, cross-origin/SSRF/redirect/CAPTCHA human retry then bounded duplicate guard; denial, revocation, timeout, offline, source freshness, cancel/restart. |
| CP-05 | Common turn coordinator and model capability choice | NativeToolTurn actual typed dispatch with scripted generator; demonstrated actual pinned Qwen3-0.6B specialized reviewed Git/media planner and reopens C5 | Normal text/image Send direct supervisor; no general same-turn choice across capabilities. User-only proposal and result callbacks not unified. Separate Browser loop. No generic execution acceptance. | Normal Send real GGUF chooses eligible tool from readiness inventory, exact typed arguments, native grant only for resource/effect, dispatch, observed result, next generation, independent oracle; multi-step across read/Reader/browser/approved mutation; verify actual model output not harness-chosen calls. Unknown/denied/unready tool, malformed, cancellation, no repeated failed effect, no fake final, no hard 8 useful-call cap. |
| CP-06 | Foundational/utility/specialist actual tool reachability | 42 named-codec matrix below; Git/media host oracles and C5 native/security; C1 current loop/restart tests; typed host effect tests | 12 withheld/unconnected names, 9 host-confirmed effects separate feedback; XLSX/ISO/archive create missing ordinary route; host-only artifact/version/background/memory need clear capability contract. | One positive and negative per supported operation using production entry/worker/receipt plus real model selection where intended; user-only actions clearly designated. No auto-write or ambient shell; test host identity, permission denial, receipt integrity, true independent outcome, post-approval continuation, failure/uncertain/recovery; live dependency reports. |
| CP-07 | Simplification and legacy retirement | Static CP-00A–D maps, old tests and source contract checks | No exhaustive compiler-certified dead-code proof; legacy windows and JSONL store retirement pending | Before/after behavior tests of every retired route, migration and original data, no stale callers, all actual original negative security/provenance and OS custody retained. No loss of useful feature while consolidating. |
| CP-08 | Unified tabs and user-facing behavior | C64 regex source tests; genuine current executable-baseline UIA five tabs install #44018707690; old scripted task board C5 | No proof actual Send/attachment/approval UX after five-tab conversion; goal/planning UI removed without normal conversational replacement | Actual installed keyboard/pointer flows, natural goal/clarification/plan proposal with human review, tasks/scope cancellation and approvals, readable errors, preserved private history, model download and four-image chips; hide technical jargon, keep meaningful consent and evidence. |
| CP-09 | KIAF practical capability paired benchmark | E1-P05 genuine native document outputs + model baseline, R00 offline classifier protocol comparison, memory feedback probes | Not the same as matched model-only vs direct authorized tool vs complete Lu orchestrator on same tasks; no valid CP-09 metric yet | Matched model/hash/quant/engine/context/seed/settings/task corpus across three arms; randomize trial order; independently check final outcome, no fabricated or denied effect; report success/selection/args/authority/feedback, token/steps, intervention, timing p50/p95 sample count, CPU/RAM/VRAM, cancellation/recovery; raw provenance and honest missing metrics. |
| CP-10 | Consolidated installed OS and physical acceptance | C5 hosted OS/job/security/real small GGUF; C1 narrow native; C6.2B5 old UI; C6.4A hosted unified tabs+browser packaging passed at executable baseline | Physical owner Qwen text only; other reported failures; true Linux delegated cgroup positive failed/unavailable, deferred; Debian/AT-SPI, live human positive approval, physical model/vision, final release open | One exact frozen repair candidate across source lock, native Windows/Linux actual workers, real GGUF, installed package and actual UI cases; separate physical R9700/Vulkan/27B/MMProj and user-supplied consent, verified packages; retain Linux kernel positive gate as not tested/deferred, no downgrade. No next installer before grouped repairs and applicable gates. |

## 5. All 42 named model-coded tools: residual reachability and acceptance coverage

Counts are the previous CP-00B inventory; distinct names counted once, not simultaneous permissions, not a general installed tool count. All 21 scoped model route definitions, 9 host-confirmed effect codecs, 12 withheld/latent covered below. Read-only native success is not synonymous with independent oracle; source digests attest identity not truth. (The 42 count excludes host-only controls and dynamic external extensions.) Actual test-body examples and historical job results above, not a claim every named operation already passed.

| Public names | Count | Present lane | Current test coverage anchors | Evidence classification / status | Required actual acceptance |
|---|---:|---|---|---|---|
| lu_files_read_text, lu_files_list_directory, lu_files_stat, lu_files_search_text, lu_files_discover | 5 | Workspace | Native real filesystem tests model_native_files/native_workspace_read_turn/native_codecs; genuine file-nonce test exists (ignored) | Scoped model loop, only approved root; current file-test historical pass not independently located | Ordinary Send explicit path → real model select all five where appropriate, actual unseen content/find, no cross-root; source digest, typed errors and cancellation. |
| lu_reader_read | 1 | Workspace / standalone Reader | native_reader_ocr_acceptance, native_workspace_read_turn, reader_desktop source checks | Real scanned PDF C5 Linux; scripted read or direct native parser, not general MMProj/document Send | Actual chat intake and model selection of PDF/Office/document + bounded original source and extracted data; OCR fail/unavailable and injection. |
| lu_archive_list, lu_artifact_inspect | 2 | Workspace | native_archive_inspect/native_artifact_inspect/native_workspace_read_turn | Native/source and scripted fixture; not independent general chat model result | Real archive/file facts compared to original manifest incl formats/limits; typed feedback. Don't confuse inspection with extraction or creation. |
| lu_git_status, lu_git_log, lu_git_diff | 3 | Git workspace | native_git_inspect; scoped native_workspace_read_turn | Real or fixture Git observation separate from stronger reviewed broker; no ordinary Send positive confirmed | Real repo with known branch/commits/dirty/staged/deleted diff, correct model choice+facts and read-only/no leak. |
| lu_reviewed_git_head, lu_reviewed_git_status, lu_reviewed_git_branch, lu_reviewed_git_history, lu_reviewed_media_facts | 5 | Reviewed Git/media | e1_reviewed_model_tools genuine_gguf, agent01g/01i, c3 genuine; independent original-source oracles | C5 real pinned Qwen3 0.6B → scoped native Git/FFprobe → oracle/restart. Harness pre-admits known resource and scripted authorizer; not normal installed Send or live human approval. | Retain genuine cases; production ordinary chat selection, fresh actual human permission, real model result/next step, deny wrong executable/source/replay/uncertain outcome. |
| lu_reviewed_pdf_facts, lu_reviewed_image_facts | 2 | Reviewed PDF/image | e1_reviewed_model_tools / native_app_typed_specialist and independent outcome oracle | Source/native contract; no independently confirmed genuine-model specialized result on current installed path | Real PDFInfo/ExifTool pinned worker, independently verify page tree/pixels; unsupported PDF variant and invalid source fail; fresh review. |
| lu_validate_schema, lu_check_env | 2 | Exact utility | native_schema_validation/native_environment_discovery/native_core_utility_turn; core_utility_desktop historical-feedback tests | Native exact typed utility and reopened data-only context; do not treat presence probe as arbitrary command execution | Actual ordinary model intent selects, schema invalid vs operation failure differentiated, exact approved environment observation and subsequent result; false success/authority rejected. |
| lu_browser_read | 1 | Scoped Browser | browser-model-read regex; browser authorization/native tests; actual headed worker + installed browser manifest | Real browser package/read component not general model web search; current user report negative | Actual search query & public origin consent, model read/feedback/citation, then CAPTCHA recovery and no repeated stale effect. |
| lu_file_copy, lu_file_move, lu_file_trash, lu_file_restore, lu_archive_extract_folder, lu_docx_create, lu_pptx_create, lu_office_create_revision, lu_office_create_batch_revision | 9 | Host-confirmed effect | native_file_transfer_tool_authority, native_file_trash_tool_authority, native_archive_tree_authority, native_docx_tool_authority/native_pptx_tool_authority, native_office_revision_tool_authority/native_office_batch_tool_authority | Typed direct registry, real outputs/receipt/negative tests; not advertised general model mutations | Actual model inert proposal + native consent → exact bytes and destination, compare digest/original preserved → model continued on outcome; deny cancellation/revoked/replayed/uncertain/no-clobber; no silent retry. |
| lu_browser_interact | 1 | Withheld by policy | native_browser_tool and browser_authorized_execution exact effects | Intentionally outside normal browser allowlist; no NetworkWrite authorization from read | Verify not advertised/dispatched by read-only chat; any future interactive action independent explicit review, not unblock by relaxing current guard. |
| lu_web_fetch, lu_web_academic_search | 2 | Unconnected codec | native_web_research tests and direct research service tests | Core registrations no production caller established; academic DOI leads not verified search citations | Unify supported network read via host approved source; actual live leads, verified text, citation, network limits; otherwise explicit unavailable. |
| lu_background_status, lu_background_result, lu_background_start, lu_background_cancel | 4 | Latent model catalogs | native_background_result and native_background_control tests; background-worker-e2e actual scanner harness | Task scope and receipt fixtures, owned controls, UI task APIs and injected status separate; no model production registration | Model only sees owner task and verified terminal result; start only reviewed worker plan, one-use cancel, no arbitrary cmd; actual task restart/feedback/deny across chats. |
| lu_zip_extract, lu_iso_extract, lu_linux_archive_create, lu_linux_archive_extract | 4 | Latent archive codecs | native_zip_extract_authority/native_iso9660/native_linux_archive | Member operations and create not equivalent modern archive tree; no production registration | Real ZIP/ISO selected member/new archive creation, exact reviewed destination, independent manifest/CRC/bytes, malicious members/bomb/links/no overwrite/cancel/restart. Or explicitly unavailable if not in scope. |
| lu_spreadsheet_create | 1 | Latent spreadsheet codec | native_spreadsheet_create/native_spreadsheet_tool_authority; E1-P05 task harness real XLSX | Native implementation, no ordinary desktop creation route | Model proposes structured workbook; native exact grant; publish actual xlsx, independent sheet/cell/formula oracle; malformed formulas/overwrites/limits/denial and persisted feedback. |

## 6. Residual integration audit of host-only capabilities and feedback

- **File text inspection and mutation:** [native_host_read.rs:108](https://github.com/vjmichael/Lu-AI-GUI/blob/93b63de983703bad2fb768e9d104623dcfd6c4d5/lu-desktop/src-tauri/src/native_host_read.rs#L108) pinned read/receipt; [native_host_write.rs:114](https://github.com/vjmichael/Lu-AI-GUI/blob/93b63de983703bad2fb768e9d104623dcfd6c4d5/lu-desktop/src-tauri/src/native_host_write.rs#L114) durable dispatch and :130 exact result; [native_host_edit.rs:102](https://github.com/vjmichael/Lu-AI-GUI/blob/93b63de983703bad2fb768e9d104623dcfd6c4d5/lu-desktop/src-tauri/src/native_host_edit.rs#L102) dispatch and :122 original receipt, :251 rollback. Native create/edit/rollback/read ledger tests and write/recovery tests exist, but no guaranteed common next-model observation. Exercise real native read/whole-file write/patch/rollback from normal chat request, explicit human review, exact selected file and byte comparison; refused/uncertain does not become success. 
- **Batch archives and Office replace-all:** [lib.rs:1952](https://github.com/vjmichael/Lu-AI-GUI/blob/93b63de983703bad2fb768e9d104623dcfd6c4d5/lu-desktop/src-tauri/src/lib.rs#L1952) and :7308 type labels with JSON host intent vs actual Protobuf receipt. Require schema/version compatibility, exact SHA source and published output oracle, interruption and uncertain no replay. Do not rewrite historical receipt fingerprints. Test batch/split/reassemble with hostile manifests, no partial publication and source preservation.
- **OCR/vision/image edit/Observe:** [lib.rs:2115](https://github.com/vjmichael/Lu-AI-GUI/blob/93b63de983703bad2fb768e9d104623dcfd6c4d5/lu-desktop/src-tauri/src/lib.rs#L2115) native selected image OCR and :2215 attached image, :2285 region, :2368 actual PDF page. Need real worker/bytes plus result typed as transcription not verified fact; original image edits, undo/redo and export real image hash/pixel checks; Observe double consent, native capture worker privacy/egress deny and revocation/expiry. Do not send private observation data to external tools. Real Windows physical capture missing; negative source rules remain, not proof capture works.
- **Memory/task plan:** [lib.rs:5550](https://github.com/vjmichael/Lu-AI-GUI/blob/93b63de983703bad2fb768e9d104623dcfd6c4d5/lu-desktop/src-tauri/src/lib.rs#L5550) original source context, :5334 historical verified feedback, current accepted C06 scoped read. Memory retrieval is data-only; historical completion doesn't grant current authority. Natural-language goal/clarification/plan proposal through ordinary chat (removed old controls), actual model plan, review, durable register progress, independently observed task outcome and independent new consent on next plan step, cancel/restart lineage; no model-created user turns, no fake completion. Historical real model Git/media harness positive is useful but not this UI.
- **Model/download/provider:** actual pinned transport and hf_p05_real_network.rs; HF transfer UI/source tests plus current model library. Separate actual provider HTTP, review license/revision/size/digest, download/pause/cancel/resume/verification, tamper/redirect denial and live model import (not same as transfer verified), offline failure without fake result. Observe network deny intentional. Host health/probes should derive authoritative runtime availability (not static catalog).
- **Background worker scheduling:** actual scanner harness, native tasks/receipts and scheduler tests; protected Win32 Job positive historical and Linux root-owned Git, but full delegated cgroup positive not available. Test installed owned task ordinary Chat/Tasks, original task scope, explicit approval, pause/cancel, restart no auto replay, guardian death and unrelated process survival. No auto completion authority.
- **Dynamic/extension interfaces:** CE packages, schema, permissions, signing, drain, broker, worker wire negative tests; generic read-only `register_host_reviewed_application_action` unit test. No separate production-model generic host reviewed action call established. Future DLC runtime excluded; keep host-only binding and reject unsigned/unreviewed extension and unavailable prerequisites. Do not advertise invented dynamic names.
- **Common feedback:** current [agent_tool_loop.rs:658](https://github.com/vjmichael/Lu-AI-GUI/blob/93b63de983703bad2fb768e9d104623dcfd6c4d5/src-tauri/lu-core/src/agent_tool_loop.rs#L658) durable dispatch, :680 typed decoded result, :687 distinct independent verification, :721 bounded model feedback. Normal Send [lib.rs:5836](https://github.com/vjmichael/Lu-AI-GUI/blob/93b63de983703bad2fb768e9d104623dcfd6c4d5/lu-desktop/src-tauri/src/lib.rs#L5836) and :5846 raw text/image supervisor, not common tool loop. [lib.rs:5948](https://github.com/vjmichael/Lu-AI-GUI/blob/93b63de983703bad2fb768e9d104623dcfd6c4d5/lu-desktop/src-tauri/src/lib.rs#L5948) separately stages native proposals; historical/effect/UI result must be tied to live original call without new authority. Browser [browser_model_desktop.rs:468](https://github.com/vjmichael/Lu-AI-GUI/blob/93b63de983703bad2fb768e9d104623dcfd6c4d5/lu-desktop/src-tauri/src/browser_model_desktop.rs#L468) fingerprints calls before dispatch; :491/:522 human verification path continues without demonstrated progress-epoch reset. Add exact deterministic CAPTCHA state transition test and actual browser fixture; only permit narrowly scoped read retry after verified changed state, no general repeat bypass. Preserve current negative protections.

## 7. Required evidence record for each repair acceptance

For every case preserve actual source SHA / source-lock digest; package/install/engine/model/projector SHA and OS/device/driver; original user request/test case ID and exact input source identities; route (including model-free CPU cases), advertised eligible tool names AND separately approved scope/effect; genuine model raw call transcript/digest or explicit host-only designation; typed arguments, host-side decoder, authorized invocation and worker identity; outcome (observed success / denied / error / cancelled / uncertain), independently verified effect where required, receipt IDs and source digest; actual next-generation feedback and independently graded answer; cancellation/replay/no side effects evidence. No private raw data in shared artifacts. Auto grader must fail fabricated receipts, wrong source/model, skipped cases and unexpected unauthorized effects. Actual test logs/JSON and positive+negative counts; skipped/blocked/unavailable not success. One common schema, not another runtime authority system. For CP-09 matched three-arm protocol requires same model and tasks, genuine standalone tool executions and genuine complete Lu orchestration, no model pre-supplied secret, evidence of correct final outcome including denied cases, actual resource measurements or explicit NOT_MEASURED.

## 8. Gate order and disposition

CP-00A static bridge, CP-00B named-tool and specified reverse registration map, CP-00C whole-source scan, CP-00D historical reconciliation, CP-00E functional acceptance coverage/residual disposition **complete within stated audit scope**. Unverified individual indirect/dynamic/host behavior and exact-SHA tests explicitly become CP-01–10 implementation acceptance requirements; not marked passed. No additional generic audit loop required before repairs. CP-01 next: one transactional attachment lifecycle, typed source identity and atomic draft/send preservation; CP-02 documents should reuse that contract; CP-03 approved path resource, CP-04 public web search; coherent CP-05–06 execution integration; CP-07–08 simplify/UX; CP-09 real empirical benchmark; CP-10 final grouped native/installed acceptance. No new installer until group repaired and applicable acceptance. Physical Windows R9700 target and independently qualified delegated Linux positive tests remain distinct. C_practical = f(K,I,A,F), judge actual performance by verified successful task behavior not tool names.

## Appendix — complete test/benchmark/workflow inventory (not a pass list)

The 246 test paths include fixtures/manifest files; the 45 benchmark paths include evidence and verifier unit tests; the 78 workflow paths include many manual/native gating definitions. Group by root; individual bodies not all re-read or executed in this turn. Scope lists are direct frozen Git-tree entries to support further acceptance work.

### tests/

- `lu-browser-real.test.mjs`
- `lu-browser-worker.test.mjs`
- `operational-memory-harness/Cargo.toml`
- `operational-memory-harness/src/lib.rs`
- `source-identity.test.mjs`

### lu-desktop/tests/

- `agent01e-progress-ui.test.mjs`
- `agent01f-model-plan-ingress.test.mjs`
- `agent01g-approved-step-integration.test.mjs`
- `agent01g-physical-installer-contract.test.mjs`
- `agent01g-real-model-chain-contract.test.mjs`
- `agent01h-next-step-preview.test.mjs`
- `agent01i-fresh-continuation.test.mjs`
- `agent01j-lineage-recovery.test.mjs`
- `agent01k-independent-recovery.test.mjs`
- `agent01l-linux-cgroup-v2.test.mjs`
- `agent01l-os-custody.test.mjs`
- `background-pane.test.mjs`
- `background-scheduler.test.mjs`
- `background-worker-e2e.test.mjs`
- `browser-ci-candidate.test.mjs`
- `browser-ci-executable.test.mjs`
- `browser-model-read.test.mjs`
- `browser-native-authorization.test.mjs`
- `browser-research-evidence.test.mjs`
- `browser-staging.test.mjs`
- `browser-visibility-settings.test.mjs`
- `c64-unified-tabs.test.mjs`
- `core01d-normal-chat-boundary.test.mjs`
- `filesystem-discovery.test.mjs`
- `image-assembly-geometry.test.mjs`
- `image-edit-intent.test.mjs`
- `image-workspace-effects.test.mjs`
- `image-workspace-history.test.mjs`
- `m2-ansf-memory.test.mjs`
- `m3-ansf-recall.test.mjs`
- `m3-asset-resume.test.mjs`
- `m3-continuity-bridge.test.mjs`
- `m3-effect-receipts.test.mjs`
- `m3-history-pane.test.mjs`
- `m3-local-embedding.test.mjs`
- `m3-operational-assets.test.mjs`
- `m3-scroll-geometry.test.mjs`
- `m3-semantic-contract.test.mjs`
- `m3-windowed-chat.test.mjs`
- `m3-windows-physical-contract.test.mjs`
- `m4-belief-contract.test.mjs`
- `m4-installed-acceptance-contract.test.mjs`
- `m5-workorders-contract.test.mjs`
- `memory-loop01-agent-handoff.test.mjs`
- `memory-loop02-real-qwen-contract.test.mjs`
- `memory-loop02-verified-feedback.test.mjs`
- `memory-loop02b-host-safe-decision.test.mjs`
- `memory-loop02c-operator-provenance.test.mjs`
- `memory-loop03a-goal-recovery.test.mjs`
- `memory-loop03b-source.test.mjs`
- `memory-loop03e-crash-recovery.test.mjs`
- `native-auth/Cargo.toml`
- `native-auth/src/lib.rs`
- `native-chat/Cargo.toml`
- `native-chat/src/lib.rs`
- `native-dx/Cargo.toml`
- `native-dx/src/lib.rs`
- `native-file-scan/Cargo.lock`
- `native-file-scan/Cargo.toml`
- `native-file-scan/src/main.rs`
- `native-reviewed-recovery/Cargo.toml`
- `native-reviewed-recovery/src/lib.rs`
- `p0-browser-bundle.test.mjs`
- `p0-browser-real-ci-gate.test.mjs`
- `p0-hf-transfer-transaction.test.mjs`
- `p0-installed-gui-smoke.test.mjs`
- `p0-memory-telemetry.test.mjs`
- `p0-native-intent.test.mjs`
- `p0-native-window-separation.test.mjs`
- `p0-physical-ux-contract.test.mjs`
- `p0-qwen38-radeon-handoff.test.mjs`
- `p0-research-provider-bundle.test.mjs`
- `p0-sec-guardian-hardening.test.mjs`
- `p0-sec05-adversarial.test.mjs`
- `p0-sec06-consolidated.test.mjs`
- `p03-ai-ops.test.mjs`
- `p06-observe-privacy.test.mjs`
- `p07-research-styles.test.mjs`
- `p08-archive-split.test.mjs`
- `p09-office-boundary.test.mjs`
- `p10-image-boundary.test.mjs`
- `perf01-baseline.test.mjs`
- `perf01-cpu-first.test.mjs`
- `perf01-p0d-closeout.test.mjs`
- `perf01-p3-stream-recovery.test.mjs`
- `perf01-p4-interactive-priority.test.mjs`
- `phase2-agent-replan.test.mjs`
- `phase3-agent-memory.test.mjs`
- `preview-dual-backend.test.mjs`
- `r17-interface.test.mjs`
- `r18-queue.test.mjs`
- `r19-desktop-ux.test.mjs`
- `r20-markdown-attachments.test.mjs`
- `r20-markdown-render.jsx`
- `r21-image-ux.test.mjs`
- `r23-profile.test.mjs`
- `reader-desktop.test.mjs`
- `research-citation-enforcement.test.mjs`
- `research-roadmap-acceptance.test.mjs`
- `research-source-route.test.mjs`
- `reviewed-app-desktop.test.mjs`
- `reviewed-task-continuity.test.mjs`
- `reviewed-tool-feedback.test.mjs`
- `task-board.test.mjs`
- `tools09-bounded-search.test.mjs`
- `ux-chat01-message-interactions.test.mjs`
- `ux-chat02-revision-reply-provenance.test.mjs`
- `windows-physical/Lu_R13_Vision_Test_Fixture.png`
- `windows-physical/Lu_R14_Vision_Test_Fixture.png`
- `windows-physical/README.txt`
- `windows-physical/README_R14.txt`
- `windows-physical/Run-LuR13-WindowsAcceptance.ps1`
- `windows-physical/Run-LuR14-WindowsAcceptance.ps1`
- `windows-physical/START_WINDOWS_LU_R13_TEST.cmd`
- `windows-physical/START_WINDOWS_LU_R14_TEST.cmd`
- `windows-physical/test_win_manifest_regression.py`
- `workspace-desktop.test.mjs`
- `write-01e-edit-ux.test.mjs`

### src-tauri/lu-core/tests/

- `alpha_support_contracts.rs`
- `ansf.rs`
- `ansf_cognitum_bridge.rs`
- `ansf_import.rs`
- `ansf_library.rs`
- `ansf_revisions.rs`
- `ansf_sync.rs`
- `benchmark_contract.rs`
- `browser_native_acceptance.rs`
- `ce_approval_commit_expiry.rs`
- `ce_broker_permissions.rs`
- `ce_drain.rs`
- `ce_durable_trust.rs`
- `ce_host_profile.rs`
- `ce_packages.rs`
- `ce_revocation_notifications.rs`
- `ce_staging_identity.rs`
- `ce_task_effect_fence.rs`
- `ce_worker_authority.rs`
- `ce_worker_wire_adversarial.rs`
- `compute_autotune.rs`
- `compute_inventory.rs`
- `compute_probe.rs`
- `compute_runtime.rs`
- `continuity.rs`
- `continuity_conversation_scope.rs`
- `continuity_effect_receipts.rs`
- `conversation_authority.rs`
- `core_extension_lifecycle.rs`
- `cpu_autotune.rs`
- `cpu_instructions.rs`
- `cpu_topology.rs`
- `cross_chat_reference.rs`
- `desktop_owner_recovery.rs`
- `diagnostic_contract.rs`
- `diagnostic_localization.rs`
- `document_preferences.rs`
- `durable_dispatch.rs`
- `e1_artifact_custody.rs`
- `e1_reviewed_execution.rs`
- `e1_reviewed_model_tools.rs`
- `edit_safety.rs`
- `effect_journal.rs`
- `export_contract.rs`
- `fixtures/rar4_basic.rar.uu`
- `fixtures/rar5_compressed.rar.uu`
- `hf_p05_real_network.rs`
- `host_resource_authority.rs`
- `host_reviewed_route_durable.rs`
- `host_session.rs`
- `linux_device_hints.rs`
- `linux_gpu_nodes.rs`
- `m5_effect_authority_acceptance.rs`
- `model_json_boundary.rs`
- `model_native_files.rs`
- `model_turn_journal.rs`
- `mythos_privilege_guard.rs`
- `native_7z_rar.rs`
- `native_agent_tool_turn.rs`
- `native_archive_create.rs`
- `native_archive_inspect.rs`
- `native_archive_split.rs`
- `native_archive_tree.rs`
- `native_archive_tree_authority.rs`
- `native_artifact_inspect.rs`
- `native_atomic_write.rs`
- `native_codecs.rs`
- `native_create_new_authority.rs`
- `native_csv_edit.rs`
- `native_dispatch_error_codes.rs`
- `native_docx_create.rs`
- `native_docx_tool_authority.rs`
- `native_edit_existing_authority.rs`
- `native_environment_discovery.rs`
- `native_file_transfer.rs`
- `native_file_transfer_tool_authority.rs`
- `native_file_trash_tool_authority.rs`
- `native_git_inspect.rs`
- `native_iso9660.rs`
- `native_linux_archive.rs`
- `native_linux_packages.rs`
- `native_loop_progress_acceptance.rs`
- `native_office_batch_tool_authority.rs`
- `native_office_revision.rs`
- `native_office_revision_tool_authority.rs`
- `native_pptx_create.rs`
- `native_pptx_tool_authority.rs`
- `native_reader_ocr_acceptance.rs`
- `native_rpm.rs`
- `native_schema_validation.rs`
- `native_spreadsheet_create.rs`
- `native_spreadsheet_tool_authority.rs`
- `native_startup_recovery.rs`
- `native_stream_loop.rs`
- `native_tool_lifecycle.rs`
- `native_workspace_read_turn.rs`
- `native_zip_extract.rs`
- `native_zip_extract_authority.rs`
- `operational_memory.rs`
- `ownership_store.rs`
- `p0_win_chat_real_mmproj.rs`
- `p2_inference_lifecycle.rs`
- `passive_diagnostics.rs`
- `perf01_kv_cache_real.rs`
- `platform_compute.rs`
- `policy_native.rs`
- `protobuf_wire.rs`
- `read_approval_ledger.rs`
- `real_gguf_import.rs`
- `real_gguf_tool_json.rs`
- `registry_contracts.rs`
- `saved_chat_prompt_limits.rs`
- `supervised_native_turn.rs`
- `supervisor_dynamic_catalog.rs`
- `task_checkpoints.rs`
- `unified_native_catalog.rs`
- `web_research_acceptance.rs`
- `web_research_citation_acceptance.rs`
- `web_research_roadmap_acceptance.rs`
- `workspace_quarantine.rs`
- `write_approval_ledger.rs`
- `write_format.rs`
- `write_recovery.rs`

### benchmarks/

- `e1/agent01g_qwen_plan_probe.py`
- `e1/agent01l_cgroup_escape_crash_acceptance.py`
- `e1/e1_p05_real_three_arm.py`
- `e1/e1_p05_xml_oracle.py`
- `e1/e1_p07_llama_completion.py`
- `e1/e1_p08_linux_group_guard.py`
- `e1/e1_p08_setsid_escape_probe.py`
- `e1/evidence/2026-11-09-qwen03c-baseline.json`
- `e1/evidence/2026-11-09-qwen03c-direct.json`
- `e1/memory_loop02_genuine_qwen_feedback.py`
- `e1/memory_loop03c_positive_oracle_cases.py`
- `e1/memory_loop03c_real_qwen_comprehension.py`
- `e1/memory_loop03c_reviewed_worker_cases.py`
- `e1/p0_sec02_linux_guardian_negative.py`
- `e1/p0_sec05_native_adversarial_manual.sh`
- `e1/plan04_real_gguf_synthesis.py`
- `e1/plan07b_real_gguf_task_updates.py`
- `e1/r00_genuine_kiaf_comparison.py`
- `e1/sec06c_browser_guardian_loss.py`
- `e1/sec06c_browser_host_crash.py`
- `e1/sec06c_browser_real_native.py`
- `e1/sec06c_browser_xauthority_contract.py`
- `e1/sec06c_retired_guardian_denial.py`
- `e1/sec06c_root_owned_systemd_boundary.py`
- `e1/test_agent01g_qwen_plan_probe.py`
- `e1/test_e1_p05_raw_receipt.py`
- `e1/test_e1_p05_real_contract.py`
- `e1/test_e1_p05_xml_oracle.py`
- `e1/test_e1_p07_llama_completion.py`
- `e1/test_memory_loop02_qwen_feedback.py`
- `e1/test_memory_loop03c_positive_git.py`
- `e1/test_memory_loop03c_real_qwen_comprehension.py`
- `e1/test_memory_loop03c_reviewed_workers.py`
- `e1/test_plan07b_real_gguf_task_updates.py`
- `e1/test_r00_genuine_kiaf_comparison.py`
- `e1/verify_e1_p05_raw.py`
- `memory/cognitum_three_arm_real_model.py`
- `memory/m4_real_local_model.py`
- `memory/m5_comparative_acceptance.py`
- `memory/m5_repeated_task_real_model.py`
- `memory/test_cognitum_three_arm.py`
- `memory/test_m5_comparative_acceptance.py`
- `memory/test_m5_repeated_task_contract.py`
- `memory/test_three_arm_real_model.py`
- `memory/three_arm_real_model.py`

### .github/workflows/

- `lu-agent01a-durable-plan.yml`
- `lu-agent01b-plan-revisions.yml`
- `lu-agent01c-deliverable-receipts.yml`
- `lu-agent01d-host-scheduler.yml`
- `lu-agent01e-broker-progress.yml`
- `lu-agent01g-physical-windows-nsis.yml`
- `lu-agent01i-fresh-continuation.yml`
- `lu-agent01j-lineage-recovery.yml`
- `lu-agent01k-independent-recovery.yml`
- `lu-agent01l-linux-cgroup-v2.yml`
- `lu-agent01l-os-custody.yml`
- `lu-ansf-direct-ids-artifacts.yml`
- `lu-ansf-three-arm-benchmark.yml`
- `lu-browser-research-acceptance.yml`
- `lu-ci.yml`
- `lu-core-c1-five-gaps-acceptance.yml`
- `lu-core-c4-delegated-kernel-original-sha.yml`
- `lu-core-c6-hosted-dual-nsis.yml`
- `lu-core-c62a-windows-browser-candidate.yml`
- `lu-core-c62b-reviewed-browser-nsis.yml`
- `lu-core-c62t-browser-review-transport.yml`
- `lu-core-merge-c4-grouped.yml`
- `lu-development-ledger.yml`
- `lu-e1-app-admission.yml`
- `lu-e1-p03-execution.yml`
- `lu-e1-p04-artifact-custody.yml`
- `lu-e1-p04-output-oracle.yml`
- `lu-e1-p05-three-arm.yml`
- `lu-e1-p06-thirdparty-adapters.yml`
- `lu-e1-p06b-real-thirdparty.yml`
- `lu-e1-p06c-typed-git.yml`
- `lu-e1-p06d-git-context.yml`
- `lu-e1-p06e-ffprobe.yml`
- `lu-e1-p07-genuine-gguf.yml`
- `lu-e1-p07-reviewed-app.yml`
- `lu-e1-p08-reviewed-recovery.yml`
- `lu-e1-p09-reviewed-audit.yml`
- `lu-e1-p10-independent-outcomes.yml`
- `lu-e1-p11-independent-git-history.yml`
- `lu-e1-p12-independent-semantic-verifiers.yml`
- `lu-e1-p13-resource-revision.yml`
- `lu-e1-p15-conversation-recovery.yml`
- `lu-kiaf-execution.yml`
- `lu-m2-ansf-cutover.yml`
- `lu-m2-memory.yml`
- `lu-m3-ansf-recall.yml`
- `lu-m3-cognitum-real-benchmark.yml`
- `lu-m4-verified-memory.yml`
- `lu-m5-memory-replay.yml`
- `lu-memory-loop02-real-qwen.yml`
- `lu-memory-loop02-verified-feedback.yml`
- `lu-memory-loop02b-safe-decision.yml`
- `lu-memory-loop02c-provenance.yml`
- `lu-memory-loop03a-goal-recovery.yml`
- `lu-memory-loop03b-native.yml`
- `lu-p03-ai-ops.yml`
- `lu-p06-observe-privacy.yml`
- `lu-p07-research-citation-styles.yml`
- `lu-p08-archive-split.yml`
- `lu-p09-office-boundary.yml`
- `lu-p10-image-basics.yml`
- `lu-p11-git-core.yml`
- `lu-perf01-p0-p0d-exact-head.yml`
- `lu-plan07d-installed.yml`
- `lu-repo01-full-source.yml`
- `lu-tools-01-05-integration.yml`
- `lu-tools-02-json-boundary.yml`
- `lu-tools-03-git.yml`
- `lu-tools-04-archive.yml`
- `lu-tools-11-background.yml`
- `lu-ux-chat01-message-interactions.yml`
- `lu-ux-chat02-revisions-replies.yml`
- `lu-v1-observe-learn.yml`
- `lu-wave1-native-tools.yml`
- `lu-wave2-files.yml`
- `lu-wave3-conversation-tools.yml`
- `lu-wave4-contextual-tools.yml`
- `lu-win-load-01.yml`
