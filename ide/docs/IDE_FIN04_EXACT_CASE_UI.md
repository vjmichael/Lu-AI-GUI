# FIN-04 exact Rust case UI

Native saved-source candidates remain explicitly unverified. For a
bounded Rust `#[test]` candidate only, the Test Explorer offers a
separate user action to request the protected OS-owned Cargo worker.
The UI verifies the saved test revision and the current native discovered
`Cargo.toml` revision; it may not send a command string or tool path.
The native backend independently rechecks both before grant.

Original case output is accepted only when the reporter name exactly
matches `cargo_libtest_exact_filter`, the job/source/manifests/receipts
correlate and an actual case record is observed. Cargo exiting 0 without
one observed exact case does not show a passing test. No source edit or
automatic task acceptance occurs. Static discovery remains incomplete.

Native compiler/installed OS execution and human approval/refusal
matrix is not yet proven.
