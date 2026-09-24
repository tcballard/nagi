"""Native Nagi WebKitGTK compatibility probe against a local fixture."""
import http.server
import json
import pathlib
import subprocess
import tempfile
import threading

root = pathlib.Path(__file__).resolve().parents[1]


class Page(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        body = b'<!doctype html><title>Fixture</title><h1>Compatibility fixture</h1><p>Page text for the screenshot check.</p><span id="account">Signed in</span>'
        self.send_response(200)
        self.send_header('Content-Type', 'text/html')
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *_):
        pass


server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Page)
threading.Thread(target=server.serve_forever, daemon=True).start()
try:
    with tempfile.TemporaryDirectory() as directory:
        directory = pathlib.Path(directory)
        sites = {'sites': [dict(id='local', url=f'http://127.0.0.1:{server.server_port}/',
                                category='fixture', critical=True, auth_required=False,
                                checks=[{'type': 'loads'}, {'type': 'renders'},
                                        {'type': 'no_blocker_console_errors'}])]}
        site_file = directory / 'sites.yaml'
        site_file.write_text(json.dumps(sites))
        command = ['python3', str(root / 'compat/run.py'), 'run', '--sites', str(site_file),
                   '--profile', str(directory / 'profile'), '--output', str(directory / 'report'),
                   '--repeat', '2']
        subprocess.run(command, check=True, timeout=100)
        result = json.loads((directory / 'report/report.json').read_text())
        assert result['metrics']['pass_rate']['rate'] == 1, result
        assert result['metrics']['critical_pass_rate']['rate'] == 1, result
        assert result['flaky_sites'] == [], result
        assert (directory / 'report/report.md').read_text().startswith('# Nagi compatibility')
        assert result['sites'][0]['screenshot']
finally:
    server.shutdown()
