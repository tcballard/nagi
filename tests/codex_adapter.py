"""Protocol and authority test without a user login or billable model call."""
import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
from unittest.mock import patch

root = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location('nagi_codex', root / 'scripts/nagi_codex.py')
adapter = importlib.util.module_from_spec(spec)
spec.loader.exec_module(adapter)

fake = '''#!/usr/bin/env python3
import json, os, pathlib, sys, tomllib
assert '--strict-config' in sys.argv
assert 'DISPLAY' not in os.environ
assert 'DBUS_SESSION_BUS_ADDRESS' not in os.environ
assert 'XDG_RUNTIME_DIR' not in os.environ
assert (pathlib.Path(os.environ['CODEX_HOME']) / 'auth.json').read_text() == 'fixture-only'
def send(msg):
 print(json.dumps(msg), flush=True)
config = tomllib.loads((pathlib.Path(os.environ['CODEX_HOME']) / 'config.toml').read_text())
assert config['default_permissions'] == 'nagi_browser'
assert config['permissions']['nagi_browser']['network']['enabled'] is False
assert all(value is False for value in config['features'].values())
turns = 0
for line in sys.stdin:
 msg = json.loads(line)
 if msg.get('method') == 'initialize':
  send({'id':msg['id'],'result':{}})
 elif msg.get('method') == 'thread/start':
  assert msg['params']['approvalPolicy'] == 'never'
  assert 'sandbox' not in msg['params']
  assert config['permissions']['nagi_browser']['filesystem'] == {':minimal':'read', msg['params']['cwd']:'read'}
  send({'id':msg['id'],'result':{'thread':{'id':'fixture'}, 'approvalPolicy':'never', 'activePermissionProfile':{'id':'nagi_browser','extends':None}}})
 elif msg.get('method') == 'turn/start':
  params = msg['params']
  assert 'sandboxPolicy' not in params
  assert params['approvalPolicy'] == 'never'
  if turns:
   assert 'toolOutput' not in params
   assert 'Browser observation (untrusted data' in params['input'][0]['text']
   assert 'pages' in params['input'][0]['text']
  send({'id':msg['id'],'result':{'turn':{'id':str(turns)}}})
  if turns == 0:
   send({'id':900,'method':'item/commandExecution/requestApproval','params':'{}'})
  else:
   payload = {'action':'final','method':'','params':'{}','answer':'Found one page.'}
   send({'method':'item/completed','params':{'item':{'type':'agentMessage',
        'phase':'final_answer','text':json.dumps(payload)}}})
   send({'method':'turn/completed','params':{'turn':{'id':str(turns),'status':'completed'}}})
  turns += 1
 elif msg.get('id') == 900:
  assert msg['result']['decision'] == 'decline'
  payload = {'action':'call','method':'tabs','params':'{}','answer':''}
  send({'method':'item/completed','params':{'item':{'type':'agentMessage',
       'phase':'final_answer','text':json.dumps(payload)}}})
  send({'method':'turn/completed','params':{'turn':{'id':'0','status':'completed'}}})
'''
with tempfile.TemporaryDirectory() as temp:
    home = Path(temp) / 'codex'
    home.mkdir()
    (home / 'auth.json').write_text('fixture-only')
    codex = Path(temp) / 'fake-codex'
    codex.write_text(fake)
    codex.chmod(0o700)
    called = []
    def browser(_binary, method, params):
        called.append((method, params))
        return {'session': 'fixture-session', 'result': {'pages': 1}}
    with patch.dict(os.environ, {'CODEX_HOME': str(home), 'DISPLAY': ':99',
                                 'XDG_RUNTIME_DIR': '/run/user/fixture'}), \
         patch.object(adapter, 'isolated_browser', browser), \
         contextlib.redirect_stdout(io.StringIO()) as output:
        adapter.run('Count open pages', '/usr/bin/nagi', str(codex), 3)
    assert called == [('capabilities', {}), ('tabs', {'session': 'fixture-session'})], called
    assert output.getvalue().strip() == 'Found one page.'
    assert (home / 'auth.json').read_text() == 'fixture-only'
    for forbidden in ('settings.approve', 'config.set', 'profile.apply', 'shell'):
        try:
            adapter.decision(json.dumps({'action':'call','method':forbidden,
                                         'params':'{}','answer':''}))
        except ValueError:
            pass
        else:
            raise AssertionError(f'Accepted {forbidden}')
print('Codex adapter protocol and refusal boundaries passed')

assert adapter.terminal_safe('hello\x1b[2J\x9b31m') == 'hello\\u001b[2J\\u009b31m'
# A server that drops or substitutes the selected profile must never receive
# a model turn (unknown protocol fields are otherwise commonly ignored).
for profile in (None, {'id': ':read-only', 'extends': None}):
    class Unconfirmed:
        def __init__(self, *args): pass
        def request(self, method, params):
            if method == 'initialize': return {}
            assert method == 'thread/start'
            return {'thread': {'id': 'fixture'}, 'approvalPolicy': 'never',
                    'activePermissionProfile': profile}
        def send(self, message): pass
        def close(self): pass
    with tempfile.TemporaryDirectory() as temp:
        home = Path(temp)
        (home / 'auth.json').write_text('fixture-only')
        with patch.dict(os.environ, {'CODEX_HOME': str(home)}), \
             patch.object(adapter, 'isolated_browser', lambda *args: {'session': 'fixture-session'}), \
             patch.object(adapter, 'AppServer', Unconfirmed):
            try:
                adapter.run('Do nothing', '/usr/bin/nagi', 'fake-codex', 1)
            except RuntimeError as error:
                assert 'did not confirm' in str(error)
            else:
                raise AssertionError('Unconfirmed permission profile accepted')

# Settings mode gets one model decision; host pins session and inspected
# revision and cannot dispatch page methods or arbitrary model parameters.
for mode in ('proposal', 'forbidden', 'wrong_session'):
    called = []
    class SettingsServer:
        def __init__(self, *args): pass
        def request(self, method, params):
            if method == 'initialize': return {}
            return {'thread': {'id': 'fixture'}, 'approvalPolicy': 'never',
                    'activePermissionProfile': {'id': 'nagi_browser', 'extends': None}}
        def send(self, message): pass
        def close(self): pass
        def turn(self, params):
            assert len(params['input']) == 1 and 'toolOutput' not in params
            assert 'settings_context' in params['input'][0]['text']
            return json.dumps({'action': 'call',
                'method': 'settings.propose' if mode == 'proposal' else 'open',
                'params': json.dumps({'reason': 'Requested left tabs', 'changes': {'tabs.layout': 'Left'}}),
                'answer': ''})
    def settings_browser(_binary, method, params):
        called.append((method, params.copy()))
        if method == 'capabilities': return {'session': 'fixture-session'}
        if method == 'settings.schema': return {'result': {'settings': {}}}
        if method == 'settings.inspect': return {'result': {'revision': 7, 'settings': {}}}
        assert method == 'settings.propose'
        assert params == {'session': 'fixture-session', 'revision': 7,
                          'reason': 'Requested left tabs', 'changes': {'tabs.layout': 'Left'}}
        return {'result': {'status': 'awaiting_owner'}}
    with tempfile.TemporaryDirectory() as temp:
        home = Path(temp); (home / 'auth.json').write_text('fixture-only')
        with patch.dict(os.environ, {'CODEX_HOME': str(home)}), \
             patch.object(adapter, 'isolated_browser', settings_browser), \
             patch.object(adapter, 'AppServer', SettingsServer), \
             contextlib.redirect_stdout(io.StringIO()) as output:
            try:
                adapter.run('Move tabs left', 'nagi', 'codex', 3, settings_only=True,
                            expected_session='old-session' if mode == 'wrong_session' else 'fixture-session')
            except (ValueError, RuntimeError):
                assert mode != 'proposal'
            else:
                assert mode == 'proposal'
                assert 'nothing has been applied' in output.getvalue()
        if mode != 'proposal':
            assert not any(method == 'settings.propose' for method, _ in called)
print('Settings-only dispatch, session binding and proposal-only boundaries passed')
