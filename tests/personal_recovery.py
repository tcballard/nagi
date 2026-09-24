"""Upgrade/export/import and interrupted CLI writes preserve a usable profile."""
import fcntl
import json
import os
import pathlib
import subprocess
import tempfile
import time

binary = str(pathlib.Path('target/debug/nagi').resolve())
with tempfile.TemporaryDirectory(prefix='nagi-recovery-') as directory:
    env = dict(os.environ, XDG_STATE_HOME=directory)
    def cli(*args, input=None, ok=True):
        result = subprocess.run([binary,*args],env=env,input=input,text=True,capture_output=True,timeout=5)
        assert (result.returncode==0)==ok, result.stdout+result.stderr
        return json.loads(result.stdout if ok else result.stderr)
    state = pathlib.Path(directory)/'nagi'
    state.mkdir()
    # This is the pre-transaction flat configuration shipped by PR #3.
    (state/'settings.json').write_text('{"search":"Google","tab_layout":"Left"}')
    cli('config','apply','{"layout.density":"Compact","appearance.accent":"#123456"}')
    profile = cli('profile','export','My browser')
    assert profile['settings']['search']=='Google'
    assert profile['settings']['tab_layout']=='Left'
    cli('config','set','tabs.layout','Top')
    cli('profile','check',input=json.dumps(profile))
    assert cli('config','get','tabs.layout')=='Top'
    cli('profile','apply',input=json.dumps(profile))
    assert cli('config','get')==profile['settings']
    original=(state/'settings.json').read_bytes()
    cli('profile','apply',input=json.dumps(dict(profile,schema=99)),ok=False)
    assert (state/'settings.json').read_bytes()==original
    unknown=json.loads(json.dumps(profile))
    unknown['settings']['future_key']='preserve me'
    cli('profile','apply',input=json.dumps(unknown),ok=False)
    assert (state/'settings.json').read_bytes()==original
    # An externally held lock produces a bounded error and no lost write.
    with (state/'settings.lock').open('r+') as lock:
        fcntl.flock(lock,fcntl.LOCK_EX)
        assert 'busy' in cli('config','set','zoom','1.2',ok=False)['error']
    # Terminate actual writer processes at different points; each committed
    # document must remain parseable, and the OS must release the writer lock.
    for i in range(20):
        child=subprocess.Popen([binary,'config','set','zoom',str(1+i%5/10)],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        time.sleep((i%4)*.003)
        if child.poll() is None: child.kill()
        child.wait()
        document=cli('config','inspect')
        assert document['schema']==1
        assert document['settings']['search']=='Google'
        cli('config','set','zoom','1.0')
print('personalisation upgrade and interrupted-write recovery passed')
