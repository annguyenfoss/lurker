package org.lurker.freebsdjfuse.tests;

import java.nio.file.FileStore;
import java.nio.file.Path;
import java.util.ServiceLoader;
import java.util.Set;
import org.cryptomator.integrations.mount.MountService;
import org.cryptomator.jfuse.api.FuseBuilder;

/** Native registration/configuration probe only. Never builds or mounts a FUSE filesystem. */
public final class ProviderProbe {
    public static void main(String[] args) throws Exception {
        if (!System.getProperty("os.name").equals("FreeBSD")
                || !System.getProperty("os.arch").equals("amd64")) throw new AssertionError("Native FreeBSD required");
        var builder = FuseBuilder.getSupported();
        if (!builder.getClass().getName().equals("org.cryptomator.jfuse.freebsd.amd64.FreeBsdFuseBuilder")) {
            throw new AssertionError("Incorrect native jFUSE provider");
        }
        var service = ServiceLoader.load(MountService.class).stream()
            .filter(p -> p.type().getName().equals("org.cryptomator.frontend.fuse.mount.FreeBsdFuseMountProvider"))
            .map(ServiceLoader.Provider::get).findFirst().orElseThrow();
        if (!service.isSupported()) throw new AssertionError("Native host prerequisite/provider unavailable");
        String flags = service.getDefaultMountFlags();
        if (!flags.contains("-s ") || !flags.contains("-odefault_permissions")
                || flags.contains("allow_other") || flags.contains("auto_unmount")) throw new AssertionError("Unsafe native defaults");
        var errors = Class.forName("org.cryptomator.jfuse.api.Errno");
        var transcoder = Class.forName("org.cryptomator.frontend.fuse.FileNameTranscoder");
        var names = transcoder.getMethod("transcoder").invoke(null);
        var type = Class.forName("org.cryptomator.frontend.fuse.ReadWriteAdapter");
        // The public upstream factory calls global getfsstat(MNT_WAIT) through
        // Files.getFileStore. This is a memory-only constructor fixture, not a
        // claim that that filesystem operation succeeded. A null FileStore is
        // deliberately never queried: any new dependency on it fails this gate.
        Path directory=Path.of(args[0]);
        var attempt=service.forFileSystem(directory);
        if (!(attempt instanceof AutoCloseable) || !(attempt instanceof java.util.function.BooleanSupplier))
            throw new AssertionError("Native owned-attempt close/retry capability missing");
        if (((java.util.function.BooleanSupplier)attempt).getAsBoolean()) throw new AssertionError("Unstarted builder cannot retry busy close");
        ((AutoCloseable)attempt).close(); // never-started builder; no factory or native channel
        if (((java.util.function.BooleanSupplier)attempt).getAsBoolean()) throw new AssertionError("Closed builder cannot retry");
        var factory=Class.forName("org.cryptomator.frontend.fuse.OpenFileFactory");
        var directories=Class.forName("org.cryptomator.frontend.fuse.ReadWriteDirectoryHandler");
        var files=Class.forName("org.cryptomator.frontend.fuse.ReadWriteFileHandler");
        var handles=factory.getConstructor().newInstance();
        var directoryHandler=directories.getConstructor(transcoder).newInstance(names);
        var fileHandler=files.getConstructor(factory).newInstance(handles);
        var constructor=type.getDeclaredConstructor(errors,Path.class,int.class,
                transcoder,FileStore.class,factory,directories,files,boolean.class);
        constructor.setAccessible(true);
        boolean fsync, fsyncdir;
        int nativeVersion;
        try (var adapter=(AutoCloseable)constructor.newInstance(builder.errno(),directory,
                254,names,null,handles,directoryHandler,fileHandler,false)) {
            var operations = (Set<?>)type.getMethod("supportedOperations").invoke(adapter);
            fsync = operations.stream().anyMatch(v -> ((Enum<?>)v).name().equals("FSYNC"));
            fsyncdir = operations.stream().anyMatch(v -> ((Enum<?>)v).name().equals("FSYNCDIR"));
            builder.setLibraryPath(Path.of("/usr/local/lib/libfuse3.so").toRealPath().toString());
            // Constructing/closing an unmounted Fuse only allocates/frees its
            // callback arena; no native filesystem handle or mount is created.
            try (var bindings=builder.build((org.cryptomator.jfuse.api.FuseOperations)adapter)) {
                nativeVersion=(int)Class.forName("org.cryptomator.jfuse.freebsd.amd64.FreeBsdNative")
                        .getMethod("fuseVersion").invoke(null);
            }
        }
        if (!fsync || fsyncdir) throw new AssertionError("Unexpected upstream sync operation inventory");
        System.out.println("{\"native_os\":\"FreeBSD\",\"architecture\":\"amd64\","
            +"\"jfuse_builder\":\""+builder.getClass().getName()+"\","
            +"\"mount_provider\":\""+service.getClass().getName()+"\","
            +"\"default_flags\":\""+flags+"\",\"native_fuse_version\":"+nativeVersion+",\"fsync_registered\":true,"
            +"\"fsyncdir_registered\":false,\"autocloseable_builder\":true,\"terminal_busy_supplier\":true,\"upstream_filestore_factory_exercised\":false,\"mount_invoked\":false}");
    }
}
