"""Manifest validation and installation never silently grant execution."""
import json
import os
import pathlib
import subprocess
import tempfile

root = pathlib.Path(__file__).resolve().parent.parent
binary = str(root/'target/debug/nagi')
example = json.loads((root/'examples/extensions/research.json').read_text())
with tempfile.TemporaryDirectory() as directory:
    env = dict(os.environ, XDG_CONFIG_HOME=directory, XDG_STATE_HOME=directory+'/state')
    def cli(command, manifest=None, ok=True):
        p = subprocess.run([binary, 'extension', command], env=env, input=json.dumps(manifest) if manifest else '', text=True, capture_output=True)
        assert (p.returncode==0)==ok, p.stdout+p.stderr
        return json.loads(p.stdout if ok else p.stderr)
    assert cli('check', example)['valid']
    assert not (pathlib.Path(directory)/'nagi/extensions/research').exists()
    cli('install', example)
    assert cli('list')[0]['enabled'] is False
    path = pathlib.Path(directory)/'nagi/extensions/research/manifest.json'
    assert path.stat().st_mode & 0o777 == 0o600
    original = path.read_bytes()
    cli('install', dict(example, api=99), ok=False)
    cli('install', dict(example, id='../escape'), ok=False)
    cli('install', dict(example, sites=[{'origin':'https://example.com/path','script':'true'}]), ok=False)
    assert path.read_bytes()==original
    cli('install', example)
    assert cli('list')[0]['enabled'] is False
print('extension install boundaries passed')
