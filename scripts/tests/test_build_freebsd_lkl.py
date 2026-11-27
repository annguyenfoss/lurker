import hashlib
import importlib.util
import os
from pathlib import Path
import platform
import subprocess
import tempfile
import unittest
from unittest import mock


SCRIPT = Path(__file__).resolve().parents[1] / "build-freebsd-lkl.py"
SPEC = importlib.util.spec_from_file_location("build_freebsd_lkl", SCRIPT)
BUILD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BUILD)


class ExternalHostToolInputs(unittest.TestCase):
    def test_restage_rejects_foreign_patch_and_corrupted_binary_before_output(self):
        import json
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            previous = root / "previous"
            previous.mkdir()
            (previous / "lklfuse").write_bytes(b"verified-native-tool")
            original = {"source_revision": BUILD.REVISION, "source_sha256": BUILD.SOURCE_SHA256,
                        "port_revision": BUILD.PORT_REVISION, "port_patches": BUILD.PORT_PATCHES,
                        "host_patch_sha256": BUILD.HOST_PATCH_SHA256,
                        "protocol": "freebsd-lkl-host-io-1",
                        "binary_sha256": BUILD.digest(previous / "lklfuse"), "compiler": "native"}
            for key, value in [("source_revision", "foreign"), ("host_patch_sha256", "0" * 64),
                               ("binary_sha256", "f" * 64), ("protocol", "different")]:
                (previous / "host-tool.json").write_text(json.dumps({**original, key: value}))
                with self.subTest(key=key), mock.patch.object(BUILD.subprocess, "check_output", return_value="native"):
                    with self.assertRaisesRegex(RuntimeError, "exact verified"):
                        BUILD.restage(root / "output", previous, Path("/native/compiler"))
                self.assertFalse((root / "output").exists())
                self.assertEqual((previous / "lklfuse").read_bytes(), b"verified-native-tool")

    @unittest.skipUnless(platform.system() == "FreeBSD", "requires actual native host provider")
    def test_native_host_fuse_metadata_rejects_wrong_abi_or_loader_provider(self):
        binary = os.environ.get("LURKER_FREEBSD_LKL_BINARY")
        if not binary:
            self.skipTest("set verified native tool binary")
        metadata = BUILD.host_fuse_metadata(Path(binary))
        self.assertEqual(metadata["library"], "libfuse.so.2")
        self.assertEqual(metadata["sha256"], BUILD.digest(Path(metadata["path"]).resolve()))
        with mock.patch.object(BUILD.subprocess, "check_output", return_value="(SONAME) [libfuse.so.3]"):
            with self.assertRaisesRegex(RuntimeError, "wrong native SONAME"):
                BUILD.host_fuse_metadata(Path(binary))
        with mock.patch.object(BUILD.subprocess, "check_output", side_effect=[
                "(SONAME) [libfuse.so.2]", "(NEEDED) [libfuse.so.2]",
                "libfuse.so.2 => /wrong/provider"]):
            with self.assertRaisesRegex((RuntimeError, FileNotFoundError), "different|No such"):
                BUILD.host_fuse_metadata(Path(binary))

    def test_kernel_reuse_rejects_changed_source_set_and_kernel_inputs(self):
        previous = {"arch/lkl/Kconfig": {"sha256": "kernel"},
                    "tools/lkl/lklfuse.c": {"sha256": "client"},
                    "tools/lkl/lib/virtio_blk.c": {"sha256": "host"}}
        current = {**previous, "tools/lkl/lklfuse.c": {"sha256": "new-client"},
                   "tools/lkl/lib/virtio_blk.c": {"sha256": "sticky-host-io"}}
        self.assertEqual(BUILD.kernel_source_changes(previous, current),
                         ["tools/lkl/lib/virtio_blk.c", "tools/lkl/lklfuse.c"])
        with self.assertRaisesRegex(RuntimeError, "file set"):
            BUILD.kernel_source_changes(previous, {**current, "new.c": {}})
        with self.assertRaisesRegex(RuntimeError, "kernel/build inputs"):
            BUILD.kernel_source_changes(previous, {**current, "arch/lkl/Kconfig": {"sha256": "different"}})

    def test_reviewed_host_patch_has_the_pinned_digest(self):
        self.assertEqual(BUILD.digest(BUILD.ROOT / BUILD.HOST_PATCH), BUILD.HOST_PATCH_SHA256)

    def test_cached_corruption_is_preserved_and_rejected_without_download(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "input"
            path.write_bytes(b"corrupted evidence")
            with mock.patch.object(BUILD.urllib.request, "urlopen") as network:
                with self.assertRaisesRegex(RuntimeError, "Cached.*SHA-256"):
                    BUILD.fetch("https://invalid.example/input", path, "0" * 64)
            network.assert_not_called()
            self.assertEqual(path.read_bytes(), b"corrupted evidence")

    def test_download_digest_failure_never_publishes_an_input(self):
        import io
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "input"
            with mock.patch.object(BUILD.urllib.request, "urlopen", return_value=io.BytesIO(b"substituted")):
                with self.assertRaisesRegex(RuntimeError, "Downloaded.*SHA-256"):
                    BUILD.fetch("https://invalid.example/input", path, "0" * 64)
            self.assertFalse(path.exists())
            self.assertFalse(path.with_suffix(".partial").exists())

    def test_verified_cached_input_requires_no_network(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "input"
            path.write_bytes(b"verified")
            expected = hashlib.sha256(b"verified").hexdigest()
            with mock.patch.object(BUILD.urllib.request, "urlopen") as network:
                self.assertEqual(BUILD.fetch("https://invalid.example/input", path, expected), path)
            network.assert_not_called()

    def test_patch_rejects_escape_and_path_changes(self):
        for target in [b"/etc/file", b"../escape", b"a/../../escape", b"a/./file"]:
            with self.subTest(target=target), self.assertRaisesRegex(RuntimeError, "Unsafe"):
                BUILD.patch_target(target)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "source").write_text("before\n")
            patch = root / "patch"
            patch.write_text("--- a/source\n+++ b/different\n@@ -1 +1 @@\n-before\n+after\n")
            with self.assertRaisesRegex(RuntimeError, "cannot change file paths"):
                BUILD.apply_patch(root, patch)
            self.assertEqual((root / "source").read_text(), "before\n")

    def test_patch_cannot_follow_a_source_symlink(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            outside = root / "untouched"
            outside.write_text("before\n")
            (root / "source").symlink_to(outside)
            patch = root / "patch"
            patch.write_text("--- a/source\n+++ b/source\n@@ -1 +1 @@\n-before\n+after\n")
            with self.assertRaisesRegex(RuntimeError, "regular source file"):
                BUILD.apply_patch(root, patch)
            self.assertEqual(outside.read_text(), "before\n")

    def test_patch_requires_unique_exact_context(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            patch = root / "patch"
            patch.write_text("--- a/source\n+++ b/source\n@@ -1 +1 @@\n-before\n+after\n")
            source = root / "source"
            for content in ["different\n", "before\nbefore\n"]:
                source.write_text(content)
                with self.assertRaisesRegex(RuntimeError, "uniquely match"):
                    BUILD.apply_patch(root, patch)
                self.assertEqual(source.read_text(), content)
            source.write_text("before\n")
            self.assertEqual(BUILD.apply_patch(root, patch), ["source"])
            self.assertEqual(source.read_text(), "after\n")

    @unittest.skipUnless(platform.system() == "FreeBSD", "requires the native host FUSE2 ABI")
    def test_native_channel_delegation_init_failure_and_single_descriptor_close(self):
        client = os.environ.get("LURKER_FREEBSD_LKL_CLIENT_SOURCE")
        headers = os.environ.get("LURKER_FREEBSD_LKL_HEADERS")
        if not client or not headers:
            self.skipTest("set the verified native client and generated-header input paths")
        program = r'''
#define main lklfuse_program_main
#define getmntinfo test_native_mount_table
#include LKL_HOST_CLIENT
#undef getmntinfo
#undef main
#include <assert.h>
static struct statfs native_entries[2];
static int native_count;
int test_native_mount_table(struct statfs **entries, int flags)
{
    assert(flags == MNT_NOWAIT);
    *entries = native_entries;
    return native_count;
}
struct fake_channel {
    struct fuse_session *session;
    int receive_calls, send_calls, destroy_calls, send_result, descriptor;
};
static int fake_receive(struct fuse_chan **channel, char *bytes, size_t length)
{
    struct fake_channel *fake = fuse_chan_data(*channel);
    assert(fuse_chan_session(*channel) == fake->session);
    assert(length >= 4);
    memcpy(bytes, "test", 4);
    fake->receive_calls++;
    return 4;
}
static int fake_send(struct fuse_chan *channel, const struct iovec *iov, size_t count)
{
    struct fake_channel *fake = fuse_chan_data(channel);
    assert(fuse_chan_session(channel) == fake->session);
    assert(native_initialized == 0);
    fake->send_calls++;
    return fake->send_result;
}
static void fake_destroy(struct fuse_chan *channel)
{
    struct fake_channel *fake = fuse_chan_data(channel);
    fake->destroy_calls++;
    assert(close(fake->descriptor) == 0);
}
int main(void)
{
    char *argv[] = {"native-client", "-s", "-f", "-o", "direct_io", "/tmp", NULL};
    struct fuse_args arguments = FUSE_ARGS_INIT(6, argv);
    char *target = NULL; int threaded, foreground;
    assert(fuse_parse_cmdline(&arguments, &target, &threaded, &foreground) == 0);
    assert(!threaded && foreground && target);
    assert(remove_kernel_name(&arguments) == 0);
    for (int index = 0; index < arguments.argc; index++)
        assert(strstr(arguments.argv[index], "fsname=") == NULL);
    free(target); fuse_opt_free_args(&arguments);
    struct fuse_session_ops session_ops = {0};
    struct fuse_chan_ops channel_ops = {
        .receive = fake_receive, .send = fake_send, .destroy = fake_destroy,
    };
    struct fake_channel fake = {0};
    fake.descriptor = open("/dev/null", O_RDONLY);
    assert(fake.descriptor >= 0);
    fake.session = fuse_session_new(&session_ops, NULL);
    assert(fake.session);
    struct fuse_chan *inner = fuse_chan_new(&channel_ops, fake.descriptor, 64, &fake);
    struct fuse_chan *outer = observe_native_channel(inner);
    assert(inner && outer);
    fuse_session_add_chan(fake.session, outer);
    char bytes[64]; struct fuse_chan *received = outer;
    assert(fuse_chan_recv(&received, bytes, sizeof(bytes)) == 4);
    assert(received == outer && !memcmp(bytes, "test", 4));
    assert(fuse_session_next_chan(fake.session, NULL) == outer);
    assert(fuse_chan_session(inner) == NULL);
    struct fuse_conn_info connection = {.proto_major = 7, .proto_minor = 19};
    native_service_init(&connection);
    assert(!native_initialized && init_reply_pending == 1);
    fake.send_result = -EIO;
    assert(fuse_chan_send(outer, NULL, 0) == -EIO);
    assert(native_initialized == -1 && !init_reply_pending);
    assert(fuse_session_next_chan(fake.session, NULL) == outer);
    assert(fuse_chan_session(inner) == NULL);
    native_initialized = 0; fake.send_result = 0;
    assert(fuse_chan_send(outer, NULL, 0) == 0);
    assert(!native_initialized); /* A later ordinary reply cannot repair failed INIT. */
    connection.proto_major = 8; native_service_init(&connection);
    assert(fuse_chan_send(outer, NULL, 0) == 0);
    assert(native_initialized == -1);
    native_initialized = 0; connection.proto_major = 7;
    native_service_init(&connection);
    assert(!native_initialized);
    assert(fuse_chan_send(outer, NULL, 0) == 0);
    assert(native_initialized == 1 && !init_reply_pending);
    fuse_chan_destroy(outer);
    assert(fake.destroy_calls == 1 && fuse_session_next_chan(fake.session, NULL) == NULL);
    errno = 0; assert(fcntl(fake.descriptor, F_GETFD) == -1 && errno == EBADF);
    fuse_session_destroy(fake.session);
    assert(fake.destroy_calls == 1 && fake.receive_calls == 1 && fake.send_calls == 4);
    lklfuse.file = "/owned/source";
    native_count = 1;
    strcpy(native_entries[0].f_mntonname, "/unrelated");
    strcpy(native_entries[0].f_fstypename, "ufs");
    native_mount_captured = 0;
    assert(native_mounts_absent("/owned/target") < 0);
    native_mount_captured = 1;
    native_mount_fsid.val[0] = 42; native_mount_fsid.val[1] = 91;
    assert(native_mounts_absent("/owned/target") == 0);
    native_entries[0].f_fsid = native_mount_fsid; /* Moved owned FSID. */
    assert(native_mounts_absent("/owned/target") < 0);
    memset(&native_entries[0].f_fsid, 0, sizeof(fsid_t));
    strcpy(native_entries[0].f_mntonname, "/owned/target"); /* Replacement. */
    assert(native_mounts_absent("/owned/target") < 0);
    strcpy(native_entries[0].f_mntonname, "/elsewhere");
    strcpy(native_entries[0].f_fstypename, "fusefs");
    strcpy(native_entries[0].f_mntfromname, "/owned/source");
    assert(native_mounts_absent("/owned/target") < 0); /* Ambiguous source. */
    native_count = 0;
    assert(native_mounts_absent("/owned/target") < 0);
    puts("PASS native channel delegation, failed INIT stays unready, one descriptor close; global FSID/source/replacement absence");
    return 0;
}
'''
        with tempfile.TemporaryDirectory(prefix="lkl-native-channel-") as directory:
            source = Path(directory) / "channel.c"
            binary = Path(directory) / "channel"
            source.write_text(program)
            command = ["/usr/local/bin/gcc14", "-D_FILE_OFFSET_BITS=64",
                       '-DLKL_HOST_CLIENT="' + str(Path(client).resolve()) + '"',
                       "-ffunction-sections", "-fdata-sections", "-Wl,--gc-sections",
                       "-Wl,-z,noexecstack", "-O2", "-I" + str(Path(headers).resolve()),
                       "-I/usr/local/include", str(source), "-L/usr/local/lib", "-lfuse",
                       "-lpthread", "-o", str(binary)]
            subprocess.run(command, check=True, timeout=60)
            result = subprocess.run([str(binary)], check=True, capture_output=True,
                                    text=True, timeout=10)
            self.assertIn("failed INIT stays unready, one descriptor close", result.stdout)

    @unittest.skipUnless(platform.system() == "FreeBSD", "requires native compiled-client ABI")
    def test_native_restore_failure_poison_keeps_process_alive_and_blocks_requests(self):
        client = os.environ.get("LURKER_FREEBSD_LKL_CLIENT_SOURCE")
        headers = os.environ.get("LURKER_FREEBSD_LKL_HEADERS")
        if not client or not headers:
            self.skipTest("set verified native client and generated-header input paths")
        program = r'''
#define main lklfuse_program_main
#include LKL_HOST_CLIENT
#undef main
#include <assert.h>
static int calls, fail_at;
long lkl_syscall(long number, long *arguments)
{
    calls++;
    if (calls == fail_at)
        return -LKL_EIO;
    if (number == __lkl__NR_getresuid || number == __lkl__NR_getresgid) {
        *(unsigned int *)arguments[0] = 0;
        *(unsigned int *)arguments[1] = 0;
        *(unsigned int *)arguments[2] = 0;
    }
    return 0;
}
int main(void)
{
    for (int fault = 1; fault <= 12; fault++) {
        /* setfsuid/setfsgid report the previous ID, not errno; their readback
         * failures are tested at calls10/11, rather than inventing errno. */
        if (fault == 4 || fault == 5 || fault == 6)
            continue;
        calls = credential_poison = 0; fail_at = fault;
        assert(reset_request_credentials() == -LKL_EIO);
        assert(calls == 12 && credential_poison == 1);
        assert(host_error(0, 0) == -EIO);
        int previous = calls;
        assert(begin_request() == -EIO);
        assert(host_lklfuse_getattr("/must-not-be-resolved", NULL) == -EIO);
        assert(host_lklfuse_release("/ignored", NULL) == -EIO);
        assert(calls == previous); /* No emulated work after poison. */
        fail_at = 0;
        assert(reset_request_credentials() == 0);
        assert(calls == previous + 12 && credential_poison == 1);
        assert(host_error(0, 0) == -EIO); /* Later restoration cannot erase failure. */
    }
    puts("PASS restore failures attempt all fields, stay alive, poison every later request");
    return 0;
}
'''
        with tempfile.TemporaryDirectory(prefix="lkl-native-poison-") as directory:
            source = Path(directory) / "poison.c"
            binary = Path(directory) / "poison"
            source.write_text(program)
            command = ["/usr/local/bin/gcc14", "-D_FILE_OFFSET_BITS=64",
                       '-DLKL_HOST_CLIENT="' + str(Path(client).resolve()) + '"',
                       "-ffunction-sections", "-fdata-sections", "-Wl,--gc-sections",
                       "-Wl,-z,noexecstack", "-O2", "-I" + str(Path(headers).resolve()),
                       "-I/usr/local/include", str(source), "-L/usr/local/lib", "-lfuse",
                       "-lpthread", "-o", str(binary)]
            subprocess.run(command, check=True, timeout=60)
            result = subprocess.run([str(binary)], check=True, capture_output=True,
                                    text=True, timeout=10)
            self.assertIn("poison every later request", result.stdout)


if __name__ == "__main__":
    unittest.main()
