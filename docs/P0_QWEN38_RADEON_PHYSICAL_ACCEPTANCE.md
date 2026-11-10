# Qwen3.8-27B — physical Windows acceptance handoff

This is an **operator-run** test, not a completed acceptance claim. Never run it in a private production profile without reviewing the script and choosing an isolated Lu profile. The script is read-only except for its newly created evidence directory.

```powershell
pwsh -NoProfile -File .\scripts\p0-qwen38-radeon-physical.ps1 `
  -InstalledLuExe "C:\Path\To\Lu.exe" `
  -ModelGGUF "D:\Models\Qwen3.8-27B.Q4_K_M.gguf" `
  -MmprojGGUF "D:\Models\Qwen3.8-mmproj.gguf" `
  -PhysicalWindowsConfirmed -DurationSeconds 180
```

Supply your *actual* file paths; examples above are placeholders, not model locations. Begin loading Qwen through Lu while the foreground sampler runs. A compatible GGUF/MMProj is required for image tests.

**Paste into Lu with approved native tools available:**

1. What is the exact HEAD commit SHA of my selected local Git repository? Look it up in the repository; do not guess.
2. What is the duration in milliseconds and audio codec of my selected WAV file? Inspect the media file; do not guess.
3. What time is it right now? Use Lu's native host clock.
4. Inspect the files in my local project folder. Ask me to select the directory before accessing it.
5. Browse https://example.org and report the page title from actual browser output, with a consent step.
6. After I attach a known red and blue test image, describe the actual colors and shapes. Do not invent vision evidence.
7. Queue a second question during active generation, then eject the model after both finish.

**Pass criteria:** real model tool choice, fresh scoped authorization, actual OS effects, independent verification and durable receipt; no answer-only substitute. Explicitly separate native process RAM/GPU allocation from model tensor-loading percentage. The manifest hashes installed app, CPU/Vulkan engines, GGUF and MMProj; it does not authenticate their origin. The recorder collects hardware/process metadata locally and uploads nothing. Before sharing, remove any paths, machine identity or other private metadata.

**Not measured:** tensor residency percentages, absolute Vulkan offload attribution to the user's specific adapter when Windows PDH is unavailable, real model vision semantics from screenshots alone, or true GUI approval clicks unless separately captured and inspected.
