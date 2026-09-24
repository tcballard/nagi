#!/usr/bin/env python3
"""Strict local compatibility report validation and aggregation.

This module does not claim browser measurements. A future runner must supply
one measured record per configured site; missing results are an error.
"""
import argparse
import datetime
import json
from pathlib import Path

STATUSES = {'pass', 'partial', 'fail', 'blocked-auth'}
CHECKS = {'loads', 'no_blocker_console_errors', 'renders', 'interaction',
          'media_playback', 'drm_probe', 'auth_persisted'}


def inventory(path):
    # JSON is a YAML 1.2 subset, avoiding a runtime PyYAML requirement.
    data = json.loads(Path(path).read_text())
    sites = data['sites']
    ids = [site['id'] for site in sites]
    if len(ids) != len(set(ids)) or not ids:
        raise ValueError('Site ids must be unique and nonempty')
    for site in sites:
        for key in ('id', 'url', 'category', 'auth_required', 'critical', 'checks'):
            if key not in site:
                raise ValueError(f"{site['id']}: missing {key}")
        if not isinstance(site['auth_required'], bool) or not isinstance(site['critical'], bool):
            raise ValueError(f"{site['id']}: auth_required and critical must be boolean")
        types = [check['type'] for check in site['checks']]
        if set(types) - CHECKS or not {'loads', 'renders'} <= set(types):
            raise ValueError(f"{site['id']}: invalid or missing checks")
        if site['auth_required'] and 'auth_persisted' not in types:
            raise ValueError(f"{site['id']}: authenticated site needs auth_persisted")
    return sites


def metric(records):
    denominator = sum(row['status'] != 'blocked-auth' for row in records)
    passed = sum(row['status'] == 'pass' for row in records)
    return {'pass': passed, 'eligible': denominator,
            'rate': (passed / denominator if denominator else None)}


def validate(sites, result):
    by_id = {site['id']: site for site in sites}
    if len(result['sites']) != len(sites) or {row['id'] for row in result['sites']} != set(by_id):
        raise ValueError('Report must contain exactly one result for every configured site')
    for row in result['sites']:
        if row['status'] not in STATUSES:
            raise ValueError(f"{row['id']}: invalid status")
        for key in ('checks', 'elapsed_ms', 'console_error_count', 'screenshot'):
            if key not in row:
                raise ValueError(f"{row['id']}: missing {key}")
        if row['status'] == 'blocked-auth' and not by_id[row['id']]['auth_required']:
            raise ValueError(f"{row['id']}: blocked-auth on public site")
    if 'drm_probe' not in result:
        raise ValueError('DRM probe missing: engine support must be recorded once per run')


def render(sites, result):
    validate(sites, result)
    by_id = {site['id']: site for site in sites}
    critical = [row for row in result['sites'] if by_id[row['id']]['critical']]
    all_rate, critical_rate = metric(result['sites']), metric(critical)
    blocked = [row['id'] for row in critical if row['status'] == 'fail']
    result['metrics'] = {
        'pass_rate': all_rate, 'critical_pass_rate': critical_rate,
        'blocked_workflows': blocked,
        'blocked_auth': sum(row['status'] == 'blocked-auth' for row in result['sites']),
    }

    def rate(value):
        if value['rate'] is None:
            return f"unavailable (0 eligible; {value['pass']} pass)"
        return f"{100 * value['rate']:.1f}% ({value['pass']}/{value['eligible']})"

    lines = [f"# Nagi compatibility — {result['date']}", '',
             f"**DRM probe:** {result['drm_probe']}", '',
             f"Pass rate: **{rate(all_rate)}**", '',
             f"Critical pass rate: **{rate(critical_rate)}**", '',
             f"Blocked workflows: **{', '.join(blocked) if blocked else 'none'}**", '',
             f"Blocked-auth: **{result['metrics']['blocked_auth']}** (excluded from the two denominators)", '',
             '| Site | Category | Critical | Status | Time (ms) | Console errors | Screenshot |',
             '|---|---|---|---|---:|---:|---|']
    for row in result['sites']:
        site = by_id[row['id']]
        screenshot = row['screenshot'] or ''
        lines.append(f"| {row['id']} | {site['category']} | {site['critical']} | "
                     f"{row['status']} | {row['elapsed_ms']} | {row['console_error_count']} | {screenshot} |")
    lines += ['', 'Flaky sites: ' + (', '.join(result.get('flaky_sites', [])) or 'none reported'), '']
    return '\n'.join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--sites', default='compat/sites.yaml')
    parser.add_argument('--results', required=True, type=Path,
                        help='Measured raw JSON containing date, drm_probe and 52 sites')
    parser.add_argument('--output', type=Path,
                        help='Report directory; defaults to compat/reports/<date>')
    args = parser.parse_args()
    sites = inventory(args.sites)
    result = json.loads(args.results.read_text())
    datetime.date.fromisoformat(result['date'])
    text = render(sites, result)
    output = args.output or Path('compat/reports') / result['date']
    output.mkdir(parents=True, exist_ok=True)
    (output / 'report.json').write_text(json.dumps(result, indent=2) + '\n')
    (output / 'report.md').write_text(text)
    print(output / 'report.md')


if __name__ == '__main__':
    main()
