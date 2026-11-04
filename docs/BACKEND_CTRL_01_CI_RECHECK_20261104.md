# BACKEND-CTRL-01 CI recheck after public visibility — 2026-11-04

This branch remains a draft and the accepted alpha baseline is unchanged.

## Known evidence

- The repository owner changed `vjmichael/Lu-AI-GUI` to public; the GitHub repository API now confirms visibility `public`.
- Exact backend-control source head before this check: `3025eeb0e7cc5e088568460c57f1a7dccdc832e4`.
- The base TOOLS-08 commit `62992c9ac20bed96e815ba732c94ebf186899a1e` had passing CI.
- Several prior backend-control commits had workflow runs marked failure, but jobs had zero registered steps and logs failed with `BlobNotFound`. The exact failure cause is not proven; private Actions eligibility is suspected.
- A rerun of REPO-01 `41876758866` was accepted after the visibility change, but it initially remained queued.

## Fresh admission and verification

This documented, source-locked branch push deliberately triggers new GitHub Actions workflows **after public visibility**. Do not mistake a queued job for executed tests or mark any BACKEND-CTRL gate complete without exact-head job steps and passing results.

Acceptance order:
1. Source identity and frozen source lock.
2. Windows/Linux Rust compilation, Python vLLM protocol tests and native_stream_loop regressions.
3. Desktop build/Windows native tests.
4. Real llama-server/GGUF and vLLM/accelerator validation are separate gates even when CPU fake-backend CI passes.

No CI error was attributed to Rust source before log evidence exists.
