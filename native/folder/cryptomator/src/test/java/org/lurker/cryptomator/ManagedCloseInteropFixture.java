// SPDX-License-Identifier: AGPL-3.0-only
package org.lurker.cryptomator;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.nio.file.attribute.PosixFilePermissions;
import java.util.Set;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * Opt-in native Rust/Java socket fixture, never an actual filesystem mount.
 * Main.runManaged and ManagedClose are genuine; every mount/CryptoFS object is
 * an in-memory fake. The only input secret arrives on stdin, not argv or env.
 */
public final class ManagedCloseInteropFixture {
    private ManagedCloseInteropFixture() { }

    private static void trace(Path file, String event) throws IOException {
        Files.writeString(file, event + "\n", StandardOpenOption.APPEND);
    }

    public static void main(String[] args) throws Exception {
        if (args.length != 2 || !Set.of("normal", "busy", "startup-failure", "sticky-cryptofs").contains(args[1]))
            throw new IOException("Invalid fake managed-close fixture arguments");
        Path directory = Path.of(args[0]);
        String mode = args[1];
        String token = Main.readManagedToken();
        var control = ManagedClose.open(directory, token, Files.getOwner(directory, LinkOption.NOFOLLOW_LINKS));
        Path events = Files.createFile(directory.resolve("fake-close.trace"),
                PosixFilePermissions.asFileAttribute(PosixFilePermissions.fromString("rw-------")));
        var providerCalls = new AtomicInteger();
        AutoCloseable provider = () -> {
            trace(events, "provider-close");
            if (mode.equals("busy") && providerCalls.incrementAndGet() == 1)
                throw new IOException("Fake provider busy");
        };
        AutoCloseable cryptoFs = () -> {
            trace(events, "cryptofs-close");
            if (mode.equals("sticky-cryptofs")) throw new IOException("Fake CryptoFS close failure");
        };
        // This returned fake is retained by ManagedClose, never a second closer.
        AutoCloseable returnedMount = () -> trace(events, "unexpected-returned-mount-close");
        System.out.println("FIXTURE_READY");
        System.out.flush();
        Main.runManaged(control, provider, cryptoFs, () -> true, () -> {
            if (mode.equals("startup-failure")) throw new IOException("Fake startup failure");
            return returnedMount;
        }, new char[] { 'f', 'a', 'k', 'e' }, System.out, System.err);
        // No System.exit, shutdown hook, native mount or test completion file.
        // Main emits independent closed stdout only after a live bound reply.
    }
}
