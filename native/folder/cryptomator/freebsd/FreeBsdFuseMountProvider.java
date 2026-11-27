/* Native FreeBSD MountService for FUSE-NIO 5.1.0, AGPL-3.0-only. */
package org.cryptomator.frontend.fuse.mount;

import org.cryptomator.frontend.fuse.FileNameTranscoder;
import org.cryptomator.frontend.fuse.FuseNioAdapter;
import org.cryptomator.frontend.fuse.ReadWriteAdapter;
import org.cryptomator.integrations.mount.*;
import org.cryptomator.jfuse.api.Fuse;
import org.cryptomator.jfuse.api.FuseMountFailedException;
import org.cryptomator.jfuse.freebsd.amd64.FreeBsdNative;
import org.cryptomator.jfuse.freebsd.amd64.FreeBsdMountControl;

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

    private static final class Builder extends AbstractMountBuilder implements AutoCloseable,java.util.function.BooleanSupplier {
        private Fuse ownedFuse;
        private FuseNioAdapter ownedAdapter;
        private Mounted ownedMount;
        private boolean buildStarted,adapterCloseAttempted,closed;
        private Throwable adapterCloseFailure;
        Builder(Path root) { super(root); }
        @Override public MountBuilder setMountpoint(Path point) {
            if (!Files.isDirectory(point,LinkOption.NOFOLLOW_LINKS)) throw new IllegalArgumentException("Existing native directory required");
            return super.setMountpoint(point);
        }
        @Override public synchronized Mount mount() throws MountFailedException {
            if (buildStarted || closed) throw new IllegalStateException("Native builder attempt cannot be reused");
            buildStarted = true;
            FreeBsdNative.requireLoginUser();
            Objects.requireNonNull(mountPoint); Objects.requireNonNull(mountFlags);
            if (mountFlags.stream().anyMatch(f -> f.contains("allow_other") || f.contains("allow_root")
                    || f.contains("auto_unmount") || f.contains("kernel_cache") || f.contains("writeback_cache"))) {
                throw new IllegalArgumentException("Private direct-I/O mount profile required");
            }
            var builder = Fuse.builder();
            try { builder.setLibraryPath(library().toString()); }
            catch (IOException error) { throw new MountFailedException(error); }
            ownedAdapter = new FreeBsdDeferredAdapter(ReadWriteAdapter.create(builder.errno(),vfsRoot,
                    FuseNioAdapter.DEFAULT_MAX_FILENAMELENGTH,FileNameTranscoder.transcoder(),true));
            ownedFuse = builder.build(ownedAdapter);
            // Main keeps this standard AutoCloseable builder even if mount()
            // throws after native publication. No global retained-map lookup.
            ownedMount = new Mounted(this,ownedFuse,ownedAdapter,mountPoint);
            try {
                ownedFuse.mount("lurker-cryptomator-"+java.util.UUID.randomUUID().toString().replace("-",""),
                    mountPoint,mountFlags.toArray(String[]::new));
                return ownedMount;
            } catch (FuseMountFailedException error) { throw new MountFailedException(error); }
        }
        @Override public synchronized void close() throws UnmountFailedException,IOException {
            if (closed) return;
            if (ownedFuse!=null) {
                try { ((FreeBsdMountControl)ownedFuse).closeNormally(); }
                catch (TimeoutException error) { throw new UnmountFailedException(error); }
            }
            if (ownedAdapter!=null) {
                if (adapterCloseAttempted) throw new IOException("Prior adapter close outcome uncertain; retaining caller resources",adapterCloseFailure);
                adapterCloseAttempted=true;
                try { ownedAdapter.close(); }
                catch (Throwable error) {
                    adapterCloseFailure=error;
                    throw new IOException("Adapter close failed; retain caller and CryptoFS without retry",error);
                }
            }
            closed=true;
        }
        /** Private managed-caller contract: only a verified terminal EBUSY can
         * permit a new explicit close. This getter never launches a worker. */
        @Override public synchronized boolean getAsBoolean() {
            return !closed && !adapterCloseAttempted && ownedFuse instanceof java.util.function.BooleanSupplier safe
                && safe.getAsBoolean();
        }
    }

    private static final class Mounted extends AbstractMount {
        private final Builder owner;
        Mounted(Builder owner,Fuse fuse,FuseNioAdapter adapter,Path point) { super(fuse,adapter,point);this.owner=owner; }
        @Override public synchronized void unmount() throws UnmountFailedException {
            try { owner.close(); }
            catch (IOException error) { throw new UnmountFailedException(error); }
        }
        @Override public synchronized void close() throws UnmountFailedException,IOException {
            owner.close();
        }
    }
}
