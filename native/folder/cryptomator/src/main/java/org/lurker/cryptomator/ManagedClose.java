// SPDX-License-Identifier: AGPL-3.0-only
package org.lurker.cryptomator;

import jdk.net.ExtendedSocketOptions;

import java.io.IOException;
import java.net.StandardProtocolFamily;
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
import java.nio.file.attribute.PosixFileAttributes;
import java.nio.file.attribute.PosixFilePermission;
import java.nio.file.attribute.UserPrincipal;
import java.security.MessageDigest;
import java.time.Duration;
import java.util.Arrays;
import java.util.HashSet;
import java.util.Locale;
import java.util.Objects;
import java.util.Set;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.function.BooleanSupplier;

/**
 * Private LFMC1 control, without a shutdown hook or OS unmount operation.
 * The owner installs one bound builder-attempt and CryptoFS before mount(),
 * then marks startup success/failure and awaits closed. The builder-attempt
 * is the sole mount closer; the returned mount is retained, never closed here.
 * A failed close retains references and the listener. Only a later explicit
 * CLOSE with a fresh nonce may consult the supplied retry policy, after the
 * prior action is terminal. The default policy is false; a provider policy
 * must establish a terminal owned EBUSY refusal with no release/disposal.
 * CryptoFS failure and listener failure never permit a retry or clean result.
 * awaitClosed completes only after a bound CLOSED reply is fully written.
 * close() only releases control descriptors after that acknowledgment, or before resources
 * were attached. It deliberately leaves the socket inode for verified cleanup.
 */
public final class ManagedClose implements AutoCloseable {
    public enum State { STARTING, MOUNTED, CLOSING, RETAINED, CLOSED }
    private enum Failure { NONE, MOUNT, CRYPTOFS }
    public static final String SOCKET_NAME = "control.sock";
    static final int MAX_FRAME = 128;
    static final Duration FRAME_DEADLINE = Duration.ofSeconds(1);
    private static final int MAX_CONNECTIONS = 16;
    private static final int MAX_CLOSE_REQUESTS = 64;
    private static final Set<PosixFilePermission> PRIVATE_DIRECTORY = Set.of(
            PosixFilePermission.OWNER_READ, PosixFilePermission.OWNER_WRITE,
            PosixFilePermission.OWNER_EXECUTE);
    private static final Set<PosixFilePermission> PRIVATE_SOCKET = Set.of(
            PosixFilePermission.OWNER_READ, PosixFilePermission.OWNER_WRITE);

    private final Object guard = new Object();
    private final Path socketPath;
    private final UserPrincipal expectedOwner;
    private final byte[] token;
    private final Selector selector;
    private final ServerSocketChannel listener;
    private final Thread listenerThread;
    private final CountDownLatch closed = new CountDownLatch(1);
    private final Set<String> closeRequests = new HashSet<>();
    private final long pid = ProcessHandle.current().pid();
    private State state = State.STARTING;
    private Failure failure = Failure.NONE;
    private AutoCloseable builderAttempt, cryptoFs, returnedMount;
    private BooleanSupplier retrySafeAfterMountFailure = () -> false;
    private boolean resourcesAttached, startupDone, actionRunning, listenerFailed, closedAcknowledged;
    private volatile boolean stopping;

    private static final class Connection {
        final ByteBuffer input = ByteBuffer.allocate(MAX_FRAME);
        ByteBuffer output;
        boolean closedReply;
        long deadline = System.nanoTime() + FRAME_DEADLINE.toNanos();
    }

    private record Reply(ByteBuffer bytes, boolean closed) { }

    public static ManagedClose open(Path sessionDirectory, String token64hex,
                                    UserPrincipal expectedOwner) throws IOException {
        Objects.requireNonNull(sessionDirectory);
        Objects.requireNonNull(expectedOwner);
        if (!hex(token64hex, 64)) throw new IOException("Invalid managed-close bootstrap");
        Path supplied = sessionDirectory.toAbsolutePath().normalize();
        var before = privateDirectory(supplied, expectedOwner);
        Path directory = supplied.toRealPath();
        if (!before.fileKey().equals(privateDirectory(directory, expectedOwner).fileKey()))
            throw new IOException("Managed-close directory changed");
        Path endpoint = directory.resolve(SOCKET_NAME);
        Selector selector = Selector.open();
        ServerSocketChannel listener = null;
        try {
            listener = ServerSocketChannel.open(StandardProtocolFamily.UNIX);
            // Bind fails on any existing entry. Never unlink an alleged stale socket.
            listener.bind(UnixDomainSocketAddress.of(endpoint));
            Files.setPosixFilePermissions(endpoint, PRIVATE_SOCKET);
            var socket = Files.readAttributes(endpoint, PosixFileAttributes.class, LinkOption.NOFOLLOW_LINKS);
            int mode = (int) Files.getAttribute(endpoint, "unix:mode", LinkOption.NOFOLLOW_LINKS);
            if (!socket.owner().equals(expectedOwner) || !socket.permissions().equals(PRIVATE_SOCKET)
                    || (mode & 0170000) != 0140000 || socket.fileKey() == null
                    || !before.fileKey().equals(privateDirectory(directory, expectedOwner).fileKey()))
                throw new IOException("Invalid managed-close endpoint");
            listener.configureBlocking(false);
            listener.register(selector, SelectionKey.OP_ACCEPT);
            return new ManagedClose(endpoint, expectedOwner, token64hex, selector, listener);
        } catch (IOException | RuntimeException error) {
            if (listener != null) try { listener.close(); } catch (IOException ignored) { }
            try { selector.close(); } catch (IOException ignored) { }
            throw new IOException("Cannot initialize managed-close control", error);
        }
    }

    private ManagedClose(Path path, UserPrincipal owner, String secret,
                         Selector selector, ServerSocketChannel listener) {
        this.socketPath = path;
        this.expectedOwner = owner;
        this.token = secret.getBytes(StandardCharsets.US_ASCII);
        this.selector = selector;
        this.listener = listener;
        this.listenerThread = new Thread(this::serve, "lurker-managed-close-control");
        this.listenerThread.setDaemon(false);
        this.listenerThread.start();
    }

    private static PosixFileAttributes privateDirectory(Path path, UserPrincipal owner) throws IOException {
        var attributes = Files.readAttributes(path, PosixFileAttributes.class, LinkOption.NOFOLLOW_LINKS);
        if (!attributes.isDirectory() || attributes.isSymbolicLink() || attributes.fileKey() == null
                || !attributes.owner().equals(owner) || !attributes.permissions().equals(PRIVATE_DIRECTORY))
            throw new IOException("Private caller-owned managed-close directory required");
        return attributes;
    }

    public Path socketPath() { return socketPath; }
    public State state() { synchronized (guard) { return state; } }
    public boolean listenerFailed() { synchronized (guard) { return listenerFailed; } }

    public void setResources(AutoCloseable builderAttempt, AutoCloseable cryptoFs) {
        setResources(builderAttempt, cryptoFs, () -> false);
    }

    public void setResources(AutoCloseable builderAttempt, AutoCloseable cryptoFs,
                             BooleanSupplier retrySafeAfterMountFailure) {
        Objects.requireNonNull(builderAttempt);
        Objects.requireNonNull(cryptoFs);
        synchronized (guard) {
            if (resourcesAttached || startupDone || listenerFailed || stopping)
                throw new IllegalStateException("Managed-close resources cannot be replaced");
            this.builderAttempt = builderAttempt;
            this.cryptoFs = cryptoFs;
            this.retrySafeAfterMountFailure = retrySafeAfterMountFailure == null ? () -> false : retrySafeAfterMountFailure;
            this.resourcesAttached = true;
        }
    }

    public void markMounted(AutoCloseable returnedMount) {
        Objects.requireNonNull(returnedMount);
        synchronized (guard) {
            if (!resourcesAttached || startupDone) throw new IllegalStateException("Invalid managed-close startup transition");
            this.returnedMount = returnedMount; // strong lifetime reference, not a second close authority
            startupDone = true;
            state = listenerFailed ? State.RETAINED : State.MOUNTED;
        }
    }

    public void markStartupFailed() {
        synchronized (guard) {
            if (!resourcesAttached || startupDone) throw new IllegalStateException("Invalid managed-close startup transition");
            startupDone = true;
            state = State.RETAINED;
        }
    }

    public void awaitClosed() throws InterruptedException { closed.await(); }
    public boolean awaitClosed(long timeout, TimeUnit unit) throws InterruptedException { return closed.await(timeout, unit); }

    private State requestClose(String request) {
        synchronized (guard) {
            // Even a STARTING/CLOSING request is consumed. Replaying it after
            // a later transition must never acquire a new close authority.
            if (closeRequests.contains(request)) return state;
            if (closeRequests.size() >= MAX_CLOSE_REQUESTS) return State.RETAINED;
            closeRequests.add(request);
            if (listenerFailed) return State.RETAINED;
            if (state == State.CLOSED || actionRunning || !startupDone || !resourcesAttached) return state;
            if (failure == Failure.CRYPTOFS) return State.RETAINED;
            boolean retry = failure == Failure.MOUNT;
            actionRunning = true;
            state = State.CLOSING;
            var action = new Thread(() -> performClose(retry), "lurker-managed-close-action");
            action.setDaemon(false);
            try { action.start(); }
            catch (RuntimeException | Error error) { actionRunning = false; state = State.RETAINED; failure = Failure.MOUNT; }
            return state;
        }
    }

    private void performClose(boolean retry) {
        boolean mountClosed = false;
        try {
            if (retry) {
                boolean safe;
                try { safe = retrySafeAfterMountFailure.getAsBoolean(); }
                catch (RuntimeException | Error error) { safe = false; }
                if (!safe) return;
            }
            synchronized (guard) { if (listenerFailed) return; }
            builderAttempt.close();
            mountClosed = true;
            synchronized (guard) { if (listenerFailed) return; }
            cryptoFs.close();
            synchronized (guard) {
                if (!listenerFailed) { state = State.CLOSED; failure = Failure.NONE; }
            }
        } catch (Exception | LinkageError error) {
            synchronized (guard) { failure = mountClosed ? Failure.CRYPTOFS : Failure.MOUNT; }
        } finally {
            synchronized (guard) {
                actionRunning = false;
                if (state != State.CLOSED) {
                    state = State.RETAINED;
                    if (failure == Failure.NONE) failure = mountClosed ? Failure.CRYPTOFS : Failure.MOUNT;
                }
            }
        }
    }

    private static boolean hex(String value, int size) {
        if (value == null || value.length() != size) return false;
        for (int i = 0; i < value.length(); i++) {
            char c = value.charAt(i);
            if (!(c >= '0' && c <= '9') && !(c >= 'a' && c <= 'f')) return false;
        }
        return true;
    }

    private Reply reply(byte[] frame, int count) {
        // Exactly one bounded ASCII LF frame, no CR, extra arguments or paths.
        for (int i = 0; i < count; i++) if (frame[i] < 0x20 || frame[i] > 0x7e) {
            if (i != count - 1 || frame[i] != '\n') return null;
        }
        String text = new String(frame, 0, count - 1, StandardCharsets.US_ASCII);
        String[] fields = text.split(" ", -1);
        if (fields.length != 4 || !fields[0].equals("LFMC1")
                || (!fields[1].equals("status") && !fields[1].equals("close"))
                || !hex(fields[2], 64) || !hex(fields[3], 32)) return null;
        byte[] supplied = fields[2].getBytes(StandardCharsets.US_ASCII);
        boolean matches = MessageDigest.isEqual(token, supplied);
        Arrays.fill(supplied, (byte) 0);
        if (!matches) return null;
        State result = fields[1].equals("close") ? requestClose(fields[3]) : state();
        return new Reply(StandardCharsets.US_ASCII.encode("LFMC1 " + result.name().toLowerCase(Locale.ROOT)
                + " " + fields[3] + " " + pid + "\n"), result == State.CLOSED);
    }

    private void accept() throws IOException {
        SocketChannel channel;
        for (int accepted = 0; accepted < MAX_CONNECTIONS && (channel = listener.accept()) != null; accepted++) {
            try {
                // No fallback to path modes or an unauthenticated peer.
                if (selector.keys().size() > MAX_CONNECTIONS
                        || !channel.getOption(ExtendedSocketOptions.SO_PEERCRED).user().equals(expectedOwner)) {
                    channel.close();
                    continue;
                }
                channel.configureBlocking(false);
                channel.register(selector, SelectionKey.OP_READ, new Connection());
            } catch (IOException | RuntimeException error) { try { channel.close(); } catch (IOException ignored) { } }
        }
    }

    private static void drop(SelectionKey key) {
        if (key.attachment() instanceof Connection connection) Arrays.fill(connection.input.array(), (byte) 0);
        key.cancel();
        try { key.channel().close(); } catch (IOException ignored) { }
    }

    private void serve() {
        try {
            while (!stopping) {
                if (!listener.isOpen() || !selector.isOpen()) throw new IOException("Managed-close listener unavailable");
                selector.select(50);
                var keys = selector.selectedKeys().iterator();
                while (keys.hasNext()) {
                    SelectionKey key = keys.next(); keys.remove();
                    if (!key.isValid()) continue;
                    if (key.channel() == listener) { accept(); continue; }
                    var channel = (SocketChannel) key.channel();
                    var connection = (Connection) key.attachment();
                    try {
                        if (key.isReadable()) {
                            int read = channel.read(connection.input);
                            if (read < 0) { drop(key); continue; }
                            int count = connection.input.position(), lf = -1;
                            for (int i = 0; i < count; i++) if (connection.input.get(i) == '\n') { lf = i; break; }
                            if (lf >= 0) {
                                if (lf != count - 1) { drop(key); continue; }
                                Reply response = reply(connection.input.array(), count);
                                Arrays.fill(connection.input.array(), (byte) 0);
                                if (response == null) { drop(key); continue; }
                                connection.output = response.bytes();
                                connection.closedReply = response.closed();
                                connection.deadline = System.nanoTime() + FRAME_DEADLINE.toNanos();
                                key.interestOps(SelectionKey.OP_WRITE);
                            } else if (!connection.input.hasRemaining()) { drop(key); }
                        } else if (key.isWritable()) {
                            channel.write(connection.output);
                            if (!connection.output.hasRemaining()) {
                                drop(key);
                                if (connection.closedReply) synchronized (guard) {
                                    if (state == State.CLOSED && !listenerFailed) {
                                        closedAcknowledged = true;
                                        closed.countDown();
                                    }
                                }
                            }
                        }
                    } catch (IOException | RuntimeException error) { drop(key); }
                }
                long now = System.nanoTime();
                for (SelectionKey key : selector.keys())
                    if (key.attachment() instanceof Connection connection && now - connection.deadline >= 0) drop(key);
            }
        } catch (IOException | RuntimeException error) {
            // A control failure cannot authorize backend/CryptoFS disposal.
        } finally {
            synchronized (guard) {
                if (!stopping) { listenerFailed = true; if (!closedAcknowledged) state = State.RETAINED; }
            }
            try { for (SelectionKey key : selector.keys()) drop(key); } catch (RuntimeException ignored) { }
            try { listener.close(); } catch (IOException ignored) { }
            try { selector.close(); } catch (IOException ignored) { }
            // An unexpected control failure must not let GC or main returning
            // discard a published attempt. No backend action is triggered here.
            synchronized (guard) {
                while (!stopping && resourcesAttached && !closedAcknowledged) {
                    try { guard.wait(); }
                    catch (InterruptedException ignored) { /* retention is not a close authority */ }
                }
            }
        }
    }

    @Override public void close() throws IOException {
        synchronized (guard) {
            if (resourcesAttached && (state != State.CLOSED || !closedAcknowledged))
                throw new IOException("Managed-close resources retained; listener must remain live");
            stopping = true;
            guard.notifyAll();
        }
        selector.wakeup();
        if (Thread.currentThread() != listenerThread) {
            try { listenerThread.join(2000); }
            catch (InterruptedException error) { Thread.currentThread().interrupt(); throw new IOException("Control shutdown interrupted"); }
            if (listenerThread.isAlive()) throw new IOException("Control listener remains live");
        }
        Arrays.fill(token, (byte) 0);
        // Never delete socketPath, including after successful close.
    }
}
