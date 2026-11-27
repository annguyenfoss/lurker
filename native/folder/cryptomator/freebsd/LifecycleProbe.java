/* LGPL-3.0-or-later. Memory-only close-policy controls. All native channel,
 * unmount and disposal functions are fake counters; no namespace is changed. */
package org.cryptomator.jfuse.freebsd.amd64;

import java.io.IOException;
import java.util.List;
import java.util.concurrent.TimeoutException;

public final class LifecycleProbe {
    private static int controls;
    static final String SOURCE="lurker-cryptomator-00000000000000000000000000000000";
    static final String TARGET="/memory-only-owned-target";
    static FreeBsdMounts.Entry entry(int a,int b,String source,String target) {
        return new FreeBsdMounts.Entry(a,b,0L,FreeBsdNative.uid(),"fusefs",source,target);
    }
    static FreeBsdUnmountWorker.Request request(FreeBsdMounts.Entry owned) {
        var self=ProcessHandle.current();
        return new FreeBsdUnmountWorker.Request("0123456789abcdef0123456789abcdef",
            self.pid(),FreeBsdUnmountWorker.birth(self),FreeBsdNative.uid(),owned);
    }
    private static final class Fake implements FreeBsdMountLease.Backend {
        List<FreeBsdMounts.Entry> rows=List.of(entry(123,456,SOURCE,TARGET)),afterUnmount=List.of();
        int exit,destroys,unmounts;
        boolean busy,workerLive,destroyFailure,loopFailure,wrongReceipt;
        public List<FreeBsdMounts.Entry> snapshot() { return rows; }
        public int loop() { if (loopFailure) throw new IllegalStateException("injected loop failure");return exit; }
        public void destroy() { destroys++;if (destroyFailure) throw new IllegalStateException("injected destruction ambiguity"); }
        public FreeBsdUnmount.Receipt normalUnmount(FreeBsdMounts.Entry owned) throws IOException {
            unmounts++;
            if (busy) throw new IOException("injected EBUSY");
            rows=afterUnmount;
            return new FreeBsdUnmount.Receipt(request(wrongReceipt?entry(789,111,SOURCE,TARGET):owned));
        }
        public void requireWorkerTerminal() throws IOException {
            if (workerLive) throw new IOException("injected live normal-unmount worker");
        }
        public boolean retryableBusy(FreeBsdMounts.Entry owned) { return busy && !workerLive; }
    }
    @FunctionalInterface interface Checked { void run() throws Exception; }
    static void refused(Checked action) throws Exception {
        try { action.run();throw new AssertionError("unsafe close accepted"); }
        catch (IOException | TimeoutException expected) { controls++; }
    }
    static void check(boolean value) { if (!value) throw new AssertionError("lifecycle control failed"); }
    private static FreeBsdMountLease bound(Fake fake) throws Exception {
        var lease=new FreeBsdMountLease(fake,List.of(),SOURCE,TARGET,20);
        lease.attemptingMount();lease.captureAfterInit();return lease;
    }
    public static void main(String[] args) throws Exception {
        FreeBsdNative.requireLoginUser();
        if (args.length!=4) throw new IllegalArgumentException("Native control paths and diagnostic log required");
        System.setErr(new java.io.PrintStream(java.nio.file.Files.newOutputStream(java.nio.file.Path.of(args[3]),
            java.nio.file.StandardOpenOption.CREATE_NEW,java.nio.file.StandardOpenOption.WRITE),true,java.nio.charset.StandardCharsets.UTF_8));
        var partial=new Fake();partial.rows=List.of();
        var unknown=new FreeBsdMountLease(partial,List.of(),SOURCE,TARGET,20);
        unknown.attemptingMount();refused(unknown::dispose);check(partial.destroys==0);
        // NOWAIT omission plus a natural zero loop is expressly NOT authority.
        var omitted=new Fake();var omittedLease=bound(omitted);omittedLease.loop();omitted.rows=List.of();
        refused(omittedLease::closeNormally);refused(omittedLease::dispose);
        check(omitted.destroys==0 && omitted.unmounts==0);
        // An authoritative fake normal-unmount receipt without loop completion
        // still must not dispose. The small test-only wait keeps this bounded.
        var live=new Fake();var liveLease=bound(live);
        refused(liveLease::closeNormally);refused(liveLease::dispose);check(live.destroys==0 && live.unmounts==1);
        var failed=new Fake();var failedLease=bound(failed);failed.exit=5;failedLease.loop();
        refused(failedLease::closeNormally);check(failed.destroys==0 && failed.unmounts==1);
        var exceptional=new Fake();var exceptionalLease=bound(exceptional);exceptional.loopFailure=true;
        try { exceptionalLease.loop();throw new AssertionError(); } catch (IllegalStateException expected) {}
        refused(exceptionalLease::closeNormally);check(exceptional.destroys==0);
        // Even after our receipt, published/moved/replaced identities retain.
        for (var changed:List.of(entry(123,456,SOURCE,TARGET),entry(123,456,SOURCE,"/moved"),
                entry(789,111,"replacement",TARGET),entry(789,111,SOURCE,"/elsewhere"))) {
            var fake=new Fake();var lease=bound(fake);lease.loop();fake.afterUnmount=List.of(changed);
            refused(lease::closeNormally);refused(lease::dispose);check(fake.destroys==0 && fake.unmounts==1);
        }
        var busy=new Fake();var busyLease=bound(busy);busy.busy=true;
        refused(busyLease::closeNormally);check(busy.unmounts==1 && busy.destroys==0);
        check(busyLease.retryableBusy());
        // A later explicit close may succeed after a known busy refusal.
        busy.busy=false;busyLease.loop();busyLease.closeNormally();busyLease.dispose();
        check(busy.unmounts==2 && busy.destroys==1);controls++;
        check(!busyLease.retryableBusy());
        var worker=new Fake();var workerLease=bound(worker);workerLease.loop();worker.workerLive=true;
        refused(workerLease::closeNormally);refused(workerLease::dispose);check(worker.destroys==0);
        var wrong=new Fake();var wrongLease=bound(wrong);wrongLease.loop();wrong.wrongReceipt=true;
        refused(wrongLease::closeNormally);refused(wrongLease::dispose);check(wrong.destroys==0);
        var clean=new Fake();var cleanLease=bound(clean);cleanLease.loop();
        cleanLease.closeNormally();cleanLease.dispose();cleanLease.dispose();
        check(clean.destroys==1 && clean.unmounts==1);controls++;
        var uncertain=new Fake();var uncertainLease=bound(uncertain);uncertainLease.loop();uncertainLease.closeNormally();
        uncertain.destroyFailure=true;
        try { uncertainLease.dispose();throw new AssertionError(); } catch (IllegalStateException expected) {}
        refused(uncertainLease::dispose);check(uncertain.destroys==1);
        var duplicate=new Fake();var duplicateLease=bound(duplicate);duplicateLease.loop();
        duplicate.afterUnmount=List.of(duplicate.rows.getFirst(),entry(789,111,SOURCE,"/elsewhere"));
        refused(duplicateLease::closeNormally);check(duplicate.destroys==0);
        int protocolControls=WorkerProtocolProbe.run(java.util.Arrays.copyOf(args,3));
        System.out.println("{\"controls\":"+controls+",\"worker_protocol_controls\":"+protocolControls+
            ",\"failures\":0,\"native_mount_or_channel_operations\":0}");
    }
}
