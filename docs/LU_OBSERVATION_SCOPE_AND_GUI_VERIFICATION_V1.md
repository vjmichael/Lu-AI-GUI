# Lu v1.0 — selective observation and GUI build verification

Status: OBSERVE-SCOPE-03 and OBSERVE-GUI-VERIFY-04 are native source contracts, NOT a functioning OS screen capture or self-building agent.

## Selective observation

One trusted OS observation broker supports a typed primary target selection:
- One or several specific window instances;
- One or several selected applications;
- One or several physical monitors;
- One or several virtual desktops (for window-membership filtering).

Optional application, monitor and virtual-desktop filters are INTERSECTIONS. Choosing application A on monitor B does not allow everything on monitor B. Unselected processes, windows, monitors and workspaces are denied. Scope widening requires fresh user selection and consent. Identity binds to native process lifetime and window creation, not just PID or title; verify again after a restart, hotplug, window creation or workspace change. All grants expire. Protected windows and other logon/secure desktops are excluded.

Screenshots need additional explicit permission, trusted OS picker selection, mandatory redaction and ephemeral buffers; a semantic UI grant does not grant screenshots. A full-monitor screenshot cannot be used as an application-only observation, because unrelated applications could appear in it. Observation grants no click/keyboard execution, file changes, network, arbitrary shell, build, installation or learning authority.

Windows Graphics Capture supports window/monitor picker selection. Windows virtual-desktop APIs provide window desktop membership and current-workspace checks, but do NOT enable hidden/offscreen desktop capture. Lu never changes the active desktop for monitoring. The Linux xdg-desktop-portal ScreenCast API supports selected window and monitor sources, including multiple sources where implemented. Linux virtual-workspace membership varies by compositor; if unsupported or unverified, disable that scope rather than falling back to unrestricted capture.

A physical monitor is not a virtual desktop and neither is a separate Windows login or privileged secure desktop.

## Lu builds Lu: intended vs actual

1. Lu IDE's current authorized development ledger defines the intended source commit, expected controls, visual layout and native outcomes.
2. The existing supervised IDE worker builds an explicitly approved source snapshot and records the actual build artifact SHA-256; Observe & Learn does not create a second agent worker.
3. A trusted host starts the approved test instance and attests the running process, window and exact artifact.
4. The selected scope observer gathers structured UI/accessibility observations, optionally redacted visual checks, and independent effect/test receipts.
5. The native GUI verification module compares intended checks to the actual running artifact. PASS requires every check to be independently verified with the right proof type; an independently proven difference is FAIL; missing, conflicting, model-invented or wrong-binary evidence is INCONCLUSIVE.
6. Lu may explain differences and propose a source fix through the already-authorized IDE workflow. New builds, computer control and file changes require separate permission. Human-approved verified lessons may enter existing ANSF memory; no automatic model-weight training.

## Exact implementation boundary

OBSERVE-SCOPE-03 implements source-level scope validation, OS-identity/evidence requirements, snapshot source-type separation, selected window/app/monitor/virtual desktop sets and intersection filters, with native tests. The existing Lu-only semantic pilot now binds to this typed scope and remains limited to Lu itself.

OBSERVE-GUI-VERIFY-04 implements a deterministic, offline evidence comparator. It does NOT invoke a compiler, start applications, capture pixels, automate clicks, or establish that a real OS test passed.

OPEN: actual Windows UI Automation/GraphicsCapture adapter and OS pickers, Linux portal and AT-SPI adapter, real virtual-desktop OS checks, redaction/zero-raw-retention evidence, controlled GUI action execution, independently witnessed tests and installed Windows acceptance. CI source/tests are not acceptance until verified.
