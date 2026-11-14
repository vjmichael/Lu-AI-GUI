# IDE Group 6F — Foreign-lock safety continuation (2026-11-13)

## Scope

The consolidated Group 6F recovery veto checks nine original local Git operation domains and separately classifies uncertain remote publication outcomes. This continuation closes a narrow **source-level detection gap**, not Group 6F acceptance.

## Demonstrated review gap and change

Before this change, the consolidated snapshot checked `.git/index.lock`, `.git/HEAD.lock`, and `.git/packed-refs.lock`, but did not inspect the **current branch's** `.git/refs/heads/<branch>.lock` or `.git/config.lock`. A competing ref/config writer could therefore leave these locks present while the aggregate `foreign_git_lock_present` remained false.

The source now checks both additional lock paths. The branch path is derived from the repository's actual attached HEAD ref, never a renderer-supplied path. A new native fixture creates each lock independently, verifies the consolidated new-operation preflight is denied, and verifies original lock contents are not deleted. No new Git execution, remote write, automatic recovery, lock deletion or approval surface is introduced.

**Commit:** `1432f5ccc1d4d8f9b7ceea08cf256c8fe348eb74` on draft PR #230's `ide/fin03-6e05-protected-remote-preflight-20261113` branch.

## Acceptance status

- Source implementation committed. **Compiler and original Windows/Linux native regression at this new SHA have not been verified**. Prior passing receipts at earlier SHAs cannot certify this commit.
- 6F consolidated nine-domain acceptance and interrupted remote-publication recovery still require original exact-head negative and restart fixtures.
- 6E-05B still requires actual credential-provisioned, host-only GitHub branch/ref and evaluated-rules reads under Windows Credential Manager/Linux Secret Service. Unit-fixture and OS-backend compilation are not live API acceptance.
- 6E-05C remains separate and blocked pending genuine native one-use operator authorization, atomic live remote compare-and-swap, fsynced write receipts, post-write independent readback and competing-writer/restart denial.
- 6G source/native/installed consolidation remains open.
- FIN-04 C2 original simulated benign SCM/Job/WFP receipt tests are separate; no physical approval, LLDB, W2-D, installer, merge or release authority.

## Next engineering steps

Run exact-head original native Rust and Windows/Linux integration for this source, reconcile failures without weakening recovery checks, then extend original 6F interrupted-state and remote uncertainty fixtures. Work on other independent backlog tasks while GitHub operations are pending.
