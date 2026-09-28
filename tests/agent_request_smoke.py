"""Ctrl+L -> settings-only adapter -> pending native proposal; never approve."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
import pyatspi

root = Path(__file__).resolve().parent.parent
binary = root / 'target/debug/nagi'
with tempfile.TemporaryDirectory(prefix='nagi-request-fixture-') as temporary:
    base = Path(temporary)
    env = dict(os.environ, GTK_A11Y='atspi', NO_AT_BRIDGE='0')
    for key in ('XDG_CONFIG_HOME', 'XDG_DATA_HOME', 'XDG_STATE_HOME', 'XDG_CACHE_HOME', 'XDG_RUNTIME_DIR'):
        path = base / key
        path.mkdir(mode=0o700)
        env[key] = str(path)
    login = base / 'login'; login.mkdir(mode=0o700)
    (login / 'auth.json').write_text('fixture-only')
    env['CODEX_HOME'] = str(login)
    executables = base / 'bin'; executables.mkdir()
    fake = executables / 'codex'
    fake.write_text('''#!/usr/bin/env python3
import json, sys
for line in sys.stdin:
 m=json.loads(line)
 if m.get('method')=='initialize':
  print(json.dumps({'id':m['id'],'result':{}}),flush=True)
 elif m.get('method')=='thread/start':
  print(json.dumps({'id':m['id'],'result':{'thread':{'id':'fixture'},'approvalPolicy':'never','activePermissionProfile':{'id':'nagi_browser','extends':None}}}),flush=True)
 elif m.get('method')=='turn/start':
  assert 'settings_context' in m['params']['input'][0]['text']
  assert 'Move tabs left' in m['params']['input'][0]['text']
  print(json.dumps({'id':m['id'],'result':{'turn':{'id':'turn'}}}),flush=True)
  action={'action':'call','method':'settings.propose','params':json.dumps({'reason':'Fixture owner request','changes':{'tabs.layout':'Left'}}),'answer':''}
  print(json.dumps({'method':'item/completed','params':{'item':{'type':'agentMessage','phase':'final_answer','text':json.dumps(action)}}}),flush=True)
  print(json.dumps({'method':'turn/completed','params':{'turn':{'id':'turn','status':'completed'}}}),flush=True)
''')
    fake.chmod(0o700)
    adapter = executables / 'nagi-codex'
    adapter.write_bytes((root / 'scripts/nagi_codex.py').read_bytes())
    adapter.chmod(0o700)
    env['PATH'] = str(executables) + ':' + os.environ['PATH']
    log = base / 'browser.log'
    with log.open('w') as output:
        app = subprocess.Popen([str(binary), '--agent-control'], env=env, stdout=output, stderr=output)
    def cli(*args):
        return json.loads(subprocess.check_output([str(binary), *args], env=env, text=True))
    def wait(check):
        deadline = time.monotonic() + 25
        while time.monotonic() < deadline:
            assert app.poll() is None, log.read_text()
            try:
                value = check()
                if value: return value
            except (OSError, subprocess.CalledProcessError): pass
            time.sleep(.15)
        raise AssertionError('Timed out: ' + log.read_text())
    def accessible(name):
        pending = [pyatspi.Registry.getDesktop(0)]
        for _ in range(2000):
            if not pending: return None
            node = pending.pop()
            try:
                if node.name == name and node.getState().contains(pyatspi.STATE_SHOWING): return node
                pending.extend(node)
            except Exception: pass
    try:
        wait(lambda: (Path(env['XDG_RUNTIME_DIR']) / 'nagi-control/browser.sock').exists())
        subprocess.run(['xdotool', 'key', 'ctrl+l'], env=env, check=True)
        wait(lambda: accessible('Ask agent'))
        subprocess.run(['xdotool', 'type', '--clearmodifiers', 'Move tabs left'], env=env, check=True)
        button = wait(lambda: accessible('Ask agent'))
        assert button.queryAction().doAction(0)
        pending = wait(lambda: cli('browser', 'settings.status')['result']['pending'])
        assert cli('config', 'get', 'tabs.layout') == 'Top', 'Proposal applied without owner'
        document = cli('config', 'inspect')
        assert document['revision'] == 0
        assert document['audit'][-1]['outcome'] == 'proposed'
        assert document['audit'][-1]['proposal'] == pending
        # Stop uses the public native control; no fixture ever clicks Apply.
        assert wait(lambda: accessible('Stop agent control')).queryAction().doAction(0)
        wait(lambda: cli('config', 'inspect')['audit'][-1]['outcome'] == 'stopped')
        assert cli('config', 'get', 'tabs.layout') == 'Top'
        print('Ctrl+L native request creates proposal only; Stop invalidates it')
    finally:
        app.terminate()
        try: app.wait(timeout=8)
        except subprocess.TimeoutExpired: app.kill(); app.wait()
