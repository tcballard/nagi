"""Native consent, CSP, site hooks, revocation and hung renderer recovery."""
import http.server
import json
import os
import pathlib
import subprocess
import sys
import tempfile
import threading
import time
import pyatspi

root = pathlib.Path(__file__).resolve().parent.parent
binary = str(root/'target/debug/nagi')
out = root/'target/evidence'
out.mkdir(parents=True, exist_ok=True)
requests = []
class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        requests.append(self.path)
        body = b'<title>Extension fixture</title><h1>Original page</h1>'
        self.send_response(200); self.send_header('Content-Type','text/html'); self.end_headers(); self.wfile.write(body)
    def log_message(self,*_): pass
server = http.server.ThreadingHTTPServer(('127.0.0.1',0),Handler)
threading.Thread(target=server.serve_forever,daemon=True).start()
origin = f'http://127.0.0.1:{server.server_port}'
with tempfile.TemporaryDirectory(prefix='nagi-extensions-') as directory:
    env = dict(os.environ, GTK_A11Y='atspi', NO_AT_BRIDGE='0')
    for variable, suffix in [('XDG_CONFIG_HOME','config'),('XDG_DATA_HOME','data'),('XDG_STATE_HOME','state'),('XDG_CACHE_HOME','cache'),('XDG_RUNTIME_DIR','runtime')]:
        env[variable] = directory+'/'+suffix
        pathlib.Path(env[variable]).mkdir(mode=0o700)
    def cli(*args,input=None,ok=True):
        result = subprocess.run([binary,*args],env=env,input=input,text=True,capture_output=True,timeout=22)
        assert (result.returncode==0)==ok, result.stdout+result.stderr
        return json.loads(result.stdout if ok else result.stderr)
    manifest = {'api':1,'id':'fixture','name':'Extension fixture','description':'Test-only extension in a disposable profile',
        'commands':[{'id':'sidebar','label':'Open fixture sidebar','action':{'kind':'sidebar'}},
                    {'id':'configure','label':'Change tab layout',
                     'action':{'kind':'configure','changes':{'tabs.layout':'Left'}}}],
        'sidebar_html':f'<h2>Offline sidebar</h2><script>fetch("{origin}/leak").catch(()=>{{}})</script>',
        'sites':[{'origin':origin,'script':'document.querySelector("h1").textContent="Extension active";','css':'h1 { color: rgb(1,2,3); }'}],
        'events':[{'kind':'navigation.finished','origin':origin,'message':'Fixture navigation completed'}]}
    cli('extension','install',input=json.dumps(manifest))
    log=(out/'extensions.log').open('w')
    app=subprocess.Popen([binary,'--agent-control','--extensions',origin+'/'],env=env,stdout=log,stderr=log)
    def wait(fn,seconds=20):
        deadline=time.monotonic()+seconds
        while time.monotonic()<deadline:
            assert app.poll() is None, 'Nagi exited; see extensions.log'
            try:
                result=fn()
                if result: return result
            except (AssertionError,subprocess.CalledProcessError,FileNotFoundError): pass
            time.sleep(.15)
        raise AssertionError('Timed out')
    def accessible(name):
        pending=[pyatspi.Registry.getDesktop(0)]
        count=0
        while pending and count<2000:
            node=pending.pop(); count+=1
            try:
                if node.name==name and node.getState().contains(pyatspi.STATE_SHOWING): return node
                pending.extend(node)
            except Exception: pass
        return None
    def click(name):
        node=wait(lambda:accessible(name))
        assert node.queryAction().doAction(0), name
        time.sleep(.2)
    def approve():
        click('Review and enable'); click('Enable')
        wait(lambda:cli('extension','list')[0]['enabled'])
    try:
        approve()
        commands=cli('browser','extension.commands')['result'][0]['commands']
        assert [command['id'] for command in commands]==['sidebar'], commands
        assert cli('config','get','tabs.layout')=='Top'
        denied=cli('browser','extension.run','{"extension":"fixture","command":"configure"}',ok=False)
        assert 'require native preview' in denied['error'], denied
        assert cli('config','get','tabs.layout')=='Top'
        click('Change tab layout')
        wait(lambda:accessible('Apply changes'))
        click('Cancel')
        assert cli('config','get','tabs.layout')=='Top'
        click('Change tab layout')
        wait(lambda:accessible('Apply changes'))
        cli('config','set','zoom','1.1')
        click('Apply changes')
        wait(lambda:not accessible('Apply changes'))
        assert cli('config','get','tabs.layout')=='Top', 'Stale preview applied despite revision conflict'
        time.sleep(.5)
        click('Change tab layout')
        wait(lambda:accessible('Apply changes'))
        click('Apply changes')
        wait(lambda:cli('config','get','tabs.layout')=='Left')
        subprocess.run(['xdotool','key','ctrl+comma'],env=env,check=True)
        wait(lambda:accessible('Undo last settings change'))
        window_ids=subprocess.check_output(['xdotool','search','--pid',str(app.pid)],env=env,text=True).splitlines()
        geometries=[subprocess.check_output(['xdotool','getwindowgeometry','--shell',wid],env=env,text=True) for wid in window_ids]
        windows=[dict(line.split('=',1) for line in geometry.splitlines() if '=' in line) for geometry in geometries]
        window=max(windows,key=lambda w:int(w['WIDTH'])*int(w['HEIGHT']))
        subprocess.run(['xdotool','mousemove',str(int(window['X'])+int(window['WIDTH'])-250),str(int(window['Y'])+267),'click','1'],env=env,check=True)
        wait(lambda:cli('config','get','tabs.layout')=='Top')
        assert cli('config','get','zoom')==1.1, 'Undo reverted more than the approved transaction'
        # Deterministic hostile-agent calls: no tool path carries owner approval.
        session=cli('browser','capabilities')['session']
        original=cli('config','get')
        def propose(changes=None, **extra):
            payload=dict(session=session, revision=cli('config','inspect')['revision'],
                         reason='Fixture: put a back action in the toolbar')
            if changes is not None: payload['changes']=changes
            payload.update(extra)
            result=subprocess.run([binary,'agent-run','--','/usr/bin/env','nagi','browser','settings.propose',json.dumps(payload)],
                                  input='',text=True,capture_output=True,env=env,timeout=25)
            assert result.returncode==0,result.stdout+result.stderr
            return json.loads(result.stdout)['result']['proposal']
        def outcome(proposal, expected):
            wait(lambda:any(row['proposal']==proposal and row['outcome']==expected
                            for row in cli('config','inspect')['audit']))
        def review():
            click('Review agent change')
            wait(lambda:accessible('Apply proposal'))
        proposal=propose({'toolbar.actions':['back']})
        for method in ['settings.approve','settings.apply','config.set','profile.apply']:
            cli('browser',method,json.dumps(dict(proposal=proposal,approved=True)),ok=False)
        assert cli('config','get')==original
        review(); click('Reject proposal')
        outcome(proposal,'rejected')
        assert cli('config','get')==original
        assert cli('browser','settings.status',json.dumps(dict(proposal=proposal)))['result']['last_outcome']['outcome']=='rejected'
        proposal=propose({'toolbar.actions':['back']})
        review()
        cli('config','set','zoom','1.2')
        click('Apply proposal')
        outcome(proposal,'failed')
        assert cli('config','get','toolbar.actions')==[]
        assert cli('config','get','zoom')==1.2
        assert cli('config','inspect')['audit'][-1]['outcome']=='failed'
        proposal=propose({'toolbar.actions':['back']},ttl_seconds=1)
        review()
        wait(lambda:cli('browser','settings.status')['result']['pending'] is None)
        click('Apply proposal')
        assert cli('config','get','toolbar.actions')==[]
        proposal=propose({'toolbar.actions':['back']})
        review()
        cli('browser','settings.cancel',json.dumps(dict(proposal=proposal)))
        click('Apply proposal')
        assert cli('config','get','toolbar.actions')==[]
        proposal=propose({'toolbar.actions':['back']})
        review()
        cli('browser','revoke',json.dumps(dict(origin=origin)))
        click('Apply proposal')
        assert cli('config','get','toolbar.actions')==[]
        proposal=propose({'toolbar.actions':['back']})
        review(); click('Apply proposal')
        wait(lambda:accessible('back'))  # application UI, independently of disk
        assert cli('config','get','toolbar.actions')==['back']
        audit=cli('config','inspect')['audit']
        assert audit[-1]['proposal']==proposal and audit[-1]['outcome']=='applied'
        assert audit[-1]['to_revision']==audit[-1]['from_revision']+1
        cli('browser','settings.approve',json.dumps(dict(proposal=proposal)),ok=False)
        cli('config','undo')
        wait(lambda:accessible('back') is None)
        assert cli('config','get','toolbar.actions')==[]
        assert cli('config','get','zoom')==1.2
        # A fake Codex app-server exercises the entire host -> bwrap -> native
        # approval route, with no account login or model call in CI.
        login=pathlib.Path(directory)/'codex-login'
        login.mkdir()
        (login/'auth.json').write_text('fixture-only')
        fake=pathlib.Path(directory)/'fake-codex'
        proposed=dict(session=session,revision=cli('config','inspect')['revision'],
                      reason='Fixture Codex host proposal',changes={'toolbar.actions':['back']})
        action=json.dumps(dict(action='call',method='settings.propose',params=json.dumps(proposed),answer=''))
        final=json.dumps(dict(action='final',method='',params='{}',answer='Waiting for native review.'))
        fake.write_text('''#!/usr/bin/env python3
import json, os, sys
assert 'DISPLAY' not in os.environ and 'XDG_RUNTIME_DIR' not in os.environ
assert open(os.path.join(os.environ['CODEX_HOME'],'auth.json')).read() == 'fixture-only'
def emit(value): print(json.dumps(value),flush=True)
step=0
for line in sys.stdin:
 msg=json.loads(line)
 if msg.get('method')=='initialize': emit({'id':msg['id'],'result':{}})
 elif msg.get('method')=='thread/start': emit({'id':msg['id'],'result':{'thread':{'id':'fixture'},'approvalPolicy':'never','activePermissionProfile':{'id':'nagi_browser','extends':None}}})
 elif msg.get('method')=='turn/start':
  assert 'sandboxPolicy' not in msg['params']
  emit({'id':msg['id'],'result':{'turn':{'id':str(step)}}})
  value = %r if step==0 else %r
  emit({'method':'item/completed','params':{'item':{'type':'agentMessage','phase':'final_answer','text':value}}})
  emit({'method':'turn/completed','params':{'turn':{'id':str(step),'status':'completed'}}})
  step+=1
''' % (action,final))
        fake.chmod(0o700)
        result=subprocess.run([sys.executable,str(root/'scripts/nagi_codex.py'),
                               'Put a back button in my toolbar','--nagi',binary,
                               '--codex',str(fake)],env=dict(env,CODEX_HOME=str(login)),
                              capture_output=True,text=True,timeout=45)
        assert result.returncode==0,result.stdout+result.stderr
        assert 'Waiting for native review' in result.stdout
        assert cli('config','get','toolbar.actions')==[]
        review(); click('Apply proposal')
        wait(lambda:accessible('back'))
        assert cli('config','get','toolbar.actions')==['back']
        cli('config','undo')
        wait(lambda:accessible('back') is None)
        # Explicit extension proposals bind the approved manifest digest.
        proposal=propose(extension=dict(id='fixture',command='configure'))
        review()
        manifest['description']='Changed after proposal'
        cli('extension','install',input=json.dumps(manifest))
        click('Apply proposal')
        outcome(proposal,'failed')
        assert cli('config','get','tabs.layout')=='Top'
        # Digest revocation deliberately terminates affected WebKit processes.
        # Begin the separate site-hook fixture in a fresh browser session.
        old_session=session
        app.terminate(); app.wait(timeout=5)
        app=subprocess.Popen([binary,'--agent-control','--extensions',origin+'/reattach'],env=env,stdout=log,stderr=log)
        wait(lambda:'/reattach' in requests)
        session=cli('browser','capabilities')['session']
        assert session!=old_session
        refused=cli('browser','settings.propose',json.dumps(dict(session=old_session,
                    revision=cli('config','inspect')['revision'],reason='Old session replay',
                    changes={'toolbar.actions':['back']})),ok=False)
        assert refused['error']=='Stale browser session',refused
        assert cli('config','get','toolbar.actions')==[]
        approve()
        attach=subprocess.Popen([binary,'browser','attach','{}'],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
        click('Allow reading and interaction')
        stdout,stderr=attach.communicate(timeout=20)
        assert attach.returncode==0,stderr
        tab=json.loads(stdout)['result']['tab']
        cli('browser','navigate',json.dumps({'tab':tab,'url':origin+'/again'}))
        cli('browser','wait',json.dumps({'tab':tab}))
        wait(lambda:'Extension active' in cli('browser','snapshot',json.dumps({'tab':tab}))['result']['text'])
        cli('browser','extension.run','{"extension":"fixture","command":"sidebar"}')
        time.sleep(1)
        assert '/leak' not in requests, requests
        subprocess.run(['import','-window','root',str(out/'extensions-panel.png')],env=env,check=True)
        # Reinstalling even identical code revokes the existing grant.
        cli('extension','install',input=json.dumps(manifest))
        wait(lambda:not cli('extension','list')[0]['enabled'])
        time.sleep(1)
        cli('browser','wait',json.dumps({'tab':tab}),ok=False)
        # The extension renderer can hang without hanging GTK; watchdog disables it.
        manifest['sites']=[]
        manifest['sidebar_html']='<h2>Watchdog fixture</h2><script>while(true){}</script>'
        cli('extension','install',input=json.dumps(manifest))
        subprocess.run([binary,'--extensions'],env=env,check=True)
        approve()
        cli('browser','extension.run','{"extension":"fixture","command":"sidebar"}')
        wait(lambda:not cli('extension','list')[0]['enabled'],seconds=10)
        assert app.poll() is None
        assert cli('browser','capabilities')['result']['api']==1
        proposal=propose({'toolbar.actions':['back']})
        review()
        cli('browser','stop')
        click('Apply proposal')
        assert cli('config','get','toolbar.actions')==[]
        outcome(proposal,'stopped')
        # Safe startup bypasses settings/extensions/control without rewriting them.
        preserved=(pathlib.Path(env['XDG_CONFIG_HOME'])/'nagi/extensions/grants.json').read_bytes()
        app.terminate(); app.wait(timeout=5)
        app=subprocess.Popen([binary,'--safe-mode','--agent-control',origin+'/safe'],env=env,stdout=log,stderr=log)
        wait(lambda:'/safe' in requests)
        cli('browser','capabilities',ok=False)
        assert (pathlib.Path(env['XDG_CONFIG_HOME'])/'nagi/extensions/grants.json').read_bytes()==preserved
    finally:
        app.terminate()
        try:app.wait(timeout=5)
        except subprocess.TimeoutExpired:app.kill();app.wait()
        log.close()
server.shutdown()
print('extension runtime boundaries passed')
