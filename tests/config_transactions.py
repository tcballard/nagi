"""Real CLI processes: atomic batches, optimistic conflicts and competing writers."""
import concurrent.futures
import json
import os
import pathlib
import subprocess
import tempfile
import time

binary = str(pathlib.Path('target/debug/nagi').resolve())
with tempfile.TemporaryDirectory(prefix='nagi-config-') as root:
    env = dict(os.environ, XDG_STATE_HOME=root)

    def cli(*args, ok=True):
        p = subprocess.run([binary, 'config', *args], env=env, text=True, capture_output=True)
        if ok:
            assert p.returncode == 0, p.stderr
            return json.loads(p.stdout)
        assert p.returncode != 0, p.stdout
        return json.loads(p.stderr)

    assert cli('schema')['api'] == 1
    preview = cli('apply', '{"tabs.layout":"Left","zoom":1.2}', '--dry-run')
    assert preview['settings']['tab_layout'] == 'Left'
    assert cli('get', 'tabs.layout') == 'Top'
    first = cli('apply', '{"tabs.layout":"Left","zoom":1.2}', '--if-revision', '0')
    assert first['revision'] == 1
    cli('set', 'zoom', '1.3', '--if-revision', '0', ok=False)
    cli('apply', '{"tabs.layout":"Top","zoom":9}', ok=False)
    assert cli('inspect')['revision'] == 1
    assert cli('get', 'tabs.layout') == 'Left'

    def writer(i):
        for _ in range(100):
            p = subprocess.run([binary, 'config', 'set', f'shortcuts.{i[0]}', i[1]], env=env, capture_output=True, text=True)
            if p.returncode == 0:
                return
            assert 'busy' in p.stderr, p.stderr
            time.sleep(.02)
        raise AssertionError('lock never released')

    edits = [('search', '<Control><Alt>p'), ('address', '<Control><Alt>o'), ('new', '<Control><Alt>n')]
    with concurrent.futures.ThreadPoolExecutor() as pool:
        list(pool.map(writer, edits))
    settings = cli('get')
    for key, value in edits:
        assert settings['shortcuts'][key] == value
    before = cli('inspect')
    undone = cli('undo', '--if-revision', str(before['revision']))
    assert undone['settings'] == before['history'][-1]['settings']
    path = pathlib.Path(root)/'nagi/settings.json'
    assert path.stat().st_mode & 0o777 == 0o600
    future = dict(undone, schema=99)
    path.write_text(json.dumps(future))
    cli('set', 'zoom', '1', ok=False)
    assert json.loads(path.read_text()) == future
print('configuration transactions passed')
