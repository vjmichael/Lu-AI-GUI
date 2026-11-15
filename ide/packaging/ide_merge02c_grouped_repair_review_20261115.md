# IDE-MERGE-02C — original failure reconciliation and reviewed repair scope

This record is a **source review**, not native, installed, physical, release, or `main` merge acceptance.
The published draft PR #236 / integration branch remains frozen at `f4e8a1a587c4aadd3d7293896324ceb8fa299738`.
Any descendant commit created from this review remains unpublished until authorized exact-SHA batch acceptance.

## Original exact-SHA evidence

- Final pinned LLVM run [#43743661529](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/43743661529), source `f4e8a1a587c4aadd3d7293896324ceb8fa299738`: Windows job #132902112135 **failed overall**, Linux #132902112391 passed.
- Windows original LLDB-DAP test run: **25 passed, 0 failed**, including previously failing `only_native_sealed_project_path_can_form_project_launch_without_arguments`. Pinned LLVM18 compilation also succeeded. The next strict Rustfmt check failed on unformatted host source/tests, including `host/src/source_file_transactions.rs`. No actual installed/release approval was inferred.
- Consolidated original run [#43743660974](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/43743660974): native Git/restart Ubuntu #132902109472 and Windows #132902109711 **passed**; frontend/Tauri jobs #132902109658/#132902109702 and DEV-01 package-build jobs #132902109659/#132902109682 **failed**.
- Four frontend jobs independently saw the identical five original assertions fail in a **475-test suite (470 passed, 5 failed)**. Installed DEV-01 tests never reached OS interaction, restart, or uninstall acceptance.
- Overall automated PR wave at `f4e8a1a587c4aadd3d7293896324ceb8fa299738`: **62 workflows terminal, 22 success, 40 failure**, none active when this repair group was prepared. A workflow-level success is scoped to its original job's actual coverage.

## Narrow source-grounded contract repairs grouped here

1. `dev03_rust_build_contract.test.mjs`: check the current, manifest-digest-bound `ide_build_run_profile` call and current-outcome revalidation instead of historical `ide_build_start`; leave native approval and non-arbitrary command checks intact.
2. `fin04_native_control_evidence.test.mjs`: normalize multi-character Rust operators `!=` and `|=` in lexical source assertions. Continue requiring all-project original journal verification and global cleanup aggregation **before** filtering project-specific presentation, and all existing denial, non-replay, identity, and custody flags.
3. `job_render_guard.test.mjs`: distinguish necessary project-matched editor-tab selection from prohibited unscoped native job receipt rendering. Retain all exact `belongsToSelectedProject` guards and negative native-result assertions.
4. `standalone.test.mjs`: correct the whitespace regex that parses Tauri `generate_handler!` registration, while retaining exhaustive matching of all 144 distinct literal WebView commands against the 164 native registrations.

The source contracts were checked against the actual `f4e8a1a587c4aadd3d7293896324ceb8fa299738` JavaScript and Rust source. **No original negative security assertions were removed.** This unpublished review is not evidence of fresh Node, Rust, OS, or installed test execution.

## Separate demonstrated failures still requiring review before full grouped acceptance

- **Rustfmt:** Original strict formatting fails on numerous historical host/supervisor Rust source and tests. Run genuine pinned toolchain `cargo fmt --all -- --check` after formatting mechanically and inspecting behavioral diff; never bypass the format gate.
- **Cargo locks:** Some native jobs fail `--locked` because `ide/host/Cargo.lock` is absent. CI-resolved lock artifacts are not reviewed immutable release pins. Reconcile deterministic lockfiles and original P06 graph/license checks without lowering `--locked`.
- **P05 source integrity:** run #43743660871 rejected changed historical workflow `.github/workflows/ide-d7-windows-worker.yml`. Preserve the original workflow boundary; do not revise a hash/pin or allowlist without independent evidence and authorization.
- **P06 dependency/legal review:** run #43743661400 rejected `merge02_desktop_delta_unreviewed`. Review the dependency delta, immutable lock graphs, notices, and license provenance. Do not claim release acceptance from CI lock generation.
- **Historical B4 authority boundaries:** old source-specific jobs reject broad changes to worker/source-write authority. Reconcile their original source requirements separately rather than weakening a protected behavior.
- **Physical execution:** effective Windows SCM/AppContainer/WFP, genuine Windows R9700/installed NSIS, delegated Linux cgroup-v2 and kernel custody, legal/release signoff, and independent `main` merge authorization are **not accepted by this record**.

## Required next batch

Hold branch `f4e8a1a587c4aadd3d7293896324ceb8fa299738` stable; inspect this candidate's exact files, resolve remaining genuine Rustfmt/lock/CI policy defects **as one reviewed group**, then advance only by a non-force expected-head update after all relevant branch CI is quiescent. Obtain Windows/Linux/frontend/security/recovery plus real DEV-01 installed tests at the **same new SHA** before recording scoped acceptance. No overlapping CI and no premature `main` merge.
