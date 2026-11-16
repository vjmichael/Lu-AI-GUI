# Lu Core C6 — Windows installed and physical Radeon acceptance

**Gate status: OPEN.** An actual Windows NSIS file is a prerequisite, not physical evidence. The separate C1 reviewed source/native acceptance passed, but it did not load Qwen3.8-27B on the Radeon AI PRO R9700. Do not turn CI results, scripted Qwen responses, human checkbox reports, or Windows process RAM usage into a physical acceptance claim.

## Source-bound hosted installer preflight

The manually admitted one-SHA workflow is '.github/workflows/lu-core-c6-hosted-dual-nsis.yml'. It checks the complete source lock and builds genuine pinned CPU and Vulkan llama.cpp servers from upstream commit e0f142d5ddc1461ce721644d92af33db23545105; compiles current native Lu, checks the Windows PE manifest, produces a SHA-256-verified unsigned NSIS preview, installs it into a disposable hosted Windows runner, verifies installed CPU/Vulkan/research bytes, opens installed WebView2/Settings/Model Library using native UI evidence, uninstalls, and checks AppData preservation. A terminal job with its original 'installer-checksums.json', 'BUILD_PROVENANCE.json' and 'm3-nsis-cycle.json' is needed before calling this hosted preflight PASS. Only a real uploaded installer with matching source SHA may be used for subsequent physical work.

The hosted candidate deliberately **does not bundle Chromium/Node/Playwright** without a separately reviewed source-pinned browser package. The browser tool and full release remain OPEN. The hosted runner cannot prove R9700 Vulkan offload, a genuine Qwen3.8-27B response, MMProj vision, human tool approval or production kernel process custody. Do not confuse a desktop/compiler placeholder or a standalone llama-server response with a real installed Lu session.

## Physical owner-run acceptance on the Radeon AI PRO R9700

Use a *disposable or backup-safe* profile; do not run an unverified unsigned executable. Independently verify the downloaded installer SHA-256 against 'installer-checksums.json' and 'BUILD_PROVENANCE.json', and check both list the same exact source SHA. Install the genuine current preview (not alpha), check the native main chat, independent Settings and Model Library windows, and select **Vulkan** in Lu Settings before restarting. Import the real locally-held Qwen3.8-27B GGUF and its matching vision projector via Lu's human model workflow. Never auto-import model paths from prompt text.

The foreground physical recorder is 'scripts/p0-qwen38-radeon-physical.ps1'. It requires explicit Windows physical confirmation and now refuses non-R9700 adapter enumeration or a missing MMProj, matches the observed Vulkan engine to a live parent Lu desktop process and its exact installed binary location, records engine/model/projector hashes and separate RAM/process-GPU memory observations, and leaves true backend tensor-offload percentage **unmeasured**. It uploads nothing. Example (replace every example path):

~~~powershell
pwsh -NoProfile -File .\scripts\p0-qwen38-radeon-physical.ps1 -InstalledLuExe "C:\Path\To\LuCurrentPreview\lu-desktop.exe" -ModelGGUF "D:\Models\Qwen3.8-27B.gguf" -MmprojGGUF "D:\Models\matching-mmproj.gguf" -PhysicalWindowsConfirmed -DurationSeconds 180
~~~

While foreground sampling runs, actually load Qwen and perform these distinct cases; record the original result, native authoritative receipt, operator approval, and independent oracle where available:

1. **Real local model:** Actual GGUF header/catalog identity, 27B model weight admission, host-owned Vulkan engine selection; separate OS/driver Vulkan device evidence. A requested GPU-layer value alone is **not** proof of actual offload. Check real llama.cpp model loading logs for actual Vulkan tensors, and preserve a redacted physical witness.
2. **Matching MMProj vision:** Supply the exact known red/blue test PNG in 'lu-desktop/tests/windows-physical/' using the installed attachment UI. Verify the model output against the independent image fixture and ensure mismatched MMProj is refused, not guessed.
3. **Native tool choice and authorization:** Ask Lu to read a selected actual local directory, retrieve actual Git HEAD/status and inspect a disposable WAV. Require native directory picker/one-use user permission; compare Git and WAV results against separate ground-truth native tool output, and observe exact source/worker receipts. An answer without real tool dispatch fails.
4. **Human-driven workflow:** Queue a second prompt during active generation; cancel the first, verify worker custody and no stale-tool retry, then finish the second. Revoke a grant, try to reuse it and confirm fail-closed. A second independently approved step must obtain a fresh user decision and new journal/grant.
5. **Restart/recovery:** Eject the model and verify no host-owned llama-server remains; restart the installed app, confirm chat/ANSF current revisions and authentic original receipts, and deny replay of historical effects or permissions. Check model/worker unload and re-admission from original source. Inspect installation/uninstallation with user data preserved.
6. **Browser:** Do not award a pass when the package lacks the reviewed Chromium/Node/Playwright bundle. Record NOT_PACKAGED; web search/browse requiring that native runtime remains an independent packaging/release blocker.

**Acceptance rule:** No single operator self-report, process-working-set number, SDK listing, generated prose or hosted CI can promote C6 to passed. A reviewed original physical receipt must bind installer SHA, installed Lu SHA, separately pinned CPU/Vulkan engine SHAs, Qwen GGUF hash, MMProj hash, real GPU model/driver, original human consent, native receipts and independent ground-truth checks. Sanitize hostnames, paths, screenshots and personal source contents before sharing. User completion and formal release are further separate gates; Linux delegated-cgroup positive remains deferred.
