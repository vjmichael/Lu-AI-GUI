# FIN-04B — Native source-bound test symbol candidates

The registered-project host now scans a *selected previously discovered
saved test filename*, using its existing capability-scoped, bounded
UTF-8 source reader and SHA-256 revision. It recognizes conservative
static Python `test_*`, Rust `#[test]`, JS/TS `test/it/describe`,
and C++ `TEST/TEST_F` patterns, never evaluating imports or scripts.
Only 128 symbol names and line numbers are returned; source text stays
inside the host and the scan never grants execution authority.

The standalone Test Explorer can request a selected saved source's
candidates and display its observed revision. It cannot run a single
test or treat symbols as framework-discovered test IDs. A source revision
change requires a new native observation. Production debugger Step
Over/Breakpoints/Variables remains disabled outside the existing
developer-only Windows dialog until independent OS approval and
debugger owner acceptance is completed.
