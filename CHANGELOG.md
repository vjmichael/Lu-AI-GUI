# Lu changelog

This file records **accepted product milestones and releases**, not all branches where code has been written. Use the [central development ledger](docs/LU_DEVELOPMENT_LEDGER.md) for work in progress, draft PRs, exact source revisions, open integration gates and partial CI results. A draft PR becoming green does **not** automatically make its features available in the installed Lu application.

## Unreleased — v1.0 development

No v1.0 release or integrated native-tool milestone has been accepted here. Source implementations under development include the filesystem/edit and archive tools, Reader, browser/research, background tasks, Git inspection, JSON/schema validation, environment discovery, OCR routing and loop/backend control. Their current source/desktop/model/installed statuses are tracked **only** in the [development ledger](docs/LU_DEVELOPMENT_LEDGER.md); do not describe them as shipped without new acceptance evidence.

**2026-11-05 process change:** centralized Lu development history and active PR tracking on `main`. Implementation order: consolidate and close native Core tools, then integrate ANSF/SQLite, operational memory, continuity/context assembly, then test the complete v1.0 application. IDE coding/debugging and NVP video processing remain specialized workspaces, not new Core tools. TOOLS-10 already owns web browsing and research.

## 0.1.0-alpha.2 — functional Windows alpha accepted 2026-11-03

**Milestone:** project owner accepted the functional Windows development alpha after physical checks covering GGUF with paired MMProj, streaming responses, conversation/image/document persistence, FIFO chat request admission, cancellation and direct Retry. The application displayed `Lu 0.1.0-alpha.2` in the sidebar.

**Limits:** this was **not** a claim of production stability, a signed/SmartScreen-trusted installer, completed native tools or v1.0 security acceptance. Known work included Windows taskbar icon identity, temporary unresponsiveness during model load, image preview/import feedback, signing, worker/backend diagnostics and broader Windows failure/recovery tests.

**Provenance:** [owner-accepted alpha baseline and outstanding gates](docs/LU_ALPHA_01_ACCEPTED_BASELINE_20261103.md). The physical acceptance receipt does not by itself identify the later `main` head or any draft PR as the exact installed binary.

---

## Rules for future entries

1. Add implementation details and source-only work to the **development ledger/PR receipt**, not a released-version heading.
2. Add a changelog item only when the scope was actually accepted and the tested source/build/artifact identity is recorded. Distinguish development milestone from public release.
3. Never retroactively credit an accepted installer with features from later branches.
4. Preserve historical entries; append or correct with provenance, rather than silently rewriting prior acceptance claims.
