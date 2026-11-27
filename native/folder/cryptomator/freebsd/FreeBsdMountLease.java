/* LGPL-3.0-or-later. Never calls a native unmount/exit API. */
package org.cryptomator.jfuse.freebsd.amd64;

import org.cryptomator.jfuse.api.FuseMount;
import org.cryptomator.jfuse.freebsd.amd64.extr.fuse3.fuse_h;
import java.io.IOException;
import java.lang.foreign.MemorySegment;
import java.util.List;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;

final class FreeBsdMountLease implements FuseMount {
    interface Backend {
        List<FreeBsdMounts.Entry> snapshot() throws IOException;
        int loop();
        void destroy();
        FreeBsdUnmount.Receipt normalUnmount(FreeBsdMounts.Entry owned) throws IOException,TimeoutException;
        void requireWorkerTerminal() throws IOException;
        boolean retryableBusy(FreeBsdMounts.Entry owned);
    }
    private static final class NativeBackend implements Backend {
        private final MemorySegment fuse;
        private FreeBsdUnmount.Worker worker;
        private final java.util.List<FreeBsdUnmount.Worker> retainedWorkers=new java.util.ArrayList<>();
        NativeBackend(MemorySegment fuse) { this.fuse=fuse; }
        public List<FreeBsdMounts.Entry> snapshot() throws IOException { return FreeBsdMounts.snapshot(); }
        public int loop() { return fuse_h.fuse_loop(fuse); }
        public void destroy() { fuse_h.fuse_destroy(fuse); }
        public FreeBsdUnmount.Receipt normalUnmount(FreeBsdMounts.Entry owned) throws IOException,TimeoutException {
            // Only a later explicit close may retry a proved terminal EBUSY.
            // Its process/evidence remain retained; unknown/ENOENT are sticky.
            if (worker==null || worker.retryableBusy()) {
                worker=new FreeBsdUnmount.Worker(owned);
                retainedWorkers.add(worker);
            }
            worker.start();
            return worker.await(10_000);
        }
        public void requireWorkerTerminal() throws IOException {
            for (var retained:retainedWorkers) retained.requireTerminal();
        }
        public boolean retryableBusy(FreeBsdMounts.Entry owned) {
            return worker!=null && worker.boundTo(owned) && worker.retryableBusy();
        }
    }
    private final Backend backend;
    private final List<FreeBsdMounts.Entry> before;
    private final String source,target;
    private final long drainMilliseconds;
    private final CountDownLatch terminal=new CountDownLatch(1);
    private volatile Integer result;
    private volatile Throwable loopFailure;
    private FreeBsdMounts.Entry owned;
    private FreeBsdUnmount.Receipt cleanReceipt;
    private boolean attempted,disposalAttempted,destroyed;
    FreeBsdMountLease(MemorySegment fuse,List<FreeBsdMounts.Entry> before,String source,String target) {
        this(new NativeBackend(fuse),before,source,target);
    }
    FreeBsdMountLease(Backend backend,List<FreeBsdMounts.Entry> before,String source,String target) {
        this(backend,before,source,target,10_000);
    }
    FreeBsdMountLease(Backend backend,List<FreeBsdMounts.Entry> before,String source,String target,long drainMilliseconds) {
        if (drainMilliseconds<=0 || drainMilliseconds>10_000) throw new IllegalArgumentException("Invalid bounded close wait");
        this.backend=backend;this.before=before;this.source=source;this.target=target;this.drainMilliseconds=drainMilliseconds;
    }
    void attemptingMount() { attempted=true; }
    @Override public int loop() {
        try { int value=backend.loop(); result=value; return value; }
        catch (Throwable error) { loopFailure=error; throw error; }
        finally { terminal.countDown(); }
    }
    void captureAfterInit() throws IOException,TimeoutException {
        long until=System.nanoTime()+TimeUnit.SECONDS.toNanos(10);
        do {
            if (terminal.getCount()==0) throw new IOException("Native loop ended before mount identity capture");
            owned=FreeBsdMounts.capture(before,backend.snapshot(),source,target,FreeBsdNative.uid());
            if (owned!=null) return;
            pause();
        } while (System.nanoTime()<until);
        throw new TimeoutException("Native helper publication unproven; retaining channel and callbacks");
    }
    private static void pause() throws TimeoutException {
        try { Thread.sleep(10); }
        catch (InterruptedException error) { Thread.currentThread().interrupt(); throw new TimeoutException("Interrupted; retaining native lease"); }
    }
    private boolean absent() throws IOException {
        if (owned==null) throw new IOException("Native mounted identity was not captured; retaining channel");
        return FreeBsdMounts.absent(owned,backend.snapshot());
    }
    private void requireDrained() throws IOException {
        if (cleanReceipt==null || !cleanReceipt.matches(owned) || !absent()
                || terminal.getCount()!=0 || loopFailure!=null || result==null || result!=0)
            throw new IOException("Owned successful FSID normal-unmount receipt, identity absence and natural loop0 all required; retaining channel");
        backend.requireWorkerTerminal();
    }
    synchronized void closeNormally() throws IOException,TimeoutException {
        if (destroyed) return;
        if (cleanReceipt==null) {
            if (absent()) throw new IOException("Mount already absent/unconfirmed; no owned normal-unmount receipt; retaining lease");
            var receipt=backend.normalUnmount(owned);
            if (receipt==null || !receipt.matches(owned)) throw new IOException("Normal-unmount receipt identity differs; retaining lease");
            cleanReceipt=receipt;
        }
        backend.requireWorkerTerminal();
        long until=System.nanoTime()+TimeUnit.MILLISECONDS.toNanos(drainMilliseconds);
        while (terminal.getCount()!=0 && System.nanoTime()<until) pause();
        requireDrained();
    }
    synchronized void dispose() throws IOException {
        if (destroyed) return;
        if (disposalAttempted) throw new IOException("Prior native destruction outcome uncertain; retaining arena");
        if (attempted) requireDrained();
        disposalAttempted=true; // never repeat an ambiguous native destruction call
        backend.destroy(); // public API explicitly does not unmount
        destroyed=true;
    }
    synchronized boolean retryableBusy() {
        return attempted && owned!=null && !destroyed && !disposalAttempted && cleanReceipt==null
            && backend.retryableBusy(owned);
    }
    @Override public void unmount() {
        throw new UnsupportedOperationException("Use the identity-checked normal-close boundary");
    }
    @Override public void destroy() {
        throw new UnsupportedOperationException("Use the retained lease disposal boundary");
    }
}
