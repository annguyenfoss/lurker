"""Derive FreeBSD bindings exclusively from its native compiled header probe.

This module only writes source. Compilation and execution belong on the native
target. Upstream jFUSE wrappers are prepared separately from verified sources.
"""
from __future__ import annotations
import json
from pathlib import Path
from abi_schema import STRUCTS, CALLBACKS, CONSTANTS, C_MEMBERS

PACKAGE = 'org.cryptomator.jfuse.freebsd.amd64'
BASE = Path(*PACKAGE.split('.'))

# Native C pointer types are deliberately explicit. The compiler checks every
# prototype against fuse_operations, including FreeBSD's 16-bit mode_t.
C_ARGS = {
    'getattr': ['const char *', 'struct stat *', 'struct fuse_file_info *'],
    'readlink': ['const char *', 'char *', 'size_t'],
    'mkdir': ['const char *', 'mode_t'],
    'unlink': ['const char *'], 'rmdir': ['const char *'],
    'symlink': ['const char *', 'const char *'],
    'rename': ['const char *', 'const char *', 'unsigned int'],
    'chmod': ['const char *', 'mode_t', 'struct fuse_file_info *'],
    'chown': ['const char *', 'uid_t', 'gid_t', 'struct fuse_file_info *'],
    'truncate': ['const char *', 'off_t', 'struct fuse_file_info *'],
    'open': ['const char *', 'struct fuse_file_info *'],
    'read': ['const char *', 'char *', 'size_t', 'off_t', 'struct fuse_file_info *'],
    'write': ['const char *', 'const char *', 'size_t', 'off_t', 'struct fuse_file_info *'],
    'statfs': ['const char *', 'struct statvfs *'],
    'flush': ['const char *', 'struct fuse_file_info *'],
    'release': ['const char *', 'struct fuse_file_info *'],
    'fsync': ['const char *', 'int', 'struct fuse_file_info *'],
    'setxattr': ['const char *', 'const char *', 'const char *', 'size_t', 'int'],
    'getxattr': ['const char *', 'const char *', 'char *', 'size_t'],
    'listxattr': ['const char *', 'char *', 'size_t'],
    'removexattr': ['const char *', 'const char *'],
    'opendir': ['const char *', 'struct fuse_file_info *'],
    'readdir': ['const char *', 'void *', 'fuse_fill_dir_t', 'off_t',
                'struct fuse_file_info *', 'enum fuse_readdir_flags'],
    'releasedir': ['const char *', 'struct fuse_file_info *'],
    'fsyncdir': ['const char *', 'int', 'struct fuse_file_info *'],
    'init': ['struct fuse_conn_info *', 'struct fuse_config *'],
    'destroy': ['void *'], 'access': ['const char *', 'int'],
    'create': ['const char *', 'mode_t', 'struct fuse_file_info *'],
    'utimens': ['const char *', 'const struct timespec *', 'struct fuse_file_info *'],
}


def write_c_include(path: Path) -> None:
    out = []
    for name, (result, _) in CALLBACKS.items():
        ret = {'I': 'int', 'P': 'void *', 'V': 'void'}[result]
        out += [f'typedef {ret} (*check_{name})({", ".join(C_ARGS[name])});',
                f'_Static_assert(__builtin_types_compatible_p(check_{name}, '
                f'__typeof__(((struct fuse_operations *)0)->{name})), "{name} signature");']
    out += ['static void print_structs(void) {', 'printf("\\\"structs\\\":{");']
    structs = {**STRUCTS, 'fuse_operations': [(n, 'address') for n in CALLBACKS]}
    fields = []
    for i, (name, members) in enumerate(structs.items()):
        out += [f'printf("{"," if i else ""}\\\"{name}\\\":{{\\\"size\\\":%zu,\\\"align\\\":%zu,\\\"fields\\\":{{", sizeof(struct {name}), _Alignof(struct {name}));']
        for j, (member, kind) in enumerate(members):
            c_member = C_MEMBERS.get((name, member), member)
            out += [f'printf("{"," if j else ""}\\\"{member}\\\":{{\\\"offset\\\":%zu,\\\"size\\\":%zu,\\\"api\\\":\\\"{kind}\\\"}}", offsetof(struct {name}, {c_member}), sizeof(((struct {name} *)0)->{c_member}));']
            fields.append((name, member, kind))
        out += ['printf("}}");']
    out += ['printf("},");', '}', 'static void print_constants(void) {',
            'printf("\\\"constants\\\":{");']
    for i, (group, constants) in enumerate(CONSTANTS.items()):
        out += [f'printf("{"," if i else ""}\\\"{group}\\\":{{");']
        for j, constant in enumerate(constants):
            out += [f'printf("{"," if j else ""}\\\"{constant}\\\":%lld", (long long){constant});']
        out += ['printf("}");']
    out += ['printf("}");', '}',
            'uint64_t jfuse_offset(int id) { switch (id) {']
    for i, (name, member, _) in enumerate(fields):
        out += [f'case {i}: return offsetof(struct {name}, {C_MEMBERS.get((name, member), member)});']
    out += ['default: return UINT64_MAX; }}',
            'uint64_t jfuse_dimension(int id, int alignment) { switch (id) {']
    for i, name in enumerate(structs):
        out += [f'case {i}: return alignment ? _Alignof(struct {name}) : sizeof(struct {name});']
    out += ['default: return UINT64_MAX; }}',
            'uint64_t jfuse_constant(int id) { switch (id) {']
    for i, constant in enumerate(n for values in CONSTANTS.values() for n in values):
        out += [f'case {i}: return (uint64_t)(int64_t){constant};']
    out += ['default: return UINT64_MAX; }}',
            'uint64_t jfuse_field(int id, void *memory, int writing, uint64_t bits) {',
            'switch (id) {']
    for i, (name, member, kind) in enumerate(fields):
        if kind == 'timespec':
            continue
        value = f'((struct {name} *)memory)->{C_MEMBERS.get((name, member), member)}'
        if kind == 'bytes':
            out += [f'case {i}: if (writing) memset({value}, (unsigned char)bits, sizeof({value})); '
                    f'return (unsigned char){value}[0];']
        elif kind == 'address':
            out += [f'case {i}: if (writing) {value} = (__typeof__({value}))(uintptr_t)bits; '
                    f'return (uint64_t)(uintptr_t){value};']
        elif kind == 'double':
            out += [f'case {i}: if (writing) memcpy(&{value}, &bits, sizeof(bits)); '
                    f'else memcpy(&bits, &{value}, sizeof(bits)); return bits;']
        else:
            out += [f'case {i}: if (writing) {value} = (__typeof__({value}))bits; '
                    f'return (uint64_t){value};']
    out += ['default: return UINT64_MAX;', '}', '}']
    out += ['int jfuse_callbacks(struct fuse_operations *ops) {']
    for i, (name, (ret, args)) in enumerate(CALLBACKS.items()):
        expressions = []
        for j, (kind, ctype) in enumerate(zip(args, C_ARGS[name], strict=True)):
            if kind == 'P': value = f'({ctype})(uintptr_t){0x1000 + i * 256 + j * 8}'
            elif kind == 'M': value = '(mode_t)0xfedc'
            elif kind in ('Z', 'O'): value = f'({ctype})UINT64_C({0x100000011 + i * 257 + j})'
            else: value = f'({ctype}){1234 + j}'
            expressions.append(value)
        call = f'ops->{name}({", ".join(expressions)})'
        if ret == 'V': out += [call + ';']
        elif ret == 'P': out += [f'if ({call} != (void *)(uintptr_t)0x7000) return {i + 1};']
        else: out += [f'if ({call} != {i + 1}) return {i + 1};']
    out += ['return 0;', '}',
            'const char *jfuse_native_string(void) { return "genuine FreeBSD native string"; }',
            'int jfuse_string_callback(int (*callback)(const char *)) { return callback(jfuse_native_string()); }']
    path.write_text('\n'.join(out) + '\n')


def validate_profile(profile: dict) -> None:
    if (profile.get('schema') != 1 or profile.get('os') != 'FreeBSD'
            or profile.get('architecture') != 'amd64' or profile.get('uid') == 0
            or profile.get('uid') != profile.get('euid')):
        raise ValueError('Native FreeBSD login-user ABI profile required')
    if profile['types'] != dict(int=4, long=8, pointer=8, mode_t=2,
                                size_t=8, off_t=8, uid_t=4, gid_t=4):
        raise ValueError('Unqualified FreeBSD ABI widths')
    expected = {**STRUCTS, 'fuse_operations': [(n, 'address') for n in CALLBACKS]}
    if set(profile['structs']) != set(expected):
        raise ValueError('Native structure inventory differs')
    for name, members in expected.items():
        record = profile['structs'][name]
        if record['size'] <= 0 or record['align'] not in (1, 2, 4, 8, 16):
            raise ValueError('Invalid native structure dimensions')
        if set(record['fields']) != {n for n, _ in members}:
            raise ValueError('Native field inventory differs: ' + name)
        for field, kind in members:
            value = record['fields'][field]
            if (value['api'] != kind or value['offset'] < 0 or value['size'] <= 0
                    or value['offset'] + value['size'] > record['size']):
                raise ValueError('Native field lies outside its structure')
            if kind not in ('timespec', 'bytes') and value['size'] not in (1, 2, 4, 8):
                raise ValueError('Unsupported native scalar width')
    if set(profile['constants']) != set(CONSTANTS):
        raise ValueError('Native constant groups differ')
    for name, entries in CONSTANTS.items():
        if set(profile['constants'][name]) != set(entries):
            raise ValueError('Native constant inventory differs')


def java_header(package: str) -> str:
    return ('// Generated from compiled native FreeBSD headers; see abi-profile.json.\n'
            f'package {package};\nimport java.lang.foreign.*;\n'
            'import java.lang.invoke.*;\n')


def location(name: str) -> str:
    return 'fuse3_lowlevel' if name == 'fuse_cmdline_opts' else 'fuse3'


def generate_struct(name: str, record: dict) -> str:
    text = java_header(PACKAGE + '.extr.' + location(name))
    text += f'public final class {name} {{\n'
    text += 'private static final AddressLayout POINTER=ValueLayout.ADDRESS.withTargetLayout(MemoryLayout.sequenceLayout(Long.MAX_VALUE,ValueLayout.JAVA_BYTE));\n'
    text += f'public static long sizeof() {{ return {record["size"]}L; }}\n'
    text += f'public static long alignof() {{ return {record["align"]}L; }}\n'
    text += ('public static MemorySegment allocate(SegmentAllocator a) { '
             'return a.allocate(sizeof(), alignof()).fill((byte)0); }\n'
             'public static MemorySegment asSlice(MemorySegment m, long i) { '
             'return m.reinterpret(Math.multiplyExact(i+1,sizeof())).asSlice(Math.multiplyExact(i,sizeof()),sizeof()); }\n')
    for field, value in record['fields'].items():
        offset, width, api = value['offset'], value['size'], value['api']
        text += f'public static long {field}$offset() {{ return {offset}L; }}\n'
        if api in ('timespec', 'bytes'):
            text += f'public static MemorySegment {field}(MemorySegment m) {{ return m.reinterpret(sizeof()).asSlice({offset},{width}); }}\n'
            continue
        native = 'ADDRESS' if api == 'address' else 'JAVA_DOUBLE' if api == 'double' else {1:'JAVA_BYTE', 2:'JAVA_SHORT', 4:'JAVA_INT', 8:'JAVA_LONG'}[width]
        layout = 'POINTER' if api == 'address' else 'ValueLayout.' + native
        carrier = {'int':'int', 'long':'long', 'double':'double', 'address':'MemorySegment'}[api]
        cast = {'JAVA_BYTE':'byte', 'JAVA_SHORT':'short', 'JAVA_INT':'int', 'JAVA_LONG':'long', 'JAVA_DOUBLE':'double', 'ADDRESS':'MemorySegment'}[native]
        read = f'm.reinterpret(sizeof()).get({layout},{offset})'
        if api in ('int', 'long') and width < 4:
            read = ('Byte' if width == 1 else 'Short') + '.toUnsignedInt(' + read + ')'
        text += f'public static {carrier} {field}(MemorySegment m) {{ return {read}; }}\n'
        text += f'public static void {field}(MemorySegment m, {carrier} v) {{ '
        if api in ('int', 'long') and width < (4 if api == 'int' else 8):
            text += f'if (v < 0 || v > {(1 << (8*width))-1}L) throw new IllegalArgumentException("native {field} overflow"); '
        text += f'm.reinterpret(sizeof()).set({layout},{offset},({cast})v); }}\n'
    return text + '}\n'


def callback_class(name: str, result: str, args: str) -> str:
    layouts = dict(P='POINTER', I='ValueLayout.JAVA_INT',
                   U='ValueLayout.JAVA_INT', M='ValueLayout.JAVA_SHORT',
                   Z='ValueLayout.JAVA_LONG', O='ValueLayout.JAVA_LONG')
    carriers = dict(P='MemorySegment', I='int', U='int', M='short', Z='long', O='long', V='void')
    ret = carriers[result]
    arguments = ', '.join(f'{carriers[k]} p{i}' for i, k in enumerate(args))
    descriptor = ('FunctionDescriptor.ofVoid(' if result == 'V' else
                  'FunctionDescriptor.of(' + layouts[result] + (', ' if args else ''))
    descriptor += ', '.join(layouts[k] for k in args) + ')'
    classes = ', '.join(carriers[k] + '.class' for k in args)
    return f'''public static final class {name} {{
public interface Function {{ {ret} apply({arguments}); }}
private static final FunctionDescriptor DESC = {descriptor};
public static MemorySegment allocate(Function f, Arena a) {{
 try {{ return Linker.nativeLinker().upcallStub(MethodHandles.lookup().findVirtual(Function.class,"apply",MethodType.methodType({ret}.class{', ' if classes else ''}{classes})).bindTo(f),DESC,a); }}
 catch (ReflectiveOperationException e) {{ throw new AssertionError(e); }}
}}
}}
'''


def write_bindings(profile: dict, destination: Path) -> None:
    validate_profile(profile)
    for name, record in profile['structs'].items():
        text = generate_struct(name, record)
        if name == 'fuse_operations':
            text = text[:-2] + ''.join(callback_class(n, *sig) for n, sig in CALLBACKS.items()) + '}\n'
        path = destination / BASE / 'extr' / location(name) / (name + '.java')
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
    for group, constants in profile['constants'].items():
        folder = {'fcntl_h':'fcntl', 'errno_h':'errno', 'stat_h':'stat', 'mount_h':'mount'}[group]
        text = java_header(PACKAGE + '.extr.' + folder) + f'public final class {group} {{\n'
        for name, value in constants.items():
            text += f'public static int {name}() {{ return {value}; }}\n'
        path = destination / BASE / 'extr' / folder / (group + '.java')
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text + '}\n')


def write_memory_test(profile: dict, destination: Path) -> None:
    """C writes -> Java reads; Java writes -> C reads; every bound upcall.

    The test DSO is never shipped in the runtime image. No callback dereferences
    the synthetic pointer sentinels, and failed checks never throw through C.
    """
    validate_profile(profile)
    text = java_header('org.lurker.freebsdjfuse.tests')
    text += f'import {PACKAGE}.extr.fuse3.*;\n'
    text += f'import {PACKAGE}.extr.fuse3_lowlevel.*;\n'
    text += '''public final class NativeMemoryTest {
static int failures, fields, callbacks;
static void check(boolean ok) { if (!ok) failures++; }
public static void main(String[] args) throws Throwable {
 if (!System.getProperty("os.name").equals("FreeBSD") || !System.getProperty("os.arch").equals("amd64")) throw new AssertionError("native FreeBSD amd64 required");
 System.load(java.nio.file.Path.of(args[0]).toRealPath().toString());
 var lookup = SymbolLookup.loaderLookup();
 var field = Linker.nativeLinker().downcallHandle(lookup.find("jfuse_field").orElseThrow(),FunctionDescriptor.of(ValueLayout.JAVA_LONG,ValueLayout.JAVA_INT,ValueLayout.ADDRESS,ValueLayout.JAVA_INT,ValueLayout.JAVA_LONG));
 var call = Linker.nativeLinker().downcallHandle(lookup.find("jfuse_callbacks").orElseThrow(),FunctionDescriptor.of(ValueLayout.JAVA_INT,ValueLayout.ADDRESS));
 var offset = Linker.nativeLinker().downcallHandle(lookup.find("jfuse_offset").orElseThrow(),FunctionDescriptor.of(ValueLayout.JAVA_LONG,ValueLayout.JAVA_INT));
 var dimension = Linker.nativeLinker().downcallHandle(lookup.find("jfuse_dimension").orElseThrow(),FunctionDescriptor.of(ValueLayout.JAVA_LONG,ValueLayout.JAVA_INT,ValueLayout.JAVA_INT));
 var constant = Linker.nativeLinker().downcallHandle(lookup.find("jfuse_constant").orElseThrow(),FunctionDescriptor.of(ValueLayout.JAVA_LONG,ValueLayout.JAVA_INT));
 try (var arena = Arena.ofConfined()) {
'''
    field_id = 0
    for structure_id, (name, record) in enumerate(profile['structs'].items()):
        text += f'var m_{name} = {name}.allocate(arena);\n'
        text += f'check(m_{name}.byteSize()=={record["size"]} && m_{name}.address()%{record["align"]}==0);\n'
        text += f'check((long)dimension.invokeExact({structure_id},0)=={name}.sizeof() && (long)dimension.invokeExact({structure_id},1)=={name}.alignof());\n'
        for member, value in record['fields'].items():
            api, width = value['api'], value['size']
            text += f'check({name}.{member}$offset()=={value["offset"]}L);\n'
            text += f'check((long)offset.invokeExact({field_id})=={name}.{member}$offset());\n'
            if api == 'bytes':
                text += f'long ignored_{field_id}=(long)field.invokeExact({field_id},m_{name},1,0x43L); check({name}.{member}(m_{name}).toArray(ValueLayout.JAVA_BYTE).length=={width}); '
                text += f'for(byte b:{name}.{member}(m_{name}).toArray(ValueLayout.JAVA_BYTE)) check(b==0x43); '
                text += f'{name}.{member}(m_{name}).fill((byte)0x56); check((long)field.invokeExact({field_id},m_{name},0,0L)==0x56L); fields++;\n'
            elif api == 'timespec':
                text += f'check({name}.{member}(m_{name}).address()==m_{name}.address()+{value["offset"]}L);\n'
            elif api == 'address':
                text += f'long ignored_{field_id}=(long)field.invokeExact({field_id},m_{name},1,0x4320L); check({name}.{member}(m_{name}).address()==0x4320); '
                text += f'{name}.{member}(m_{name},MemorySegment.ofAddress(0x5670)); check((long)field.invokeExact({field_id},m_{name},0,0L)==0x5670L); fields++;\n'
            else:
                bits = 'Double.doubleToRawLongBits(123.5)' if api == 'double' else ('0x1234L' if width < 4 else '0x12345678L' if width == 4 else '0x1122334455667788L')
                read = f'{name}.{member}(m_{name})'
                if api == 'double': read = 'Double.doubleToRawLongBits(' + read + ')'
                cast = {'int':'int', 'long':'long', 'double':'double'}[api]
                java_value = '123.5' if api == 'double' else f'({cast}){bits}'
                # invokeExact needs an explicit return carrier even when ignored.
                text += f'long ignored_{field_id}=(long)field.invokeExact({field_id},m_{name},1,{bits});\n'
                text += f'check({read}=={bits}); m_{name}.fill((byte)0); {name}.{member}(m_{name},{java_value}); check((long)field.invokeExact({field_id},m_{name},0,0L)=={bits}); fields++;\n'
            field_id += 1
    constant_id = 0
    for group, constants in profile['constants'].items():
        package = dict(fcntl_h='fcntl', errno_h='errno', stat_h='stat', mount_h='mount')[group]
        for constant_name in constants:
            text += f'check((long)constant.invokeExact({constant_id})==(long){PACKAGE}.extr.{package}.{group}.{constant_name}());\n'
            constant_id += 1
    text += 'var ops = fuse_operations.allocate(arena);\n'
    for i, (name, (ret, arguments)) in enumerate(CALLBACKS.items()):
        variables = ','.join('a' + str(j) for j in range(len(arguments)))
        checks = []
        for j, kind in enumerate(arguments):
            if kind == 'P': expression = f'a{j}.address()=={0x1000+i*256+j*8}L'
            elif kind == 'M': expression = f'Short.toUnsignedInt(a{j})==0xfedc'
            elif kind in ('O', 'Z'): expression = f'a{j}=={0x100000011+i*257+j}L'
            else: expression = f'a{j}=={1234+j}'
            checks.append(expression)
        result = '' if ret == 'V' else 'return MemorySegment.ofAddress(0x7000);' if ret == 'P' else f'return {i+1};'
        text += f'fuse_operations.{name}(ops,fuse_operations.{name}.allocate(({variables})->{{ check({" && ".join(checks)}); callbacks++; {result} }},arena));\n'
    text += f'''check((int)call.invokeExact(ops)==0); check(callbacks=={len(CALLBACKS)});
 var nativePointer=ValueLayout.ADDRESS.withTargetLayout(MemoryLayout.sequenceLayout(Long.MAX_VALUE,ValueLayout.JAVA_BYTE));
 var nativeString = Linker.nativeLinker().downcallHandle(lookup.find("jfuse_native_string").orElseThrow(),FunctionDescriptor.of(nativePointer));
 var stringCallback = Linker.nativeLinker().downcallHandle(lookup.find("jfuse_string_callback").orElseThrow(),FunctionDescriptor.of(ValueLayout.JAVA_INT,ValueLayout.ADDRESS));
 var pointer = (MemorySegment)nativeString.invokeExact(); check(pointer.getString(0).equals("genuine FreeBSD native string"));
 fuse_cmdline_opts.mountpoint(m_fuse_cmdline_opts,pointer); check(fuse_cmdline_opts.mountpoint(m_fuse_cmdline_opts).getString(0).equals("genuine FreeBSD native string"));
 var callback=fuse_operations.unlink.allocate(p->{{check(p.getString(0).equals("genuine FreeBSD native string"));return 420;}},arena);
 check((int)stringCallback.invokeExact(callback)==420);
 }}
 System.out.println("{{\\\"fields\\\":"+fields+",\\\"callbacks\\\":"+callbacks+",\\\"native_strings\\\":3,\\\"failures\\\":"+failures+"}}");
 if (failures!=0) throw new AssertionError("native ABI round trip failed");
}}
}}
'''
    path = destination / 'org/lurker/freebsdjfuse/tests/NativeMemoryTest.java'
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)


FUSE_FUNCTIONS = {
    'fuse_version': ('I', ''), 'fuse_mount': ('I', 'PP'),
    'fuse_loop': ('I', 'P'), 'fuse_exit': ('V', 'P'),
    'fuse_unmount': ('V', 'P'), 'fuse_destroy': ('V', 'P'),
    'fuse_lib_help': ('V', 'P'),
}


def write_functions(destination: Path) -> None:
    package = PACKAGE + '.extr.fuse3'
    text = java_header(package) + '''public final class fuse_h {
public static final AddressLayout C_POINTER=ValueLayout.ADDRESS.withTargetLayout(MemoryLayout.sequenceLayout(Long.MAX_VALUE,ValueLayout.JAVA_BYTE));
public static final ValueLayout.OfLong C_LONG=ValueLayout.JAVA_LONG;
private static MethodHandle function(String name,FunctionDescriptor descriptor) {
 return Linker.nativeLinker().downcallHandle(SymbolLookup.loaderLookup().find(name).orElseThrow(()->new UnsatisfiedLinkError(name)),descriptor);
}
'''
    layouts = dict(P='ValueLayout.ADDRESS', I='ValueLayout.JAVA_INT')
    types = dict(P='MemorySegment', I='int', V='void')
    for name, (ret, args) in FUSE_FUNCTIONS.items():
        parameters = ','.join(f'{types[k]} a{i}' for i, k in enumerate(args))
        desc = 'FunctionDescriptor.ofVoid(' if ret == 'V' else 'FunctionDescriptor.of(' + layouts[ret] + (',' if args else '')
        desc += ','.join(layouts[k] for k in args) + ')'
        text += f'private static final MethodHandle H_{name}=function("{name}",{desc});\n'
        invoke = f'H_{name}.invokeExact({",".join("a"+str(i) for i in range(len(args)))})'
        result = invoke + ';' if ret == 'V' else 'return (' + types[ret] + ')' + invoke + ';'
        text += f'public static {types[ret]} {name}({parameters}) {{ try {{ {result} }} catch(Throwable e) {{ throw new AssertionError("native {name} failed",e); }} }}\n'
    path = destination / BASE / 'extr/fuse3/fuse_h.java'
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text + '}\n')
    filler = java_header(package) + '''public final class fuse_fill_dir_t {
private static final MethodHandle CALL=Linker.nativeLinker().downcallHandle(FunctionDescriptor.of(ValueLayout.JAVA_INT,ValueLayout.ADDRESS,ValueLayout.ADDRESS,ValueLayout.ADDRESS,ValueLayout.JAVA_LONG,ValueLayout.JAVA_INT));
public static int invoke(MemorySegment fn,MemorySegment buf,MemorySegment name,MemorySegment stat,long offset,int flags) {
 try { return (int)CALL.invokeExact(fn,buf,name,stat,offset,flags); }
 catch(Throwable e) { throw new AssertionError("native directory filler failed",e); }
}
}
'''
    (path.parent / 'fuse_fill_dir_t.java').write_text(filler)
