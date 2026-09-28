# A3: proposals and an isolated agent process

Development implementation; G2 still requires live Omarchy T4 acceptance.

## Supported authority boundary

The owner runs Nagi normally with `--agent-control`. A trusted launcher starts
an **offline worker** with `nagi agent-run -- /usr/bin/COMMAND ...`, supplying
stdin, stdout and stderr as pipes. Commands must be installed under `/usr`;
source can be sent to an interpreter through stdin. Install `bubblewrap` first.
Failure to create the sandbox aborts execution, with no host fallback.

The worker receives a private home, process/network/IPC namespaces, system
executables and libraries read-only, the Nagi executable and one bound browser
socket. It receives no host home, settings, profile, session bus, Wayland/X11
socket, input devices, host network, host environment secrets or terminal.
It cannot reach a replacement browser socket after Stop/restart. Any config,
profile or extension commands executed inside it affect only its temporary home.
The trusted host must treat worker output as data, never execute it or forward
terminal escape sequences. Do not pass additional host handles or tool access.

The worker remains offline. `nagi-codex` is an optional, separate host for a
locally signed-in Codex CLI. Its app-server handles the model connection; the
host executes model-chosen Nagi calls through this offline worker. The Codex
login cache stays with the model host, outside the worker. Other CLIs need
their own adapters and equivalent enforced tool restrictions.
Giving an agent a general shell, desktop-control tool or writable host files
outside this launcher is outside the protected mode. The launcher does not
retroactively isolate an already-running agent. Host/kernel compromise and
malicious system binaries are outside this boundary.

## Codex CLI adapter (development preview)

Install Codex CLI, run `codex login` on the same machine, and start Nagi with
`--agent-control`. Then run:

```sh
nagi-codex 'Summarise my open tabs and propose putting tabs on the left'
```

`nagi-codex --model MODEL 'TASK'` chooses a model available to that login. It
uses the CLI's file-backed `auth.json`; if the login is stored only in an OS
keyring, this adapter currently exits without a fallback. It copies the cache
into a mode-0700 temporary home for one task and deletes the copy on exit. The
user's normal Codex configuration, plugins and session files are not loaded.
The model host receives no desktop environment variables or runtime socket.
Model-generated commands get a restricted, read-only Codex sandbox with only
an empty task directory as a readable root. Codex permission escalation is
always refused. All browser calls are allowlisted and executed by `agent-run`
inside the separate bubblewrap boundary. Model text cannot invoke an approval
method, shell command or arbitrary host executable through the adapter.

The adapter uses the Codex app-server protocol's `thread/start`, `turn/start`,
`outputSchema` and `toolOutput` fields; it bounds each task to 20 model steps
(at most 30 via `--max-steps`). A proposal remains pending until the owner
reviews it inside Nagi. Stop in Nagi to invalidate that session. The adapter
has no built-in login flow and should not be passed an API key or a Codex auth
file manually. It has not yet been exercised with a signed-in CLI on a real
Omarchy desktop. The protocol fixture and native browser tests are distinct
from that required end-to-end acceptance.

## Proposal protocol

Inside the worker, `nagi browser capabilities` returns the current session.
`settings.schema` describes allowable settings; `settings.inspect` supplies
settings and the current revision. Submit:

```json
{"session":"SESSION_FROM_CAPABILITIES","revision":0,"reason":"Keep tabs visible beside my work","changes":{"tabs.layout":"Left"}}
```

Use `nagi browser settings.propose 'JSON'`. For an approved extension command,
replace `changes` with `"extension":{"id":"research","command":"reading-layout"}`.
Only configure commands can take that route. Exactly one proposal may await
review. The default lifetime is 300 seconds; `ttl_seconds` may shorten it to
1–300 seconds. `settings.status` returns the pending proposal ID or null. Supply a `proposal`
ID to retrieve its last durable outcome and revision within the current session.
`settings.cancel` accepts `{"proposal":"ID"}`. No approve/apply tool exists.

The owner clicks **Review agent change** in the control bar, reads the exact
old/new values and agent-supplied reason, then applies or rejects. Proposal text
is explicitly untrusted and cannot expand permission. Approval is checked
against the pending ID, lifetime, current control session, settings revision
and any bound extension digest. Stop, revoke, cancellation and expiry invalidate
an already-open review dialog. Approval consumes the proposal once.

An approved change updates persistent and live settings. **Settings → Undo last
settings change** reverses the latest transaction. Owner changes concurrent
with a proposal make it stale. Human input is separated from the worker by the
absence of desktop and host capabilities, not by trying to detect synthetic
input within GTK. Unrestricted local automation can still operate native UI.

## Audit and migration

Settings envelope schema 2 adds durable audit records, including proposal ID,
reason, outcome and before/after revision numbers. An applied receipt, settings
and undo history share one atomic file replacement. Interrupted writes therefore
cannot commit new settings without their applied receipt. Schema 1 and legacy
flat settings migrate on their next write. Older binaries reject schema 2;
export preferences before a downgrade and restore a flat settings document
while Nagi is closed. Profile exports remain preferences-only schema 1.

Proposed, rejected, cancelled, expired, stopped, revoked and failed outcomes
are recorded. Owner configuration transactions also receive a generic receipt.
No page contents are automatically collected; agent-provided reasons can still
contain sensitive text, so the entire file stays private (0600). Revoke/Stop
always invalidate authority even if recording their outcome fails; the UI
reports that audit failure. Audit history is not silently truncated. At the
existing 1 MiB document limit, new writes fail closed; preserve the document
before owner-managed archival. This version has no automatic archive rotation.

## Acceptance evidence

- `cargo test --locked`: schema migration, atomic audit/settings state, stale
  transaction rejection and preservation when the audit exceeds its bound.
- `tests/agent_sandbox.py`: real bubblewrap execution; host files, environment,
  network, processes and desktop endpoints absent; only the chosen socket works.
- `tests/codex_adapter.py`: protocol mock checks a signed-in file-cache copy,
  restricted sandbox request, refused escalation, browser allowlist and
  host-to-worker dispatch; it makes no model call.
- `tests/extensions_smoke.py`: proposals from the isolated process, owner
  approval/rejection, exact UI effect, undo, stale revisions, expiry, cancellation,
  revocation, Stop, extension digest changes and unavailable approval methods.
  A fake Codex app-server also runs through the adapter and real worker to a
  pending proposal, then the native review updates the visible toolbar.
- Existing configuration and recovery tests cover competing/interrupted writers.

These are deterministic attacker actions, not a measured model-susceptibility
score. Required real Omarchy acceptance remains T4: an hour of testing and at
least three novel attacks, including Cua/desktop input attempts from the worker,
with exact Omarchy, Hyprland and bubblewrap versions and evidence. G2 is NOT RUN
until those checks pass. This implementation alone does not permit demos or
recruitment under DECISIONS.md.
