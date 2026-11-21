# FIN-04 — Single native Rust libtest invocation

A dedicated Tauri command selects only a previously discovered saved
Rust test symbol and binds the exact source content revision and
`Cargo.toml` manifest SHA. The native host re-reads both, seals the
full tracked source snapshot, rehashes the selected source inside the
frozen snapshot and constructs a strictly validated immutable
`rust_exact_case:<name>` JobPlan recipe.

The pre-existing OS-supervised Cargo worker on Windows/Linux recognizes
this fixed subrecipe and runs `cargo test --locked --offline --manifest-path
Cargo.toml <name> -- --exact` with **no shell**, restricted execution
resources, job-scoped OS permission and a fresh native human confirmation.
No dynamic executable path, cargo flags, project script, Python import or
Git ref mutation crosses from the renderer.

The original test reporter accepts case events only matching the exact
selection. A Cargo exit code of zero with no observed matching case
does **not** mean the selected test passed; original case output remains
required for presentation. Static discovery is never framework-level
test enumeration and a test named inside multiple binaries can still
require independent disambiguation. Installed UI integration,
physical OS approvals, cancellation and restart acceptance remain open.
