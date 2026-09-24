# CLI personalisation and control development

This series is built on PR #3. It keeps GTK4/WebKitGTK and adds no MCP server.
Build with Rust 1.89+ and the native dependencies in README: `cargo build --locked`.
Use `./target/debug/nagi` to run without replacing your installed package.

## 1. Configuration transactions

`nagi config schema` describes keys and constraints. `get [KEY]` retains the
existing output. `inspect` returns schema, revision, settings and the last 32
snapshots. `set KEY VALUE`, `apply JSON`, and `undo` accept `--dry-run` and
`--if-revision N`. All output is JSON; failures use stderr and exit 2.

Example: `nagi config apply '{"tabs.layout":"Left","zoom":1.2}' --if-revision 0`.
Read the actual revision with `inspect` first. Dry runs return the candidate
document without writing settings. The GTK panel uses the same transaction
path; unrelated concurrent edits are retained. A busy lock returns an error
instead of freezing the GTK thread. Agents may retry busy transactions, but
must inspect and reconsider revision conflicts.

The existing settings path is retained for compatibility. Legacy flat settings
are migrated on first successful change. Newer schemas and invalid documents
are preserved. A separate advisory lock serializes read/modify/write; unique
temporary files, fsync and atomic replacement protect the committed document.
History is committed with the settings, so interrupted writes cannot produce
a settings/history mismatch. Undo creates a new revision and retains history.

Older binaries cannot interpret the new envelope. Before a binary downgrade,
export `nagi config get` to a backup and restore that flat JSON as settings.json
while the browser is closed. Uninstall continues to preserve browsing data.

## Planned dependent PRs

2. Versioned personalisation: profiles, appearance/layout preferences and safe mode.
3. Local CLI control: scoped session access, tabs, page observations/actions,
   events, cancellation, visible control state and stop.
4. Isolated extension hooks: commands, sidebars, new tabs, navigation/download
   events and site styles/scripts, with explicit grants and bounded execution.

## Evidence

Initial base: PR #3 at 8e43d33d827acbffbdacc3044d3dfcb02d148a3e.
This workspace has no Rust/GTK toolchain. Compilation, Rust tests and the real
GTK fixture are delegated to each PR's GitHub Actions run, not claimed locally.
New CLI integration test: `python3 tests/config_transactions.py` after debug build.
Live Omarchy/Wayland, authenticated sites, media and long-session acceptance
remain required. Existing VERIFICATION.md results apply only to their stated revisions.

## 2. Portable personalisation

New schema-discoverable settings: `layout.density`, `tabs.sidebar_width`,
`appearance.accent`, `new_tab.url`, and `toolbar.actions`. Settings exposes the
same values; changes apply live. Custom new-tab URLs require HTTP(S), cannot
embed credentials, and never replace private new tabs.

`nagi profile export research > research.json` exports preferences only.
`nagi profile check < research.json` validates without changing settings.
`nagi profile apply --if-revision N < research.json` imports atomically; undo
works normally. Profiles are portable JSON with their own version; store them
in your own Git repository if desired. They contain no history/cookies/tokens.
Profiles are snapshots applied to the current browser, not isolated login accounts.

Close all Nagi windows and launch `nagi --safe-mode` to bypass personalisation.
It starts with defaults, does not restore/save sessions or edit settings, and
will not load extensions or start agent control. Normal startup restores your
saved preferences. Safe mode is selected at startup; sending it to an already
running browser does not change that process's mode.

## 3. Local browser control

Start a new process with `nagi --agent-control`. Existing instances must be
closed first. The CLI is `nagi browser METHOD '[JSON_PARAMS]' [REQUEST_ID]`.
`capabilities` discovers the supported methods. No HTTP server or MCP adapter.
The Unix socket is mode 0600 in a mode-0700 runtime directory; this is a local
same-user interface, not isolation from hostile processes running as you.

Run `nagi browser attach` to share the active normal tab, or
`nagi browser grant '{"origin":"https://example.com"}'` before opening a tab.
Approve read access or read-and-interaction in the native browser dialog.
These grants allow authenticated page content/actions on that exact origin;
only approve interaction when your agent is authorised to act there. Grants
are session-only, never provided by webpage text. Private tabs are excluded.

Examples (replace TAB with the numeric ID returned by attach/open):

- `nagi browser tabs`
- `nagi browser open '{"url":"https://example.com"}'`
- `nagi browser wait '{"tab":TAB}'`
- `nagi browser snapshot '{"tab":TAB}'`
- `nagi browser click '{"tab":TAB,"ref":"REF_FROM_SNAPSHOT"}'`
- `nagi browser type '{"tab":TAB,"ref":"REF","text":"hello"}'`
- `nagi browser select '{"tab":TAB,"ref":"REF","value":"option-value"}'`
- `nagi browser scroll '{"tab":TAB,"y":600}'`
- `nagi browser screenshot '{"tab":TAB}'` returns PNG as base64, no filesystem writes.
- `nagi browser events '{"after":0}'` returns bounded events, cursor and gap flag.
- `nagi browser revoke '{"origin":"https://example.com"}'`
- `nagi browser cancel '{"id":"REQUEST_ID"}'`
- `nagi browser stop`

Responses include a session identifier. Pass `session` in params to reject a
stale browser session. IDs are unique within a session, and repeated request
IDs are rejected; do not blindly retry timed-out mutations. Completion of an
input operation does not prove completion of a website transaction. Observe
again to verify. `wait` waits for the current load, not future SPA network work.
Snapshots invalidate old element references. Navigation invalidates observations.
Page content is explicitly untrusted. No arbitrary JavaScript evaluation API.
Password/payment/OTP/file inputs require manual entry. Cross-origin frames and
closed shadow roots are not exposed; synthetic input may not work on all sites.

Requests are bounded to 64 KiB, 16 connections, 15 seconds and 10,000 IDs per
session. Events retain the last 512 entries and no page contents. Cancellation
stops pending work; it cannot undo already completed website actions. The
visible Stop button revokes all access and closes the socket. Restart Nagi to
enable another control session. Downloads continue to use native save prompts.

## 4. Personal extensions

`nagi extension schema` describes the API. Install the bundled example with
`nagi extension install < examples/extensions/research.json`. Installation
validates and stores code in `~/.config/nagi/extensions/`; it does not enable it.
Open `nagi --extensions`, review the access description and enable the extension.
`nagi extension list` reports IDs, hashes and current approval. `disable ID`
revokes it. Changing installed content invalidates approval. Never edit grants
as a substitute for owner approval.

Manifests (API 1) register up to 16 named commands: open an HTTP(S) URL, show
an offline sidebar, or apply a validated settings batch. The Extensions panel
runs these commands; an enabled control session also provides
`nagi browser extension.commands` and
`nagi browser extension.run '{"extension":"research","command":"reading-layout"}'`.

Sidebar/new-tab HTML uses an ephemeral WebKit view with restrictive CSP: no
network, remote frames, form submission, native filesystem or shell bridge.
JavaScript can manipulate the panel DOM. Storage is temporary; the example
explicitly warns that notes must be copied before closing. Select a provider
with `nagi config set new_tab.extension research`; keep `new_tab.url` empty.
Private tabs never use extensions. A hung panel is terminated and disabled
by a three-second responsiveness watchdog; the native browser stays available.

Site hooks contain exact `origin`, optional `css`, and optional `script`.
They run after navigation in an isolated JavaScript world, only on approved
normal-tab origins. This isolates JavaScript globals, not DOM effects or
network activity: site scripts/styles are powerful and may interact with
signed-in pages. Review that access before approving. No privileged host API
is exposed. A failing/unresponsive extension is disabled. Revocation stops
affected renderer pages to end lingering timers; reload to resume without it.
Already completed website effects cannot be undone by disabling an extension.

Event hooks support `navigation.finished` and successful `download.finished`
for exact origins, delivering bounded plain-text notices. Commands are invoked
explicitly, preventing event-triggered navigation/configuration loops. Agent
navigation/download events are also available through the control event cursor.

Personal code lives outside the installed package. Package upgrades preserve
it; incompatible API versions and changed hashes remain disabled. The manifest
and example are the v1 contract; future capabilities require an explicit API
revision and migration. No general-purpose native plugin loader is included.
