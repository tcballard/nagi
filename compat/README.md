# Site compatibility (A1, preparation)

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

Use a dedicated local persistent Nagi profile; log into each auth-required
site yourself once before measuring. Existing Nagi control grants are
session-only and require native consent, so they cannot silently stand in for
persistent login or be reused as unattended test permissions.

The browser runner is **not yet implemented**. `report.py` validates measured
raw JSON and generates `compat/reports/<date>/{report.json,report.md}`:

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

A1 remains blocked on an unattended runner exercising **Nagi itself** under
GTK4/system WebKitGTK. A separate headless Chromium or Playwright WebKit run
cannot establish Nagi compatibility. A first baseline must be run on real
Omarchy, followed by a second full run to show at least 95% matching statuses;
document each flaky site. Do not claim this from the inventory or report unit
tests.
