# IDE 6B-05 — installed commit/amend and recovery acceptance

**Source:** `ide/group06b-advanced-staging-20261110`. **Disposition:** not closed until separately witnessed native Windows NSIS and Debian runs pass every OS-dialog case. Keep the installed acceptance result attached to an **exact source SHA**, package SHA-256, installed executable SHA-256, and process IDs before/after restart. This matrix is not an authorization to merge, publish, or ship a signed release.

## Boundaries already present

- 6B-03 protects selected-hunk staging with one-use OS authorization, exclusive index publication, durable journal and read-only recovery.
- 6B-04 requires an **independent** OS authorization for a new local commit, and two distinct OS confirmations for an explicit local-HEAD amendment. Cancellation is valid only until the irreversible branch-ref commit point. Recovery never automatically retries a commit, deletes a foreign Git lock, or force-updates a ref.
- Previewing, clicking a WebView button, source tests, an Actions runner, or a simulated dialog do not constitute consent by the human owner.
- Only self-created disposable local repositories are used. Never point an acceptance test at the user's working repository, another IDE project, a linked worktree or a remote.

## Native CI scope — independently check the run before making any claim

`.github/workflows/ide-group06b-native.yml` runs the Rust/Node/Tauri source gate on Windows/Linux and the 12 real-Git oracle unit tests on both. It also builds the actual unsigned Windows NSIS and Linux Debian packages using exact-source, collected third-party notices, installs those packages on isolated runners, starts the installed app twice, verifies the executable and source Git fixture remain intact, and uninstalls. The packaged Windows and Linux artifacts, license inventory and digests may be available from that exact run for physical-machine testing.

The scripted installed smoke does **not** click a commit/amend button, approve or refuse a dialog, assert installed Source Control accessibility, witness a human, or interrupt a ref write. CI is not the 6B-05 physical acceptance certificate.

## Independent Git oracle

`ide/app/fixtures/g06b05_git_oracle.py` uses the system `git` CLI as a **test-only independent witness**. The IDE itself does not run it. A fixture is a new isolated `main` branch with two separated edit hunks, another tracked file, and an untracked test marker. The oracle denies use on arbitrary real repositories, linked gitdirs, remotes, or unusual branches.

```bash
# Run from the repository root. Choose a never-before-used EMPTY destination.
python ide/app/fixtures/g06b05_git_oracle.py fixture --directory /tmp/g06b05-create-001 --mode create
# BEFORE an attempted staging operation, require unchanged clean index.
python ide/app/fixtures/g06b05_git_oracle.py snapshot --repo /tmp/g06b05-create-001 --manifest /tmp/g06b05-create-before.json --mode create --expect-index initial
# After a refused/cancelled stage, before new approvals:
python ide/app/fixtures/g06b05_git_oracle.py check --manifest /tmp/g06b05-create-before.json --outcome denied
```

The stage-success path uses a **new** fixture: register its directory through the installed IDE's actual native OS folder picker, review both hunks, **select only the first** and approve the native stage dialog. Then make an independent fresh `snapshot --expect-index selected` to prove the staged blob is exactly the first hunk (the second must remain unselected). Take that snapshot *before* asking the IDE to create a commit:

```bash
python ide/app/fixtures/g06b05_git_oracle.py snapshot --repo /tmp/g06b05-create-002 --manifest /tmp/g06b05-commit-before.json --mode create --expect-index selected
# Physically execute only the reviewed commit in the installed IDE and accept its distinct native OS dialog.
python ide/app/fixtures/g06b05_git_oracle.py check --manifest /tmp/g06b05-commit-before.json --outcome created
```

For amendment, make a different `fixture --mode amend`, stage only the first hunk through the installed IDE, and `snapshot --mode amend --expect-index selected`. After reviewing and accepting **both** native amend dialogs (second says FINAL HISTORY-REWRITE CONSENT), require `check --outcome amended`. The oracle checks the new commit has the exact selected index tree, that the original commit still exists in Git object storage, the branch ref moved only as expected, the original parent was preserved, and the full unrelated index/worktree/ref inventory has not changed.

For commit/amend refusal, cancellation, or a verified interruption *before* the ref publication fence, use a **new** staged fixture snapshot with `--expect-index selected` and check `--outcome denied`, `cancelled`, or `interrupted_prepublication`. These outcomes demand bit-identical Git index bytes and unchanged original branch ref. For foreign-lock denial, place a nonempty test-only `.git/index.lock` in the disposable fixture, snapshot with `--expect-foreign-lock`, and verify the exact foreign lock bytes are preserved. A published commit is never rolled back to make a cancellation test pass.

The independent oracle's local self-tests have 12 cases, including real Git create/amend and hostile mutations of the second hunk, an unrelated file/index path, a second commit, a foreign lock, and a non-fixture repository. They exercise the oracle's ability to **reject** corruption; they are not proof of a user's OS interaction.

## Physical installed matrix — independently repeat on Windows and Debian

| ID | Operator action in actual installed Source Control | Required independent result |
|---|---|---|
| G06B05-01 | Open project A via native folder picker, switch to B, return to A; restart installed app | Correct project/branch/plan on return, no stale grant or data; process PID changes, installed executable SHA unchanged |
| G06B05-02 | Create a fresh fixture; decline the **stage** OS dialog, then cancel a separate stage attempt | No index/ref modifications; second hunk and other files untouched; no persisted approval |
| G06B05-03 | Fresh fixture; approve stage of first hunk only, then refuse the distinct **commit** OS dialog | `snapshot selected`, then `check denied`; branch and original parent unchanged; stage remains reviewed only |
| G06B05-04 | New selected staged fixture; approve distinct commit OS dialog | `check created`; one new local commit, selected-only tree, unchanged worktree, unrelated index/refs, original history retained |
| G06B05-05 | New amend fixture; refuse first amend dialog; separately refuse **FINAL HISTORY-REWRITE CONSENT** | Both denials preserve old HEAD and original commit; no write grant survives dialog or restart |
| G06B05-06 | New amend fixture; approve **both** OS dialogs | `check amended`; exact staged tree, unchanged predecessor parent, old commit object retained; no remote push |
| G06B05-07 | Issue native cancel while approval is pending; repeat with the process killed *before ref publication*; relaunch | `check cancelled` or `interrupted_prepublication`; original HEAD, index, unrelated bytes intact; journal classified, never retried automatically |
| G06B05-08 | Kill/restart only *after* a published local commit or amend, inspect recovery without reapproval | Independent `check created/amended` still passes; old authorization cannot be replayed; no duplicate commit or forced ref move |
| G06B05-09 | Place foreign `.git/index.lock`, attempt protected stage/commit/reconcile | Refusal and byte-for-byte foreign lock preservation; no implicit lock deletion |
| G06B05-10 | Uninstall NSIS/Debian and relaunch after reinstall with retained user app data | App data and project records preserved, but no active operator grant restored; executable and package digests recorded |

A process kill while an OS dialog is merely open proves only *pre-publication* interruption, **not** a crash between durable `publishing` and `ref_published`. For that narrower crash window, require a separately reviewed native fault-injection acceptance fixture and real observed journal/branch recovery before declaring G06B05-07 complete. Do not fabricate a synthetic positive OS dialog response in CI.

## Evidence to retain, and closeout rule

For **each OS and each matrix case** retain: exact source commit SHA; NSIS/.deb SHA-256; installed binary SHA-256; actual native window and OS-dialog title(s) observed; actual user approval/refusal/cancel; project ID and Git HEAD before/after; pre/post independent oracle output; journal status; native process PID before/restart; and abnormal termination or recovery result. Include screenshot and OS accessibility evidence when collected; do not put captured approval tokens or private filesystem contents into an uploaded receipt. Operator evidence must describe the real interaction, not repeat the Rust unit-test assertion.

**6B-05 stays OPEN** until both real installed OS pathways, the interruption/restart matrix, and independent Git-source integrity have actually passed at one reconciled exact source identity. A green compiler or package smoke run cannot override this condition. After 6B-05, proceed to Group 6C/6D only as separate development; unresolved interrupted operations and foreign lock ownership belong to Group 6D rather than being silently auto-repaired here.
