#!/usr/bin/env python3
"""Real GTK/X11 smoke checks. Run under dbus-run-session + xvfb-run.
All web content is served by a local fixture. This is not Hyprland acceptance.
"""
import http.server, json, os, pathlib, subprocess, sys, tempfile, threading, time
ROOT=pathlib.Path(__file__).resolve().parents[1]
BINARY=pathlib.Path(sys.argv[1] if len(sys.argv)>1 else ROOT/'target/debug/nagi').resolve()
OUT=ROOT/'target/evidence'; OUT.mkdir(parents=True,exist_ok=True)
profile=tempfile.TemporaryDirectory(prefix='nagi-gui-')
env=os.environ.copy()
for name,folder in [('XDG_CONFIG_HOME','config'),('XDG_DATA_HOME','data'),('XDG_STATE_HOME','state'),('XDG_CACHE_HOME','cache')]:
    env[name]=str(pathlib.Path(profile.name)/folder)
env['GDK_BACKEND']='x11'
class Fixture(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path=='/download':
            body=b'Nagi download fixture.\n';self.send_response(200);self.send_header('Content-Type','application/octet-stream');self.send_header('Content-Disposition','attachment; filename="nagi-test.txt"')
        else:
            body=(ROOT/'tests/fixture.html').read_bytes() if self.path!='/second' else b'<title>Second page</title><h1>Second page</h1>'
            self.send_response(200);self.send_header('Content-Type','text/html; charset=utf-8')
        self.send_header('Content-Length',str(len(body)));self.end_headers();self.wfile.write(body)
    def log_message(self,*args):pass
server=http.server.ThreadingHTTPServer(('127.0.0.1',0),Fixture)
threading.Thread(target=server.serve_forever,daemon=True).start()
url=f'http://127.0.0.1:{server.server_port}/'
log=(OUT/'gui.log').open('w')
p=subprocess.Popen([str(BINARY),url],env=env,stdout=log,stderr=log)
def wait_for(fn,seconds=20):
    end=time.time()+seconds
    while time.time()<end:
        if p.poll() is not None:raise AssertionError(f'browser exited {p.returncode}; see gui.log')
        try:
            value=fn()
            if value:return value
        except (FileNotFoundError,json.JSONDecodeError,subprocess.CalledProcessError):pass
        time.sleep(.15)
    raise AssertionError('Timed out')
def xd(*args):return subprocess.check_output(['xdotool',*args],env=env,text=True).strip()
def key(k):xd('key','--clearmodifiers',k);time.sleep(.4)
def navigate(uri):key('ctrl+l');xd('type','--clearmodifiers','--delay','1',uri);key('Return')
def state():return json.loads((pathlib.Path(env['XDG_STATE_HOME'])/'nagi/state.json').read_text())
try:
    window=wait_for(lambda:xd('search','--onlyvisible','--name','Nagi').splitlines()[0]);xd('windowfocus',window)
    wait_for(lambda:any(v['url']==url for v in state()['history']))
    key('ctrl+d');wait_for(lambda:any(v['url']==url for v in state()['bookmarks']))
    key('ctrl+f');xd('type','quiet-water');key('Escape')
    key('ctrl+shift+r');time.sleep(1);key('ctrl+shift+r')
    key('ctrl+shift+n');navigate(url+'private-secret');time.sleep(1)
    assert not any('private-secret' in v['url'] for v in state()['history'])
    assert not any('private-secret' in v['url'] for v in state()['tabs'])
    key('ctrl+w');key('ctrl+t');navigate(url+'second')
    wait_for(lambda:any(v['url']==url+'second' for v in state()['history']))
    key('ctrl+w');key('ctrl+shift+t');wait_for(lambda:any(v['url']==url+'second' for v in state()['tabs']))
    key('ctrl+b');subprocess.run(['import','-window','root',str(OUT/'nagi-browser.png')],check=True,env=env);key('Escape')
    key('ctrl+t');subprocess.run(['import','-window','root',str(OUT/'nagi-welcome.png')],check=True,env=env)
    key('ctrl+q');p.wait(timeout=15);assert p.returncode==0
    saved=state();assert len(saved['bookmarks'])==1
    p=subprocess.Popen([str(BINARY)],env=env,stdout=log,stderr=log)
    window=wait_for(lambda:xd('search','--onlyvisible','--name','Nagi').splitlines()[0]);xd('windowfocus',window)
    time.sleep(1);assert state()['tabs']==saved['tabs']
    key('ctrl+q');p.wait(timeout=15);assert p.returncode==0
    result={'result':'pass','backend':'GTK X11 / Xvfb','checks':['HTTP page render','bookmark save','find action','reader round trip','private state exclusion','tab close/reopen','session save/reopen'],'profile':profile.name}
    (OUT/'gui-result.json').write_text(json.dumps(result,indent=2));print(json.dumps(result,indent=2))
finally:
    if p.poll() is None:p.terminate();p.wait(timeout=10)
    server.shutdown();log.close();profile.cleanup()
