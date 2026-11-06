# OBSERVE-WIN-CAPTURE-05 — Windows native picker and one-frame capture

Date: 2026-11-06
State: **source implemented; Windows compilation, installer packaging, and physical picker/frame acceptance pending**.

## What was added

The separate native package `src-tauri/lu-observe-capture-worker/` invokes the **actual Windows.Graphics.Capture system picker** (windows-capture 2.0.1), initialized with an OS-owned picker window. The user selects ONE app window OR ONE physical display. It uses the Windows Graphics Capture/D3D11 frame callback to read one bounded BGRA frame (max 7680×4320); no recording or on-disk screenshot takes place. The first frame is digested with a fresh per-operation nonce and the mapped pixel buffer is dropped. It outputs only `P\t1\t1` and `D\t<64 lowercase hex>` through the existing BackgroundProcessSupervisor's short progress pipe, then stops capture.

This receipt proves *pixel bytes were read from the chosen WGC source*, but it is **not** a redacted screenshot, proof of which application/desktop was selected, a vision result, or verified GUI success.

Lu Tauri adds `lu_observe_capture_available` and `lu_observe_capture_one_shot`. The user must first start the existing Lu-only Observe pilot, then click **Select window/display · capture one frame**. A second native dialog explicitly warns that choosing a monitor may reveal unrelated windows. Only after affirmative confirmation does the digest-pinned host launch its reviewed worker. The worker executable and argument/nonce are selected by the trusted host, never the model, attachment, webpage or WebView. On cancellation, expiry, chat switching or Stop, the existing BackgroundProcessSupervisor is used to terminate and reap the picker/capture process tree. A 90-second host deadline bounds a stalled picker and records TIMEOUT.

The child is a **native supervised tool process**, not an additional long-lived agent runtime. It has no network, file, mouse, microphone, clipboard or model access by design, and its OS process gets an empty inherited environment. The digest-pinned worker must live beside the compiled Lu desktop executable; the host refuses to run if the expected build-time `LU_OBSERVE_CAPTURE_WORKER_SHA256` or matching installed executable is missing. The production installer must build worker first, embed its verified digest in the host compilation, and package the exact binary beside Lu.

## Native Windows build/verification workflow

The dedicated `.github/workflows/lu-v1-observe-learn.yml` now builds the actual Windows worker from its independent manifest on Windows 2022, records SHA-256, sets `LU_OBSERVE_CAPTURE_WORKER_SHA256` for the Tauri host build, then checks host and existing ObserveHost tests. The worker executable and SHA are uploaded as a short-lived GitHub Actions artifact for isolated acceptance. The worker has a separate dependency graph and currently regenerates its own Cargo.lock during compilation; **a committed frozen Windows worker lockfile and artifact packaging acceptance are still required**. The main Lu desktop Cargo.lock remains unchanged.

## Acceptance tasks that are NOT complete

1. Windows CI compilation for worker and desktop must pass on exact head, then a *physical interactive Windows installation* must show the OS picker, successful one-shot `COMPLETED` receipt and no screenshot file, with cancel/close/timeout cases verified.
2. The user-selected GraphicsCaptureItem from this picker does not expose an independently attested HWND/HMONITOR to the current adapter. The operation is one-off **system-picked window OR display**. It must **not** be mixed into Lu's existing app-only observation scope, and no arbitrary app identity is inferred from a window title. Actual window-versus-monitor identification and OS-verifiable stable process/window IDs are the next security gate before persistent scoped capture.
3. Pixels are temporarily accessible inside the worker and have **not** undergone mandatory sensitive-region redaction. Thus no pixels are provided to a model/ANSF memory or UI preview, and no pixel serialization or disk path exists in the worker. Further redaction (including protected/secure windows), encrypted transient host IPC, independent app identity, and native visual evaluation require a separate review.
4. The current adapter is one-shot, not continuous event/frame streaming and not multi-window/multi-monitor observation in one session. Per-app multiple windows, monitor sets and Windows virtual desktops remain source contracts until verified OS selector adapters are built.
5. Every GUI PASS/FAIL must still be independently correlated with exact source, running binary SHA and an effect/test receipt (OBSERVE-GUI-VERIFY-04). Capturing a frame alone cannot validate a Lu build.

**Release gate remains OPEN:** no claim that Lu can already see the desktop or that self-building GUI automation is complete. This is real native capture source code and a host-supervised execution path awaiting Windows CI/installed verification.


## Optional pinned Windows package-build path

`scripts/package-observe-capture-windows.ps1` builds the reviewed worker on the Windows build host, hashes the exact EXE, stages it next to the compiled Lu binary (non-installer) and injects the digest through `LU_OBSERVE_CAPTURE_WORKER_SHA256`. With `-BuildInstaller`, it invokes the Tauri v2 CLI using `lu-desktop/src-tauri/tauri.observe.capture.conf.json`, an opt-in resource overlay which places the EXE next to Lu.exe inside the NSIS installation. Normal alpha builds do not silently include an unverified placeholder binary. The full installer requires existing inference-engine resources, trusted NSIS/Tauri tools and native installed verification, which remain open. This script is for CI/release engineering and does not ask the end user to compile Lu.

The worker's transitive dependency graph and generated Cargo.lock must be reviewed and committed before reproducible release acceptance. An exact worker digest is recorded in the build; it is never accepted from user/model input at runtime.


## OBSERVE-WIN-06 — pause/revocation hardening (source, native acceptance pending)

The native ObserveHost now routes **Pause, Stop, expiry and conversation switch** through its existing process supervisor before reporting a paused state. Pause refuses to report success while a picker is still active; if process cancellation cannot be verified it revokes the entire volatile observation permission. Stop/revoke now clears the observation lease **before** attempting potentially blocking process cancellation, so a failure to reap an OS capture worker cannot leave a valid observation grant. The supervisor retains its original process-tree deadline and durable task receipt. Native Host regression tests cover stale child references and pause cleanup, but real Windows picker cancellation and installed process-tree assertions remain open.

**Picker source identity caution:** In `windows-capture 2.0.1`, the public `PickedGraphicsCaptureItem::try_into()` converts OS picker selections to `GraphicsCaptureItemType::Unknown` even for a valid selected window or monitor. This generic wrapper is *not* proof that a user picked a specific app or that a frame came from a monitor. Do not attempt to classify its `Unknown` variant as a capture failure or accept it as a verified window/process identity. App-only long-lived observation requires a separate host-owned HWND/process-lifetime/OS identity-binding adapter, while an explicitly approved one-shot system picker can remain separate and ephemeral. <https://docs.rs/windows-capture/2.0.1/windows_capture/graphics_capture_picker/struct.PickedGraphicsCaptureItem.html>


### OBSERVE-WIN-07 — D3D11 frame stride admission

A native frame-layout validator checks width/height bounds, BGRA row pitch, upper padding limits and truncation against the actual mapped D3D11 frame before reading bytes. Windows cargo unit tests are part of the capture worker build gate. The OS-screen capture callback still only produces a digest and cannot be forwarded to the model until source identity, redaction and installed acceptance are verified.
