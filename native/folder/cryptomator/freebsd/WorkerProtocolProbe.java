/* LGPL-3.0-or-later. Test-only patched module; never shipped as an executable
 * worker. Child fixtures use fake backends and perform no native unmount. */
package org.cryptomator.jfuse.freebsd.amd64;

import org.cryptomator.jfuse.freebsd.amd64.extr.errno.errno_h;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.util.List;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;

public final class WorkerProtocolProbe {
    private static List<String> command(String[] paths,String mode) {
        return List.of(paths[0],"-Xms8m","-Xmx32m",
            "--enable-native-access=org.cryptomator.jfuse.freebsd.amd64",
            "--module-path",paths[1],"--patch-module","org.cryptomator.jfuse.freebsd.amd64="+paths[2],
            "-m","org.cryptomator.jfuse.freebsd.amd64/org.cryptomator.jfuse.freebsd.amd64.WorkerProtocolProbe",mode);
    }
    static int run(String[] paths) throws Exception {
        if (paths.length!=3) throw new IllegalArgumentException("Exact native java/mods/test-classes paths required");
        var owned=LifecycleProbe.entry(123,456,LifecycleProbe.SOURCE,LifecycleProbe.TARGET);
        int controls=0;
        var actualCommand=FreeBsdUnmount.Worker.command();
        LifecycleProbe.check(Path.of(actualCommand.getFirst()).toRealPath().equals(Path.of(paths[0]).toRealPath()));
        Files.writeString(Path.of(paths[2]).getParent().resolve("production-worker-command.txt"),
            String.join("\n",actualCommand)+"\n",StandardCharsets.UTF_8);
        controls++;System.err.println("worker-control actual private-JRE/module command/no dispatch PASS");
        var success=new FreeBsdUnmount.Worker(owned,command(paths,"ok"));success.start();
        var proof=success.await(5000);
        LifecycleProbe.check(proof.matches(owned) && proof.request().pid()==success.processForControl().pid());
        success.requireTerminal();controls++;System.err.println("worker-control owned-PID/birth normal0 receipt PASS");
        for (String mode:List.of("wrongpid","wrongbirth","wronguid","wrongnonce","wrongfsid",
                "wrongsource","wrongtarget","badexit","missingack","repeatedack","malformed",
                "busy","enoent","omission","eperm","eio","unknown")) {
            var worker=new FreeBsdUnmount.Worker(owned,command(paths,mode));worker.start();
            try { worker.await(5000);throw new AssertionError("Untrusted worker accepted: "+mode); }
            catch (IOException expected) {
                worker.requireTerminal();
                LifecycleProbe.check(worker.retryableBusy()==mode.equals("busy"));
                controls++;System.err.println("worker-control "+mode+" retained PASS "+worker.evidenceForControl());
            }
        }
        var late=new FreeBsdUnmount.Worker(owned,command(paths,"latebusy"));late.start();
        try { late.await(50);throw new AssertionError("Late worker accepted early"); }
        catch (TimeoutException expected) { LifecycleProbe.check(!late.retryableBusy()); }
        long latePid=late.processForControl().pid();
        if (!late.processForControl().waitFor(5,TimeUnit.SECONDS)) throw new AssertionError("Late fake fixture failed to finish");
        LifecycleProbe.check(late.retryableBusy() && late.retryableBusy() && late.processForControl().pid()==latePid);
        late.requireTerminal();controls++;
        System.err.println("worker-control late verified EBUSY getter/no launch PASS "+late.evidenceForControl());
        var timeout=new FreeBsdUnmount.Worker(owned,command(paths,"wait"));timeout.start();
        try { timeout.await(50);throw new AssertionError("Live worker accepted"); }
        catch (TimeoutException expected) {
            LifecycleProbe.check(timeout.processForControl().isAlive());
            try { timeout.requireTerminal();throw new AssertionError("Live worker forgotten"); }
            catch (IOException retained) { controls++; }
            Files.writeString(timeout.evidenceForControl().resolve("test-retained-worker.txt"),
                "retained_before_test_cleanup=true\npid="+timeout.processForControl().pid()+"\n",StandardCharsets.US_ASCII);
            System.err.println("worker-control live timeout retained PASS; stopping only the test fake fixture pid="+timeout.processForControl().pid());
        } finally {
            // This child has no native backend or filesystem operation. Assert
            // retention first, then terminate only this owned fake test fixture.
            timeout.processForControl().destroy();
            if (!timeout.processForControl().waitFor(2,TimeUnit.SECONDS)) {
                timeout.processForControl().destroyForcibly();
                if (!timeout.processForControl().waitFor(2,TimeUnit.SECONDS)) throw new AssertionError("Fake fixture cleanup unproven");
            }
            Files.writeString(timeout.evidenceForControl().resolve("test-cleanup.txt"),
                "fake_fixture_terminal=true\nexit="+timeout.processForControl().exitValue()+"\n",StandardCharsets.US_ASCII);
        }
        return controls;
    }
    private static FreeBsdUnmountWorker.Request changed(FreeBsdUnmountWorker.Request request,String mode) {
        var e=request.owned();
        int uid=request.uid()+(mode.equals("wronguid")?1:0);
        return new FreeBsdUnmountWorker.Request(mode.equals("wrongnonce")?"ffffffffffffffffffffffffffffffff":request.token(),
            request.pid()+(mode.equals("wrongpid")?1:0),request.birth()+(mode.equals("wrongbirth")?1:0),uid,
            new FreeBsdMounts.Entry(e.fsid0()+(mode.equals("wrongfsid")?1:0),e.fsid1(),e.flags(),uid,e.type(),
                mode.equals("wrongsource")?"lurker-cryptomator-ffffffffffffffffffffffffffffffff":e.source(),
                mode.equals("wrongtarget")?e.target()+"/replacement":e.target()));
    }
    public static void main(String[] args) throws Exception {
        FreeBsdNative.requireLoginUser();
        String mode=args[0];
        var request=FreeBsdUnmountWorker.Request.decode(FreeBsdUnmountWorker.line(System.in.readNBytes(8193)));
        if (request.pid()!=ProcessHandle.current().pid() || request.birth()!=FreeBsdUnmountWorker.birth(ProcessHandle.current())
                || request.uid()!=FreeBsdNative.uid()) throw new AssertionError("Test fixture identity differs");
        if (mode.equals("wait")) { new java.util.concurrent.CountDownLatch(1).await();return; }
        if (mode.equals("latebusy")) Thread.sleep(300);
        if (mode.equals("missingack")) return;
        if (mode.equals("malformed")) { System.out.println("not a native receipt");return; }
        final int[] calls={0};
        var fake=new FreeBsdUnmountWorker.Backend() {
            public List<FreeBsdMounts.Entry> snapshot() { return mode.equals("omission")?List.of():List.of(request.owned()); }
            public FreeBsdUnmountWorker.NativeResult unmount(int a,int b) {
                calls[0]++;
                if (a!=request.owned().fsid0() || b!=request.owned().fsid1()) throw new AssertionError("Wrong FSID selected");
                int error=switch(mode) {case "busy","latebusy"->errno_h.EBUSY();case "enoent"->errno_h.ENOENT();
                    case "eperm"->errno_h.EPERM();case "eio"->errno_h.EIO();default->0;};
                return new FreeBsdUnmountWorker.NativeResult(mode.equals("unknown")?7:error==0?0:-1,error);
            }
        };
        var reply=FreeBsdUnmountWorker.execute(request,fake);
        if (mode.startsWith("wrong")) reply=new FreeBsdUnmountWorker.Reply(changed(request,mode),reply.result(),reply.error(),reply.invoked(),reply.status());
        if (mode.equals("omission") && calls[0]!=0) throw new AssertionError("Missing row triggered a fake native call");
        System.err.println("test-only worker fixture "+mode+" fake_native_calls="+calls[0]+" actual_native_unmounts=0");
        System.out.println(reply.encode());
        if (mode.equals("repeatedack")) System.out.println(reply.encode());
        System.out.flush();
        System.exit(mode.equals("badexit")?4:reply.exit());
    }
}
