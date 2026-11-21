# Windows AppContainer + Job Object native admission fixture

This is a fixed Windows OS security test, **not an installed IDE build/test
worker**. The only child executable is a copy of the probe itself, staged in
the newly-created profile's private LocalState directory.

The test creates a unique AppContainer profile with **zero network
capabilities**. `CreateProcessW` receives the AppContainer SID through
`SECURITY_CAPABILITIES` in `STARTUPINFOEXW`, starts `CREATE_SUSPENDED`
and does not inherit handles. A private Windows Job Object is created with
kill-on-close, a 128 MiB total job memory limit and three active processes,
then the suspended child is assigned before `ResumeThread`. If any Job
admission step fails, the child is terminated without executing instructions.

The fixed child checks `TokenIsAppContainer=1` and attempts to read a host
secret in the caller's temporary directory, which must be denied. It must
exit with zero, and the parent verifies the Job has zero active processes
before reporting success. The profile and staged test files are cleaned up
on the normal success path. No arbitrary executable, environment or source
path is supplied by the user, LLM or IDE WebView.

**Scope limitations:** this does not yet test restricted network access by
making a real connection, does not package a pinned Rust/Cargo toolchain,
does not verify artifact publication or crash/restart orphan reconciliation,
and does not authorize project execution. An installed Windows production
worker needs all of those gates before exposing a Run command.

CI is in `.github/workflows/ide-appcontainer-windows.yml`. If AppContainer
profile creation or suspended launch is unavailable on the host, the fixture
must fail, not silently run an unrestricted process.
