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
