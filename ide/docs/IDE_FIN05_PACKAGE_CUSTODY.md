# FIN-05 — Protected package lifecycle source

Native private output audit rows are now validated before desktop
presentation. Foreign/duplicate jobs, unknown terminal states,
unverified SHA-256 fingerprints and any claim of project installation or
automatic retry are rejected. Exact state names match the existing
native package_lifecycle implementation. This is a view of its original
durable OS-journal-correlated evidence, not independent provenance.

Offline uv Sync and separately approved private cleanup also require
all FIN-01 global custody flags to be explicitly clear: no foreign
running worker, no unreconciled native job, and no occupied supervisor.
An available pinned uv runtime is not permission to run Sync.

Still missing: general package Add/Remove/Upgrade execution, vcpkg/Conan
protected worker adapters, complete third-party dependency closure,
Windows AppContainer uv source ACL proof, and physical installed
consent/recovery. None is silently enabled. No CI, PR or native test
run was performed.
