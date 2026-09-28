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
import json, os, pathlib, sys
assert 'DISPLAY' not in os.environ
assert 'DBUS_SESSION_BUS_ADDRESS' not in os.environ
assert 'XDG_RUNTIME_DIR' not in os.environ
assert (pathlib.Path(os.environ['CODEX_HOME']) / 'auth.json').read_text() == 'fixture-only'
def send(msg):
 print(json.dumps(msg), flush=True)
turns = 0
for line in sys.stdin:
 msg = json.loads(line)
 if msg.get('method') == 'initialize':
  send({'id':msg['id'],'result':{}})
 elif msg.get('method') == 'thread/start':
  assert msg['params']['approvalPolicy'] == 'never'
  assert msg['params']['sandbox'] == 'readOnly'
  send({'id':msg['id'],'result':{'thread':{'id':'fixture'}}})
 elif msg.get('method') == 'turn/start':
  params = msg['params']; policy = params['sandboxPolicy']
  assert policy['type'] == 'readOnly'
  assert policy['access']['type'] == 'restricted'
  assert not policy['access']['includePlatformDefaults']
  assert os.environ['CODEX_HOME'] not in str(policy['access']['readableRoots'])
  assert params['approvalPolicy'] == 'never'
  if turns:
   assert json.loads(params['toolOutput']['output']) == {'result':{'pages':1}}
  send({'id':msg['id'],'result':{'turn':{'id':str(turns)}}})
  if turns == 0:
   send({'id':900,'method':'item/commandExecution/requestApproval','params':{}})
  else:
   payload = {'action':'final','method':'','params':{},'answer':'Found one page.'}
   send({'method':'item/completed','params':{'item':{'type':'agentMessage',
        'phase':'final_answer','text':json.dumps(payload)}}})
   send({'method':'turn/completed','params':{'turn':{'id':str(turns),'status':'completed'}}})
  turns += 1
 elif msg.get('id') == 900:
  assert msg['result']['decision'] == 'decline'
  payload = {'action':'call','method':'tabs','params':{},'answer':''}
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
        return {'result': {'pages': 1}}
    with patch.dict(os.environ, {'CODEX_HOME': str(home), 'DISPLAY': ':99',
                                 'XDG_RUNTIME_DIR': '/run/user/fixture'}), \
         patch.object(adapter, 'isolated_browser', browser), \
         contextlib.redirect_stdout(io.StringIO()) as output:
        adapter.run('Count open pages', '/usr/bin/nagi', str(codex), 3)
    assert called == [('capabilities', {}), ('tabs', {})], called
    assert output.getvalue().strip() == 'Found one page.'
    assert (home / 'auth.json').read_text() == 'fixture-only'
    for forbidden in ('settings.approve', 'config.set', 'profile.apply', 'shell'):
        try:
            adapter.decision(json.dumps({'action':'call','method':forbidden,
                                         'params':{},'answer':''}))
        except ValueError:
            pass
        else:
            raise AssertionError(f'Accepted {forbidden}')
print('Codex adapter protocol and refusal boundaries passed')
