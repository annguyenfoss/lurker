#!/usr/bin/env python3
"""Qualify a real X11/winit window as the login user, without storage operations."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import secrets
import signal
import stat
import struct
import subprocess
import tempfile
import time


class PrerequisiteError(Exception):
    pass


def system_tool(name):
    for directory in ("/usr/X11R7/bin", "/usr/X11R6/bin", "/usr/local/bin", "/usr/pkg/bin", "/usr/bin"):
        path = Path(directory) / name
        if path.is_file() and os.access(path, os.X_OK):
            return str(path)
    raise PrerequisiteError("Missing X11 test tool: " + name)


def private_output(requested=None):
    temporary = Path(tempfile.gettempdir()).resolve()
    if requested is None:
        return Path(tempfile.mkdtemp(prefix="lurker-desktop-window-", dir=temporary))
    path = Path(requested).absolute()
    if not path.parent.resolve().is_relative_to(temporary):
        raise ValueError("output must have an existing parent under the temporary directory")
    path.mkdir(mode=0o700, exist_ok=False)
    info = path.lstat()
    if info.st_uid != os.getuid() or stat.S_IMODE(info.st_mode) != 0o700:
        raise ValueError("output must be a private directory owned by the test user")
    return path.resolve()


def frame_metrics(data):
    if len(data) < 100:
        raise ValueError("truncated XWD header")
    fields = struct.unpack(">25I", data[:100])
    header_size, version, width, height = fields[0], fields[1], fields[4], fields[5]
    stride, bits, colors = fields[12], fields[11], fields[19]
    if version != 7 or header_size < 100 or bits not in (16, 24, 32):
        raise ValueError("unsupported XWD image format")
    step = (bits + 7) // 8
    offset = header_size + colors * 12
    if not (300 <= width <= 8192 and 300 <= height <= 8192) or stride < width * step:
        raise ValueError("invalid or unexpectedly small window geometry")
    if offset + stride * height > len(data):
        raise ValueError("truncated XWD pixel data")
    pixels = memoryview(data)[offset:]
    samples = {bytes(pixels[y * stride + x * step:y * stride + (x + 1) * step])
               for y in range(0, height, 2) for x in range(0, width, 2)}
    if len(samples) <= 32:
        raise ValueError("window did not produce sufficiently varied rendered pixels")
    return {"pixel_width": width, "pixel_height": height, "sampled_pixel_colors": len(samples),
            "frame_sha256": hashlib.sha256(data).hexdigest(), "frame_bytes": len(data)}


def stop_owned(child):
    if child.poll() is None:
        try:
            os.killpg(child.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        try:
            child.wait(timeout=8)
        except subprocess.TimeoutExpired:
            os.killpg(child.pid, signal.SIGKILL)
            child.wait()


def display_environments(display, auth, libraries):
    app = {key: value for key, value in os.environ.items() if not key.startswith("LD_")}
    app.update(DISPLAY=display, XAUTHORITY=str(auth), SLINT_BACKEND="winit-software", XDG_SESSION_TYPE="x11")
    app.pop("WAYLAND_DISPLAY", None)
    server = app.copy()
    if libraries:
        server["LD_LIBRARY_PATH"] = ":".join(str(path.resolve(strict=True)) for path in libraries)
    return app, server


def probe(args, output):
    result = {"source_commit": args.commit, "uid": os.getuid(), "route": "real winit X11 window/software renderer",
              "storage_operations": False, "probe_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    children, logs = [], []
    auth, display = output / "Xauthority", None
    try:
        if os.getuid() == 0 or os.getuid() != args.expected_uid:
            raise ValueError("run as the declared non-root login user")
        binary = args.binary.resolve(strict=True)
        with binary.open("rb") as stream:
            digest = hashlib.file_digest(stream, "sha256").hexdigest()
        if digest != args.binary_sha256:
            raise ValueError("desktop binary differs from the supplied artifact digest")
        result["desktop_sha256"] = digest
        tools = {name: system_tool(name) for name in ("xauth", "xdpyinfo", "xwininfo", "xwd")}
        tools["Xvfb"] = str(args.xvfb.resolve(strict=True)) if args.xvfb else system_tool("Xvfb")
        if args.xvfb:
            with Path(tools["Xvfb"]).open("rb") as stream:
                server_digest = hashlib.file_digest(stream, "sha256").hexdigest()
            if server_digest != args.xvfb_sha256:
                raise ValueError("private Xvfb differs from the supplied signed-package artifact digest")
            result["private_server_sha256"] = server_digest
        result["tools"] = tools
        result["server_note"] = args.server_note
        number = next((n for n in range(87, 187) if not Path(f"/tmp/.X{n}-lock").exists()
                       and not Path(f"/tmp/.X11-unix/X{n}").exists()), None)
        if number is None:
            raise PrerequisiteError("no unused private X display number available")
        display = f":{number}"
        # The cookie travels only over stdin and is never included in results/logs.
        subprocess.run([tools["xauth"], "-f", str(auth)], text=True,
                       input=f"add {display} MIT-MAGIC-COOKIE-1 {secrets.token_hex(16)}\n",
                       capture_output=True, check=True, timeout=10)
        auth.chmod(0o600)
        env, server_env = display_environments(display, auth, args.server_library_path)
        result["loader_environment_overrides"] = False

        def launch(argv, filename, child_env=None):
            log = (output / filename).open("wb")
            logs.append(log)
            child = subprocess.Popen(argv, env=child_env or env, stdin=subprocess.DEVNULL,
                                     stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
            children.append(child)
            return child

        def run(argv):
            return subprocess.run(argv, env=env, capture_output=True, timeout=10, check=True)

        extension_options = []
        if args.disable_extension:
            help_result = subprocess.run([tools["Xvfb"], "-help"], env=server_env, capture_output=True, timeout=10)
            help_text = (help_result.stdout + help_result.stderr).decode(errors="replace")
            (output / "server-options.log").write_text(help_text)
            if "-extension" not in help_text:
                raise ValueError("Xvfb does not advertise extension-disable support")
            for name in args.disable_extension:
                if not re.search(r"(?<![A-Za-z0-9_-])" + re.escape(name) + r"(?![A-Za-z0-9_-])", help_text):
                    raise ValueError("Xvfb does not advertise extension " + name)
                extension_options += ["-extension", name]
        server = launch([tools["Xvfb"], display, "-screen", "0", "1280x900x24", "-nolisten", "tcp",
                         "-auth", str(auth), "-noreset", *extension_options], "xvfb.log", server_env)
        deadline = time.monotonic() + 20
        while True:
            if server.poll() is not None:
                raise RuntimeError("private Xvfb exited; inspect xvfb.log")
            ready = subprocess.run([tools["xdpyinfo"]], env=env, capture_output=True, timeout=3)
            if ready.returncode == 0:
                (output / "display-info.log").write_bytes(ready.stdout)
                break
            if time.monotonic() > deadline:
                raise RuntimeError("private authenticated X display did not become ready")
            time.sleep(0.2)
        if args.disable_extension:
            extensions = run([tools["xdpyinfo"], "-queryExtensions"]).stdout.decode(errors="replace")
            (output / "server-extensions.log").write_text(extensions)
            for name in args.disable_extension:
                if re.search(r"(?m)^\s*" + re.escape(name) + r"(?:\s+\(|\s*$)", extensions):
                    raise ValueError("Xvfb still exposes disabled extension " + name)
            result["disabled_extensions_verified"] = args.disable_extension
        app = launch([str(binary)], "desktop.log")
        result.update(children={"xvfb": server.pid, "desktop": app.pid}, tcp_disabled=True)
        deadline = time.monotonic() + 20
        window = None
        while time.monotonic() < deadline:
            if app.poll() is not None:
                raise RuntimeError("desktop exited before creating an X11 window; inspect desktop.log")
            tree = run([tools["xwininfo"], "-root", "-tree"]).stdout.decode(errors="replace")
            (output / "window-tree.log").write_text(tree)
            match = re.search(r'(0x[0-9a-fA-F]+)\s+"' + re.escape(args.title) + '"', tree)
            if match:
                window = match.group(1)
                break
            time.sleep(0.3)
        if window is None:
            raise RuntimeError("desktop X11 window not found")
        time.sleep(2)
        frame = output / "frame.xwd"
        run([tools["xwd"], "-silent", "-id", window, "-out", str(frame)])
        if frame.stat().st_size > 64 * 1024 * 1024:
            raise ValueError("rendered frame exceeds the bounded evidence size")
        result.update(frame_metrics(frame.read_bytes()))
        if app.poll() is not None:
            raise RuntimeError("desktop exited during rendering")
        result.update(status="passed", window_id=window)
    except PrerequisiteError as error:
        result.update(status="blocked", error=str(error))
    except KeyboardInterrupt:
        result.update(status="failed", error="window probe interrupted")
    except Exception as error:
        result.update(status="failed", error=str(error))
    finally:
        for child in reversed(children):
            stop_owned(child)
        for log in logs:
            log.close()
        if auth.exists():
            auth.unlink()
        result.update(children_reaped=all(child.poll() is not None for child in children), private_auth_removed=not auth.exists())
        if display:
            number = display[1:]
            result.update(private_x_lock_removed=not Path(f"/tmp/.X{number}-lock").exists(),
                          private_x_socket_removed=not Path(f"/tmp/.X11-unix/X{number}").exists())
            if result.get("status") == "passed" and not all(result[key] for key in
                    ("children_reaped", "private_auth_removed", "private_x_lock_removed", "private_x_socket_removed")):
                result.update(status="failed", error="owned X11 resources remain after cleanup")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--binary-sha256", required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--expected-uid", type=int, required=True)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--title", default="Lurker")
    parser.add_argument("--xvfb", type=Path)
    parser.add_argument("--xvfb-sha256")
    parser.add_argument("--server-library-path", action="append", type=Path, default=[])
    parser.add_argument("--disable-extension", action="append", default=[])
    parser.add_argument("--server-note", default="system Xvfb; renderer qualification only")
    args = parser.parse_args()
    if not re.fullmatch(r"[0-9a-f]{40}", args.commit) or not re.fullmatch(r"[0-9a-f]{64}", args.binary_sha256):
        parser.error("full source commit and artifact SHA256 are required")
    if bool(args.xvfb) != bool(args.xvfb_sha256) or (args.xvfb_sha256 and not re.fullmatch(r"[0-9a-f]{64}", args.xvfb_sha256)):
        parser.error("a private Xvfb requires its exact artifact SHA256")
    if args.server_library_path and not args.xvfb:
        parser.error("server-only library paths require an explicitly verified private Xvfb")
    if any(not p.is_absolute() or not p.is_dir() for p in args.server_library_path):
        parser.error("server library paths must be existing absolute directories")
    if any(not re.fullmatch(r"[A-Za-z0-9_-]{1,64}", name) for name in args.disable_extension):
        parser.error("invalid extension name")
    os.umask(0o077)

    def interrupted(_signal, _frame):
        raise KeyboardInterrupt

    signal.signal(signal.SIGTERM, interrupted)
    output = private_output(args.output)
    result = probe(args, output)
    path = output / "result.json"
    temp = output / "result.json.tmp"
    temp.write_text(json.dumps(result, indent=2) + "\n")
    temp.replace(path)
    print(json.dumps({"status": result["status"], "result": str(path)}))
    return {"passed": 0, "failed": 1, "blocked": 2}[result["status"]]


if __name__ == "__main__":
    raise SystemExit(main())
