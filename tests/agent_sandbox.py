"""Exercise the real bubblewrap boundary; unsupported namespaces fail this test."""
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import threading

binary = str(Path('target/debug/nagi').resolve())
with tempfile.TemporaryDirectory(prefix='nagi-host-secret-') as temporary:
    root = Path(temporary)
    runtime = root / 'runtime'
    (runtime / 'nagi-control').mkdir(parents=True)
    secret = root / 'private'
    secret.write_text('host-only')
    settings = root / 'state/nagi/settings.json'
    settings.parent.mkdir(parents=True)
    settings.write_text('{"zoom":1.0}')
    endpoint = socket.socket(socket.AF_UNIX)
    endpoint.bind(str(runtime / 'nagi-control/browser.sock'))
    endpoint.listen()
    def respond():
        connection, _ = endpoint.accept()
        with connection:
            assert connection.recv(4096) == b'probe\n'
            connection.sendall(b'only-approved-socket\n')
    worker = threading.Thread(target=respond, daemon=True)
    worker.start()
    network = socket.socket()
    network.bind(('127.0.0.1', 0))
    network.listen()
    code = '''
import json, os, pathlib, socket, subprocess
assert 'DISPLAY' not in os.environ
assert 'WAYLAND_DISPLAY' not in os.environ
assert 'DBUS_SESSION_BUS_ADDRESS' not in os.environ
assert 'NAGI_TEST_SECRET' not in os.environ
assert not pathlib.Path(HOST_SECRET).exists()
assert not pathlib.Path('/tmp/.X11-unix').exists()
assert not pathlib.Path('/run/user').exists()
assert not pathlib.Path('/dev/input').exists()
assert not pathlib.Path('/proc/' + str(HOST_PID)).exists()
s = socket.socket()
try:
    s.connect(('127.0.0.1', HOST_PORT))
except OSError:
    pass
else:
    raise AssertionError('Reached host network')
control = socket.socket(socket.AF_UNIX)
control.connect('/run/nagi-control/browser.sock')
control.sendall(b'probe\\n')
assert control.recv(100) == b'only-approved-socket\\n'
# Clearing flags or running config/profile commands cannot reach host settings.
r = subprocess.run(['/opt/nagi/nagi', 'config', 'set', 'zoom', '1.2'], capture_output=True)
assert r.returncode == 0, r.stderr
assert pathlib.Path('/home/agent/.local/state/nagi/settings.json').exists()
print(json.dumps({'isolated':True,'socket':True,'host_unchanged':True}))
'''
    code = f'HOST_SECRET={str(secret)!r}\nHOST_PID={os.getpid()}\nHOST_PORT={network.getsockname()[1]}\n' + code
    env = dict(os.environ, XDG_RUNTIME_DIR=str(runtime), XDG_STATE_HOME=str(root/'state'), NAGI_TEST_SECRET='do-not-inherit')
    result = subprocess.run([binary, 'agent-run', '--', '/usr/bin/python3', '-'],
                            input=code, capture_output=True, text=True, env=env, timeout=30)
    assert result.returncode == 0, result.stdout + result.stderr
    assert json.loads(result.stdout)['isolated']
    assert secret.read_text() == 'host-only'
    assert settings.read_text() == '{"zoom":1.0}'
    worker.join(timeout=2)
    assert not worker.is_alive()
    endpoint.close()
    network.close()
print('real agent sandbox isolation passed')
