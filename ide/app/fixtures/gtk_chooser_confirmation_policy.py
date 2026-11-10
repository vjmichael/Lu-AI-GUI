"""Pure decision policy for a visible native GTK folder chooser.

The actual chooser stays OS-owned: callers must independently witness AT-SPI
dialog ancestry, enabled action, real keyboard/mouse dispatch, dialog closure
and exact registration in installed Lu IDE. This code grants no filesystem
permission and NEVER marks a click as successful selection.
"""

EXPLICIT_FOLDER_ACTIONS = (
    "select folder", "choose folder", "select", "open folder",
)
GENERIC_NAVIGATION_ACTION = "open"


def candidate_captions(names, attempt):
    """Ordered allowed action names for an attempt.

    A single 'Open' is permitted on first attempt, because some GTK versions
    label the affirmative folder button Open. Once observed without closing
    the visible chooser, it is NOT retried: 'Open' may be navigation.
    Instead, the caller should use independently witnessed native X11
    keyboard/AT-SPI geometry fallbacks, or fail closed.
    """
    if not isinstance(attempt, int) or attempt < 0 or attempt > 3:
        return ()
    available = set()
    for value in names:
        if isinstance(value, str):
            available.add(value.strip().casefold())
    choices = EXPLICIT_FOLDER_ACTIONS + (
        (GENERIC_NAVIGATION_ACTION,) if attempt == 0 else ()
    )
    return tuple(caption for caption in choices if caption in available)


def retry_requires_native_fallback(names, attempt):
    """A previously attempted sole generic 'Open' must not spin forever."""
    return not candidate_captions(names, attempt)


def exact_native_directory_row(name, text, basename, role):
    """Classify an exact GTK native directory row, not approximate typeahead.

    This is a *candidate identifier*, not a permission or successful picker
    result. The caller must prove a visible OS chooser, a native selection
    action and exact canonical registration in the installed host/WebView.
    """
    if not isinstance(basename, str) or not basename or "/" in basename or "\\" in basename:
        return False
    if not isinstance(role, str) or not any(part in role.casefold() for part in
            ("row", "cell", "list item", "tree item", "icon", "label", "text")):
        return False
    return (isinstance(name, str) and name.strip() == basename) or (
        isinstance(text, str) and text.strip() == basename)
