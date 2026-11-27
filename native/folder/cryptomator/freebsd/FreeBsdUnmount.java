/* LGPL-3.0-or-later. Own the actual worker and its bound completion receipt.
 * A timeout never kills or forgets a potentially blocked native syscall. */
package org.cryptomator.jfuse.freebsd.amd64;

import org.cryptomator.jfuse.freebsd.amd64.extr.errno.errno_h;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.nio.file.attribute.BasicFileAttributes;
import java.nio.file.attribute.PosixFilePermissions;
import java.util.List;
import java.util.UUID;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;

final class FreeBsdUnmount {
    record Receipt(FreeBsdUnmountWorker.Request request) {
        boolean matches(FreeBsdMounts.Entry owned) {
            return request.owned().equals(owned) && request.uid()==FreeBsdNative.uid()
                && request.pid()>0 && request.birth()>0;
        }
    }
    static Receipt verify(FreeBsdUnmountWorker.Request request,FreeBsdUnmountWorker.Reply reply,int exit) throws IOException {
        if (!reply.request().equals(request) || reply.exit()!=exit)
            throw new IOException("Native normal-unmount reply identity or exit differs; retaining lease");
        if (!reply.positive() || exit!=0)
            throw new IOException("Native normal-unmount "+reply.status()+" result="+reply.result()+" errno="+reply.error()+"; retaining lease");
        return new Receipt(request);
    }
    static final class Worker {
        private final FreeBsdMounts.Entry owned;
        private final List<String> testCommand;
        private final Path directory,stdout,stderr;
        private final Object stdoutKey;
        private boolean started,dispatched;
        private Process child;
        private FreeBsdUnmountWorker.Request request;
        private FreeBsdUnmountWorker.Reply reply;
        private Integer exit;
        private boolean replyBound;
        private IOException replyFailure;
        Worker(FreeBsdMounts.Entry owned) throws IOException { this(owned,null); }
        // The injected command is package-private, for test-only patched-module
        // fixtures. Production has no test mode or caller-selected worker argv.
        Worker(FreeBsdMounts.Entry owned,List<String> testCommand) throws IOException {
            this.owned=owned;this.testCommand=testCommand==null?null:List.copyOf(testCommand);
            directory=Files.createTempDirectory("lurker-freebsd-close-",PosixFilePermissions.asFileAttribute(PosixFilePermissions.fromString("rwx------")));
            stdout=directory.resolve("stdout.txt");stderr=directory.resolve("stderr.txt");
            Files.createFile(stdout,PosixFilePermissions.asFileAttribute(PosixFilePermissions.fromString("rw-------")));
            Files.createFile(stderr,PosixFilePermissions.asFileAttribute(PosixFilePermissions.fromString("rw-------")));
            stdoutKey=Files.readAttributes(stdout,BasicFileAttributes.class,LinkOption.NOFOLLOW_LINKS).fileKey();
            if (stdoutKey==null) throw new IOException("Worker result inode unproven; retained at "+directory);
        }
        static List<String> command() throws IOException {
            try {
                Path module=Path.of(FreeBsdUnmountWorker.class.getProtectionDomain().getCodeSource().getLocation().toURI()).toRealPath();
                Path java=Path.of(System.getProperty("java.home"),"bin","java").toRealPath();
                if (!Files.isRegularFile(module) || !module.getFileName().toString().endsWith(".jar")
                        || !Files.isRegularFile(java) || !Files.isExecutable(java))
                    throw new IOException("Private JRE/module worker inputs unproven");
                return List.of(java.toString(),"-Xms8m","-Xmx32m",
                    "--enable-native-access=org.cryptomator.jfuse.freebsd.amd64",
                    "--module-path",module.getParent().toString(),"-m",
                    "org.cryptomator.jfuse.freebsd.amd64/org.cryptomator.jfuse.freebsd.amd64.FreeBsdUnmountWorker");
            } catch (java.net.URISyntaxException error) { throw new IOException("Private worker source location invalid",error); }
        }
        void start() throws IOException {
            if (started) {
                if (!dispatched) throw new IOException("Earlier worker request dispatch unproven; retained at "+directory);
                return;
            }
            started=true;
            var argv=testCommand==null?command():testCommand;
            var builder=new ProcessBuilder(argv).redirectOutput(stdout.toFile()).redirectError(stderr.toFile());
            builder.environment().clear();builder.environment().put("LC_ALL","C");
            child=builder.start();
            // No native action is dispatched until this birth-bound stdin
            // request is complete. An unsupported identity query retains the
            // blocked child, its evidence and caller resources.
            long birth;
            try { birth=FreeBsdUnmountWorker.birth(child.toHandle()); }
            catch (RuntimeException error) { throw new IOException("Worker birth unproven; retained pid="+child.pid()+" at "+directory,error); }
            request=new FreeBsdUnmountWorker.Request(UUID.randomUUID().toString().replace("-",""),
                child.pid(),birth,FreeBsdNative.uid(),owned);
            String wire=request.encode()+"\n";
            Files.writeString(directory.resolve("request.txt"),wire,StandardCharsets.US_ASCII,
                StandardOpenOption.CREATE_NEW,StandardOpenOption.WRITE);
            Files.setPosixFilePermissions(directory.resolve("request.txt"),PosixFilePermissions.fromString("rw-------"));
            Files.writeString(directory.resolve("launch.txt"),String.join("\n",argv)+"\npid="+child.pid()+"\nbirth="+birth+"\n",
                StandardCharsets.UTF_8,StandardOpenOption.CREATE_NEW,StandardOpenOption.WRITE);
            Files.setPosixFilePermissions(directory.resolve("launch.txt"),PosixFilePermissions.fromString("rw-------"));
            try (var stdin=child.getOutputStream()) { stdin.write(wire.getBytes(StandardCharsets.US_ASCII)); }
            dispatched=true;
        }
        Receipt await(long milliseconds) throws IOException,TimeoutException {
            if (child==null || request==null || !dispatched) throw new IOException("Native close worker not identity-bound/dispatched; retaining lease at "+directory);
            try {
                if (!child.waitFor(milliseconds,TimeUnit.MILLISECONDS))
                    throw new TimeoutException("Native FSID unmount worker pid="+child.pid()+" remains live; retaining lease and "+directory);
            } catch (InterruptedException error) {
                Thread.currentThread().interrupt();throw new IOException("Native close wait interrupted; retaining worker and "+directory,error);
            }
            readTerminalReply();
            var receipt=verify(request,reply,exit);
            System.err.println("Native FSID normal-unmount receipt pid="+child.pid()+" retained at "+directory);
            return receipt;
        }
        private synchronized void readTerminalReply() throws IOException {
            if (replyFailure!=null) throw replyFailure;
            if (replyBound) return;
            if (child==null || child.isAlive() || request==null || !dispatched)
                throw new IOException("Native close worker not conclusively terminal");
            exit=child.exitValue();
            try {
                var attributes=Files.readAttributes(stdout,BasicFileAttributes.class,LinkOption.NOFOLLOW_LINKS);
                if (!attributes.isRegularFile() || !stdoutKey.equals(attributes.fileKey()) || attributes.size()>8192
                        || (int)Files.getAttribute(stdout,"unix:uid",LinkOption.NOFOLLOW_LINKS)!=FreeBsdNative.uid()
                        || (int)Files.getAttribute(stdout,"unix:nlink",LinkOption.NOFOLLOW_LINKS)!=1
                        || ((int)Files.getAttribute(stdout,"unix:mode",LinkOption.NOFOLLOW_LINKS) & 0077)!=0)
                    throw new IOException("Native worker result file identity differs");
                byte[] bytes;
                try (var stream=Files.newInputStream(stdout,LinkOption.NOFOLLOW_LINKS)) { bytes=stream.readNBytes(8193); }
                reply=FreeBsdUnmountWorker.Reply.decode(FreeBsdUnmountWorker.line(bytes));
                if (!reply.request().equals(request) || reply.exit()!=exit)
                    throw new IOException("Native worker reply identity/exit differs");
                replyBound=true;
            } catch (IllegalArgumentException error) {
                replyFailure=new IOException("Native close reply malformed; retaining worker, lease and "+directory,error);
                throw replyFailure;
            } catch (IOException error) {
                replyFailure=error;throw error;
            }
        }
        boolean retryableBusy() {
            // May validate a late terminal EBUSY after an earlier timeout.
            // This performs no new process launch, snapshot or native syscall.
            if (child==null || child.isAlive()) return false;
            try { readTerminalReply(); } catch (IOException error) { return false; }
            return replyBound && reply!=null && request!=null
                && reply.status().equals("refused") && reply.invoked()
                && reply.result()==-1 && reply.error()==errno_h.EBUSY();
        }
        boolean boundTo(FreeBsdMounts.Entry entry) { return request!=null && request.owned().equals(entry); }
        void requireTerminal() throws IOException {
            if (child==null || child.isAlive()) throw new IOException("Native close worker unstarted/live; retaining lease and "+directory);
        }
        Process processForControl() { return child; }
        Path evidenceForControl() { return directory; }
    }
}
