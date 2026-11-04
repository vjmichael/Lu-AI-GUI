# Deferred Windows tests — v1.0 release-candidate phase

**Decision:** No routine physical retesting while building Lu v1.0. GitHub CI verifies committed source only; these scenarios remain explicitly **not yet accepted**. Physical tests resume near v1.0.

| ID | Near-v1.0 test | Expected |
|---|---|---|
| M-01 | Win10/11 clean install | Taskbar and Start Menu icon, shortcut grouping, uninstall entry |
| M-02 | Upgrade accepted alpha.2 | Conversation, image, document and settings preservation |
| M-03 | Import/cancel/retry GGUF + MMProj | Measured bytes and atomic registration; no orphan imports |
| M-04 | Repeated Qwen load with AMD Vulkan | Responsive UI, verified layer offload and memory accounting |
| M-05 | Paste, reopen and retry images after restart | Correct persisted image and prompt; no missing thumbnails |
| M-06 | Cross-chat FIFO and cancel middle job | Deterministic queue movement and durable partial output |
| M-07 | Native tool authorization | Untrusted prompt cannot obtain a grant or bypass approval |
| M-08 | Crash during contained file write | No duplicate effect, worker escape, lost journal |
| M-09 | ANSF memory contradiction and recovery | Accurate, editable, source-linked memory |
| M-10 | Backup/restore, uninstall/reinstall | Explicit retention and safe deletion, consistent migration |

**Do not infer any manual test passed from CI.** Store Windows screenshots, host traces and build hashes when final testing resumes.
