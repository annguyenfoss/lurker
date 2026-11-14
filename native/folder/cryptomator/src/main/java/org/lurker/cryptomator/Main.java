// SPDX-License-Identifier: AGPL-3.0-only
package org.lurker.cryptomator;

import org.cryptomator.cryptofs.CryptoFileSystemProperties;
import org.cryptomator.cryptofs.CryptoFileSystemProvider;
import org.cryptomator.cryptofs.VaultConfig;
import org.cryptomator.cryptolib.api.CryptorProvider;
import org.cryptomator.cryptolib.api.Masterkey;
import org.cryptomator.cryptolib.common.MasterkeyFileAccess;
import org.cryptomator.integrations.mount.MountCapability;
import org.cryptomator.integrations.mount.Mount;
import org.cryptomator.integrations.mount.MountFailedException;
import org.cryptomator.integrations.mount.MountService;

import java.io.IOException;
import java.io.PrintStream;
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
    private static final String MAC_RUNTIME_HELP = "Cryptomator requires macOS 26 or later and FUSE-T with its FSKit extension enabled in System Settings > General > Login Items & Extensions > File System Extensions.";
    private static final class PrerequisiteException extends IOException {
        private PrerequisiteException(String fixedDiagnostic) { super(fixedDiagnostic); }
    }
    private Main() {}

    public static void main(String[] args) {
        try {
            if (args.length == 1 && args[0].equals("--version")) {
                System.out.println("lurker-cryptomator 1; cryptofs 2.8.0; fuse-nio 5.1.0");
                return;
            }
            if (args.length == 1 && args[0].equals("--capabilities")) {
                int fence = System.getProperty("os.name").toLowerCase(Locale.ROOT).contains("mac") ? 1 : 0;
                System.out.println("{\"protocol\":1,\"init\":true,\"mount\":true,\"readonly\":true,\"password_stdin\":true,\"fenced_mountpoint_protocol\":" + fence + "}");
                return;
            }
            if (args.length < 2 || (!args[0].equals("init") && !args[0].equals("mount"))) {
                throw new IllegalArgumentException("Expected init DIRECTORY or mount VAULT MOUNTPOINT [--readonly]");
            }
            boolean init = args[0].equals("init");
            if ((init && args.length != 2) || (!init && (args.length < 3 || args.length > 5))) {
                throw new IllegalArgumentException("Invalid backend arguments");
            }
            boolean readonly = false, fenced = false;
            if (!init) for (int i = 3; i < args.length; i++) {
                if (args[i].equals("--readonly") && !readonly) readonly = true;
                else if (args[i].equals("--fenced-mountpoint") && !fenced
                        && System.getProperty("os.name").toLowerCase(Locale.ROOT).contains("mac")) fenced = true;
                else throw new IllegalArgumentException("Invalid backend arguments");
            }
            char[] password = readPassword();
            try {
                if (init) {
                    initialize(Path.of(args[1]), password);
                    System.out.println("{\"protocol\":1,\"status\":\"initialized\"}");
                } else {
                    mount(Path.of(args[1]), Path.of(args[2]), password, readonly, fenced);
                }
            } finally {
                Arrays.fill(password, '\0');
            }
        } catch (Exception error) {
            // Exception messages/stack traces can include attacker-controlled vault contents.
            System.err.println(error instanceof PrerequisiteException ? error.getMessage()
                    : "Cryptomator operation failed (" + error.getClass().getSimpleName() + ")");
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

    static String providerSuffix(String operatingSystem) throws IOException {
        var os = operatingSystem.toLowerCase(Locale.ROOT);
        if (os.contains("linux")) return ".LinuxFuseMountProvider";
        else if (os.contains("mac")) return ".FuseTMountProvider";
        else throw new IOException("Unsupported Cryptomator platform");
    }

    static String transportFlags(String operatingSystem, String version, String defaults) throws IOException {
        if (!operatingSystem.toLowerCase(Locale.ROOT).contains("mac")) return defaults;
        int major;
        try { major = Integer.parseInt(version.split("\\.", 2)[0]); }
        catch (NumberFormatException error) { major = 0; }
        if (major < 26) throw new PrerequisiteException("Cryptomator's FUSE-T FSKit transport requires macOS 26 or later.");
        // The pinned upstream FUSE-T provider defaults to SMB. Its byte-range
        // locks reject SQLite WAL; NFS refuses WAL too. Never fall back to either.
        var tokens = Arrays.asList(defaults.trim().split("\\s+"));
        if (!tokens.contains("-obackend=smb")
                || tokens.stream().filter(flag -> flag.startsWith("-obackend=")).count() != 1) {
            throw new PrerequisiteException("Unexpected FUSE-T provider defaults; refusing an unqualified transport.");
        }
        return defaults.replace("-obackend=smb", "-obackend=fskit");
    }

    private static MountService mountService() throws IOException {
        var suffix = providerSuffix(System.getProperty("os.name"));
        // No WebDAV/network fallback and no user-selected provider or mount flags.
        return ServiceLoader.load(MountService.class).stream().map(ServiceLoader.Provider::get)
                .filter(service -> service.getClass().getName().endsWith(suffix) && service.isSupported())
                .findFirst().orElseThrow(() -> new PrerequisiteException(suffix.equals(".FuseTMountProvider")
                        ? MAC_RUNTIME_HELP : "Cryptomator requires the Linux libfuse3 runtime."));
    }

    static boolean closeResources(AutoCloseable mount, AutoCloseable fs, PrintStream out, PrintStream err) {
        boolean clean = true;
        for (var resource : new AutoCloseable[] {mount, fs}) {
            try {
                resource.close();
            } catch (Exception error) {
                clean = false;
                // Class names only: messages may contain attacker-controlled data.
                err.println("Cryptomator graceful close failed (" + error.getClass().getSimpleName() + ")");
            }
        }
        out.println(clean ? "{\"protocol\":1,\"status\":\"closed\"}"
                : "{\"protocol\":1,\"status\":\"close-failed\"}");
        out.flush();
        return clean;
    }

    static void validateMountpoint(Path mountpoint, boolean fenced) throws IOException {
        if (!Files.isDirectory(mountpoint, LinkOption.NOFOLLOW_LINKS)) throw new IOException("Invalid mountpoint");
        if (fenced) {
            // Core proved emptiness through its original held FD after fencing.
            // Listing by path is intentionally impossible with underlying mode 000.
            if (!Files.getPosixFilePermissions(mountpoint, LinkOption.NOFOLLOW_LINKS).isEmpty()
                    || !Files.getOwner(mountpoint, LinkOption.NOFOLLOW_LINKS).getName().equals(System.getProperty("user.name"))) {
                throw new IOException("Fenced mountpoint must be mode 000 and owned by this user");
            }
        } else {
            try (var entries = Files.list(mountpoint)) {
                if (entries.findAny().isPresent()) throw new IOException("Mountpoint must be empty");
            }
        }
    }

    private static void mount(Path vault, Path mountpoint, char[] password, boolean readonly, boolean fenced) throws Exception {
        validateMountpoint(mountpoint, fenced);
        var fs = CryptoFileSystemProvider.newFileSystem(vault, properties(vault, password, readonly));
        try {
            var service = mountService();
            var builder = service.forFileSystem(fs.getPath("/"));
            builder.setMountpoint(mountpoint);
            String flags = transportFlags(System.getProperty("os.name"), System.getProperty("os.version"), service.getDefaultMountFlags());
            // Do not use auto_unmount: a backend crash must retain discoverable recovery state.
            flags = flags.replace("-oauto_unmount", "").trim();
            if (readonly && service.hasCapability(MountCapability.READ_ONLY)) builder.setReadOnly(true);
            if (readonly) flags += " -oro";
            builder.setMountFlags(flags);
            final Mount mount;
            try { mount = builder.mount(); }
            catch (MountFailedException error) {
                if (providerSuffix(System.getProperty("os.name")).equals(".FuseTMountProvider")) {
                    throw new PrerequisiteException("FUSE-T FSKit mount failed. " + MAC_RUNTIME_HELP);
                }
                throw error;
            }
            var finished = new CountDownLatch(1);
            Runtime.getRuntime().addShutdownHook(new Thread(() -> {
                try {
                    // No unmountForced(), even on shutdown failure. The supervisor
                    // accepts SIGTERM only after the explicit successful close ACK.
                    closeResources(mount, fs, System.out, System.err);
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
