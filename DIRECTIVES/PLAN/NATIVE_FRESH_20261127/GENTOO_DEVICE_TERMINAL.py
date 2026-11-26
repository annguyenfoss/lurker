#!/usr/bin/env python3
"""Frozen 576 PTY primitive plus authorized scratch-device Quick Format acknowledgement."""
import importlib.util
from pathlib import Path
spec=importlib.util.spec_from_file_location('shared',Path(__file__).with_name('test-filesystem-cli.py'))
shared=importlib.util.module_from_spec(spec);spec.loader.exec_module(shared)
for name in ('pty','os','select','errno','time','termios','re','MAX_TRANSCRIPT','PROMPT','ECHO_STABLE_SECONDS','ECHO_WAIT_TIMEOUT','redact','stop_owned_child'):
    globals()[name]=getattr(shared,name)

def terminal(args, password, timeout, log, authorize_quick=False):
    """Drive real CLI/native password prompts over its own controlling TTY.

    Only the process we forked is signalled on timeout. Its storage is retained
    for investigation; killing a CLI is never treated as successful cleanup.
    """
    pid, master = pty.fork()
    if pid == 0:
        try:
            os.execv(args[0], args)
        except OSError as error:
            try:
                os.write(2, f"Cannot execute CLI: {error.strerror}\n".encode())
            finally:
                os._exit(127)
    transcript = bytearray()
    consumed = 0
    prompts = 0
    quick_confirmations = 0
    deadline = time.monotonic() + timeout
    finished = False
    timed_out = False
    eof = False
    status = None
    pending_prompt = None
    echo_off_since = None
    try:
        while time.monotonic() < deadline:
            ready = [] if eof else select.select([master], [], [], 0.01 if pending_prompt else 0.1)[0]
            if ready:
                try:
                    chunk = os.read(master, 65536)
                except OSError as error:
                    if error.errno != errno.EIO:
                        raise
                    chunk = b""
                if not chunk:
                    eof = True
                else:
                    transcript.extend(chunk)
                    if len(transcript) > MAX_TRANSCRIPT:
                        raise RuntimeError("CLI exceeded bounded transcript limit")
                    # sudo may prefix a native prompt with OSC 3008 session
                    # metadata. Match displayed text; retain raw, redacted logs.
                    visible = re.sub(rb"\x1b\].*?(?:\x07|\x1b\\)", b"", bytes(transcript), flags=re.S)
                    visible = re.sub(rb"\x1b\[[0-?]*[ -/]*[@-~]", b"", visible)
                    quick = b"Are you sure you want to use Quick Format? (y=Yes/n=No) [No]:"
                    if authorize_quick and quick_confirmations == 0 and quick in visible[consumed:]:
                        # The caller already verified this new scratch disk and --force.
                        # This public acknowledgement never supplies an admin credential.
                        time.sleep(0.05)
                        os.write(master, b"y\n")
                        quick_confirmations += 1
                        consumed = len(visible)
                    match = None if pending_prompt else PROMPT.search(visible[consumed:])
                    if match:
                        # This suite never supplies administrator credentials.
                        if match.group("authorization"):
                            raise RuntimeError("Unexpected authorization prompt; run with the explicit required privilege route")
                        if password is None:
                            raise RuntimeError("Unexpected credential prompt")
                        consumed += match.end()
                        pending_prompt = time.monotonic()
                        echo_off_since = None
            child, status = os.waitpid(pid, os.WNOHANG)
            if child:
                finished = True
                break
            if pending_prompt is not None:
                # cryptsetup may print before tcsetattr(TCSAFLUSH) disables
                # echo and flushes queued input. Prompt text is not readiness.
                now = time.monotonic()
                echo_off = not (termios.tcgetattr(master)[3] & termios.ECHO)
                if echo_off:
                    if echo_off_since is None:
                        echo_off_since = now
                    if now - echo_off_since >= ECHO_STABLE_SECONDS:
                        prompts += 1
                        if prompts > 8:
                            raise RuntimeError("CLI exceeded password prompt limit")
                        os.write(master, password.encode() + b"\n")
                        pending_prompt = None
                        echo_off_since = None
                else:
                    echo_off_since = None
                if pending_prompt is not None and now - pending_prompt >= ECHO_WAIT_TIMEOUT:
                    raise RuntimeError("Credential prompt never disabled terminal echo; no credential sent")
            if eof:
                time.sleep(0.02)
        if not finished:
            timed_out = True
            status = stop_owned_child(pid)
            finished = True
        # Drain already available output after the child has exited.
        while not eof and select.select([master], [], [], 0)[0]:
            try:
                chunk = os.read(master, 65536)
            except OSError as error:
                if error.errno != errno.EIO:
                    raise
                break
            if not chunk:
                break
            transcript.extend(chunk)
            if len(transcript) > MAX_TRANSCRIPT:
                raise RuntimeError("CLI exceeded bounded transcript limit")
        if password and password.encode() in transcript:
            raise RuntimeError("CLI exposed a credential in its terminal output")
        return {"exit_code": os.waitstatus_to_exitcode(status), "prompts": prompts,
                "timed_out": timed_out, "quick_format_confirmations": quick_confirmations}
    finally:
        if not finished:
            # Direct owned child only, never an inferred PID/session or VM.
            try:
                stop_owned_child(pid)
            except (ProcessLookupError, ChildProcessError):
                pass
        os.close(master)
        log.write_text(redact(bytes(transcript), [password]))
        os.chmod(log, 0o600)
