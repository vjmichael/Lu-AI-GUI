# FIN-05B — Real vcpkg.json declaration draft, not resolved packages

The existing project-scoped native PackagePreviewRequest can now produce
an exact reviewed vcpkg.json Add or Remove draft after *fresh native*
preview SHA and evidence-fingerprint verification.

The edit is limited to direct dependency entries. JSON structure is parsed,
unknown repository commands are not evaluated, duplicate references are
denied and existing fields preserved by the JSON model. Version `>=`
requires an existing baseline and is explicitly reported as a **minimum
constraint**, not an exact locked version. Native review of the full
proposed text calculates a source-diff, refuses truncated previews and
rechecks the original file revision. No package-manager worker launches,
lockfile resolution, registry network, file write or approval occurs.

Actual write (with fresh one-use human confirmation and another source
revision check) is a distinct follow-up. vcpkg/Conan native tool execution
and third-party license/provenance remain open. No native, installed or
CI tests were performed.
