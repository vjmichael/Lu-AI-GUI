# FIN-04 — Test Explorer UI and production debugger boundary

Source implementation adds an independently labelled Test Explorer panel.
Existing native registered-project fixed-directory discovery supplies
candidate filenames; native reviewed `ide_preview_source_file` is
reused for opening them. There is no direct process launch or inferred
individual-case execution authority. Original in-process native protected
Build/Test reporter case evidence is separately projected through the
existing INT03-E validator; failed-case navigation still goes through
`ide_build_review_test_target` with exact original job/plan/receipt and
fresh saved source SHA verification. Missing or stale output is simply
unavailable; no fabricated test results across restart.

**Production debugger controls remain open.** Developer-only Windows
`dev046d-native-dialog` is expressly compile-denied for release builds;
it must not be rebranded as a production action without a real
one-use OS consent implementation, native protected LLDB stop/step and
breakpoint-hit proof, durable recovery, physical owner acceptance and
release-pin evidence. This slice adds no unsafe debug privilege.

All new tests are source fixtures only; no CI/PR/native or installed
acceptance was executed.
