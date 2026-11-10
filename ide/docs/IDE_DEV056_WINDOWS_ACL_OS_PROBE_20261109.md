# IDE-05.6 Windows AppContainer positive OS write-denial fixture

This is a first-party, disposable **test-only** Windows kernel probe,
not a feature in Lu's installed IDE. The Cargo feature
`ide056-ci-acl-positive` is not enabled by the default package, Tauri,
NSIS or Linux builds and does not register a WebView command. The
production `uv_sync_offline` Windows worker still rejects
`NoContainment` before a profile is created.

The native Windows source-gate example:
- creates a fresh AppContainer profile with an exact package SID;
- stages a private native acceptance executable and fixed source under
  that profile's `LocalState`;
- applies protected explicit DACLs for SYSTEM, the host user, and the
  exact AppContainer SID (read/execute on source/executable, private write
  on output);
- audits the source tree with the original strict
  `inspect_staged_readonly_tree` observer;
- launches the exact staged binary as a **real AppContainer** with zero
  optional capabilities and an atomically assigned kill-on-close Job;
- checks kernel job membership and child token AppContainer state before
  resuming, waits for the process to terminate, then verifies the child
  independently read source bytes, failed writes, rename and delete, and
  succeeded in writing only to its private output;
- independently re-reads original source bytes and output evidence after
  termination. No fake child output or source-only acceptance is allowed.

**Boundaries:** Even if this probe passes, it is not production Windows
package authorization or independent human consent. It does not prove
hostile effective access through every reparse/owner/DACL/MIC route,
SCM-owned recovery custody, stale plan changes during an actual native
approval dialog or physical installed-app human refusal. Those tests
must remain open until separately accepted. In particular the staging
owner may retain implicit `WRITE_DAC` rights and the package's
mandatory integrity label is not independently attested by this
first fixture. Never remove the existing `NoContainment` early denial
on this fixture alone.

**Evidence:** record the exact SHA and only mark the specific native
write/rename/delete/private-output outcomes PASS after seeing the
actual Windows GitHub Actions exit status and log markers. Never
promote if the CI example only compiled, or if the run fails after
a positive marker.

## Extended native child-owner and mandatory-integrity negative probes

The disposable Windows child must additionally obtain its own native
`TokenIntegrityLevel` from the OS and prove its integrity RID is **Low
or lower**. It must independently request `WRITE_DAC` and
`WRITE_OWNER` through `CreateFileW` against the original source
file and receive **kernel denial** for each. A child that can alter its
own source security descriptor must fail acceptance even if ordinary
`fs::write` and rename failed. These assertions run inside the real
restricted process, not against synthetic Windows ACL strings.

The native source job emits a separate
`IDE056_WINDOWS_REAL_APPCONTAINER_LOW_IL_AND_OWNER_CONTROL_DENIED=PASS`
marker only after the child and source/output verification return
success. It is NOT an assertion about an explicit file MIC SACL
label, the actual installed protected SCM service, restart custody or
physical one-use human approval. Those remain mandatory independent
positive gates, and production Windows `uv_sync_offline` stays
`NoContainment` denied.
