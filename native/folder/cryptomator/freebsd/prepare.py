"""Prepare narrowly derived platform modules from checksum-verified releases.

No Linux generated ABI class is retained. Metadata/crypto algorithms are not
implemented here: CryptoFS and its locked jar remain unchanged.
"""
from pathlib import Path
import hashlib
import re
import stat
import zipfile

OLD = 'org.cryptomator.jfuse.linux.amd64'
NEW = 'org.cryptomator.jfuse.freebsd.amd64'


def replace_once(text, before, after):
    if text.count(before) != 1:
        raise ValueError('Pinned source transformation anchor differs: ' + before[:70])
    return text.replace(before, after)


def java_sources(archive, destination):
    destination.mkdir(parents=True)
    with zipfile.ZipFile(archive) as entries:
        for record in entries.infolist():
            name = Path(record.filename)
            if name.is_absolute() or '..' in name.parts or stat.S_ISLNK(record.external_attr >> 16):
                raise ValueError('Unsafe source archive member')
            if not str(name).endswith('.java'):
                continue
            target = destination / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(entries.read(record))


def prepare_api(archive, destination):
    java_sources(archive, destination)
    enum = destination / 'org/cryptomator/jfuse/api/platforms/OperatingSystem.java'
    text = replace_once(enum.read_text(), '\tUNKNOWN;', '\tFREEBSD,\n\tUNKNOWN;')
    text = replace_once(text, 'if (OS_NAME.contains("linux")) {',
                        'if (OS_NAME.equals("freebsd")) {\n\t\t\treturn FREEBSD;\n\t\t} else if (OS_NAME.contains("linux")) {')
    enum.write_text(text)
    module = destination / 'module-info.java'
    text = module.read_text()
    before = 'org.cryptomator.jfuse.mac, org.cryptomator.jfuse.win;'
    if text.count(before) != 2:
        raise ValueError('jFUSE API qualified export inventory differs')
    module.write_text(text.replace(before, before[:-1] + ', ' + NEW + ';'))


def prepare_platform(archive, destination, owned):
    destination.mkdir(parents=True)
    with zipfile.ZipFile(archive) as entries:
        for name in entries.namelist():
            if not name.endswith('.java') or '/extr/' in name or name == 'module-info.java':
                continue
            if name.startswith('/') or '..' in Path(name).parts:
                raise ValueError('Unsafe upstream wrapper path')
            text = entries.read(name).decode().replace(OLD, NEW)
            text = text.replace('LinuxFuseBuilder', 'FreeBsdFuseBuilder').replace('LinuxErrno', 'FreeBsdErrno')
            output = destination / name.replace(OLD.replace('.', '/'), NEW.replace('.', '/'))
            output = output.with_name(output.name.replace('Linux', 'FreeBsd'))
            if output.name == 'FuseMountImpl.java':
                # A deliberate single-threaded native first profile. No Linux
                # versioned loop/config ABI or ignored multithreading promise.
                text = f'''package {NEW};
import org.cryptomator.jfuse.api.FuseMount;
import {NEW}.extr.fuse3.fuse_h;
import java.lang.foreign.MemorySegment;
record FuseMountImpl(MemorySegment fuse, FuseArgs fuseArgs) implements FuseMount {{
 public int loop() {{ return fuse_h.fuse_loop(fuse); }}
 public void unmount() {{ fuse_h.fuse_exit(fuse); fuse_h.fuse_unmount(fuse); }}
 public void destroy() {{ fuse_h.fuse_destroy(fuse); }}
}}
'''
            elif output.name == 'FreeBsdFuseBuilder.java':
                text = text.replace('OperatingSystem.LINUX', 'OperatingSystem.FREEBSD')
                old = '''		if (libraryPath != null) {
			System.load(libraryPath);
		} else {
			System.loadLibrary(DEFAULT_LIBNAME);
		}'''
                text = replace_once(text, old, '''		FreeBsdNative.requireLoginUser();
		if (libraryPath == null) throw new UnsatisfiedLinkError("Explicit native host FUSE3 path required");
		System.load(libraryPath);''')
            elif output.name == 'StatImpl.java':
                text = replace_once(text, 'return new TimeSpecImpl(stat.st_ctim(segment));\n\t}\n\n\t@Override\n\tpublic void setMode',
                    'return new TimeSpecImpl(stat.st_birthtim(segment));\n\t}\n\n\t@Override\n\tpublic void setMode')
            elif output.name == 'FuseImpl.java':
                text = text.replace('connInfo.want() | FuseConnInfo.FUSE_CAP_READDIRPLUS',
                                    'connInfo.want() | (connInfo.capable() & FuseConnInfo.FUSE_CAP_READDIRPLUS)')
                text = replace_once(text, 'var config = new FuseConfigImpl(cfg);',
                    f'{NEW}.extr.fuse3.fuse_config.direct_io(cfg, 1);\n\t\tvar config = new FuseConfigImpl(cfg);')
                for method in ['chmod', 'create', 'mkdir']:
                    text = replace_once(text, f'private int {method}(MemorySegment path, int mode',
                                        f'private int {method}(MemorySegment path, short mode')
                    text = replace_once(text, f'fuseOperations.{method}(path.getString(0), mode',
                                        f'fuseOperations.{method}(path.getString(0), Short.toUnsignedInt(mode)')
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_text(text)
    root = destination / Path(*NEW.split('.'))
    (root / 'FreeBsdNative.java').write_text((owned / 'FreeBsdNative.java').read_text())
    (destination / 'module-info.java').write_text(f'''module {NEW} {{
 requires static org.jetbrains.annotations;
 requires org.cryptomator.jfuse.api;
 exports {NEW} to org.cryptomator.frontend.fuse;
 provides org.cryptomator.jfuse.api.FuseBuilder with {NEW}.FreeBsdFuseBuilder;
}}
''')


def prepare_provider(archive, destination, owned):
    java_sources(archive, destination)
    module = destination / 'module-info.java'
    text = replace_once(module.read_text(), 'requires org.cryptomator.jfuse;',
                        'requires org.cryptomator.jfuse;\n\trequires ' + NEW + ';')
    text = replace_once(text, 'provides MountService with LinuxFuseMountProvider,',
                        'provides MountService with org.cryptomator.frontend.fuse.mount.FreeBsdFuseMountProvider, LinuxFuseMountProvider,')
    module.write_text(text)
    output = destination / 'org/cryptomator/frontend/fuse/mount/FreeBsdFuseMountProvider.java'
    output.write_text((owned / output.name).read_text())
