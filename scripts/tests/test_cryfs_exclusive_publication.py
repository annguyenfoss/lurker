"""Compile and exercise the reviewed Linux CryFS atomic publication header."""
import errno
import json
from pathlib import Path
import platform
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
HEADER = 'src/cpp-utils/io/DurableFile.h'
PATCH = ROOT / 'native/folder/cryfs/linux-exclusive-publication.patch'


def base_header():
    selected = body = False
    lines = []
    for line in (ROOT / 'native/folder/cryfs/linux-localstate.patch').read_text().splitlines():
        if line.startswith('diff --git '):
            if selected:
                break
            selected = line == f'diff --git a/{HEADER} b/{HEADER}'
            continue
        if selected and line.startswith('@@'):
            body = True
        elif selected and body and line.startswith('+'):
            lines.append(line[1:])
    if not lines or not any('inline bool store' in line for line in lines):
        raise ValueError('Reviewed DurableFile header missing from the preceding patch')
    return '\n'.join(lines) + '\n'


def compile_control(destination):
    source = destination / 'source'
    header = source / HEADER
    header.parent.mkdir(parents=True)
    header.write_text(base_header())
    subprocess.run(['git', 'apply', '--check', str(PATCH)], cwd=source, check=True, capture_output=True)
    subprocess.run(['git', 'apply', str(PATCH)], cwd=source, check=True, capture_output=True)
    binary = destination / 'publication-control'
    compiled = subprocess.run([shutil.which('c++') or '/missing/c++', '-std=c++17', '-O2', '-I', str(source),
                               str(ROOT / 'scripts/tests/fixtures/cryfs_exclusive_publication.cpp'),
                               '-lboost_filesystem', '-o', str(binary)], capture_output=True, text=True, timeout=120)
    if compiled.returncode:
        raise RuntimeError('Native publication control compilation failed: ' + compiled.stderr[-5000:])
    return binary


@unittest.skipUnless(platform.system() == 'Linux', 'Linux-specific CryFS patch')
@unittest.skipUnless(shutil.which('c++') and Path('/usr/include/boost/filesystem.hpp').is_file(),
                     'requires a native C++ compiler and Boost filesystem development headers')
class ExclusivePublicationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temporary = tempfile.TemporaryDirectory(prefix='lurker-cryfs-publication-')
        cls.root = Path(cls.temporary.name)
        cls.binary = compile_control(cls.root)

    @classmethod
    def tearDownClass(cls):
        cls.temporary.cleanup()

    def run_control(self, mode, suffix, error=None):
        root = self.root / suffix
        root.mkdir(mode=0o700)
        argv = [str(self.binary), mode, str(root)]
        if error is not None:
            argv.append(str(error))
        result = subprocess.run(argv, capture_output=True, text=True, timeout=30)
        self.assertEqual(result.returncode, 0, result.stderr)
        receipt = json.loads(result.stdout)
        self.assertEqual(receipt['status'], 'passed')
        return root, receipt

    def test_absent_collision_and_replace_preserve_durability_order(self):
        root, receipt = self.run_control('normal', 'normal')
        self.assertEqual((root / 'publication').read_bytes(), b'updated')
        self.assertEqual(receipt['link_calls'], 0)
        self.assertIn('rename-noreplace:17', receipt['events'])
        self.assertEqual(receipt['events'].count('fsync-directory:0'), 2)

    def test_eight_creators_retain_exactly_one_complete_winner(self):
        root, _ = self.run_control('race', 'race')
        self.assertIn((root / 'race').read_text(), {f'writer-{n}' for n in range(8)})

    def test_only_unsupported_primitives_use_atomic_hardlink_fallback(self):
        for error in (errno.ENOSYS, errno.EINVAL, errno.EOPNOTSUPP):
            with self.subTest(error=error):
                _, receipt = self.run_control('error', f'unsupported-{error}', error)
                self.assertEqual(receipt['link_calls'], 1)
                self.assertEqual(receipt['events'][0], 'fsync-file:0')
                self.assertEqual(receipt['events'][-1], 'fsync-directory:0')

    def test_permission_and_io_errors_do_not_fallback_or_publish(self):
        for error in (errno.EPERM, errno.EACCES, errno.EIO):
            with self.subTest(error=error):
                root, receipt = self.run_control('error', f'error-{error}', error)
                self.assertEqual(receipt['link_calls'], 0)
                self.assertEqual(list(root.iterdir()), [])

    def test_existing_winner_never_uses_fallback(self):
        root, receipt = self.run_control('error', 'winner', errno.EEXIST)
        self.assertEqual((root / 'publication').read_bytes(), b'winner')
        self.assertEqual(receipt['link_calls'], 0)


if __name__ == '__main__':
    unittest.main()
