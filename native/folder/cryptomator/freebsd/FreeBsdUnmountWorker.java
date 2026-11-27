/* LGPL-3.0-or-later. Private user-only worker; public libc ABI, no fallback.
 * This entry point is never used by the memory controls. Its parent supplies a
 * bound request on stdin only after capturing this child's PID and birth. */
package org.cryptomator.jfuse.freebsd.amd64;

import org.cryptomator.jfuse.freebsd.amd64.extr.mount.mount_h;
import org.cryptomator.jfuse.freebsd.amd64.extr.errno.errno_h;
import java.io.IOException;
import java.lang.foreign.*;
import java.lang.invoke.MethodHandle;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.Base64;
import java.util.List;

public final class FreeBsdUnmountWorker {
    static final String MAGIC="LURKER_FREEBSD_NORMAL_UNMOUNT_1";
    record Request(String token,long pid,long birth,int uid,FreeBsdMounts.Entry owned) {
        Request {
            if (!token.matches("[a-f0-9]{32}") || pid<=0 || birth<=0 || uid==0
                    || owned.owner()!=uid || !owned.type().equals("fusefs")
                    || !owned.source().matches("lurker-cryptomator-[a-f0-9]{32}")
                    || (owned.fsid0()==0 && owned.fsid1()==0)
                    || (owned.flags() & mount_h.MNT_RDONLY())!=0
                    || !Path.of(owned.target()).isAbsolute()
                    || !Path.of(owned.target()).normalize().toString().equals(owned.target())
                    || owned.target().indexOf('\0')>=0) throw new IllegalArgumentException("Invalid owned FSID request");
        }
        String encode() {
            return String.join("\t",MAGIC,token,Long.toString(pid),Long.toString(birth),
                Integer.toString(uid),Integer.toString(owned.fsid0()),Integer.toString(owned.fsid1()),
                Long.toString(owned.flags()),encoded(owned.source()),encoded(owned.target()));
        }
        static Request decode(String text) {
            String[] v=text.split("\t",-1);
            if (v.length!=10 || !v[0].equals(MAGIC)) throw new IllegalArgumentException("Invalid request inventory");
            int uid=integer(v[4]);
            return new Request(v[1],number(v[2]),number(v[3]),uid,
                new FreeBsdMounts.Entry(integer(v[5]),integer(v[6]),number(v[7]),uid,"fusefs",decoded(v[8]),decoded(v[9])));
        }
    }
    record NativeResult(int result,int error) {}
    record Reply(Request request,int result,int error,boolean invoked,String status) {
        String encode() { return request.encode()+"\t"+result+"\t"+error+"\t"+(invoked?"1":"0")+"\t"+status; }
        int exit() {
            return switch(status) { case "ok"->0;case "unconfirmed"->3;case "refused"->4;case "precheck"->6;default->5; };
        }
        boolean positive() { return status.equals("ok") && invoked && result==0 && error==0; }
        static Reply decode(String text) {
            String[] v=text.split("\t",-1);
            if (v.length!=14 || (!v[12].equals("0") && !v[12].equals("1"))
                    || !List.of("ok","unconfirmed","refused","unknown","precheck").contains(v[13]))
                throw new IllegalArgumentException("Invalid normal-unmount reply");
            return new Reply(Request.decode(String.join("\t",java.util.Arrays.copyOf(v,10))),
                integer(v[10]),integer(v[11]),v[12].equals("1"),v[13]);
        }
    }
    interface Backend {
        List<FreeBsdMounts.Entry> snapshot() throws IOException;
        NativeResult unmount(int fsid0,int fsid1) throws IOException;
    }
    private static final class NativeBackend implements Backend {
        private static final StructLayout STATE=Linker.Option.captureStateLayout();
        private static final long ERRNO=STATE.byteOffset(MemoryLayout.PathElement.groupElement("errno"));
        private static final MethodHandle UNMOUNT=Linker.nativeLinker().downcallHandle(
            Linker.nativeLinker().defaultLookup().find("unmount").orElseThrow(),
            FunctionDescriptor.of(ValueLayout.JAVA_INT,ValueLayout.ADDRESS,ValueLayout.JAVA_INT),
            Linker.Option.captureCallState("errno"));
        public List<FreeBsdMounts.Entry> snapshot() throws IOException { return FreeBsdMounts.snapshot(); }
        public NativeResult unmount(int a,int b) throws IOException {
            try (var arena=Arena.ofConfined()) {
                var state=arena.allocate(STATE);
                var fsid=arena.allocateFrom("FSID:"+a+":"+b);
                // No caller-selected flags and no pathname branch. The native
                // header probe checks this public signature and exact constant.
                int result=(int)UNMOUNT.invokeExact(state,fsid,mount_h.MNT_BYFSID());
                return new NativeResult(result,result==0?0:state.get(ValueLayout.JAVA_INT,ERRNO));
            } catch (Throwable error) { throw new IOException("Native FSID-only unmount outcome unknown",error); }
        }
    }
    static Reply execute(Request request,Backend backend) {
        try {
            // Missing NOWAIT rows are unconfirmed, never permission to dispose
            // or to select a replacement pathname. Recheck in this worker too.
            if (FreeBsdMounts.absent(request.owned(),backend.snapshot()))
                return new Reply(request,-1,0,false,"unconfirmed");
        } catch (IOException error) {
            System.err.println("Native normal-unmount precheck retained: "+error.getMessage());
            return new Reply(request,-1,0,false,"precheck");
        }
        try {
            var result=backend.unmount(request.owned().fsid0(),request.owned().fsid1());
            String status=result.result()==0?"ok":result.result()!=-1?"unknown":
                result.error()==errno_h.ENOENT()?"unconfirmed":"refused";
            return new Reply(request,result.result(),result.result()==0?0:result.error(),true,status);
        } catch (IOException error) {
            System.err.println("Native normal-unmount call retained: "+error.getMessage());
            return new Reply(request,-1,0,true,"unknown");
        }
    }
    static String line(byte[] bytes) {
        if (bytes.length>8192 || bytes.length==0 || bytes[bytes.length-1]!='\n')
            throw new IllegalArgumentException("Bounded complete protocol line required");
        for (int i=0;i<bytes.length-1;i++) if (bytes[i]<0 || bytes[i]=='\n' || bytes[i]=='\r')
            throw new IllegalArgumentException("Single ASCII protocol line required");
        return new String(bytes,0,bytes.length-1,StandardCharsets.US_ASCII);
    }
    private static long number(String value) {
        long result=Long.parseLong(value);
        if (!Long.toString(result).equals(value)) throw new IllegalArgumentException("Noncanonical integer");return result;
    }
    private static int integer(String value) { return Math.toIntExact(number(value)); }
    private static String encoded(String value) { return Base64.getUrlEncoder().withoutPadding().encodeToString(value.getBytes(StandardCharsets.UTF_8)); }
    private static String decoded(String value) {
        try {
            String result=StandardCharsets.UTF_8.newDecoder().decode(ByteBuffer.wrap(Base64.getUrlDecoder().decode(value))).toString();
            if (!encoded(result).equals(value)) throw new IllegalArgumentException("Noncanonical identity encoding");return result;
        } catch (java.nio.charset.CharacterCodingException error) { throw new IllegalArgumentException("Invalid native identity encoding",error); }
    }
    static long birth(ProcessHandle process) {
        return process.info().startInstant().orElseThrow(()->new IllegalStateException("Native process birth unproven")).toEpochMilli();
    }
    public static void main(String[] args) throws Exception {
        FreeBsdNative.requireLoginUser();
        if (args.length!=0) throw new IllegalArgumentException("Worker accepts only a bound stdin request");
        var request=Request.decode(line(System.in.readNBytes(8193)));
        var self=ProcessHandle.current();
        if (request.pid()!=self.pid() || request.birth()!=birth(self) || request.uid()!=FreeBsdNative.uid())
            throw new IllegalArgumentException("Native worker identity differs");
        Reply reply=execute(request,new NativeBackend());
        System.out.println(reply.encode());
        System.out.flush();
        System.exit(reply.exit());
    }
}
