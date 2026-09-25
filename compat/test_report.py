import copy
import unittest
from pathlib import Path

from report import inventory, metric, render
from run import preflight


class ReportTests(unittest.TestCase):
    def test_preflight_finds_placeholder_urls_and_protects_everyday_profile(self):
        sites = inventory('compat/sites.yaml')
        blockers, warnings = preflight(sites, Path.home() / '.local/share/nagi')
        self.assertTrue(any('bank: replace placeholder URL' in item for item in blockers))
        self.assertTrue(any('profile: use a separate directory' in item for item in blockers))
        self.assertTrue(any('gmail: configure a logged-in selector' in item for item in warnings))

    def test_all_52_sites_are_seeded(self):
        sites = inventory('compat/sites.yaml')
        self.assertEqual(len(sites), 52)
        self.assertTrue(any(s['id'] == 'bank' and s['critical'] for s in sites))

    def test_exact_gate_metrics_and_missing_auth(self):
        sites = [{'id': 'mail', 'category': 'work', 'critical': True},
                 {'id': 'bank', 'category': 'work', 'critical': True},
                 {'id': 'media', 'category': 'media', 'critical': False},
                 {'id': 'docs', 'category': 'work', 'critical': True}]
        rows = [dict(id=id, status=status, checks=[], elapsed_ms=5,
                     console_error_count=0, screenshot=None)
                for id, status in [('mail', 'pass'), ('bank', 'blocked-auth'),
                                   ('media', 'partial'), ('docs', 'fail')]]
        sites[1]['auth_required'] = True
        result = dict(date='2026-09-24', sites=rows, drm_probe='unsupported')
        markdown = render(sites, result)
        self.assertEqual(result['metrics']['pass_rate'],
                         {'pass': 1, 'eligible': 3, 'rate': 1 / 3})
        self.assertEqual(result['metrics']['critical_pass_rate'],
                         {'pass': 1, 'eligible': 2, 'rate': 0.5})
        self.assertEqual(result['metrics']['blocked_workflows'], ['docs'])
        self.assertIn('**DRM probe:** unsupported', markdown)
        missing = copy.deepcopy(result)
        missing['sites'].pop()
        with self.assertRaises(ValueError):
            from report import validate
            validate(sites, missing)
        self.assertIsNone(metric([rows[1]])['rate'])

    def test_two_runs_keep_both_statuses_and_drm_results(self):
        sites = [{'id': 'site', 'category': 'dev', 'critical': True,
                  'auth_required': False}]
        first = dict(id='site', status='fail', checks={}, elapsed_ms=1,
                     console_error_count=0, screenshot='run-1/site.png')
        second = dict(first, status='pass', screenshot='run-2/site.png')
        report = dict(date='2026-09-25', sites=[second], runs=[[first], [second]],
                      repeat_runs=2, drm_probe='supported',
                      drm_probes=['unsupported', 'supported'], flaky_sites=['site'])
        markdown = render(sites, report)
        self.assertIn('Run 1 DRM: unsupported', markdown)
        self.assertIn('Run 2 DRM: supported', markdown)
        self.assertIn('| site | fail | run-1/site.png |', markdown)
        self.assertIn('| site | pass | run-2/site.png |', markdown)
        del report['drm_probes'][0]
        with self.assertRaises(ValueError):
            render(sites, report)


if __name__ == '__main__':
    unittest.main()
