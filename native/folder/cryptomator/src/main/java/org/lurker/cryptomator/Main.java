// SPDX-License-Identifier: AGPL-3.0-only
package org.lurker.cryptomator;

import org.cryptomator.cryptofs.CryptoFileSystemProperties;
import org.cryptomator.cryptofs.CryptoFileSystemProvider;
import org.cryptomator.cryptofs.VaultConfig;
import org.cryptomator.cryptolib.api.CryptorProvider;
import org.cryptomator.cryptolib.api.Masterkey;
import org.cryptomator.cryptolib.common.MasterkeyFileAccess;
import org.cryptomator.integrations.mount.MountCapability;
import org.cryptomator.integrations.mount.MountService;

import java.io.IOException;
import java.net.URI;
import java.nio.CharBuffer;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.Path;
import java.security.SecureRandom;
import java.util.Arrays;
import java.util.Locale;
import java.util.ServiceLoader;
import java.util.concurrent.CountDownLatch;

/** Narrow private process protocol. Passwords are never accepted through argv or env. */
public final class Main {
    static final URI KEY_ID = URI.create("masterkeyfile:masterkey.cryptomator");
    private static final byte[] PEPPER = new byte[0];
    private Main() {}

    public static void main(String[] args) {
        try {
            if (args.length == 1 && args[0].equals("--version")) {
                System.out.println("lurker-cryptomator 1; cryptofs 2.8.0; fuse-nio 5.1.0");
                return;
            }
            if (args.length == 1 && args[0].equals("--capabilities")) {
                System.out.println("{\"protocol\":1,\"init\":true,\"mount\":true,\"readonly\":true,\"password_stdin\":true}");
                return;
            }
            if (args.length < 2 || (!args[0].equals("init") && !args[0].equals("mount"))) {
                throw new IllegalArgumentException("Expected init DIRECTORY or mount VAULT MOUNTPOINT [--readonly]");
            }
            boolean init = args[0].equals("init");
            if ((init && args.length != 2) || (!init && args.length != 3 && args.length != 4)
                    || (args.length == 4 && !args[3].equals("--readonly"))) {
                throw new IllegalArgumentException("Invalid backend arguments");
            }
            char[] password = readPassword();
            try {
                if (init) {
                    initialize(Path.of(args[1]), password);
                    System.out.println("{\"protocol\":1,\"status\":\"initialized\"}");
                } else {
                    mount(Path.of(args[1]), Path.of(args[2]), password, args.length == 4);
                }
            } finally {
                Arrays.fill(password, '\0');
            }
        } catch (Exception error) {
            // Exception messages/stack traces can include attacker-controlled vault contents.
            System.err.println("Cryptomator operation failed (" + error.getClass().getSimpleName() + ")");
            System.exit(1);
        }
    }

    static char[] readPassword() throws IOException {
        byte[] bytes = new byte[65536];
        int length = 0;
        try {
            int value;
            while ((value = System.in.read()) != -1 && value != '\n') {
                if (value == 0 || value == '\r' || length == bytes.length) {
                    throw new IOException("Invalid password transport");
                }
                bytes[length++] = (byte) value;
            }
            if (value != '\n' || length == 0) throw new IOException("Missing password line");
            var decoded = StandardCharsets.UTF_8.newDecoder()
                    .onMalformedInput(CodingErrorAction.REPORT).onUnmappableCharacter(CodingErrorAction.REPORT)
                    .decode(java.nio.ByteBuffer.wrap(bytes, 0, length));
            char[] result = new char[decoded.remaining()];
            decoded.get(result);
            if (decoded.hasArray()) Arrays.fill(decoded.array(), '\0');
            return result;
        } finally {
            Arrays.fill(bytes, (byte) 0);
        }
    }

    static void initialize(Path vault, char[] password) throws Exception {
        if (!Files.isDirectory(vault, LinkOption.NOFOLLOW_LINKS)) throw new IOException("Vault is not a directory");
        try (var entries = Files.list(vault)) {
            if (entries.findAny().isPresent()) throw new IOException("Vault must be empty");
        }
        var random = SecureRandom.getInstanceStrong();
        try (var masterkey = Masterkey.generate(random)) {
            new MasterkeyFileAccess(PEPPER, random).persist(masterkey, vault.resolve("masterkey.cryptomator"), CharBuffer.wrap(password));
            var properties = CryptoFileSystemProperties.cryptoFileSystemProperties()
                    .withCipherCombo(CryptorProvider.Scheme.SIV_GCM)
                    .withKeyLoader(ignored -> masterkey.copy()).build();
            CryptoFileSystemProvider.initialize(vault, properties, KEY_ID);
        }
        // Rust stages, fsyncs and publishes this directory only after successful completion.
    }

    static CryptoFileSystemProperties properties(Path vault, char[] password, boolean readonly) throws IOException {
        var random = new SecureRandom();
        var config = VaultConfig.decode(Files.readString(vault.resolve("vault.cryptomator"), StandardCharsets.US_ASCII));
        var builder = CryptoFileSystemProperties.cryptoFileSystemProperties()
                .withShorteningThreshold(config.allegedShorteningThreshold())
                .withKeyLoader(id -> {
                    if (!KEY_ID.equals(id)) throw new IllegalArgumentException("Unsupported key location");
                    return new MasterkeyFileAccess(PEPPER, random).load(vault.resolve("masterkey.cryptomator"), CharBuffer.wrap(password));
                });
        if (readonly) builder.withFlags(CryptoFileSystemProperties.FileSystemFlags.READONLY);
        return builder.build();
    }

    private static MountService mountService() throws IOException {
        var os = System.getProperty("os.name").toLowerCase(Locale.ROOT);
        String suffix;
        if (os.contains("linux")) suffix = ".LinuxFuseMountProvider";
        else if (os.contains("mac")) suffix = ".MacFuseMountProvider";
        else throw new IOException("Unsupported Cryptomator platform");
        // No WebDAV/network fallback and no user-selected provider or mount flags.
        return ServiceLoader.load(MountService.class).stream().map(ServiceLoader.Provider::get)
                .filter(service -> service.getClass().getName().endsWith(suffix) && service.isSupported())
                .findFirst().orElseThrow(() -> new IOException("Required FUSE provider unavailable"));
    }

    private static void mount(Path vault, Path mountpoint, char[] password, boolean readonly) throws Exception {
        if (!Files.isDirectory(mountpoint, LinkOption.NOFOLLOW_LINKS)) throw new IOException("Invalid mountpoint");
        try (var entries = Files.list(mountpoint)) {
            if (entries.findAny().isPresent()) throw new IOException("Mountpoint must be empty");
        }
        var fs = CryptoFileSystemProvider.newFileSystem(vault, properties(vault, password, readonly));
        try {
            var service = mountService();
            var builder = service.forFileSystem(fs.getPath("/"));
            builder.setMountpoint(mountpoint);
            String flags = service.getDefaultMountFlags();
            // Do not use auto_unmount: a backend crash must retain discoverable recovery state.
            flags = flags.replace("-oauto_unmount", "").trim();
            if (readonly && service.hasCapability(MountCapability.READ_ONLY)) builder.setReadOnly(true);
            if (readonly) flags += " -oro";
            builder.setMountFlags(flags);
            var mount = builder.mount();
            var finished = new CountDownLatch(1);
            Runtime.getRuntime().addShutdownHook(new Thread(() -> {
                try {
                    // No unmountForced(), even on shutdown failure.
                    mount.close();
                    fs.close();
                } catch (Exception error) {
                    System.err.println("Cryptomator graceful close incomplete; retain session for recovery");
                } finally {
                    finished.countDown();
                }
            }, "lurker-cryptomator-close"));
            // The key loader has run by this point; erase the caller's password now.
            Arrays.fill(password, '\0');
            System.out.println("{\"protocol\":1,\"status\":\"mounted\"}");
            System.out.flush();
            finished.await();
        } catch (Exception error) {
            fs.close();
            throw error;
        }
    }
}
