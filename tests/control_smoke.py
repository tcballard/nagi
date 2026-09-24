"""Actual GTK/WebKit agent boundary test, under Xvfb and a session bus."""
import http.server
import json
import os
import pathlib
import subprocess
import tempfile
import threading
import time

root = pathlib.Path(__file__).resolve().parent.parent
binary = str(root/'target/debug/nagi')
out = root/'target/evidence'
out.mkdir(parents=True, exist_ok=True)

class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        data = b'<title>Agent fixture</title><h1>Agent fixture</h1><input aria-label="Notes"><input type="password" aria-label="Secret"><button onclick="document.querySelector(\'h1\').textContent=\'Clicked\'">Change title</button><select aria-label="Choice"><option value="a">A</option><option value="b">B</option></select>'
        self.send_response(200)
        self.send_header('Content-Type', 'text/html')
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        self.wfile.write(data)
    def log_message(self, *_):
        pass

server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
threading.Thread(target=server.serve_forever, daemon=True).start()
url = f'http://127.0.0.1:{server.server_port}/'
with tempfile.TemporaryDirectory(prefix='nagi-control-') as directory:
    env = dict(os.environ, XDG_STATE_HOME=directory+'/state', XDG_DATA_HOME=directory+'/data', XDG_CACHE_HOME=directory+'/cache', XDG_RUNTIME_DIR=directory+'/runtime')
    pathlib.Path(env['XDG_RUNTIME_DIR']).mkdir(mode=0o700)
    log = (out/'control.log').open('w')
    app = subprocess.Popen([binary, '--agent-control', url], env=env, stdout=log, stderr=log)
    def call(method, params=None, ok=True, request_id=None):
        args = [binary, 'browser', method, json.dumps(params or {})]
        if request_id:
            args.append(request_id)
        p = subprocess.run(args, env=env, capture_output=True, text=True, timeout=22)
        if ok:
            assert p.returncode == 0, p.stderr
            return json.loads(p.stdout)['result']
        assert p.returncode != 0, p.stdout
        return json.loads(p.stderr)
    def wait(fn, seconds=20):
        end = time.monotonic()+seconds
        while time.monotonic()<end:
            assert app.poll() is None, 'Nagi exited; inspect control.log'
            try:
                result = fn()
                if result:
                    return result
            except (AssertionError, FileNotFoundError, subprocess.CalledProcessError):
                pass
            time.sleep(.1)
        raise AssertionError('Timed out')
    def xd(*args):
        return subprocess.check_output(['xdotool', *args], env=env, text=True).strip()
    try:
        wait(lambda: pathlib.Path(directory+'/runtime/nagi-control/browser.sock').exists())
        assert call('tabs') == []
        subprocess.run([binary,'--private',url],env=env,check=True)
        time.sleep(.5)
        call('attach',ok=False)
        xd('key','ctrl+w')
        time.sleep(.5)
        call('open', {'url':url}, ok=False)
        call('snapshot', {'tab':1}, ok=False)
        grant = subprocess.Popen([binary,'browser','attach','{}'], env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        time.sleep(1)
        xd('key','Tab'); xd('key','Tab'); xd('key','Return')
        stdout, stderr = grant.communicate(timeout=20)
        assert grant.returncode == 0, stderr
        permission = json.loads(stdout)['result']
        assert permission['interaction'], permission
        tab = permission['tab']
        call('wait', {'tab':tab})
        snapshot = call('snapshot', {'tab':tab})
        assert snapshot['untrusted']
        element = lambda label: next(e['ref'] for e in snapshot['elements'] if e['label']==label)
        call('type', {'tab':tab,'ref':element('Notes'),'text':'Research notes'})
        call('type', {'tab':tab,'ref':element('Secret'),'text':'do not enter'}, ok=False)
        call('select', {'tab':tab,'ref':element('Choice'),'value':'b'})
        call('click', {'tab':tab,'ref':element('Change title')}, request_id='click-once')
        call('click', {'tab':tab,'ref':element('Change title')}, ok=False, request_id='click-once')
        assert 'Clicked' in call('snapshot', {'tab':tab})['text']
        call('click', {'tab':tab,'ref':element('Change title')}, ok=False)
        image = call('screenshot', {'tab':tab})
        import base64
        png = base64.b64decode(image['base64'])
        assert png.startswith(b'\x89PNG')
        (out/'agent-page.png').write_bytes(png)
        assert call('events')['cursor'] > 0
        call('navigate', {'tab':tab,'url':'https://example.com'}, ok=False)
        call('revoke', {'origin':url})
        call('snapshot', {'tab':tab}, ok=False)
        assert call('tabs')[0]['access']=='unavailable'
        # Cancellation closes the native permission prompt and cannot grant access.
        pending = subprocess.Popen([binary,'browser','grant',json.dumps({'origin':'http://127.0.0.1:9'}),'pending-grant'],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
        time.sleep(.5)
        assert call('cancel',{'id':'pending-grant'})['cancelled']
        pending.communicate(timeout=5)
        assert pending.returncode != 0
        call('open',{'url':'http://127.0.0.1:9/'},ok=False)
        # A read-only origin grant permits observations but never interaction.
        read_only = subprocess.Popen([binary,'browser','grant',json.dumps({'origin':url})],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
        time.sleep(.5)
        xd('key','Tab'); xd('key','Return')
        stdout,stderr=read_only.communicate(timeout=20)
        assert read_only.returncode==0,stderr
        assert json.loads(stdout)['result']['interaction'] is False
        observed=call('snapshot',{'tab':tab})
        call('click',{'tab':tab,'ref':observed['elements'][0]['ref']},ok=False)
        call('stop')
        wait(lambda:not pathlib.Path(directory+'/runtime/nagi-control/browser.sock').exists())
        call('capabilities', ok=False)
    finally:
        app.terminate()
        try: app.wait(timeout=5)
        except subprocess.TimeoutExpired:
            app.kill(); app.wait()
        log.close()
server.shutdown()
print('agent control boundaries passed')
