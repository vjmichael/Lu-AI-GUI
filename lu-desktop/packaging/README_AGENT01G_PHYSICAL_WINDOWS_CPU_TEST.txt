LU AGENT-01G — PHYSICAL WINDOWS CPU ACCEPTANCE BUILD (NOT ALPHA)
===================================================================
This is a CURRENT-SOURCE, unsigned, isolated, test-only Windows x64
installer. It is deliberately NOT the normal Lu R22 installer, v1.0,
a release candidate, or AMD Vulkan performance build.

MODEL ENGINE
- Real source-built pinned llama.cpp llama-server.exe, CPU-only.
- Source revision and exact binary SHA-256 recorded in BUILD_PROVENANCE.json.
- The engine SHA-256 is embedded into the compiled Lu desktop at build
  time and checked again when it starts. This avoids an unreviewed DLL
  or user-selected executable silently becoming an AI backend.
- No GGUF is bundled. Import/select a SMALL local GGUF that you own.
  A large 27B model on CPU will be slow; this package is for functional
  authorization/independent-result testing, NOT model performance.
- A Windows 11 x64 installation with Microsoft Edge WebView2 is needed.

SAFETY / SEPARATION
- Installs only to %LOCALAPPDATA%\Programs\LuAGENT01GPhysicalTest
  (or a manually selected isolated program directory).
- Uses a distinct application identity: org.lu.agent01g.devtest.
- Does NOT automatically reuse or delete your main Lu chat/model profile.
- Do not use irreplaceable Git repositories for the first test. Make a
  small disposable repository. Only reviewed READ-ONLY Git operations.
- No additional model-produced step may execute without fresh review.
- Failed or uncertain operations are NOT automatically retried.
- Installation may be blocked by Windows SmartScreen because the unsigned
  engineering build has not undergone publisher/signing review.
  Do not disable security globally. Only run after reviewing the exact
  GitHub commit and the emitted build provenance.

PHYSICAL ACCEPTANCE CHECKLIST — REPORT PASS/FAIL/NOT_TESTED FOR EACH
A. INSTALL/LAUNCH: note Windows version, app opening, missing DLL errors,
   WebView2 errors and whether Lu starts without an engine-pin warning.
B. LOAD REAL GGUF: import a small GGUF, explicitly load it, ask '2 + 2',
   and verify real model output. Record a model ID/size, NOT private paths.
C. ORIGINAL ANSF TASK: in a fresh chat describe the disposable Git repo
   and request a read-only independent Git HEAD verification. Confirm
   the goal remains visible in the original conversation/memory view.
D. MODEL PLAN REVIEW: use 'Draft plan with Lu'. Confirm a review-only plan
   is proposed, does not immediately execute Git, and human review of
   the exact plan revision is required.
E. FRESH AUTHORIZATION: approve one eligible INDEPENDENT_CHECK step,
   click 'Review and run one step', select your disposable local repo in
   Windows' native picker, and verify a SECOND native confirmation shows
   exact plan, revision, step, operation and source. Cancel once and
   confirm no operation was treated as authorized.
F. REAL WORKER/ORACLE: explicitly approve and run; verify the loaded
   GGUF chooses exactly one reviewed read-only Git capability; inspect
   host progress and independent verified-result note. Compare reported
   HEAD with `git rev-parse HEAD` on the disposable test repo. If Lu
   claims success without source proof, mark FAIL and save diagnostic logs.
G. RECOVERY/REPLAY: restart after a verified step and ensure it remains
   historic; do not re-authorize the same old claim automatically.
   Cancel or interrupt a NEW step and confirm it is UNCERTAIN/needs
   review rather than silently replayed. Never repeatedly run a
   failed/uncertain effect simply to force a PASS.
H. ISOLATED UNINSTALL: close Lu first, uninstall the AGENT-01G test
   installation; verify program files are gone but your main Lu data and
   original GGUF locations were not removed.

RECORD EVIDENCE
- Save screenshots of native approvals, model-selected tool activity,
  host independent evidence text and errors. Exclude private paths,
  API tokens, memory contents and sensitive documents.
- The provenance JSON identifies exactly which app + engine you tested.
- Report which checklist items PASS, FAIL or NOT_TESTED, and any
  crashing/hung worker PID/process names. Avoid speculative claims.
- A small local acceptance receipt can be written manually using
  docs/acceptance/LU_AGENT01G_PHYSICAL_WIN_CPU_20261108.json.template
  in the source repo.

GATES NOT CLEARED BY THIS INSTALLER ALONE
* This CPU build does not prove AMD Vulkan/ROCm GPU offload.
* A passing human UI run does not prove Windows Job Object descendant
  process containment or Linux cgroup-v2 protection.
* One verified Git call does not prove generalized FFprobe or
  utility/specialist application routing.
* The native GitHub exact-commit compiler and independent model tests
  remain separate; this is not a release or code-signing approval.
* No automatic multi-step continuation is permitted.

SOURCE / BUILDER: https://github.com/vjmichael/Lu-AI-GUI
