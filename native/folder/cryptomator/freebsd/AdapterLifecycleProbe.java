/* AGPL-3.0-only. Test-only patched module, no native channel or filesystem I/O. */
package org.cryptomator.frontend.fuse.mount;

import org.cryptomator.frontend.fuse.FuseNioAdapter;
import org.cryptomator.jfuse.api.*;
import java.io.IOException;
import java.util.Set;

public final class AdapterLifecycleProbe {
    static final class Fake implements FuseNioAdapter {
        private final Errno errno;
        int closes,destroys,syncs;
        boolean fail;
        Fake(Errno errno) { this.errno=errno; }
        public Errno errno() { return errno; }
        public Set<Operation> supportedOperations() { return Set.of(Operation.DESTROY,Operation.FSYNC); }
        public boolean isInUse() { return false; }
        public void destroy() { destroys++; }
        public int fsync(String path,int datasync,FileInfo info) { syncs++;return -errno.eio(); }
        public void close() throws IOException { closes++;if (fail) throw new IOException("injected adapter close EIO"); }
    }
    private static void check(boolean ok) { if (!ok) throw new AssertionError("Adapter lifecycle control failed"); }
    public static void main(String[] args) throws Exception {
        int controls=0;
        var errors=FuseBuilder.getSupported().errno();
        var raw=new Fake(errors);var adapter=new FreeBsdDeferredAdapter(raw);
        adapter.destroy();adapter.destroy();check(adapter.destroyCallbacks()==2 && raw.closes==0 && raw.destroys==0);controls++;
        check(adapter.supportedOperations().equals(raw.supportedOperations()));controls++;
        check(adapter.fsync("/memory-only",0,null)==-errors.eio() && raw.syncs==1);controls++;
        adapter.close();adapter.close();check(raw.closes==1);controls++;
        var failed=new Fake(errors);failed.fail=true;var retained=new FreeBsdDeferredAdapter(failed);
        retained.destroy();check(failed.closes==0);controls++;
        for (int attempt=0;attempt<2;attempt++) {
            try { retained.close();throw new AssertionError("Adapter error swallowed"); }
            catch (IOException expected) { check(failed.closes==1);controls++; }
        }
        retained.destroy();check(failed.closes==1 && failed.destroys==0);controls++;
        System.out.println("{\"controls\":"+controls+",\"failures\":0,\"native_mount_or_channel_operations\":0}");
    }
}
