/* Native FreeBSD MountService for FUSE-NIO 5.1.0, AGPL-3.0-only. */
package org.cryptomator.frontend.fuse.mount;

import org.cryptomator.frontend.fuse.FileNameTranscoder;
import org.cryptomator.frontend.fuse.FuseNioAdapter;
import org.cryptomator.frontend.fuse.ReadWriteAdapter;
import org.cryptomator.integrations.mount.*;
import org.cryptomator.jfuse.api.Fuse;
import org.cryptomator.jfuse.api.FuseMountFailedException;
import org.cryptomator.jfuse.freebsd.amd64.FreeBsdNative;

import java.io.IOException;
import java.nio.file.*;
import java.util.EnumSet;
import java.util.Objects;
import java.util.Set;
import java.util.concurrent.TimeoutException;

public final class FreeBsdFuseMountProvider implements MountService {
    private static Path library() throws IOException {
        for (String name : new String[]{"/usr/local/lib/libfuse3.so.4", "/usr/local/lib/libfuse3.so.3"}) {
            Path alias = Path.of(name);
            if (!Files.exists(alias)) continue;
            Path resolved = alias.toRealPath();
            if (!resolved.getParent().equals(Path.of("/usr/local/lib"))) continue;
            for (Path p = alias; p != null; p = p.getParent()) {
                if ((int)Files.getAttribute(p,"unix:uid",LinkOption.NOFOLLOW_LINKS) != 0
                        || (!Files.isSymbolicLink(p) && ((int)Files.getAttribute(p,"unix:mode",LinkOption.NOFOLLOW_LINKS) & 0022) != 0)) {
                    throw new IOException("Untrusted native FUSE3 alias ancestry");
                }
            }
            if ((int)Files.getAttribute(resolved,"unix:uid") != 0
                    || ((int)Files.getAttribute(resolved,"unix:mode") & 0022) != 0
                    || !Files.isRegularFile(resolved)) throw new IOException("Untrusted native FUSE3 provider");
            return resolved;
        }
        throw new IOException("Separately installed native host FUSE3 required");
    }

    @Override public String displayName() { return "FreeBSD FUSE3"; }
    @Override public boolean isSupported() {
        try { FreeBsdNative.requireLoginUser(); library(); return true; }
        catch (RuntimeException | IOException | LinkageError error) { return false; }
    }
    @Override public Set<MountCapability> capabilities() {
        return EnumSet.of(MountCapability.MOUNT_FLAGS, MountCapability.MOUNT_TO_EXISTING_DIR);
    }
    @Override public String getDefaultMountFlags() {
        FreeBsdNative.requireLoginUser();
        return "-s -odefault_permissions -ouid="+Integer.toUnsignedString(FreeBsdNative.uid())
                +" -ogid="+Integer.toUnsignedString(FreeBsdNative.gid())
                +" -oattr_timeout=0 -oentry_timeout=0 -onegative_timeout=0";
    }
    @Override public MountBuilder forFileSystem(Path root) { return new Builder(root); }

    private static final class Builder extends AbstractMountBuilder {
        Builder(Path root) { super(root); }
        @Override public MountBuilder setMountpoint(Path point) {
            if (!Files.isDirectory(point,LinkOption.NOFOLLOW_LINKS)) throw new IllegalArgumentException("Existing native directory required");
            return super.setMountpoint(point);
        }
        @Override public Mount mount() throws MountFailedException {
            FreeBsdNative.requireLoginUser();
            Objects.requireNonNull(mountPoint); Objects.requireNonNull(mountFlags);
            if (mountFlags.stream().anyMatch(f -> f.contains("allow_other") || f.contains("allow_root")
                    || f.contains("auto_unmount") || f.contains("kernel_cache") || f.contains("writeback_cache"))) {
                throw new IllegalArgumentException("Private direct-I/O mount profile required");
            }
            var builder = Fuse.builder();
            try { builder.setLibraryPath(library().toString()); }
            catch (IOException error) { throw new MountFailedException(error); }
            var adapter = ReadWriteAdapter.create(builder.errno(),vfsRoot,
                    FuseNioAdapter.DEFAULT_MAX_FILENAMELENGTH,FileNameTranscoder.transcoder(),true);
            var fuse = builder.build(adapter);
            try {
                fuse.mount("fuse-nio-adapter",mountPoint,mountFlags.toArray(String[]::new));
                return new Mounted(fuse,adapter,mountPoint);
            } catch (FuseMountFailedException error) { throw new MountFailedException(error); }
        }
    }

    private static final class Mounted extends AbstractMount {
        private boolean unmounted;
        Mounted(Fuse fuse,FuseNioAdapter adapter,Path point) { super(fuse,adapter,point); }
        @Override public void unmount() throws UnmountFailedException {
            if (unmounted) return;
            try {
                // Native normal unmount only. A nonzero/busy result preserves
                // the live resources; no forced/lazy/Linux helper fallback.
                var child = new ProcessBuilder("/sbin/umount",mountpoint.toAbsolutePath().toString()).start();
                ProcessHelper.waitForSuccess(child,10,"FreeBSD normal unmount");
                fuse.close();
                unmounted = true;
            } catch (InterruptedException error) {
                Thread.currentThread().interrupt(); throw new UnmountFailedException(error);
            } catch (IOException | TimeoutException | ProcessHelper.CommandFailedException error) {
                throw new UnmountFailedException(error);
            }
        }
        @Override public void close() throws UnmountFailedException,IOException {
            if (!unmounted) unmount();
            super.close();
        }
    }
}
