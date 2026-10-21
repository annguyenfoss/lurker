#!/usr/bin/env python3
"""Smoke-test the real winit/software event loop in a private virtual Wayland session.
Requires kwin_wayland and dbus-run-session. No host display or config is used.
"""
import os
import json
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time

binary = Path(sys.argv[1] if len(sys.argv) > 1 else 'target/release/lurker-desktop').resolve()
x11 = '--x11' in sys.argv[2:]
with tempfile.TemporaryDirectory(prefix='lurker-wayland-', dir='/tmp') as directory:
    root = Path(directory)
    env = os.environ.copy()
    for name in ['DISPLAY', 'WAYLAND_DISPLAY', 'DBUS_SESSION_BUS_ADDRESS']:
        env.pop(name, None)
    for key, name in [('XDG_RUNTIME_DIR', 'runtime'), ('XDG_CONFIG_HOME', 'config'), ('XDG_CACHE_HOME', 'cache'), ('XDG_DATA_HOME', 'data')]:
        path = root / name
        path.mkdir(mode=0o700)
        env[key] = str(path)
    env.update(QT_QPA_PLATFORM='offscreen', KWIN_COMPOSE='QPainter', XWAYLAND_NO_GLAMOR='1')
    compositor_log = open(root / 'compositor.log', 'w+')
    desktop_log = open(root / 'desktop.log', 'w+')
    command = ['dbus-run-session', '--', 'kwin_wayland', '--virtual', '--no-lockscreen', '--no-global-shortcuts', '--no-kactivities', '--width', '1340', '--height', '860', '--socket', 'lurker-test']
    if x11:
        capture = root / 'capture-display'
        capture.write_text('#!/usr/bin/env python3\nimport os, json\nfrom pathlib import Path\nPath(' + repr(str(root / 'display.json')) + ').write_text(json.dumps({k: os.environ[k] for k in ["DISPLAY", "XAUTHORITY"] if k in os.environ}))\n')
        capture.chmod(0o755)
        command.extend(['--xwayland', str(capture)])
    compositor = subprocess.Popen(command, env=env, stdout=compositor_log, stderr=subprocess.STDOUT, start_new_session=True)
    desktop = None
    try:
        deadline = time.monotonic() + 15
        while not (Path(env['XDG_RUNTIME_DIR']) / 'lurker-test').exists():
            if compositor.poll() is not None or time.monotonic() > deadline:
                compositor_log.seek(0)
                raise AssertionError('Virtual compositor did not start:\n' + compositor_log.read())
            time.sleep(0.1)
        app_env = env | {'WAYLAND_DISPLAY': 'lurker-test', 'WINIT_UNIX_BACKEND': 'wayland'}
        if x11:
            deadline = time.monotonic() + 15
            while not (root / 'display.json').exists():
                if compositor.poll() is not None or time.monotonic() > deadline:
                    compositor_log.seek(0)
                    raise AssertionError('Private Xwayland did not start:\n' + compositor_log.read())
                time.sleep(0.1)
            app_env.update(json.loads((root / 'display.json').read_text()))
            app_env.pop('WAYLAND_DISPLAY', None)
            app_env['WINIT_UNIX_BACKEND'] = 'x11'
        desktop = subprocess.Popen([str(binary)], env=app_env, stdout=desktop_log, stderr=subprocess.STDOUT, start_new_session=True)
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            if desktop.poll() is not None:
                desktop_log.seek(0)
                compositor_log.seek(0)
                raise AssertionError(f'Desktop exited {desktop.returncode}:\n' + desktop_log.read() + '\nCompositor:\n' + compositor_log.read())
            time.sleep(0.1)
        # A live process with a Wayland connection proves backend initialization.
        sockets = [os.readlink(path) for path in Path(f'/proc/{desktop.pid}/fd').iterdir() if path.exists()]
        assert any(path.startswith('socket:') for path in sockets), 'no Wayland connection'
        print(f'PASS: release desktop winit/software event loop on {"X11/Xwayland" if x11 else "Wayland"} in a private virtual session', flush=True)
    finally:
        for process in [desktop, compositor]:
            if process is not None:
                try:
                    os.killpg(process.pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait()
        compositor_log.close()
        desktop_log.close()
