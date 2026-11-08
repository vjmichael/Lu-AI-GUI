# Lu IDE DEV-01.4 — app-data persistent unsaved draft recovery

**Status:** source implemented only, same-head native CI and installed crash/restart acceptance OPEN.
**Stack:** DEV-01.3 #123 → DEV-01.4 draft (independent of P05 B2/B3, P06 release and Lu Core).

## Behavior

A dirty CodeMirror source tab receives a **debounced native checkpoint approximately 700 ms after the most recent tab change**. The checkpoint is an atomic, synced, versioned JSON record held in the trusted Tauri user **app-data/draft-recovery** directory, not in the project or a browser storage API. Stored metadata includes native project ID, relative path, verified original saved-file SHA, monotonically increasing editor buffer revision, draft SHA, and draft text. Browser callers cannot choose the vault path.

A fresh random app-session ID is generated for every application launch. A new session **cannot overwrite** a prior-session draft checkpoint, even if the user begins typing before the recovery list finishes loading. The UI reports that the prior session needs review. A user explicitly chooses **Recover into unsaved tab**: the browser obtains a fresh native saved-file preview and checks the original SHA, adopts the precise immutable draft SHA into the new app session, rereads the native saved revision and refuses to overwrite an active dirty editor. Restored content is an unsaved editor buffer; no project file is written until a later explicit revision-checked Save. Users can explicitly discard checkpoints; only an exact SHA-matched record is removed.

The vault validates file path syntax, project identity, record version, hashes and maximum size; rejects unknown fields, symlinked vault roots, malformed or tampered records and stale revisions, and limits storage to 64 records of at most 512 KiB text each. Unix vault directory permissions are restricted to 0700 and newly created recovery files to 0600. Windows uses app data and inherited native access control; full ACL inspection remains an OS acceptance gate. No model tool or shell executes source data.

## Limitations / outstanding work

- Checkpoint debounce leaves a **bounded recent-edit loss window** during sudden power loss or a hard process crash; do not claim zero lost keystrokes without additional true OS durability proof. A successful checkpoint is synced before reporting native success.
- Draft text is local user-app-data plaintext; full encryption-at-rest, per-user privacy review, disk-full resilience and backup/export policy are separate readiness gates. There is **no** browser localStorage or automatic cloud transfer.
- When the saved file changed since checkpointing, recovery is denied and the original checkpoint is retained for explicit manual comparison; no silent overwrite or stale merge is performed.
- If a source file is moved/trashed by an external application, its recovery record retains the old path until a human resolves the conflict. The DEV-01.3 UI refuses mutations while an editor tab is open.
- Exact-head Windows 2022 / Ubuntu 24.04 native Rust, Node, Tauri source CI is required; genuine installed NSIS/deb quit/restart/process-kill and file-system fault injection is **not yet accepted**.
- DEV-01 split editor views and large-project indexing/performance follow in their own source branches; signing/legal review, P07, P03/P04 and historical CI gates remain separately OPEN.
