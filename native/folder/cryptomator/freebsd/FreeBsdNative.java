/* FreeBSD platform glue for the LGPL-3.0-or-later jFUSE module. */
package org.cryptomator.jfuse.freebsd.amd64;

import java.lang.foreign.FunctionDescriptor;
import java.lang.foreign.Linker;
import java.lang.foreign.ValueLayout;
import java.lang.invoke.MethodHandle;

public final class FreeBsdNative {
    private FreeBsdNative() {}
    private static MethodHandle function(String name) {
        return Linker.nativeLinker().downcallHandle(
            Linker.nativeLinker().defaultLookup().find(name).orElseThrow(),
            FunctionDescriptor.of(ValueLayout.JAVA_INT));
    }
    private static final MethodHandle UID = function("getuid");
    private static final MethodHandle EUID = function("geteuid");
    private static final MethodHandle GID = function("getgid");
    private static final MethodHandle EGID = function("getegid");
    private static int value(MethodHandle handle) {
        try { return (int) handle.invokeExact(); }
        catch (Throwable error) { throw new IllegalStateException("Native identity query failed", error); }
    }
    public static int uid() { return value(UID); }
    public static int gid() { return value(GID); }
    /** Loading and symbol readiness only; never calls fuse_new or mount. */
    public static int fuseVersion() {
        requireLoginUser();
        var lookup=java.lang.foreign.SymbolLookup.loaderLookup();
        for (String name:new String[]{"fuse_new_31","fuse_parse_cmdline"}) {
            lookup.find(name).orElseThrow(()->new UnsatisfiedLinkError(name));
        }
        return org.cryptomator.jfuse.freebsd.amd64.extr.fuse3.fuse_h.fuse_version();
    }
    public static void requireLoginUser() {
        if (!System.getProperty("os.name").equals("FreeBSD")
                || !System.getProperty("os.arch").equals("amd64")
                || uid() == 0 || uid() != value(EUID) || gid() != value(EGID)) {
            throw new UnsupportedOperationException("Native FreeBSD amd64 login user required");
        }
    }
}
