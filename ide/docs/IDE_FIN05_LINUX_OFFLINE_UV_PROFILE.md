# FIN05 — Opt-in Linux protected uv Sync admission

Native source feature `fin02-linux-offline-uv` builds the existing
`native-package-uv` worker on top of FIN02's reviewed alpha profile.
It is Linux-only: a Windows compile using the integrated profile plus
package uv now refuses before packaging due to missing actual source
AppContainer ACL/kernel hostile-child acceptance.

The FIN02 build script requires a **sixth Linux pin**
`LU_IDE05_LINUX_UV_SHA256`, independently rehashes staged
`resources/build-tools/linux-runtimes/uv_sync_offline` and installed
root-owned `/opt/lu-ide/runtimes/uv_sync_offline`, refuses any mismatch,
and embeds only the real matched hash. The offline prep tool accepts
`--linux-offline-uv` and an independently reviewed exact hash for this
additional runtime. Debian maps the uv root to the existing fixed
supervisor path. The native worker retains its own root/mode audit,
re-verifies the tree immediately before Bubblewrap, and requires the
already separate native user OS approval, durable supervisor journal,
cgroup-v2 custody and no-network worker.

The profile never runs a package manager from a model-selected command,
does not enable Add/Remove, and cannot install into the original source.
It is not **accepted** until a real bundle, wheelhouse closure, physical
delegated Linux worker, native approval/refusal, interruption recovery
and installed Debian package prove actual results. No binaries,
CI or physical native tasks were run in this source change.
