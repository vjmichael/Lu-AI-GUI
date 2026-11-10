# IDE-05.6 Linux GTK native chooser: selection-state repair

[Native installed run 42751072907](https://github.com/vjmichael/Lu-AI-GUI/actions/runs/42751072907)
at executable source `c32d2c1b345a82a02f84c4df70c41ee7169679c1`
again proved genuine Debian packaging, license/original-source inventory,
GTK graphical launch, native X11 chooser visibility, exact parent navigation
and delivered Enter, Alt+S, Alt+O keyboard events. It **failed** because the
selected fixture was not accepted, not because the CI runner was missing a
GUI or the CLI package build failed.

Source review found a further demonstrable test-harness state bug:
`_native_parent_and_exact_child` typed the exact fixture basename in the
verified GTK chooser's parent directory, but immediately returned false if
the GTK accessibility provider failed to expose a named row (it returned
zero rows in the actual OS log). The caller interpreted this as navigation
failure and re-entered the *full target directory path*, navigating inside
it **before trying to confirm the candidate parent-row selection**. This
undid the only attempted native parent/child selection.

The correction preserves that candidate in the same native parent dialog
until a genuine GTK confirmation is attempted. No selected folder is
inferred from a keystroke: the test still requires **both** native chooser
closure and **exact fixture project registration** in the installed Lu
WebView. If either is absent, the test denies acceptance and uninstalls.
No Tauri IPC, JS, app state injection or arbitrary OS path is introduced.

**Required:** one new grouped exact-head Windows NSIS+Ubuntu Debian installed
acceptance; do not mark pass from the code patch or a sent keyboard event.
