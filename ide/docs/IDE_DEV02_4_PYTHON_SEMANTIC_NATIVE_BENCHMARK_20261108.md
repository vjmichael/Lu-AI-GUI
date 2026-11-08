# DEV-02.4 — actual Python semantic providers: BasedPyright versus ty

**Status:** Real CI research comparison IMPLEMENTED, awaiting genuine exact-head Windows2022 and Ubuntu24 results. Python semantic provider **not selected, shipped, authorized or installed**. Ruff still supplies protected diagnostics only.

## Design decision

The user requested a reviewed Pyright-compatible semantic provider rather than pretending Ruff implements completion or Go to Definition. DEV-02.4 compares an **actual BasedPyright language-server candidate** against an independent native ty candidate on **the same immutable synthetic files**. Its scope is capability/latency assessment *outside Lu*. It deliberately does not start these processes from Lu IDE, grant project-read access, bypass protected OS worker, or set the production manifest's `releaseApproved` flag.

Fixed candidate versions observed from upstream on Nov 8, 2026:

| Provider | Exact candidate | Launch only in disposable CI |
| --- | --- | --- |
| BasedPyright (Pyright compatible) | npm `basedpyright@1.40.2` | Existing CI Node 22 invoking exact `node_modules/basedpyright/langserver.index.js --stdio`; lifecycle scripts disabled |
| ty (alternative native beta) | GitHub official `ty 0.0.99` | Original pinned archive SHA-256, only one extracted `ty` / `ty.exe` binary, `ty server` |
| Ruff | Existing native lint/diagnostic-only route | **Not** a semantic benchmark candidate |

Source references: https://github.com/DetachHead/basedpyright/releases/tag/v1.40.2 ; https://github.com/astral-sh/ty/releases/tag/0.0.99 ; https://docs.astral.sh/ty/editors/ ; https://docs.basedpyright.com/latest/installation/ides/ .

This is a **test-only** use of Node and candidate executables in trusted ephemeral CI, not an IDE configuration or a production runtime dependency. No promise that Node may later be shipped without a complete licensing and protected-OS recipe review.

## True protocol benchmark and gate

1. CI checks out exactly the triggering source commit, sets up Node 22 and Python 3.13 on genuine Windows2022 and Ubuntu24 hosts, checks the production manifest is **not approved**, and runs pure unit tests of JSON-RPC framing, URI scoping, 20 deterministic project files and the ty binary digest pins.
2. `npm install` obtains exactly BasedPyright 1.40.2 into `RUNNER_TEMP` with lifecycle scripts off and verifies the actual npm package name/version and language-server entrypoint. Its binary entrypoint/package-manifest SHA are saved in a **candidate-only** JSON receipt. A separate future gate must pin the complete tarball hash and Node runtime/dependency graph before permitting installer distribution.
3. `stage_ty_semantic_candidate_ci.py` fetches only the named GitHub `0.0.99` Win/Linux x86-64 archive, checks the published full compressed-asset SHA256 from the release API, extracts only one bounded executable, and saves its SHA256 in a separate unapproved receipt. No temporary executable is placed in the application or distributed.
4. `python_semantic_native_benchmark.py` speaks actual bounded LSP JSON-RPC over the real provider's stdout/stdin, never fabricates a completion or source location. It sends initialize, initialized, didOpen, hover, same-file definition, completion, didClose, shutdown and exit against **20 unique** source files in an ephemeral project. It rejects cross-file/cross-project definition URIs and missing completion/hover content, measures actual p50/p95 per-method latency, counts real diagnostic notifications, times out and reaps each provider process.
5. Each provider independently must initialize and have a reaped process plus at least **18/20 real hovers, 18/20 real same-file definitions and 10/20 real completions**. Exact passing measurements from CI JSON evidence are required; a submitted workflow is not a benchmark result. Failed or partial results remain in the JSON with separate failure classifications.
6. CI uploads **only fixed candidate receipt and metric JSON** (seven-day retention). It does not publish binaries or declare redistribution, OS containment or installed Lu IDE acceptance.

## Independent security and release gates

- The semantic comparison is NOT protected worker acceptance: it intentionally runs only fixed evaluation binaries under the disposable CI host, not as a Lu application tool. Never use its result to bypass Windows AppContainer/restricted LocalService, Linux root-owned Bubblewrap/cgroup delegation or end-user approval.
- A final **single pinned and reviewed semantic provider** must be chosen based on accuracy/latency and packaging cost. BasedPyright additionally needs a pinned Node runtime, its versioned npm tarball integrity, original license and transitive audit, and an OS-specific protected native one-use LSP recipe; ty requires a reviewed beta stability decision and the same native isolation and provenance controls.
- Both candidates require hostile responses (untrusted URI, spoofed source identity, wrong UTF-16 offsets, stale editor revision), bounded host memory, cancellation/descendant reaping, multi-file navigation authorization and genuine **installed** Windows/Linux UIAutomation/GTK acceptance **after** selecting one. Ruff must remain diagnostics-only until those gates.
- User-deferred publisher identity and final legal distribution signoff remain OPEN. This task does not change `ide/packaging/language-servers.manifest.json` or provide silent `npm install` at runtime.

## Evidence pointers

- Workflow: `.github/workflows/ide-dev02-python-semantic-candidate-ci.yml`
- Real framed client and 20 fixtures: `ide/packaging/python_semantic_native_benchmark.py`
- Original ty checksum/extraction: `ide/packaging/stage_ty_semantic_candidate_ci.py`
- Source unit tests: `ide/packaging/test_python_semantic_native_benchmark.py`

**Accept only exact-head native CI evidence, not a source implementation claim.**


## Real first-head failures and repaired diagnostic precision

The first real [Win2022/Ubuntu24 benchmark #42494557323](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42494557323) FAILED on both OS. This was not hypothetical: both BasedPyright and ty successfully initialized, and both returned 20/20 real hovers and completions. ty returned 20/20 same-file definitions on both platforms. BasedPyright returned 20/20 definitions on Ubuntu but **0/20 Windows definitions accepted by the first exact URI-string matcher**, so its first Windows result must not be reported as passing. ty produced a final `terminalReaped=false` in the original ambiguous benchmark, despite valid semantics. This was not proof of an orphan: the first benchmark conflated any nonzero process exit code with no OS reaping.

Source repair for the next exact-head native run:
- Normalize only valid local `file:` URI paths through OS-native path decoding/canonicalization before deciding same-file identity; remote hosts, query/fragment, cross-file navigation and non-file schemes remain denied. A failed definition now emits bounded nonidentifying response-shape classification rather than source or candidate file paths.
- Distinguish actual process reaping, LSP `shutdown` acknowledgement, `exit` notification, forced OS kill, precise exit code, clean exit and bounded stderr class. **Clean exit is a separate required acceptance gate**; nonzero exits are NOT silently converted into a passing terminal lifecycle.
- Keep the same 20 fixture files, same native source checks, and same 18/20 hover/definition and 10/20 completion acceptance floors. A candidate is not selected from an unaccepted or incomplete benchmark.

The binary test still operates in a disposable CI workspace without a production Lu worker, license approval or installed UI access. New OS receipts are needed before closing this gate.
