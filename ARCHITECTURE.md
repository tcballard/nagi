# Architecture

Nagi is a standalone GTK4 application. WebKitGTK owns rendering, TLS, networking,
web process sandboxing and website data. Nagi does not disable TLS checks or the
engine's sandbox. Keep the system WebKitGTK package updated.

`main.rs` owns application activation and the browser's lifetime. Gio forwards
new command-line activations to the existing instance under the same desktop
session. `browser.rs` owns windows, tabs and commands. Each tab owns at most one
WebView; restored background tabs are only metadata until selected. Closing a
tab stops loading and releases its widget. GTK callbacks use weak references
so closed tabs cannot keep themselves alive through their signals.

`core.rs` contains navigation parsing and serializable data without GUI imports.
`storage.rs` owns a single serial disk writer. UI changes are coalesced every
750 ms. The writer replaces a mode-0600 file atomically, fsyncs it and its parent,
and drains and joins on shutdown. A damaged or newer schema is preserved and
puts application storage into read-only mode for the session, with a visible
warning. Browser data is bounded to 5,000 history entries. Private tabs use an
ephemeral WebKit network session and are excluded from history, session saves
and reopen-closed-tab history. Explicit downloads and bookmarks remain.

`web.rs` adapts WebKit policies, permissions, downloads, content filters and
isolated-world page scripts. Permission replies are checked against the original
origin after the prompt. External protocol launches require a confirmation.
Downloads use the toolkit save dialog and are cancelled on application close.
Hidden selectors are accepted only during explicit picking, through an isolated
JavaScript world. Reading view emits escaped plain text with a restrictive CSP.

`panels.rs` presents local records and settings. `theme.rs` validates hex colours
from Omarchy's generated colors.toml. The file is reread on a short timer so
atomic directory replacements are detected. Invalid input retains the last good
palette; startup without a theme uses the built-in dark palette. GTK owns fonts,
keyboard input, accessibility and file dialog portal routing.

Resumable browser records live in XDG_STATE_HOME/nagi/state.json. Configuration
uses a separate versioned settings.json transaction envelope. CLI and GTK share
validation, advisory locking, revision checks and atomic replacement. Profiles
export preferences without browsing records. Safe startup bypasses customisation
and does not write session/settings data. Persistent website data lives in
XDG_DATA_HOME/nagi/web; disposable engine/filter caches in XDG_CACHE_HOME/nagi.
Relative XDG values are ignored. There is no autostart, service or global keybind.



`control_transport.rs` owns the bounded local Unix socket and connection workers;
GTK receives typed requests and is the only owner that touches WebViews.
`control.rs` owns session grants, shared normal tabs, deadlines/cancellation,
request IDs and bounded events. Page observations/actions run in a retained
isolated world. Navigation generations prevent old results being accepted.
Stop revokes permissions, cancels requests and joins transport workers.

`extensions.rs` validates the v1 user-owned manifest and content-bound approvals.
`extension_host.rs` owns native consent, declarative command dispatch and
WebKit extension surfaces. Panels use ephemeral storage and restrictive CSP,
without a privileged host bridge. Site hooks have explicit exact-origin access.
A watchdog stops unresponsive extension renderers; revocation stops affected
pages to terminate lingering code. Core package updates do not rewrite personal
manifests. Private tabs and safe mode exclude all personal extension execution.
