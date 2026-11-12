# FIN04 exact-case original reporter integration repair

Corrects the native coordinator's test-report eligibility so
`rust_exact_case:<name>` invokes the actual bounded in-process
framework-report parser. Without this, the OS worker could execute
yet return no test-case evidence even when libtest printed a result.

The Test Explorer now also requires the **Cargo** runtime to be
ready (not merely Node/Python/CMake availability). This still
does not prove that any named test executed or passed: that requires
an original matching native case record correlated to the sealed
source, immutable JobPlan and terminal receipt, and independent
Windows/Linux installed acceptance.
