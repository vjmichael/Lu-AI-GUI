# LU-AUTONOMY-01 — Broad access without independent authority
Date: 2026-11-03
Status: Owner-confirmed correction to earlier sandbox-first audit. Implementation still incomplete.

## Correction
The previous audit incorrectly elevated sandboxing into a universal product requirement. **It is superseded.** Lu has broad native access to its host Windows system. An OS permission or available resource does not, by itself, confer authority to act. The human's authenticated request and explicit durable delegation determine authority. Do not make a mandatory AppContainer/LPAC, isolated filesystem, per-task VM, network-deny container, Docker requirement, or workspace-only installation a Lu v1.0 prerequisite.

## What is preserved
- Source-bound, trusted human task origin; untrusted text and model responses cannot mint authorization.
- Tool/effect/target/destination checks at trusted native dispatch.
- Revocable grants and persistent approval records.
- Atomic file writes, snapshots, trash/restore, diff previews, idempotent receipts and UNKNOWN/UNCERTAIN reconciliation.
- Windows Job Object and process supervision **for child lifecycle**, not as a claim of security boundary.
- Optional task-specific sandbox or disposable worktree when requested/appropriate—not the default product model.
- A human-approved route for privileged operations. OS capabilities alone do not grant authority.
- Capability inventory, dependency resolution, local or system installation when explicitly authorized, with verified package provenance and rollback.
- Tooling without mandatory MCP; native Rust workers and Protobuf host contracts.

## Engineering implementation priorities
1. **AUTH-01:** Authenticated, durable task authority, resource/effect bound approvals, host-side recheck on every consequential operation.
2. **TOOLS-01:** Real host-native read/write/search/git/archive/process/SDK operations; allow approved paths beyond a fixed project root. Maintain authorization rules that apply even when Windows allows access.
3. **PROVISION-01:** Detect toolchain capabilities, verify package provenance, request installation grant only when needed, then perform installation appropriate to the approved scope (user-wide or system-wide).
4. **LIFECYCLE-01:** Start/stop/reap native jobs, memory/CPU limits, cancellation, resource-guardian response; not mandatory sandboxing.
5. **JOURNAL-01:** Durable effect intent/receipt, recoverable file mutations, no blind replay of uncertain effects.
6. **ORCH-01:** Persistent goal and dependency graph, automatic safe continuation inside granted authority.
7. **WIN-JOB-01:** Resolve the observed suspended-child/Job API mismatch; run Windows compile and lifetime tests before asserting containment of process children.
8. **REPO-01:** The currently published GitHub source is not yet proven identical to the latest unpublished development archive.

## Required high-level acceptance
- Lu can discover/use host compilers, installed applications, local services and files when authorized, even outside an initially selected project directory.
- Lu may perform repeated authorized edits/tests within the task without repeated approval prompts.
- An injected document cannot authorize destruction, secret export or unrelated system changes, even if Lu's OS token technically permits them.
- Fresh review is required for effects beyond the user's purpose: destructive broad deletion, unexpected external transfers, credentials, driver install, privilege changes and physical device operations.
- Revocation stops newly proposed actions and initiates cancellation/reconciliation of active effects.
- Process crashes and power loss do not silently repeat ambiguous effects.
- Optional disposable testing is a tool choice, not a global limitation.

## Distinction
**Access:** which host capabilities Lu can reach.
**OS permission:** which actions Windows could technically allow.
**Authority:** which actions the authenticated human actually delegated.
**Authorization decision:** what Lu's trusted host permits for the exact operation.
**Recovery:** how Lu prevents or repairs accidental consequences.

Access and permission must never be mistaken for authority.
