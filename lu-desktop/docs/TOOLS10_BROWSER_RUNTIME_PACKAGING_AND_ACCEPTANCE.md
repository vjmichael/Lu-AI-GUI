# Lu TOOLS-10C — Native browser runtime packaging and acceptance

**Development status:** implementation branch only; not an accepted or shipped Lu alpha feature.

## Locked behavior

Lu uses **one headed Chromium process**, initially hidden by native window management. Playwright navigates using browser-level input; it does not drive the physical mouse. The same browser session is shown for user watch mode or a verification challenge, automation pauses for human takeover, and the session is hidden after verified resumption. The selected default is `automatic_hidden`; `always_visible` is the user override.

## Runtime supply and integrity

Lu never silently downloads Chromium, Node, Playwright or xdotool, and never searches the user's PATH for an arbitrary browser on startup. The installed application embeds an exact content manifest. The default checked-in `resources/lu-browser/manifest.json` says `not-packaged`, so the development build reports unavailable and refuses startup.

A reviewed supply bundle has `manifest.json` containing:

- `schema: 1`, `runtime: "lu-browser"`, `status: "reviewed"`
- `platform: "win32"` or `"linux"`, `arch: "x64"`, `playwrightVersion: "1.64.1"`
- `files: [{path,sha256,bytes},...]` covering the **entire runtime**: Chromium binary/dependencies, Node binary, `lu_browser_worker.mjs`, Playwright and Playwright Core modules, and Linux `xdotool`.

The reviewer's manifest SHA-256 must be obtained **out of band**, not from a document or model reply. Staging is offline:

```sh
cd lu-desktop
LU_BROWSER_PACKAGE_DIR=/path/to/reviewed/lu-browser \
LU_BROWSER_MANIFEST_SHA256=<reviewed-64-hex-digest> \
npm run stage-browser
```

On Windows, supply equivalent PowerShell environment variables before `npm run stage-browser`. This requires actual reviewed binaries; the source tree does not contain them. Stage **before** building the desktop app: `include_str!` bakes the manifest into the compiled Rust binary. The NSIS resource declaration includes the browser directory. Runtime admission checks the installed manifest, each file's SHA-256 and length, platform, and absence of unlisted/symlinked files. The digest is an integrity anchor, not a publisher signature or OS containment proof.

## Native authority

The host resolves a real, persisted USER turn from the conversation journal. Browser start requires a native consent dialog naming the precise HTTPS origin and read-only network effect. A host task is minted with an expiry and per-turn scope. Click/keyboard/form operations are classified as `NetworkWrite` and require separate authorization; they are not covered by read-only consent. Window show/hide and human takeover are host-only actions, never model tools.

Stop revokes the browsing task and reaps child ownership. Stop during startup invalidates the task and prevents the late launcher from leaving an unauthorized browser alive. Timeouts poison the worker; late replies are never reused as a later command. The browser has a dedicated local profile.

## Test gates

```sh
node --test lu-desktop/tests/browser-visibility-settings.test.mjs
node --test lu-desktop/tests/browser-native-authorization.test.mjs
node --test lu-desktop/tests/browser-staging.test.mjs
node --test tests/lu-browser-worker.test.mjs
cargo test --locked --manifest-path src-tauri/lu-core/Cargo.toml --lib browser_
cargo test --locked --manifest-path src-tauri/lu-core/Cargo.toml --test browser_native_acceptance
```

The Linux TOOLS-10 CI job provisions real headed Chromium under Xvfb and an X11 window utility. It attempts **actual process ownership, hide, reveal, re-hide, and session continuity**, not just mocked window transitions.

### Required before release/merge

1. Supply, review, pin and stage the actual browser runtime for Windows and Linux; verify Tauri packaging and the installed resource paths on each OS.
2. Demonstrate exact-head Windows/Linux Rust+Node CI passing and the real Linux X11 acceptance. A queued workflow is **not** a pass.
3. Independently test Windows on a physical desktop: no mouse movement or focus theft, browser hide/watch, CAPTCHA handoff, resume, and shutdown. ShowWindow and Windows Job Object tests in CI are not a substitute.
4. Complete browser-model dispatch integration with Lu's installed model turn loop, including read/write capability separation and per-origin grants. The desktop's initial native approval flow does not itself mean autonomous model dispatch is wired.
5. Test crash recovery, browser child-tree containment, localhost DevTools attack surface, session restoration, resource budgets and Wayland behavior (or explicit unsupported status). Job assignment is defense-in-depth; current spawn/assignment retains a short race.
6. Verify all release files without unpinned or unexpected binaries. Do not merge the draft PR until these checks are satisfied.

**Do not mislabel:** This work is a substantial native integration implementation, but without the reviewed browser binaries and installed-app tests it is **not a finished production browser tool**.
