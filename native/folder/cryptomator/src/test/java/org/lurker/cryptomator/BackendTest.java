// SPDX-License-Identifier: AGPL-3.0-only
package org.lurker.cryptomator;

import org.cryptomator.cryptofs.CryptoFileSystemProvider;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.io.ByteArrayInputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.ReadOnlyFileSystemException;
import java.nio.file.StandardOpenOption;
import java.nio.channels.FileChannel;
import java.lang.reflect.Proxy;
import java.util.Set;
import static org.junit.jupiter.api.Assertions.*;

class BackendTest {
    @TempDir Path vault;

    @Test void providerPolicyNeverFallsBackToMacFuse() throws Exception {
        assertEquals(".LinuxFuseMountProvider", Main.providerSuffix("Linux"));
        assertEquals(".FuseTMountProvider", Main.providerSuffix("Mac OS X"));
        assertThrows(IOException.class, () -> Main.providerSuffix("FreeBSD"));
    }

    @Test void fencedMountpointRequiresOwnedModeZeroAndDoesNotListTheProtectedPath() throws Exception {
        var point = Files.createDirectory(vault.resolve("mountpoint"));
        assertThrows(IOException.class, () -> Main.validateMountpoint(point, true));
        var original = Files.getPosixFilePermissions(point);
        try {
            Files.setPosixFilePermissions(point, Set.of());
            Main.validateMountpoint(point, true);
            var alias = vault.resolve("alias"); Files.createSymbolicLink(alias, point);
            assertThrows(IOException.class, () -> Main.validateMountpoint(alias, true));
        } finally { Files.setPosixFilePermissions(point, original); }
        Files.writeString(point.resolve("existing"), "fixture");
        assertThrows(IOException.class, () -> Main.validateMountpoint(point, false));
    }

    @Test void macTransportRequiresFskitAndRejectsUnsupportedHostOrDefaults() throws Exception {
        String defaults = "-ononamedattr -obackend=smb -orwsize=262144 -ouid=501 -ogid=20";
        for (String version : new String[] {"26.0", "26.6.2", "27.2"}) {
            assertEquals(defaults.replace("backend=smb", "backend=fskit"), Main.transportFlags("Mac OS X", version, defaults));
        }
        assertEquals("-oauto_unmount", Main.transportFlags("Linux", "6.18", "-oauto_unmount"));
        for (String version : new String[] {"15.4", "25.9", "", "secret-attacker-value"}) {
            var error = assertThrows(IOException.class, () -> Main.transportFlags("Mac OS X", version, defaults));
            assertTrue(error.getMessage().contains("macOS 26"));
            assertFalse(error.getMessage().contains("secret"));
        }
        for (String flags : new String[] {"-obackend=nfs", "-obackend=fskit", "-obackend=smb-unqualified", "-obackend=smb -obackend=nfs"}) {
            assertThrows(IOException.class, () -> Main.transportFlags("Mac OS X", "27.2", flags));
        }
    }

    @Test void initializesPersistsReopensAndEnforcesReadonly() throws Exception {
        char[] password = "  密碼 with spaces  ".toCharArray();
        Main.initialize(vault, password);
        try (var fs = CryptoFileSystemProvider.newFileSystem(vault, Main.properties(vault, password, false))) {
            var path = fs.getPath("/hello ü.txt");
            Files.writeString(path, "durable test data");
            try (var channel = FileChannel.open(path, StandardOpenOption.WRITE)) { channel.force(true); }
        }
        assertFalse(Files.walk(vault).anyMatch(p -> p.getFileName().toString().contains("hello")));
        try (var fs = CryptoFileSystemProvider.newFileSystem(vault, Main.properties(vault, password, true))) {
            assertEquals("durable test data", Files.readString(fs.getPath("/hello ü.txt")));
            assertThrows(ReadOnlyFileSystemException.class, () -> Files.writeString(fs.getPath("/new"), "no"));
        }
        assertThrows(Exception.class, () -> CryptoFileSystemProvider.newFileSystem(vault, Main.properties(vault, "wrong".toCharArray(), false)));
        assertThrows(IOException.class, () -> Main.initialize(vault, password));
    }

    @Test void passwordTransportPreservesWhitespaceAndRejectsInvalidInput() throws Exception {
        var original = System.in;
        try {
            System.setIn(new ByteArrayInputStream("  密碼  \n".getBytes(StandardCharsets.UTF_8)));
            assertArrayEquals("  密碼  ".toCharArray(), Main.readPassword());
            for (byte[] input : new byte[][] {new byte[0], "\n".getBytes(), "a\r\n".getBytes(), new byte[]{(byte)0xff,10}, "a\0b\n".getBytes()}) {
                System.setIn(new ByteArrayInputStream(input));
                assertThrows(IOException.class, Main::readPassword);
            }
        } finally { System.setIn(original); }
    }

    @Test void fuseAdapterRegistersFsyncCallback() throws Exception {
        // Surefire loads dependencies on its classpath. Reflection keeps this
        // internal adapter integration check out of the shipped module contract.
        var errno = Class.forName("org.cryptomator.jfuse.api.Errno");
        var errors = Proxy.newProxyInstance(errno.getClassLoader(), new Class<?>[]{errno}, (proxy, method, args) -> 0);
        var transcoder = Class.forName("org.cryptomator.frontend.fuse.FileNameTranscoder");
        var names = transcoder.getMethod("transcoder").invoke(null);
        var type = Class.forName("org.cryptomator.frontend.fuse.ReadWriteAdapter");
        try (var adapter = (AutoCloseable) type.getMethod("create", errno, Path.class, int.class, transcoder, boolean.class)
                .invoke(null, errors, vault, 254, names, false)) {
            var operations = (Set<?>) type.getMethod("supportedOperations").invoke(adapter);
            assertTrue(operations.stream().anyMatch(value -> ((Enum<?>) value).name().equals("FSYNC")),
                    "The FUSE callback must be registered, not merely implemented; otherwise fsync silently loses open-file data on daemon death");
        }
    }
    @Test void closeAcknowledgementRequiresBothResourcesAndAttemptsBoth() throws Exception {
        var stdout = new java.io.ByteArrayOutputStream();
        var stderr = new java.io.ByteArrayOutputStream();
        var attempted = new java.util.concurrent.atomic.AtomicInteger();
        AutoCloseable good = () -> attempted.incrementAndGet();
        assertTrue(Main.closeResources(good, good, new java.io.PrintStream(stdout), new java.io.PrintStream(stderr)));
        assertEquals(2, attempted.get());
        assertEquals("{\"protocol\":1,\"status\":\"closed\"}", stdout.toString().trim());
        stdout.reset();
        attempted.set(0);
        AutoCloseable bad = () -> { attempted.incrementAndGet(); throw new IOException("secret attacker-controlled message"); };
        assertFalse(Main.closeResources(bad, good, new java.io.PrintStream(stdout), new java.io.PrintStream(stderr)));
        assertEquals(2, attempted.get());
        assertEquals("{\"protocol\":1,\"status\":\"close-failed\"}", stdout.toString().trim());
        assertFalse(stderr.toString().contains("secret"));
        stdout.reset();
        assertFalse(Main.closeResources(good, bad, new java.io.PrintStream(stdout), new java.io.PrintStream(stderr)));
        assertTrue(stdout.toString().contains("close-failed"));
    }

}
