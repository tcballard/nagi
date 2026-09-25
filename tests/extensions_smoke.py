"""Native consent, CSP, site hooks, revocation and hung renderer recovery."""
import http.server
import json
import os
import pathlib
import subprocess
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
        click('Undo last settings change')
        wait(lambda:cli('config','get','tabs.layout')=='Top')
        assert cli('config','get','zoom')==1.1, 'Undo reverted more than the approved transaction'
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
