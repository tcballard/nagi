# Site compatibility (A1)

`sites.yaml` is a JSON-formatted YAML 1.2 inventory. It contains all 52 sites
Tom chose to retain. Each entry has an id, URL, category, auth_required,
critical and a check list. Edit URLs for `bank`, `mastodon`, `atlassian` and
`local-dev`; the `.example` domains are deliberately unusable. Mark the
critical sites you actually need and adjust auth selectors for each login.
Never commit cookies, credentials or an authenticated test profile.

For each site, `loads` must finish within 20 seconds without a WebKit error
page or crash. `renders` saves a full-page screenshot and checks it is not more
than 98% one colour. `no_blocker_console_errors` records all errors but only
an uncaught load-time exception changes pass to partial. Optional `interaction`
tests should use a stable selector and an observable result. `media_playback`
requires 2 seconds of currentTime advancement within 15 seconds. The Widevine
DRM probe belongs once per run, outside individual site results. On auth sites,
`auth_persisted` must be checked first against a configured logged-in element;
its absence marks the site blocked-auth, not fail.

Use a dedicated persistent Nagi profile and log into each auth-required site
yourself once before measuring. The runner starts the same native Nagi GTK4
browser and system WebKitGTK engine in safe mode, without agent control or
extensions, and reuses the persistent WebKit website data from this profile.
The profile directory must be private (mode 0700). With a normal user session:

```sh
mkdir -p ~/.local/share/nagi-compat-profile/{data,state,config,cache}
chmod 700 ~/.local/share/nagi-compat-profile
XDG_DATA_HOME="$HOME/.local/share/nagi-compat-profile/data" \
XDG_STATE_HOME="$HOME/.local/share/nagi-compat-profile/state" \
XDG_CONFIG_HOME="$HOME/.local/share/nagi-compat-profile/config" \
XDG_CACHE_HOME="$HOME/.local/share/nagi-compat-profile/cache" \
dbus-run-session -- nagi https://mail.google.com
```

Repeat the interactive login for each authenticated site, then close Nagi.
Adjust every `auth_persisted` selector to a stable element visible only when
logged in; the seed placeholder deliberately marks sites blocked-auth until
configured. Never store credentials, cookies or this profile in the repo.

Run the complete suite against a debug build:

```sh
cargo build --locked
python3 compat/run.py run --profile "$HOME/.local/share/nagi-compat-profile" --repeat 2
```

The `nagi compat-probe` subprocess is a one-shot test path in the actual native
view. It has no agent socket, uses safe mode, and cannot be called through the
browser-control API. `--sites` chooses a different list; `--binary` selects a
matching Nagi build. A standard run (`--repeat 1`) needs no interaction after
login. A repeated run reports matching statuses and exits nonzero when fewer
than 95% agree. The 52-site run may take many minutes.

`run.py` writes `compat/reports/<date>/{report.json,report.md}` and individual
screenshots in `run-1/` and `run-2/`. `report.py` separately validates measured
raw JSON and regenerates the two reports:

```sh
python3 compat/report.py --sites compat/sites.yaml --results /path/to/measured.json
```

Pass rate is pass / (total minus blocked-auth); critical pass rate uses that
same formula on critical sites. A zero denominator is reported unavailable.
Critical failures form blocked workflows. A partial result never counts as a
pass. Each raw record must include id, status, checks, elapsed_ms,
console_error_count and screenshot; the run must record the DRM probe. The
report lists blocked-auth separately because excluded workflows are unproven.
Only commit screenshots/reports after checking they contain no private data.

This is a browser and engine compatibility probe. Scripted input dispatch may
not produce a trusted user gesture on media sites, so inspect media partials
by hand; do not infer WebKit incompatibility from autoplay denial alone. The
document-start listener collects uncaught exceptions and unhandled rejections;
other browser console messages are not yet captured, and the probe records that
limit in this README. The image check samples a downscaled full-document PNG;
it can mistake an intentionally uniform page for blank. Inspect flagged
screenshots manually. Do not treat blocked-auth entries as working sites.

The first full baseline still must be run on real Omarchy, with a second run
showing at least 95% matching statuses. Inspect and redact screenshots before
committing them. Native CI fixtures are useful for harness correctness, but
cannot count as that baseline.
