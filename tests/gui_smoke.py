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
subprocess.run([str(ROOT/'scripts/install-icons.sh'),str(pathlib.Path(env['XDG_DATA_HOME'])/'icons/hicolor')],check=True,env=env)
subprocess.run(['/usr/bin/python3',str(ROOT/'tests/icon_lookup.py')],check=True,env=env)
requests=[]
class Fixture(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        requests.append(self.path)
        if self.path=='/favicon.svg':
            body=b'<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><rect width="16" height="16" rx="3" fill="#30bc80"/></svg>';self.send_response(200);self.send_header('Content-Type','image/svg+xml')
        elif self.path=='/download':
            body=b'Nagi download fixture.\n';self.send_response(200);self.send_header('Content-Type','application/octet-stream');self.send_header('Content-Disposition','attachment; filename="nagi-test.txt"')
        else:
            body=(ROOT/'tests/fixture.html').read_bytes() if self.path!='/second' else b'<title>Second page</title><h1>Second page</h1>'
            body+=b'<link rel="icon" href="/favicon.svg"><script>document.addEventListener("keydown",e=>{if(e.key==="F8"){e.preventDefault();fetch("/focus-check")}})</script>'
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
    # Summon from a web page, dismiss without navigation, and return key input
    # to WebKit. F8 is observed by our local fixture, not by a test-only app API.
    def assert_page_focus():
        count=requests.count('/focus-check');key('F8')
        wait_for(lambda:requests.count('/focus-check')>count)
    assert_page_focus()
    wait_for(lambda:'/favicon.svg' in requests)
    key('ctrl+alt+l')
    subprocess.run(['import','-window',window,str(OUT/'nagi-floating-address.png')],check=True,env=env)
    xd('type','--clearmodifiers','--delay','40','quiet places to read');time.sleep(.4)
    subprocess.run(['import','-window',window,str(OUT/'nagi-composer.png')],check=True,env=env)
    key('ctrl+a');xd('type','--clearmodifiers',url+'cancelled');key('Escape')
    assert_page_focus();assert '/cancelled' not in requests
    key('ctrl+l');xd('type','--clearmodifiers',url+'overlay-navigation');key('Return')
    wait_for(lambda:'/overlay-navigation' in requests);assert_page_focus()
    # Single-instance command summons the same window and does not add a tab.
    before=len(state()['tabs'])
    subprocess.run([str(BINARY),'--focus-address'],env=env,check=True,timeout=10)
    time.sleep(.4)
    assert len(xd('search','--onlyvisible','--name','Nagi').splitlines())==1
    xd('type','--clearmodifiers',url+'remote-summon');key('Tab');key('Return')
    wait_for(lambda:'/remote-summon' in requests)
    assert len(state()['tabs'])==before
    # Outside click dismisses; a narrow window still exposes the controls.
    key('ctrl+l');xd('mousemove','--window',window,'15','120');xd('click','1');time.sleep(.3)
    assert_page_focus()
    xd('windowsize',window,'520','600');time.sleep(.4);key('ctrl+l')
    subprocess.run(['import','-window',window,str(OUT/'nagi-floating-narrow.png')],check=True,env=env)
    key('Escape');assert_page_focus();xd('windowsize',window,'1180','800');time.sleep(.4)
    navigate(url);wait_for(lambda:requests.count('/')>=2)
    key('ctrl+d');wait_for(lambda:any(v['url']==url for v in state()['bookmarks']))
    key('ctrl+f');xd('type','quiet-water');key('Escape')
    key('ctrl+shift+r');time.sleep(1);subprocess.run(['import','-window',window,str(OUT/'nagi-reader.png')],check=True,env=env);key('ctrl+shift+r')
    key('ctrl+shift+n');navigate(url+'private-secret');wait_for(lambda:'/private-secret' in requests);time.sleep(1)
    assert not any('private-secret' in v['url'] for v in state()['history'])
    assert not any('private-secret' in v['url'] for v in state()['tabs'])
    key('ctrl+w');key('ctrl+t');navigate(url+'second')
    wait_for(lambda:any(v['url']==url+'second' for v in state()['history']))
    key('ctrl+w');key('ctrl+shift+t');wait_for(lambda:any(v['url']==url+'second' for v in state()['tabs']))
    # Suggestions switch to an existing tab without navigating or duplicating it.
    key('ctrl+t');before=len(state()['tabs']);key('ctrl+alt+l')
    xd('type','--clearmodifiers','Second page');time.sleep(.4)
    subprocess.run(['import','-window',window,str(OUT/'nagi-suggestions.png')],check=True,env=env)
    key('Down');key('Return');assert_page_focus()
    wait_for(lambda:'Second page' in xd('getwindowname',window))
    assert len(state()['tabs'])==before
    # History selection navigates with the same keyboard interaction.
    key('ctrl+alt+l');xd('type','--clearmodifiers','overlay-navigation');key('Down');key('Return')
    wait_for(lambda:requests.count('/overlay-navigation')>=2);assert_page_focus()
    # Exercise the real save dialog and WebKit download lifecycle.
    navigate(url+'download')
    dialog=wait_for(lambda:xd('search','--onlyvisible','--name','Save download').splitlines()[0])
    xd('windowfocus',dialog);key('ctrl+l');key('ctrl+a')
    destination=pathlib.Path(profile.name)/'download.txt'
    xd('type','--clearmodifiers',str(destination));key('Return')
    wait_for(lambda:destination.exists() and destination.read_bytes()==b'Nagi download fixture.\n')
    xd('windowfocus',window);key('Escape')
    navigate(url);wait_for(lambda:any(v['url']==url for v in state()['history']))
    key('ctrl+b');subprocess.run(['import','-window',window,str(OUT/'nagi-browser.png')],check=True,env=env);key('Escape')
    key('ctrl+t');key('Escape');subprocess.run(['import','-window',window,str(OUT/'nagi-welcome.png')],check=True,env=env)
    xd('windowsize',window,'1040','720');time.sleep(1)
    key('ctrl+q');p.wait(timeout=15);assert p.returncode==0
    saved=state();assert len(saved['bookmarks'])==1
    assert saved['window']['width']==1040 and saved['window']['height']==720
    p=subprocess.Popen([str(BINARY)],env=env,stdout=log,stderr=log)
    window=wait_for(lambda:xd('search','--onlyvisible','--name','Nagi').splitlines()[0]);xd('windowfocus',window)
    time.sleep(1);assert state()['tabs']==saved['tabs']
    geometry=xd('getwindowgeometry','--shell',window)
    assert 'WIDTH=1040' in geometry and 'HEIGHT=720' in geometry
    key('ctrl+q');p.wait(timeout=15);assert p.returncode==0
    result={'result':'pass','backend':'GTK X11 / Xvfb','checks':['local tab and history suggestions via keyboard','favicon requested from local fixture','window dimensions restored after restart','floating bar via Ctrl+Alt+L and Ctrl+L','Escape and outside-click dismissal with web focus restored','single-instance --focus-address without extra tab','narrow floating bar capture','HTTP page render','bookmark save','find action','reader round trip','private state exclusion','tab close/reopen','session save/reopen','download through native save dialog'],'profile':profile.name}
    (OUT/'gui-result.json').write_text(json.dumps(result,indent=2));print(json.dumps(result,indent=2))
finally:
    subprocess.run(['import','-window','root',str(OUT/'last-screen.png')],env=env)
    if p.poll() is None:p.terminate();p.wait(timeout=10)
    (OUT/'profile-files.json').write_text(json.dumps([str(f.relative_to(profile.name)) for f in pathlib.Path(profile.name).rglob('*') if f.is_file()],indent=2))
    server.shutdown();log.close();profile.cleanup()
