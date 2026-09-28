#!/usr/bin/env python3
"""Run the real Nagi WebKitGTK view on each configured site, with local results."""
import argparse
import datetime
import http.server
import json
import os
from pathlib import Path
import subprocess
import sys
import threading

from PIL import Image
from report import inventory, render

ROOT = Path(__file__).resolve().parents[1]


def preflight(sites, profile):
    blockers, warnings = [], []
    for site in sites:
        url = site['url']
        if '.example' in url or 'YOUR-' in url:
            blockers.append(f"{site['id']}: replace placeholder URL {url}")
        for check in site['checks']:
            if check['type'] == 'auth_persisted' and check.get('needs_configuration'):
                warnings.append(f"{site['id']}: configure a logged-in selector or record blocked-auth")
            if check['type'] == 'media_playback' and check.get('needs_configuration'):
                warnings.append(f"{site['id']}: check media playback manually if scripted play fails")
    profile = profile.expanduser().resolve()
    # The ordinary browser's XDG directories must never be repurposed as a
    # compatibility profile. The suite visits and screenshots every site.
    home = Path.home().resolve()
    if profile in (home / '.local/share/nagi', home / '.config/nagi',
                   home / '.local/state/nagi', home / '.cache/nagi'):
        blockers.append('profile: use a separate directory, not an everyday Nagi directory')
    return blockers, warnings


def local_drm_server():
    class Page(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            content = b'<!doctype html><title>Nagi DRM probe</title><h1>Nagi DRM probe</h1>'
            self.send_response(200)
            self.send_header('Content-Type', 'text/html')
            self.send_header('Content-Length', str(len(content)))
            self.end_headers()
            self.wfile.write(content)

        def log_message(self, *_):
            pass

    server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Page)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


def profile_env(profile):
    profile = profile.resolve()
    profile.mkdir(parents=True, exist_ok=True, mode=0o700)
    if profile.stat().st_mode & 0o077:
        raise ValueError(f'Profile must be private (chmod 700): {profile}')
    env = os.environ.copy()
    for key, name in [('XDG_DATA_HOME', 'data'), ('XDG_STATE_HOME', 'state'),
                      ('XDG_CONFIG_HOME', 'config'), ('XDG_CACHE_HOME', 'cache')]:
        path = profile / name
        path.mkdir(mode=0o700, exist_ok=True)
        env[key] = str(path)
    return env


def blank_fraction(png):
    with Image.open(png) as image:
        image.thumbnail((256, 256))
        colors = image.convert('RGB').getcolors(256 * 256)
        if colors is None:
            return 0.0
        return max(count for count, _ in colors) / sum(count for count, _ in colors)


def probe(binary, env, site, directory):
    output = directory / (site['id'] + '.json')
    check_json = json.dumps(site['checks'])
    cmd = [str(binary), 'compat-probe', site['id'], site['url'], check_json, str(output)]
    try:
        completed = subprocess.run(cmd, env=env, capture_output=True, text=True, timeout=50)
        if completed.returncode or not output.exists():
            raise RuntimeError(completed.stderr.strip() or f'Process exited {completed.returncode}')
        output.chmod(0o600)
        row = json.loads(output.read_text())
    except (subprocess.TimeoutExpired, RuntimeError, ValueError) as error:
        row = dict(id=site['id'], status='fail', checks={'loads': {'ok': False,
                   'reason': str(error)[:300]}}, elapsed_ms=50000,
                   console_error_count=0, screenshot=None)
    if row.get('screenshot'):
        path = Path(row['screenshot'])
        try:
            path.chmod(0o600)
            fraction = blank_fraction(path)
            row['checks']['renders']['single_colour_fraction'] = round(fraction, 4)
            if fraction > 0.98:
                row['checks']['renders']['ok'] = False
                row['status'] = 'fail'
            row['screenshot'] = str(path.relative_to(directory.parent))
        except (OSError, ValueError) as error:
            row['checks']['renders'] = {'ok': False, 'reason': str(error)}
            row['status'] = 'fail'
            row['screenshot'] = None
    return row


def run(args):
    # Report JSON can name signed-in sites and PNGs can contain private pages.
    # Restrict every file created by this process and its Nagi children.
    os.umask(0o077)
    sites = inventory(args.sites)
    blockers, warnings = preflight(sites, args.profile)
    if blockers:
        raise ValueError('Inventory is not ready:\n  ' + '\n  '.join(blockers))
    if warnings:
        print(f'{len(warnings)} checks are unconfigured; authenticated sites may be blocked-auth.', file=sys.stderr)
    if not args.binary.is_file():
        raise ValueError(f'Build Nagi first; binary is missing: {args.binary}')
    # Never target a user's ordinary Nagi profile: this suite navigates all
    # sites, may play media, and takes screenshots of authenticated pages.
    env = profile_env(args.profile)
    day = datetime.date.today().isoformat()
    folder = args.output or ROOT / 'compat/reports' / day
    if (folder / 'report.json').exists():
        raise ValueError(f'Report already exists at {folder}; use --output for another run')
    folder.mkdir(parents=True, exist_ok=True)
    folder.chmod(0o700)
    server = local_drm_server()
    results = []
    drm_results = []
    try:
        for attempt in range(args.repeat):
            capture = folder / f'run-{attempt + 1}'
            capture.mkdir(exist_ok=True)
            capture.chmod(0o700)
            drm = probe(args.binary, env,
                        {'id': '__drm__', 'url': f'http://127.0.0.1:{server.server_port}/',
                         'checks': [{'type': 'drm_probe'}]}, capture)
            drm_results.append(drm.get('drm_probe') or 'probe unavailable')
            for site in sites:
                print(f'[{attempt + 1}/{args.repeat}] {site["id"]}', flush=True)
                results.append((attempt, probe(args.binary, env, site, capture)))
    finally:
        server.shutdown()
    drm_result = drm_results[-1]
    last = [row for attempt, row in results if attempt == args.repeat - 1]
    runs = [[row for attempt, row in results if attempt == index]
            for index in range(args.repeat)]
    flaky = []
    if args.repeat > 1:
        by_id = {site['id']: [] for site in sites}
        for _, row in results:
            by_id[row['id']].append(row['status'])
        flaky = [id for id, statuses in by_id.items() if len(set(statuses)) > 1]
        matching = 1 - len(flaky) / len(sites)
        print(f'Status agreement: {matching:.1%} ({len(sites)-len(flaky)}/{len(sites)})')
    report = dict(date=day, drm_probe=drm_result, drm_probes=drm_results,
                  sites=last, runs=runs, flaky_sites=flaky, repeat_runs=args.repeat)
    text = render(sites, report)
    for path, content in [(folder / 'report.json', json.dumps(report, indent=2) + '\n'),
                          (folder / 'report.md', text)]:
        path.write_text(content)
        path.chmod(0o600)
    print(folder / 'report.md')
    if args.repeat > 1 and len(flaky) > len(sites) * .05:
        return 1
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['preflight', 'run'])
    parser.add_argument('--sites', type=Path, default=ROOT / 'compat/sites.yaml')
    parser.add_argument('--profile', type=Path,
                        default=Path.home() / '.local/share/nagi-compat-profile')
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/debug/nagi')
    parser.add_argument('--output', type=Path)
    parser.add_argument('--repeat', type=int, choices=[1, 2], default=1)
    args = parser.parse_args()
    try:
        if args.command == 'preflight':
            sites = inventory(args.sites)
            blockers, warnings = preflight(sites, args.profile)
            if blockers or warnings:
                for problem in blockers + warnings:
                    print(problem)
                print(f'{len(blockers)} blockers; {len(warnings)} checks need review')
                return 1 if blockers else 0
            print(f'Ready to attempt {len(sites)} sites; authenticate in the dedicated profile before running.')
            return 0
        return run(args)
    except (ValueError, OSError) as error:
        parser.error(str(error))


if __name__ == '__main__':
    sys.exit(main())
