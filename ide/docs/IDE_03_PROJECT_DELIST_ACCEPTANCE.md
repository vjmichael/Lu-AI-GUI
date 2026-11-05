# IDE-03 R3.2 — Remove project from IDE (catalog-only)

**Decision:** User requested a "delete project" option but clarified that the action should merely **delist the project from the IDE's registered-project area**, not delete files from disk.

## UX contract

- **Label:** "Remove from IDE" on every registered project card (open, closed or currently unavailable).
- **Safety confirmation:** "Remove <project> from Lu IDE?" followed by an explicit statement that the **original folder and every file on disk will be kept**, and that the user can add it again with Open folder.
- If any tabs for that project have unsaved changes, append an explicit warning that those unsaved buffers will be discarded.
- **Cancel** must make no changes to the registration, tabs, or real filesystem.
- Only after the native operation succeeds may the UI drop project-specific tabs and update the list.
- Do not run the unregister action while another project operation, browser operation or save is pending.
- Distinguish **Close** (ends the active project but keeps it registered), **Remove from IDE** (removes catalog metadata), and a hypothetical future **Delete on disk** (not offered by the IDE and not implied by Remove).

## Native implementation

- `ide/host/src/lib.rs::IdeWorkspaceRegistry::unregister` accepts only a validated opaque project ID. It serializes with IDE file saves and atomically writes the updated **catalog** before changing live registration/active state.
- **No file/directory delete, trash, rename, or recursive traversal of the project root** exists in the unregister code path.
- Unregistering does not require the source directory to be present or readable, so it works for stale/missing projects.
- `ide/proto/lu/ide/v1/workspace.proto` adds `UnregisterProject` on tag 7 (the existing v1 messages remain compatible and path-free).
- `ide/app/src-tauri/src/lib.rs` adds `ide_unregister_project`, separate from source-file saving.
- Project folders remain accessible to other applications; use the trusted OS picker to register the same folder again.

## Verification plan and evidence

Native Rust tests in `ide/host/tests/lifecycle.rs` verify:
1. Unregister an open project; files and folder remain; project disappears from the catalog across restart; folder can be re-added.
2. Unregister a closed, missing project without touching the filesystem.
3. Simulated catalog-persistence error leaves the original record and active state unchanged.
4. The bounded, path-free Protobuf `UnregisterProject` operation returns an empty registration list but retains source files.

Frontend `ide/app/tests/standalone.test.mjs` asserts the command is allowlisted, the UI uses an explicit confirmation, and the native unregister code has no project-directory delete API.

**CI status:** Consult the exact revision in [standalone IDE workflow](https://github.com/vjmichael/Lu-AI-GUI/actions/workflows/ide-standalone.yml); do not inherit results from R3.1.

## Manual Windows checks for R3.2

- [ ] Register a disposable project with a recognizable file; click **Remove from IDE**, choose **Cancel**; registration and file remain.
- [ ] Choose **OK**; project disappears from the registered list, but file is still present in Explorer.
- [ ] Restart the IDE: removed project remains absent, and the original directory remains intact.
- [ ] Re-add the same folder with the OS picker; it registers successfully.
- [ ] Remove a closed project and an unavailable/moved project.
- [ ] Open an unsaved edit, request removal, choose **Cancel**; draft remains. Confirm again, accept; draft is discarded and original saved file remains unchanged.
- [ ] Verify **Close** still leaves the project registered and does not trigger filesystem deletion.

**User-facing release acceptance remains pending** until these checks are performed on a Windows desktop.
