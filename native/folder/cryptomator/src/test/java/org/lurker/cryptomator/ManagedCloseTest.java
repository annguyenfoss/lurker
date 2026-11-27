// SPDX-License-Identifier: AGPL-3.0-only
package org.lurker.cryptomator;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.Timeout;
import org.junit.jupiter.api.io.TempDir;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStreamReader;
import java.net.StandardProtocolFamily;
import java.net.SocketTimeoutException;
import java.net.UnixDomainSocketAddress;
import java.nio.ByteBuffer;
import java.nio.channels.SelectionKey;
import java.nio.channels.Selector;
import java.nio.channels.ServerSocketChannel;
import java.nio.channels.SocketChannel;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.Path;
import java.nio.file.attribute.PosixFilePermission;
import java.security.SecureRandom;
import java.time.Duration;
import java.util.ArrayList;
import java.util.HexFormat;
import java.util.List;
import java.util.Set;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.*;

@Timeout(20)
class ManagedCloseTest {
    @TempDir Path temporary;
    private static final SecureRandom RANDOM = new SecureRandom();

    private static String randomHex(int bytes) {
        byte[] value = new byte[bytes]; RANDOM.nextBytes(value);
        return HexFormat.of().formatHex(value);
    }

    private static Path directory(Path parent, String name) throws IOException {
        Path path = Files.createDirectory(parent.resolve(name));
        Files.setPosixFilePermissions(path, Set.of(PosixFilePermission.OWNER_READ,
                PosixFilePermission.OWNER_WRITE, PosixFilePermission.OWNER_EXECUTE));
        return path;
    }

    private static ManagedClose open(Path path, String token) throws IOException {
        return ManagedClose.open(path, token, Files.getOwner(path, LinkOption.NOFOLLOW_LINKS));
    }

    private static String frame(String op, String token, String request) {
        return "LFMC1 " + op + " " + token + " " + request + "\n";
    }

    private static SocketChannel connect(Path endpoint) throws IOException {
        SocketChannel channel = SocketChannel.open(StandardProtocolFamily.UNIX);
        channel.connect(UnixDomainSocketAddress.of(endpoint));
        channel.configureBlocking(false);
        return channel;
    }

    private static String readReply(SocketChannel channel) throws IOException {
        ByteBuffer input = ByteBuffer.allocate(256);
        long until = System.nanoTime() + TimeUnit.SECONDS.toNanos(4);
        boolean eof = false;
        try (Selector selector = Selector.open()) {
            channel.register(selector, SelectionKey.OP_READ);
            while (System.nanoTime() < until) {
                int count = channel.read(input);
                if (count < 0) { eof = true; break; }
                for (int i = 0; i < input.position(); i++) if (input.get(i) == '\n')
                    return new String(input.array(), 0, input.position(), StandardCharsets.US_ASCII);
                if (!input.hasRemaining()) throw new IOException("Unbounded control reply");
                selector.select(20); selector.selectedKeys().clear();
            }
        }
        if (!eof) throw new SocketTimeoutException("Control reply deadline exceeded");
        if (input.position() == 0) return "";
        throw new AssertionError("Unexpected partial control reply");
    }

    private static String exchange(Path endpoint, byte[] request) throws IOException {
        try (SocketChannel channel = connect(endpoint); Selector selector = Selector.open()) {
            channel.register(selector, SelectionKey.OP_WRITE);
            ByteBuffer bytes = ByteBuffer.wrap(request);
            long until = System.nanoTime() + TimeUnit.SECONDS.toNanos(4);
            while (bytes.hasRemaining()) {
                if (System.nanoTime() >= until) throw new SocketTimeoutException("Control write timed out");
                channel.write(bytes);
                selector.select(10); selector.selectedKeys().clear();
            }
            return readReply(channel);
        }
    }

    private static String rejected(Path endpoint, byte[] request) throws IOException {
        try { return exchange(endpoint, request); }
        catch (SocketTimeoutException error) { throw error; }
        catch (IOException resetOrRefusal) { return ""; }
    }

    private static ManagedClose.State rpc(ManagedClose service, String token, String op, String request) throws IOException {
        String reply = exchange(service.socketPath(), frame(op, token, request).getBytes(StandardCharsets.US_ASCII));
        assertFalse(reply.contains(token), "The session token must never appear in replies");
        String[] fields = reply.strip().split(" ", -1);
        assertEquals(4, fields.length);
        assertEquals("LFMC1", fields[0]);
        assertEquals(request, fields[2]);
        assertEquals(Long.toString(ProcessHandle.current().pid()), fields[3]);
        assertTrue(reply.endsWith("\n"));
        return ManagedClose.State.valueOf(fields[1].toUpperCase(java.util.Locale.ROOT));
    }

    private static void state(ManagedClose service, ManagedClose.State expected) throws InterruptedException {
        long until = System.nanoTime() + TimeUnit.SECONDS.toNanos(4);
        while (service.state() != expected && System.nanoTime() < until) Thread.sleep(2);
        assertEquals(expected, service.state());
    }

    private static void finish(ManagedClose service, String token) throws Exception {
        long until = System.nanoTime() + TimeUnit.SECONDS.toNanos(4);
        while (rpc(service, token, "status", randomHex(16)) != ManagedClose.State.CLOSED) {
            if (System.nanoTime() >= until) fail("Close did not finish");
            Thread.sleep(2);
        }
        assertTrue(service.awaitClosed(2, TimeUnit.SECONDS));
        service.close();
    }

    @Test void privateDirectoryOwnerTokenAndExistingEntryAreCheckedWithoutUnlink() throws Exception {
        Path path = directory(temporary, "private"); String token = randomHex(32);
        Files.setPosixFilePermissions(path, Set.of(PosixFilePermission.OWNER_READ, PosixFilePermission.OWNER_WRITE,
                PosixFilePermission.OWNER_EXECUTE, PosixFilePermission.GROUP_READ));
        assertThrows(IOException.class, () -> open(path, token));
        Files.setPosixFilePermissions(path, Set.of(PosixFilePermission.OWNER_READ, PosixFilePermission.OWNER_WRITE, PosixFilePermission.OWNER_EXECUTE));
        assertThrows(IOException.class, () -> ManagedClose.open(path, token, () -> "different-owner"));
        assertThrows(IOException.class, () -> open(path, "not-a-bootstrap-token"));
        Path alias = temporary.resolve("alias"); Files.createSymbolicLink(alias, path);
        assertThrows(IOException.class, () -> open(alias, token));
        Path stale = path.resolve(ManagedClose.SOCKET_NAME); Files.writeString(stale, "preserve-this-entry");
        assertThrows(IOException.class, () -> open(path, token));
        assertEquals("preserve-this-entry", Files.readString(stale));
    }

    @Test void realNativePeerReconnectAndMalformedPartialWrongTokenFrames() throws Exception {
        Path path = directory(temporary, "wire"); String token = randomHex(32);
        ManagedClose service = open(path, token);
        Path endpoint = service.socketPath(); Object inode = Files.readAttributes(endpoint, "unix:dev,ino");
        AtomicInteger actions = new AtomicInteger();
        service.setResources(actions::incrementAndGet, () -> { }); service.markMounted(() -> fail("Wrong closer"));
        try {
            assertEquals(Set.of(PosixFilePermission.OWNER_READ, PosixFilePermission.OWNER_WRITE), Files.getPosixFilePermissions(endpoint));
            for (int i = 0; i < 3; i++) assertEquals(ManagedClose.State.MOUNTED, rpc(service, token, "status", randomHex(16)));
            String request = randomHex(16);
            assertEquals("", rejected(endpoint, frame("close", randomHex(32), request).getBytes(StandardCharsets.US_ASCII)));
            for (String malformed : new String[] {
                    "LFMC2 close " + token + " " + request + "\n",
                    "LFMC1  close " + token + " " + request + "\n",
                    "LFMC1 close " + token + " " + request + " extra\n",
                    "LFMC1 close " + token + " " + request + "\r\n",
                    "LFMC1 close " + token + " not-a-request\n",
                    "X".repeat(ManagedClose.MAX_FRAME + 1), "\0\n", "\u0080\n" }) {
                assertEquals("", rejected(endpoint, malformed.getBytes(StandardCharsets.UTF_8)));
                assertEquals(ManagedClose.State.MOUNTED, rpc(service, token, "status", randomHex(16)));
                assertEquals(0, actions.get(), "Invalid control must not start a close action");
            }
            try (SocketChannel partial = connect(endpoint)) {
                partial.write(StandardCharsets.US_ASCII.encode("LFMC1 sta"));
                assertEquals(ManagedClose.State.MOUNTED, rpc(service, token, "status", randomHex(16)));
                assertEquals(0, partial.read(ByteBuffer.allocate(1)), "Partial request must not block other clients");
                assertEquals("", readReply(partial));
            }
            assertEquals(0, actions.get());
        } finally { rpc(service, token, "close", randomHex(16)); finish(service, token); }
        assertEquals(inode, Files.readAttributes(endpoint, "unix:dev,ino"), "close must leave its exact socket inode for core cleanup");
        assertThrows(IOException.class, () -> open(path, token));
        Path constructor = directory(temporary, "constructor-cleanup");
        ManagedClose empty = open(constructor, token); Path emptySocket = empty.socketPath(); empty.close();
        assertTrue(Files.exists(emptySocket, LinkOption.NOFOLLOW_LINKS));
    }

    @Test void startupFailureRetainsTheAttemptUntilAnExplicitOwnedClose() throws Exception {
        Path path = directory(temporary, "startup"); String token = randomHex(32);
        ManagedClose service = open(path, token);
        List<String> order = java.util.Collections.synchronizedList(new ArrayList<>());
        service.setResources(() -> order.add("attempt"), () -> order.add("cryptofs"));
        assertThrows(IOException.class, service::close);
        String early = randomHex(16);
        assertEquals(ManagedClose.State.STARTING, rpc(service, token, "close", early));
        assertTrue(order.isEmpty());
        service.markStartupFailed();
        assertEquals(ManagedClose.State.RETAINED, rpc(service, token, "status", randomHex(16)));
        assertEquals(ManagedClose.State.RETAINED, rpc(service, token, "close", early));
        assertTrue(order.isEmpty(), "A startup request must not become an action when replayed later");
        rpc(service, token, "close", randomHex(16)); finish(service, token);
        assertEquals(List.of("attempt", "cryptofs"), order);
    }

    @Test void blockedActionKeepsStatusLiveAndNeverStartsAConcurrentClose() throws Exception {
        Path path = directory(temporary, "blocked"); String token = randomHex(32);
        ManagedClose service = open(path, token);
        CountDownLatch entered = new CountDownLatch(1), release = new CountDownLatch(1);
        AtomicInteger attempts = new AtomicInteger(), crypto = new AtomicInteger(), returned = new AtomicInteger();
        service.setResources(() -> { attempts.incrementAndGet(); entered.countDown(); release.await(); }, crypto::incrementAndGet, () -> true);
        service.markMounted(returned::incrementAndGet);
        try {
            assertEquals(ManagedClose.State.CLOSING, rpc(service, token, "close", randomHex(16)));
            assertTrue(entered.await(2, TimeUnit.SECONDS));
            assertThrows(IOException.class, service::close);
            for (int i = 0; i < 4; i++) {
                assertEquals(ManagedClose.State.CLOSING, rpc(service, token, "status", randomHex(16)));
                assertEquals(ManagedClose.State.CLOSING, rpc(service, token, "close", randomHex(16)));
            }
            assertEquals(1, attempts.get()); assertEquals(0, crypto.get()); assertEquals(0, returned.get());
        } finally { release.countDown(); }
        finish(service, token);
        assertEquals(1, attempts.get()); assertEquals(1, crypto.get()); assertEquals(0, returned.get());
    }

    @Test void onlyAFreshExplicitCloseMayUseALateVerifiedRetryPolicy() throws Exception {
        Path path = directory(temporary, "retry"); String token = randomHex(32);
        ManagedClose service = open(path, token);
        AtomicInteger attempts = new AtomicInteger(), policyCalls = new AtomicInteger(), policy = new AtomicInteger(), crypto = new AtomicInteger();
        CountDownLatch entered = new CountDownLatch(1), release = new CountDownLatch(1);
        service.setResources(() -> {
                    if (attempts.incrementAndGet() == 1) {
                        entered.countDown(); release.await(); throw new IOException("untrusted exception text");
                    }
                },
                crypto::incrementAndGet, () -> {
                    policyCalls.incrementAndGet();
                    if (policy.get() == 1) throw new IllegalStateException("not a terminal proof");
                    return policy.get() == 2;
                });
        service.markMounted(() -> fail("Returned mount is not a second closer"));
        String first = randomHex(16), inFlight = randomHex(16);
        try {
            assertEquals(ManagedClose.State.CLOSING, rpc(service, token, "close", first));
            assertTrue(entered.await(2, TimeUnit.SECONDS));
            assertEquals(ManagedClose.State.CLOSING, rpc(service, token, "close", inFlight));
        } finally { release.countDown(); }
        state(service, ManagedClose.State.RETAINED);
        assertEquals(1, attempts.get()); assertEquals(0, crypto.get());
        assertEquals(ManagedClose.State.RETAINED, rpc(service, token, "close", first)); assertEquals(0, policyCalls.get());
        policy.set(2); // Even a later terminal/retryable proof cannot revive an old in-flight request.
        assertEquals(ManagedClose.State.RETAINED, rpc(service, token, "close", inFlight));
        assertEquals(0, policyCalls.get()); assertEquals(1, attempts.get());
        policy.set(0);
        String unknown = randomHex(16); rpc(service, token, "close", unknown); state(service, ManagedClose.State.RETAINED);
        assertEquals(1, policyCalls.get()); assertEquals(1, attempts.get());
        policy.set(1); rpc(service, token, "close", randomHex(16)); state(service, ManagedClose.State.RETAINED);
        assertEquals(2, policyCalls.get()); assertEquals(1, attempts.get());
        policy.set(2); assertEquals(ManagedClose.State.RETAINED, rpc(service, token, "close", unknown));
        assertEquals(2, policyCalls.get());
        rpc(service, token, "close", randomHex(16)); finish(service, token);
        assertEquals(2, attempts.get()); assertEquals(1, crypto.get()); assertEquals(3, policyCalls.get());
    }

    @Test void mainStyleWaiterCannotCloseListenerBeforeTheWholeClosedFrameIsWritten() throws Exception {
        Path path = directory(temporary, "ack"); String token = randomHex(32);
        ManagedClose service = open(path, token);
        service.setResources(() -> { }, () -> { }); service.markMounted(() -> fail("Wrong closer"));
        AtomicReference<Throwable> failure = new AtomicReference<>();
        Thread waiter = new Thread(() -> {
            try { service.awaitClosed(); service.close(); }
            catch (Throwable error) { failure.set(error); }
        });
        waiter.start();
        assertEquals(ManagedClose.State.CLOSING, rpc(service, token, "close", randomHex(16)));
        state(service, ManagedClose.State.CLOSED);
        assertFalse(service.awaitClosed(30, TimeUnit.MILLISECONDS));
        assertThrows(IOException.class, service::close);
        String request = randomHex(16);
        String complete = exchange(service.socketPath(), frame("status", token, request).getBytes(StandardCharsets.US_ASCII));
        assertEquals("LFMC1 closed " + request + " " + ProcessHandle.current().pid() + "\n", complete);
        waiter.join(3000); assertFalse(waiter.isAlive()); assertNull(failure.get());
        assertTrue(Files.exists(service.socketPath(), LinkOption.NOFOLLOW_LINKS));
    }

    @Test void permanentFailuresRetainJvmAndCryptoReferencesWithoutATestCleanupEscape() throws Exception {
        for (String mode : List.of("default-mount", "cryptofs", "listener", "listener-before-ack")) {
            Path path = directory(temporary, mode);
            String classpath = Path.of(ManagedCloseTest.class.getProtectionDomain().getCodeSource().getLocation().toURI())
                    + java.io.File.pathSeparator + Path.of(ManagedClose.class.getProtectionDomain().getCodeSource().getLocation().toURI())
                    + java.io.File.pathSeparator + System.getProperty("java.class.path");
            Process process = new ProcessBuilder(Path.of(System.getProperty("java.home"), "bin", "java").toString(),
                    "--add-modules", "jdk.net", "-cp", classpath, Child.class.getName(), mode, path.toString())
                    .redirectErrorStream(true).start();
            try {
                var line = CompletableFuture.supplyAsync(() -> {
                    try { return new BufferedReader(new InputStreamReader(process.getInputStream(), StandardCharsets.UTF_8)).readLine(); }
                    catch (IOException error) { throw new java.io.UncheckedIOException(error); }
                });
                assertEquals("RETAINED", line.get(10, TimeUnit.SECONDS));
                assertFalse(process.waitFor(100, TimeUnit.MILLISECONDS), "Retained listener must keep the JVM alive after main returns");
            } finally {
                // Owned synthetic test JVM only: no FUSE, native lease or CryptoFS exists.
                process.destroy();
                if (!process.waitFor(3, TimeUnit.SECONDS)) { process.destroyForcibly(); assertTrue(process.waitFor(3, TimeUnit.SECONDS)); }
            }
        }
    }

    public static final class Child {
        public static void main(String[] args) throws Exception {
            String mode = args[0], token = randomHex(32);
            ManagedClose service = open(Path.of(args[1]), token);
            AtomicInteger mounts = new AtomicInteger(), crypto = new AtomicInteger(), policy = new AtomicInteger();
            service.setResources(() -> { mounts.incrementAndGet(); if (mode.equals("default-mount")) throw new IOException("retained"); },
                    () -> { crypto.incrementAndGet(); if (mode.equals("cryptofs")) throw new IOException("sticky"); },
                    mode.equals("default-mount") ? null : () -> { policy.incrementAndGet(); return true; });
            service.markMounted(() -> fail("Returned mount must never close"));
            if (!mode.equals("listener")) {
                rpc(service, token, "close", randomHex(16));
                state(service, mode.equals("listener-before-ack") ? ManagedClose.State.CLOSED : ManagedClose.State.RETAINED);
            }
            if (mode.startsWith("listener")) {
                var field = ManagedClose.class.getDeclaredField("listener"); field.setAccessible(true);
                ((ServerSocketChannel) field.get(service)).close();
                long until = System.nanoTime() + TimeUnit.SECONDS.toNanos(3);
                while (!service.listenerFailed() && System.nanoTime() < until) Thread.sleep(2);
                assertTrue(service.listenerFailed()); assertEquals(ManagedClose.State.RETAINED, service.state());
            } else {
                rpc(service, token, "close", randomHex(16)); state(service, ManagedClose.State.RETAINED);
                assertEquals(1, mounts.get()); assertEquals(mode.equals("cryptofs") ? 1 : 0, crypto.get());
                assertEquals(0, policy.get());
            }
            assertFalse(service.awaitClosed(30, TimeUnit.MILLISECONDS));
            assertThrows(IOException.class, service::close);
            if (mode.equals("listener")) { assertEquals(0, mounts.get()); assertEquals(0, crypto.get()); }
            System.out.println("RETAINED"); System.out.flush();
            // No cleanup backdoor: the parent observes JVM retention, then ends
            // this owned synthetic test process. Real resources are never involved.
        }
    }
}
