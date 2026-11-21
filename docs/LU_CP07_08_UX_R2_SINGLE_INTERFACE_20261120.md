# CP-07/08-UX-R2 — one Lu interface, no Basic/Advanced modes

**Owner correction (2026-11-20):** The Basic/Advanced interface concept belongs to NVP, **not Lu**. Lu has one user-facing interface. Its ordinary Chat, Settings, Model Library, History and Project Tasks remain available, with straightforward names and permissions. No global mode switch or separate "advanced" navigation is required.

## What was corrected

- Removed the **Show advanced settings / Hide advanced settings** state and navigation control introduced by UX-R1. The user-facing Settings navigation is General & appearance, Models, Tools & permissions, Browser & research, Memory and Privacy & Observe.
- Kept the actual model engine selection, inference preferences and optional image-understanding check as individually expandable actions within Model settings. They do not form a second application mode, restart a loaded model or grant permission.
- Kept project planning actions available from Tools & permissions and the separate Project Tasks surface. Project evidence hashes remain expandable as **Original source records**, but the project-change review workflow is not trapped inside that disclosure.
- Kept optional two-person troubleshooting in Privacy & Observe, mounted only when the user opens its individual disclosure. The existing both-side native consent, cancellation, revocation and replay protections were not altered.
- Removed the stray main-window unused advanced state, `(advanced)` labels in chat history/source records, the obsolete "Settings → Advanced" document-help direction and the toggle-specific CSS.
- Preserved the UX-R2 plain-language task, memory and model copy improvements already prepared before the owner correction. Reconciled source checks to test the new single-interface contract instead of asserting the now-disallowed toggle.

## Scope boundaries

- **No Rust/native authorization, protocol, model runtime, GGUF, security policy, CI workflow, Windows installer, R7 branch or published commit changed.**
- Original technical source IDs, hashes and host-owned receipts are retained; choosing to view details never authorizes native work.
- Historical UX-R1 documentation records what was implemented then. This R2 correction explicitly supersedes its Basic/Advanced UI decision; it does not rewrite historical acceptance receipts.
- Visible software behavior was verified at **source/frontend level only**; installed keyboard accessibility, assistive technology, physical Windows UI and genuine inference were not run in this qualification.

## Verification

- React frontend build: PASS.
- UI source/independent contracts: 59/59 PASS.
- Single-interface/updated UX and security-focused suites: 48/48 PASS.
- Full Node UI suite: **696 passed, 2 historical failures, 5 skipped** (698 tests executed of 703). Failures are pre-existing P03 workspace-routing and P4 scheduler-priority source assertions; both are out of scope for this presentation correction.
- Previously compiled native scanner fixture was reused from an unchanged earlier source build to exercise the two media scanner tests. No fresh native scanner compilation was claimed.
- No new GitHub CI runs dispatched. Installed/physical Windows acceptance remains OPEN.

## Next gate

**CP-07/08-UX-R2-INSTALLED:** Independently verify the actual single-interface navigation, keyboard focus and screen-reader semantics, privacy disclosures, task reviews and model picker on the installed Windows app after the R7 and UX branches are reconciled. Do not reintroduce Basic/Advanced modes.
