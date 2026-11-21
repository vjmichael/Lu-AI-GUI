# IDE B8 — explicit clean background tab reclamation

## Problem

B5 caps working sets at 48 open tabs per project / 96 globally. Closing 45
clean background tabs by hand is undesirable, but silently evicting CodeMirror
states can lose Undo histories, unsaved work, private draft identity and
user context. The B7 project view must also survive safely.

## Behavior and boundaries

The new `Close clean tabs` button only appears as an explicit user action
in the editor toolbar. It is disabled while native browsing, Save, proposal
review/staging or draft recovery is busy, and on fewer than two project tabs.

The pure planner `planCleanBackgroundTabClosure` returns keys only; it does
not read the filesystem or alter editor/native state. A candidate is eligible
only if all are true:

1. The tab is in the **currently registered/selected project** and is not
   the primary or secondary visible editor tab.
2. The tab is not marked dirty, its current React draft equals its saved base,
   **and its actual CodeMirror EditorState document independently equals that
   saved base** at the time of the button action. Its tab buffer revision must
   still be **zero**: Undo-back-to-clean and already-saved edited tabs retain
   meaningful Undo history and are not eligible.
3. It is not involved in a native Save, an in-flight private checkpoint or
   pending checkpoint debounce, a reviewed source operation, or a live LSP
   worker session.

A missing live CodeMirror state, stale React draft, thrown state read or invalid
planner input **denies closure**. This avoids losing a newly typed source edit
that React has not yet published to the tab list.

The UI then removes only verified keys from the app's in-memory tab list and
editor-state map. B2's private history on disk is untouched; closing a clean tab
does not discard durable checkpoint records. The command does **not** Save,
restart workers, delete project files, unregister folders or grant new native
filesystem authority. Users retain their displayed panes, dirty tabs and
project-switch navigation context.

## Verification

`ide/app/tests/editor_clean_tab_reclamation.test.mjs` adds nine Node cases
covering live dirty state that contradicts stale React, project isolation,
dual-pane protection, missing states, a 48-tab workload and the UI contract
with no native writes.

During implementation seven focused V8 checks evaluated the exact GitHub
source and passed; these are **not** native Node 22/npm or installed Windows
/ Linux acceptance. A true installed memory benefit requires source-bound
B4/B5 OS performance receipts and a later B8 installed editing scenario.

## Source stacking

`ide/b8-safe-clean-tab-reclamation-20261108` stacks on B7
`dbe9bd026a2949e678892815fce36d6d8a07b7b9`, which stacks on B6
`a562ed08ee511fa28faf9aff1c25b89a95d86eaf`.
No original B5 #147 or earlier exact-head checks are overwritten.

## Follow-on work

B9 should focus on **persistent navigation metadata with explicit startup
revalidation** only after agreement on its privacy model and native app-data
lifecycle. B7/B8 intentionally remain RAM-only and cannot silently reopen
source files or recover unsaved drafts on restart. B2's explicit private
checkpoint/recovery remains the sole durable draft pathway.
