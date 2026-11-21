# A3 Public GitHub Evidence and CI (2026-11-05)

Status: **implementation committed; Windows native/desktop checks passed; full matrix pending**.

- Source: `ide/host/src/github_live.rs`, native `ide_compare_live_github`, typed `git.compareLive` capability, and explicit human Source control **Check GitHub** control.
- Current public GitHub origin branch is queried by fixed TLS REST endpoint, with no credentials, redirects, repository mutation, Lu connection, or automatic network requests. Host response is bounded and checked.
- Compare **local HEAD**, locally stored upstream SHA, and **time-stamped GitHub branch HEAD** separately. An unequal SHA is not proof of ancestry; local dirty/untracked changes remain independent.
- [CI 42128252074](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42128252074) Windows failed native test E0505 (dropping Git repository while borrowed tree remained live). Fixed by releasing tree first in `host/tests/git_status.rs`.
- [CI 42128928605](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42128928605) Windows host tests and Tauri check passed at last inspection; Linux/frontend still queued.
- Latest tested implementation SHA `b39c626752e84ccc65f876283bc8b5ec3bd6c505`. Installed Windows UI and private/authenticated GitHub remain unverified.
