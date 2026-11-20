#!/usr/bin/env python3
"""Verify that the packaged winit desktop commits a software buffer on Wayland."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import stat
import subprocess
import tempfile
import time


MESSAGE = re.compile(r"\b(\w+)[@#](\d+)\.(\w+)\((.*)\)$")
BUFFER = re.compile(r"new id wl_buffer[@#](\d+), \d+, (\d+), (\d+), (\d+), ")
MAX_TRACE = 2 * 1024 * 1024


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def rendered_window(trace):
    surfaces, toplevels, titles, configures = {}, {}, {}, {}
    configured, buffers, attached = set(), {}, {}
    for line in trace.splitlines():
        match = MESSAGE.search(line)
        if match is None:
            continue
        interface, obj, method, args = match.groups()
        if (interface, method) == ("xdg_wm_base", "get_xdg_surface"):
            pair = re.fullmatch(r"new id xdg_surface[@#](\d+), wl_surface[@#](\d+)", args)
            if pair:
                shell, surface = pair.groups()
                surfaces[shell] = surface
        elif (interface, method) == ("xdg_surface", "get_toplevel"):
            toplevel = re.fullmatch(r"new id xdg_toplevel[@#](\d+)", args)
            if toplevel and obj in surfaces:
                toplevels[toplevel[1]] = surfaces[obj]
        elif (interface, method) == ("xdg_toplevel", "set_title") and obj in toplevels:
            titles[toplevels[obj]] = args
        elif (interface, method) == ("xdg_surface", "configure"):
            configures[obj] = args
        elif (interface, method) == ("xdg_surface", "ack_configure") and obj in surfaces:
            if configures.get(obj) == args:
                configured.add(surfaces[obj])
        elif (interface, method) == ("wl_shm_pool", "create_buffer"):
            buffer = BUFFER.match(args)
            if buffer:
                buf, width, height, stride = buffer.groups()
                buffers[buf] = tuple(map(int, (width, height, stride)))
        elif (interface, method) == ("wl_surface", "attach"):
            buffer = re.match(r"wl_buffer[@#](\d+), ", args)
            attached[obj] = buffers.get(buffer[1]) if buffer else None
        elif (interface, method) == ("wl_surface", "commit"):
            size = attached.pop(obj, None)
            if size and obj in configured and titles.get(obj) == '"Lurker"':
                width, height, stride = size
                if 300 <= width <= 8192 and 300 <= height <= 8192 and stride >= width * 4:
                    return {"surface_id": int(obj), "buffer_width": width,
                            "buffer_height": height, "buffer_stride": stride}
    return None


def private_environment(root):
    env = {key: value for key, value in os.environ.items()
           if not key.startswith(("LD_", "WAYLAND_", "LURKER_", "WINIT_"))
           and key not in ("DISPLAY", "XAUTHORITY", "DBUS_SESSION_BUS_ADDRESS")}
    for name in ("runtime", "home", "config", "cache", "data", "state"):
        (root / name).mkdir(mode=0o700)
    env.update(HOME=str(root / "home"), XDG_RUNTIME_DIR=str(root / "runtime"),
               XDG_CONFIG_HOME=str(root / "config"), XDG_CACHE_HOME=str(root / "cache"),
               XDG_DATA_HOME=str(root / "data"), XDG_STATE_HOME=str(root / "state"),
               XDG_SESSION_TYPE="wayland", SLINT_BACKEND="winit-software", LC_ALL="C")
    return env


def stop_owned(child):
    if child.poll() is None:
        try:
            os.killpg(child.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        try:
            child.wait(timeout=8)
        except subprocess.TimeoutExpired:
            try:
                os.killpg(child.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            child.wait()


def qualify(args, output, root):
    uid = os.getuid()
    result = {"status": "failed", "source_commit": args.commit, "uid": uid,
              "probe_sha256": digest(Path(__file__)), "route": "Wayland / winit-software"}
    children = []
    env = private_environment(root)
    socket = root / "runtime" / "lurker-test"

    def start(argv, logfile, process_env):
        with (output / logfile).open("xb") as log:
            child = subprocess.Popen(argv, env=process_env, stdin=subprocess.DEVNULL,
                                     stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        children.append(child)
        return child

    try:
        if uid == 0 or uid != args.expected_uid:
            raise ValueError("run as the declared non-root login user")
        binary = args.binary.resolve(strict=True)
        result["desktop_sha256"] = digest(binary)
        if result["desktop_sha256"] != args.binary_sha256:
            raise ValueError("desktop binary differs from the supplied artifact digest")
        weston = args.weston.resolve(strict=True)
        version = subprocess.check_output([str(weston), "--version"], env=env, text=True, timeout=10).strip()
        help_result = subprocess.run([str(weston), "--help"], env=env, capture_output=True, timeout=10)
        help_text = (help_result.stdout + help_result.stderr).decode(errors="replace")
        renderer = "--renderer=pixman" if "--renderer=" in help_text else "--use-pixman"
        backend = "headless-backend.so" if "headless-backend.so" in help_text else "headless"
        result.update(compositor=version, compositor_sha256=digest(weston),
                      compositor_renderer="pixman", x11_environment=False)
        server = start([str(weston), f"--backend={backend}", renderer, "--no-config",
                        "--idle-time=0", "--socket=lurker-test", "--width=1440", "--height=1000"],
                       "compositor.log", env)
        deadline = time.monotonic() + 20
        while not socket.is_socket():
            if server.poll() is not None or time.monotonic() >= deadline:
                raise RuntimeError("private compositor did not start; inspect compositor.log")
            time.sleep(0.1)
        app = start([str(binary)], "desktop.log", env | {"WAYLAND_DISPLAY": "lurker-test",
                                                       "WAYLAND_DEBUG": "client"})
        deadline = time.monotonic() + 20
        trace = output / "desktop.log"
        while time.monotonic() < deadline:
            if app.poll() is not None or server.poll() is not None:
                raise RuntimeError("desktop or compositor exited before a frame; inspect the logs")
            if trace.stat().st_size > MAX_TRACE:
                raise RuntimeError("desktop exceeded the bounded Wayland trace")
            frame = rendered_window(trace.read_text(errors="replace"))
            if frame:
                result.update(status="passed", **frame)
                break
            time.sleep(0.1)
        else:
            raise RuntimeError("Lurker did not commit a configured shared-memory window; inspect desktop.log")
    except (OSError, RuntimeError, ValueError, subprocess.SubprocessError, KeyboardInterrupt) as error:
        result.update(error=str(error) or type(error).__name__)
    finally:
        for child in reversed(children):
            stop_owned(child)
        result.update(children_reaped=all(child.poll() is not None for child in children),
                      socket_removed=not socket.exists())
        if result["status"] == "passed" and not (result["children_reaped"] and result["socket_removed"]):
            result.update(status="failed", error="private Wayland resources remain")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--binary-sha256", required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--expected-uid", type=int, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--weston", type=Path, default=Path("/usr/bin/weston"))
    args = parser.parse_args()
    if not re.fullmatch(r"[0-9a-f]{40}", args.commit) or not re.fullmatch(r"[0-9a-f]{64}", args.binary_sha256):
        parser.error("full source commit and artifact SHA256 are required")
    os.umask(0o077)
    args.output.mkdir(mode=0o700)
    if stat.S_IMODE(args.output.lstat().st_mode) != 0o700:
        raise ValueError("output must be private")

    def interrupted(_signal, _frame):
        raise KeyboardInterrupt

    signal.signal(signal.SIGTERM, interrupted)
    with tempfile.TemporaryDirectory(prefix="lurker-wayland-", dir="/tmp") as directory:
        result = qualify(args, args.output, Path(directory))
    path = args.output / "result.json"
    path.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({"status": result["status"], "result": str(path)}))
    return 0 if result["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
