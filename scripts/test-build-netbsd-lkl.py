#!/usr/bin/env python3
"""Focused native tests for pinned inputs and bounded LKL diagnostic jobs."""
import hashlib
import importlib.util
import io
import os
from pathlib import Path
import platform
import re
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest import mock

SPEC = importlib.util.spec_from_file_location("netbsd_lkl", Path(__file__).with_name("build-netbsd-lkl.py"))
BUILDER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BUILDER)


class BuildInputs(unittest.TestCase):
    @unittest.skipUnless(platform.system() == "NetBSD", "native isolated mount observer")
    def test_native_mount_observer_protocol_and_bounded_cancellation(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = BUILDER.ROOT / "native/netbsd/lkl"
            helper = Path(os.environ["LURKER_NETBSD_MOUNT_OBSERVER"]) if os.environ.get("LURKER_NETBSD_MOUNT_OBSERVER") else Path(tmp) / "observer"
            control = Path(tmp) / "control"
            sources = [("observer-control.c", control)]
            if not os.environ.get("LURKER_NETBSD_MOUNT_OBSERVER"):
                sources.append(("mount-observer.c", helper))
            for source, binary in sources:
                subprocess.run(["/usr/bin/cc", "-std=gnu11", "-Wall", "-Wextra", "-Werror",
                                str(root / source), "-o", str(binary)], check=True, timeout=30)
            result = subprocess.run([str(control), str(helper), str(control)],
                                    capture_output=True, text=True, timeout=10)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            print("native observer tested binary SHA-256=" + BUILDER.digest(helper))
            print(result.stdout, end="")

    def test_observer_input_tampering_cannot_create_output(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            inputs = root / "native/netbsd/lkl"
            inputs.mkdir(parents=True)
            for name in BUILDER.OBSERVER_INPUTS:
                (inputs / name).write_text("unreviewed input")
            with mock.patch.object(BUILDER, "ROOT", root):
                with self.assertRaisesRegex(RuntimeError, "Observer source checksum differs"):
                    BUILDER.build_mount_observer(root / "output", 10)
            self.assertFalse((root / "output").exists())

    @unittest.skipUnless(platform.system() == "NetBSD", "native global mount inventory")
    def test_global_mount_identity_survives_path_replacement(self):
        with tempfile.TemporaryDirectory() as tmp:
            binary = Path(tmp) / "mount-control"
            control = BUILDER.ROOT / "native/netbsd/lkl/mount-control.c"
            subprocess.run(["/usr/bin/cc", "-std=gnu11", "-Wall", "-Wextra", "-Werror",
                            str(control), "-o", str(binary)], check=True, timeout=30)
            result = subprocess.run([str(binary)], capture_output=True, text=True, timeout=10)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            print(result.stdout, end="")

    @unittest.skipUnless(platform.system() == "NetBSD", "native descriptor release gate")
    def test_shutdown_retains_resources_until_verified_absence_and_halt(self):
        with tempfile.TemporaryDirectory() as tmp:
            binary = Path(tmp) / "shutdown-control"
            control = BUILDER.ROOT / "native/netbsd/lkl/shutdown-control.c"
            subprocess.run(["/usr/bin/cc", "-std=gnu11", "-Wall", "-Wextra", "-Werror",
                            str(control), "-o", str(binary)], check=True, timeout=30)
            result = subprocess.run([str(binary)], capture_output=True, text=True, timeout=10)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            print(result.stdout, end="")

    @unittest.skipUnless(platform.system() == "NetBSD" and os.environ.get("LURKER_NETBSD_LKL_SOURCE"), "built native LKL library required")
    def test_captured_credentials_umask_and_restoration_poison(self):
        source = Path(os.environ["LURKER_NETBSD_LKL_SOURCE"]).resolve(strict=True)
        with tempfile.TemporaryDirectory() as tmp:
            binary = Path(tmp) / "credential-control"
            control = BUILDER.ROOT / "native/netbsd/lkl/credential-control.c"
            subprocess.run(["/usr/bin/cc", "-O2", "-g",
                            "-I" + str(source / "tools/lkl/include"),
                            "-I" + str(source / "tools/lkl/include/lkl"), str(control),
                            str(source / "tools/lkl/liblkl.a"), "-lpthread", "-lrt", "-o", str(binary)],
                           check=True, timeout=30)
            result = subprocess.run([str(binary), str(Path(tmp) / "held-source")],
                                    capture_output=True, text=True, timeout=15)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            print(result.stdout, end="")

    @unittest.skipUnless(platform.system() == "NetBSD" and os.environ.get("LURKER_NETBSD_LKL_SOURCE"), "built native LKL UAPI required")
    def test_native_file_transport_retains_transient_io_failures(self):
        source = Path(os.environ["LURKER_NETBSD_LKL_SOURCE"]).resolve(strict=True)
        with tempfile.TemporaryDirectory() as tmp:
            binary = Path(tmp) / "disk-control"
            control = BUILDER.ROOT / "native/netbsd/lkl/disk-control.c"
            subprocess.run(["/usr/bin/cc", "-std=gnu11", "-Wall", "-Wextra", "-Werror",
                            "-isystem", str(source / "tools/lkl/include"),
                            "-isystem", str(source / "tools/lkl/include/lkl"),
                            str(control), "-o", str(binary)], check=True, timeout=30)
            result = subprocess.run([str(binary), str(Path(tmp) / "fresh-image")],
                                    capture_output=True, text=True, timeout=10)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            print(result.stdout, end="")

    def test_isolated_refuse_output_cannot_replace_existing_path(self):
        with tempfile.TemporaryDirectory() as tmp:
            output = Path(tmp) / "output"
            output.mkdir()
            (output / "preserve").write_text("existing diagnostic")
            with self.assertRaisesRegex(RuntimeError, "new isolated ReFUSE output"):
                BUILDER.build_refuse_bridge(Path(tmp) / "cache", output, 10)
            self.assertEqual((output / "preserve").read_text(), "existing diagnostic")

    @unittest.skipUnless(platform.system() == "NetBSD" and os.environ.get("LURKER_NETBSD_REFUSE_SOURCE"), "isolated native ReFUSE source required")
    def test_actual_refuse_sync_forwarding_and_request_pid(self):
        root = Path(os.environ["LURKER_NETBSD_REFUSE_SOURCE"]).resolve(strict=True)
        self.assertEqual(BUILDER.digest(root / "refuse.c"), BUILDER.REFUSE_SOURCE_SHA256)
        spec = importlib.util.spec_from_file_location("native_fuse", BUILDER.ROOT / "scripts/build-netbsd-fuse-prerequisite.py")
        base = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(base)
        with tempfile.TemporaryDirectory() as tmp:
            binary = Path(tmp) / "control"
            control = BUILDER.ROOT / "native/netbsd/lkl/refuse-bridge-control.c"
            command = ["/usr/bin/cc", "-O2", "-g", "-D_KERNTYPES", "-D_REENTRANT",
                       "-I" + str(root), "-I" + str(root.parent / "libpuffs"),
                       '-DREFUSE_IMPLEMENTATION_PATH="' + str(root / "refuse.c") + '"', str(control)]
            command.extend(str(root / name) for name in base.LIBRARIES[2][2] if name != "refuse.c")
            command.extend(["-lpuffs", "-lpthread", "-lutil", "-o", str(binary)])
            subprocess.run(command, check=True, timeout=30)
            result = subprocess.run([str(binary)], capture_output=True, text=True, timeout=10)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            print(result.stdout, end="")

    @unittest.skipUnless(platform.system() == "NetBSD", "native NetBSD pthread timers")
    def test_monotonic_timer_rearm_self_free_and_quiescence(self):
        with tempfile.TemporaryDirectory() as tmp:
            binary = Path(tmp) / "timer-control"
            source = BUILDER.ROOT / "native/netbsd/lkl/timer-control.c"
            subprocess.run(["/usr/bin/gcc", "-std=gnu11", "-Wall", "-Wextra", "-Werror", "-pthread",
                            "-o", str(binary), str(source)], check=True, timeout=30)
            result = subprocess.run([str(binary)], text=True, capture_output=True, timeout=15)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            print(result.stdout, end="")

    @unittest.skipUnless(platform.system() == "NetBSD" and os.environ.get("LURKER_NETBSD_LKL_SOURCE"), "built native LKL UAPI required")
    def test_generated_native_uapi_matches_kernel_layout(self):
        source = Path(os.environ["LURKER_NETBSD_LKL_SOURCE"]).resolve(strict=True)
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp)
            (work / "probe.c").write_text(r'''
#define _NETBSD_SOURCE
#include <sys/types.h>
#include <sys/endian.h>
#include <signal.h>
#include <netinet/in.h>
#include <stddef.h>
#include <stdio.h>
#include <unistd.h>
#include <lkl.h>
#include <lkl_host.h>
#pragma push_macro("ipi_spec_dst")
#undef ipi_spec_dst
_Static_assert(sizeof(struct lkl_stat)==128,"asm-generic 64-bit stat size");
_Static_assert(offsetof(struct lkl_stat,st_ino)==8,"stat ino offset");
_Static_assert(offsetof(struct lkl_stat,st_mode)==16,"stat mode offset");
_Static_assert(offsetof(struct lkl_stat,st_nlink)==20,"stat nlink offset");
_Static_assert(offsetof(struct lkl_stat,st_uid)==24,"stat uid offset");
_Static_assert(offsetof(struct lkl_stat,st_gid)==28,"stat gid offset");
_Static_assert(offsetof(struct lkl_stat,st_size)==48,"stat size offset");
_Static_assert(sizeof(struct lkl_icmphdr)==8,"ICMP UAPI size");
_Static_assert(offsetof(struct lkl_icmphdr,un.frag.mtu)==6,"ICMP __unused field retained");
_Static_assert(sizeof(struct lkl_in_pktinfo)==12,"in_pktinfo UAPI size");
_Static_assert(offsetof(struct lkl_in_pktinfo,ipi_spec_dst)==4,"packet spec dst distinct");
_Static_assert(offsetof(struct lkl_in_pktinfo,ipi_addr)==8,"packet addr offset");
_Static_assert(sizeof(struct lkl_siginfo)==128,"Linux siginfo size");
_Static_assert(offsetof(struct lkl_linux_dirent64,d_name)==19,"Linux dirent name offset");
#pragma pop_macro("ipi_spec_dst")
int main(void) {
    siginfo_t native={0}; native.si_signo=SIGUSR1;
    struct in_pktinfo packet={0}; packet.ipi_spec_dst.s_addr=7;
    if(native.si_signo!=SIGUSR1 || packet.ipi_addr.s_addr!=7) return 1;
    printf("native LKL strict layout passed uid=%u stat=128 uid@24 gid@28 ICMP=8 mtu@6 packet=12 siginfo=128 dirent-name@19; native aliases preserved\n",(unsigned)getuid());
    return 0;
}
''')
            subprocess.run(["/usr/bin/gcc", "-std=gnu11", "-Wall", "-Wextra", "-Werror", "-Wno-unused-parameter",
                            "-I", str(source.parent / "netbsd-include"), "-I", str(source / "tools/lkl/include"),
                            "-o", str(work / "probe"), str(work / "probe.c")], check=True, timeout=30)
            result = subprocess.run([str(work / "probe")], capture_output=True, text=True, check=True, timeout=10)
            print(result.stdout, end="")

    @unittest.skipUnless(platform.system() == "NetBSD", "native NetBSD signal aliases")
    def test_native_signal_aliases_survive_lkl_header_import(self):
        patch = (BUILDER.ROOT / BUILDER.HOST_PATCH).read_text()
        section = patch.split("+++ b/tools/lkl/include/lkl.h\n", 1)[1]
        post = "\n".join(line[1:] for line in section.splitlines()
                         if line[:1] in (" ", "+"))
        start = post.index("#if defined(__NetBSD__)")
        end = post.index('#pragma pop_macro("__unused")')
        guard = post[start:end]
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp)
            stub = work / "lkl/asm/syscalls.h"
            stub.parent.mkdir(parents=True)
            stub.write_text("struct imported_signal {int si_signo,si_errno,si_code,si_pid,si_uid;};\n"
                            "static int imported_signal_fields(void) {struct imported_signal s={1,2,3,4,5};return s.si_signo+s.si_errno+s.si_code+s.si_pid+s.si_uid;}\n"
                            "struct imported_packet {int ipi_spec_dst,ipi_addr;};\n"
                            "static int imported_packet_fields(void) {struct imported_packet p={3,4};return p.ipi_spec_dst+p.ipi_addr;}\n")
            source = "#include <signal.h>\n#include <sys/endian.h>\n#include <netinet/in.h>\n" + guard + "\n" + r'''
int main(void) {
    siginfo_t native={0};
    native.si_signo=SIGUSR1; native.si_errno=1; native.si_code=2;
    native.si_pid=123; native.si_uid=456;
    struct in_pktinfo packet={0}; packet.ipi_spec_dst.s_addr=7;
    return !(imported_signal_fields()==15 && native.si_signo==SIGUSR1 &&
             native.si_errno==1 && native.si_code==2 &&
             native.si_pid==123 && native.si_uid==456 &&
             imported_packet_fields()==7 && packet.ipi_addr.s_addr==7);
}
'''
            (work / "probe.c").write_text(source)
            subprocess.run(["/usr/bin/gcc", "-std=gnu11", "-Wall", "-Wextra", "-Werror", "-I", str(work), "-o", str(work / "probe"), str(work / "probe.c")], check=True, timeout=30)
            subprocess.run([str(work / "probe")], check=True, timeout=10)

    @unittest.skipUnless(platform.system() == "NetBSD", "native NetBSD mmap semantics")
    def test_native_exact_mmap_preserves_collision_and_cleans_only_its_mapping(self):
        patch = (BUILDER.ROOT / BUILDER.HOST_PATCH).read_text()
        match = re.search(r"^\+/\* NETBSD_EXACT_MMAP_BEGIN.*?^\+/\* NETBSD_EXACT_MMAP_END \*/", patch, re.M | re.S)
        self.assertIsNotNone(match)
        function = "\n".join(line[1:] for line in match.group().splitlines())
        source = r'''
#include <sys/types.h>
#include <sys/mman.h>
#include <stdint.h>
#include <stdlib.h>
#include <unistd.h>
#include <stdio.h>
#include <errno.h>
#include <fcntl.h>
#include <string.h>
#include <assert.h>
static void *returned, *unmapped;
static unsigned calls;
static void *observed_mmap(void *a, size_t n, int p, int f, int fd, off_t o) {
    calls++; return returned = mmap(a,n,p,f,fd,o);
}
static int observed_munmap(void *a, size_t n) {
    unmapped=a; return munmap(a,n);
}
#define mmap observed_mmap
#define munmap observed_munmap
''' + function + r'''
#undef mmap
#undef munmap
int main(void) {
    size_t n=(size_t)sysconf(_SC_PAGESIZE);
    unsigned char *occupied=mmap(NULL,n,PROT_READ|PROT_WRITE,MAP_ANON|MAP_PRIVATE,-1,0);
    assert(occupied!=MAP_FAILED); memset(occupied,0xa5,n);
    assert(netbsd_mmap_exact(occupied,n,PROT_READ|PROT_WRITE,MAP_ANON|MAP_PRIVATE,-1,0)==NULL);
    assert(errno==EEXIST && returned!=MAP_FAILED && returned!=occupied && unmapped==returned);
    for(size_t i=0;i<n;i++) assert(occupied[i]==0xa5);
    unsigned before=calls;
    assert(!netbsd_mmap_exact(occupied+1,n,PROT_READ,MAP_ANON|MAP_PRIVATE,-1,0));
    assert(!netbsd_mmap_exact(occupied,SIZE_MAX,PROT_READ,MAP_ANON|MAP_PRIVATE,-1,0));
    assert(!netbsd_mmap_exact(occupied,n,PROT_READ,MAP_ANON|MAP_PRIVATE|MAP_FIXED,-1,0));
    assert(!netbsd_mmap_exact(occupied,n,PROT_READ,MAP_SHARED,1,1));
    assert(calls==before); /* All rejected before the native mmap syscall. */
    unmapped=NULL;
    assert(!netbsd_mmap_exact(occupied,n,PROT_READ,MAP_SHARED,-2,0));
    assert(returned==MAP_FAILED && !unmapped); /* Never unmap the requested hint. */
    assert(!munmap(occupied,n));
    void *exact=netbsd_mmap_exact(occupied,n,PROT_READ|PROT_WRITE,MAP_ANON|MAP_PRIVATE,-1,0);
    if(exact!=occupied) fprintf(stderr,"anonymous hint=%p returned=%p errno=%d\n",occupied,returned,errno);
    assert(exact==occupied); memset(exact,0x4b,n); assert(!munmap(exact,n));
    char name[]="/tmp/lurker-lkl-mmap-XXXXXX";
    int fd=mkstemp(name); assert(fd>=0); assert(!unlink(name)); assert(!ftruncate(fd,n));
    void *shared_hint=(void *)(uintptr_t)0x50000000;
    unmapped=NULL;
    exact=netbsd_mmap_exact(shared_hint,n,PROT_READ|PROT_WRITE,MAP_SHARED,fd,0);
    if (exact) {
        assert(exact==shared_hint); memset(exact,0x6c,n); assert(!msync(exact,n,MS_SYNC)); assert(!fsync(fd));
        unsigned char byte; assert(pread(fd,&byte,1,0)==1 && byte==0x6c);
        assert(!munmap(exact,n));
        puts("shared fixed-base hint accepted exactly (MMU remains disabled)");
    } else {
        assert((returned==MAP_FAILED && !unmapped) ||
               (returned!=MAP_FAILED && returned!=shared_hint && unmapped==returned));
        puts("shared fixed-base hint safely rejected; no MMU availability claim");
    }
    assert(!close(fd));
    printf("native exact mmap collision/failure/alignment/anonymous/shared passed uid=%u\n",(unsigned)getuid());
    return 0;
}
'''
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp)
            (work / "probe.c").write_text(source)
            subprocess.run(["/usr/bin/gcc", "-std=gnu11", "-Wall", "-Wextra", "-Werror", "-o", str(work / "probe"), str(work / "probe.c")], check=True, timeout=30)
            result = subprocess.run([str(work / "probe")], capture_output=True, text=True, timeout=10)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            print(result.stdout, end="")

    def test_cached_archive_is_rehashed(self):
        with tempfile.TemporaryDirectory() as tmp:
            archive = Path(tmp) / "cached.tar.gz"
            archive.write_bytes(b"original")
            expected = hashlib.sha256(b"original").hexdigest()
            with mock.patch.object(BUILDER, "SOURCE_SHA256", expected):
                self.assertEqual(BUILDER.fetch(archive), archive)
                archive.write_bytes(b"replacement")
                with self.assertRaisesRegex(RuntimeError, "SHA-256"):
                    BUILDER.fetch(archive)

    def test_native_timer_copy_preserves_unknown_cache_and_requires_pinned_bytes(self):
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp)
            source = work / "source"
            target = source / "tools/lkl/lib/netbsd-timer.h"
            target.parent.mkdir(parents=True)
            header = work / "timer.h"
            header.write_bytes(b"pinned timer")
            expected = BUILDER.digest(header)
            with mock.patch.object(BUILDER, "TIMER_HEADER_SHA256", expected):
                with self.assertRaisesRegex(RuntimeError, "Missing cached"):
                    BUILDER.install_timer_header(source, header, allow_create=False)
                BUILDER.install_timer_header(source, header, allow_create=True)
                BUILDER.install_timer_header(source, header, allow_create=False)
                target.write_bytes(b"unrelated cached input")
                with self.assertRaisesRegex(RuntimeError, "Modified cached"):
                    BUILDER.install_timer_header(source, header, allow_create=True)
                self.assertEqual(target.read_bytes(), b"unrelated cached input")
                target.unlink()
                header.write_bytes(b"changed input")
                with self.assertRaisesRegex(RuntimeError, "changed before copying"):
                    BUILDER.install_timer_header(source, header, allow_create=True)
                self.assertFalse(target.exists())

    def test_modified_upstream_file_prevents_resume(self):
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp)
            source = work / "source"
            source.mkdir()
            (source / "input.c").write_bytes(b"original")
            archive = work / "input.tar"
            with tarfile.open(archive, "w") as tar:
                item = tarfile.TarInfo(f"linux-{BUILDER.REVISION}/input.c")
                item.size = len(b"original")
                tar.addfile(item, io.BytesIO(b"original"))
            self.assertEqual(BUILDER.verify_source(archive, source), 1)
            (source / "input.c").write_bytes(b"changed")
            with self.assertRaisesRegex(RuntimeError, "Modified upstream input"):
                BUILDER.verify_source(archive, source)

    def test_archive_traversal_prevents_resume(self):
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp)
            archive = work / "input.tar"
            with tarfile.open(archive, "w") as tar:
                item = tarfile.TarInfo(f"linux-{BUILDER.REVISION}/../escape")
                tar.addfile(item, io.BytesIO())
            with self.assertRaisesRegex(RuntimeError, "Unsafe source archive"):
                BUILDER.verify_source(archive, work)

    def test_source_symlink_is_not_a_regular_input(self):
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp)
            (work / "real").write_bytes(b"original")
            (work / "input.c").symlink_to("real")
            archive = work / "input.tar"
            with tarfile.open(archive, "w") as tar:
                item = tarfile.TarInfo(f"linux-{BUILDER.REVISION}/input.c")
                item.size = 8
                tar.addfile(item, io.BytesIO(b"original"))
            with self.assertRaisesRegex(RuntimeError, "Missing regular upstream input"):
                BUILDER.verify_source(archive, work)

    def test_source_directory_cannot_redirect_outside_cache(self):
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp)
            (work / "outside").mkdir()
            (work / "subdir").symlink_to("outside", target_is_directory=True)
            archive = work / "input.tar"
            with tarfile.open(archive, "w") as tar:
                item = tarfile.TarInfo(f"linux-{BUILDER.REVISION}/subdir")
                item.type = tarfile.DIRTYPE
                tar.addfile(item)
            with self.assertRaisesRegex(RuntimeError, "Modified upstream directory"):
                BUILDER.verify_source(archive, work)

    def test_safe_tar_symlink_normalization_is_preserved(self):
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp)
            (work / "subdir").mkdir()
            (work / "subdir/link").symlink_to("../target")
            archive = work / "input.tar"
            with tarfile.open(archive, "w") as tar:
                item = tarfile.TarInfo(f"linux-{BUILDER.REVISION}/subdir/link")
                item.type = tarfile.SYMTYPE
                item.linkname = ".././target"
                tar.addfile(item)
            self.assertEqual(BUILDER.verify_source(archive, work), 0)

    def test_timeout_terminates_owned_make_process_group(self):
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp)
            marker = work / "escaped-child"
            child = "import time,pathlib;time.sleep(2);pathlib.Path(%r).touch()" % str(marker)
            parent = "import subprocess,sys,time;subprocess.Popen([sys.executable,'-c',%r]);time.sleep(30)" % child
            result = BUILDER.run_build([sys.executable, "-c", parent], work, os.environ.copy(), work / "build.log", 0.2)
            self.assertTrue(result["timed_out"])
            import time
            time.sleep(2.1)
            self.assertFalse(marker.exists())

    def test_timeout_kills_term_ignoring_child_after_leader_exits(self):
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp)
            marker, ready = work / "escaped-child", work / "ready"
            child = ("import signal,time,pathlib;signal.signal(signal.SIGTERM,signal.SIG_IGN);"
                     "pathlib.Path(%r).touch();time.sleep(2);pathlib.Path(%r).touch()") % (str(ready), str(marker))
            parent = "import subprocess,sys,time;subprocess.Popen([sys.executable,'-c',%r]);time.sleep(30)" % child
            result = BUILDER.run_build([sys.executable, "-c", parent], work, os.environ.copy(), work / "build.log", 1)
            self.assertTrue(result["timed_out"])
            self.assertTrue(ready.exists(), "The grandchild must have installed its TERM handler")
            import time
            time.sleep(2.1)
            self.assertFalse(marker.exists())

    def test_reviewed_patch_rejects_changed_context_and_unapproved_paths(self):
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp)
            archive = work / "input.tar"
            name = "tools/lkl/lklfuse.c"
            with tarfile.open(archive, "w") as tar:
                item = tarfile.TarInfo(f"linux-{BUILDER.REVISION}/{name}")
                item.size = 9
                tar.addfile(item, io.BytesIO(b"original\n"))
            patch = work / "host.patch"
            contents = f"--- a/{name}\n+++ b/{name}\n@@ -1 +1 @@\n-original\n+adapted\n"
            patch.write_text(contents)
            result = BUILDER.patched_inputs(archive, [(patch, BUILDER.digest(patch))])
            self.assertEqual(result[name], b"adapted\n")
            patch.write_text(contents.replace("-original", "-different"))
            with self.assertRaisesRegex(RuntimeError, "context is not unique"):
                BUILDER.patched_inputs(archive, [(patch, BUILDER.digest(patch))])
            patch.write_text(contents.replace(name, "../escape"))
            with self.assertRaisesRegex(RuntimeError, "Unapproved host patch target"):
                BUILDER.patched_inputs(archive, [(patch, BUILDER.digest(patch))])

    def test_reviewed_patch_digest_is_mandatory(self):
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp)
            archive = work / "input.tar"
            with tarfile.open(archive, "w"):
                pass
            patch = work / "host.patch"
            patch.write_text("unreviewed")
            with self.assertRaisesRegex(RuntimeError, "SHA-256"):
                BUILDER.patched_inputs(archive, [(patch, "0" * 64)])


if __name__ == "__main__":
    unittest.main()
