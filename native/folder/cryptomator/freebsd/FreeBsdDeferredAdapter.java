/* AGPL-3.0-only. Defer upstream destructive cleanup to the proved close owner.
 * All data/permission/fsync operations remain ordinary upstream delegation. */
package org.cryptomator.frontend.fuse.mount;

import org.cryptomator.frontend.fuse.FuseNioAdapter;
import org.cryptomator.jfuse.api.FuseOperations;
import org.cryptomator.jfuse.api.FuseOperationsDecorator;
import java.io.IOException;
import java.util.concurrent.atomic.AtomicInteger;

final class FreeBsdDeferredAdapter implements FuseNioAdapter,FuseOperationsDecorator {
    private final FuseNioAdapter delegate;
    private final AtomicInteger destroys=new AtomicInteger();
    private boolean attempted,closed;
    private Throwable failure;
    FreeBsdDeferredAdapter(FuseNioAdapter delegate) { this.delegate=java.util.Objects.requireNonNull(delegate); }
    public FuseOperations delegate() { return delegate; }
    public boolean isInUse() { return delegate.isInUse(); }
    /** DESTROY is a void protocol notification. Upstream closes the adapter and
     * swallows its errors here, before the parent has its normal-unmount0
     * receipt. Record it; the builder performs actual close once after proof. */
    @Override public void destroy() { destroys.incrementAndGet(); }
    int destroyCallbacks() { return destroys.get(); }
    @Override public synchronized void close() throws IOException {
        if (closed) return;
        if (attempted) throw new IOException("Prior adapter disposal uncertain; no retry",failure);
        attempted=true;
        try { delegate.close();closed=true; }
        catch (Throwable error) { failure=error;throw new IOException("Adapter disposal failed; retain caller resources",error); }
    }
}
