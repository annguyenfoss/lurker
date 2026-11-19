// Standalone native regression linked against the patched CryFS build.
// Run only on a dedicated remote host and an explicitly supplied private /tmp.
#include <blockstore/implementations/ondisk/OnDiskBlockStore2.h>
#include <cpp-utils/io/DurableFile.h>
#include <cryfs/impl/localstate/LocalStateMetadata.h>
#include <boost/filesystem.hpp>
#include <sys/resource.h>
#include <sys/stat.h>
#include <sys/utsname.h>
#include <sys/vfs.h>
#include <fcntl.h>
#include <signal.h>
#include <unistd.h>
#include <cerrno>
#include <cstring>
#include <iostream>
#include <stdexcept>
#include <system_error>
#include <vector>
#include <thread>
#include <atomic>
#include <sstream>
#include <iomanip>

namespace bf = boost::filesystem;
using blockstore::ondisk::OnDiskBlockStore2;
using cpputils::Data;

static void require(bool condition, const char* message) {
  if (!condition) throw std::runtime_error(message);
}

static Data filled(size_t size, unsigned char value) {
  Data data(size);
  std::memset(data.data(), value, size);
  return data;
}

int main(int argc, char** argv) {
  try {
    struct utsname host{};
    require(::uname(&host) == 0 && std::string(host.nodename) != "endeavour", "Use a dedicated native test host");
    require(argc == 2 || (argc == 3 && std::string(argv[2]) == "--finite-tmpfs"), "Usage: durability-test PRIVATE_TMP_DIRECTORY [--finite-tmpfs]");
    const bf::path root = bf::canonical(argv[1]);
    require(root.string().find("/tmp/lurker-cryfs-durable-") == 0, "Private /tmp fixture required");
    struct stat metadata{};
    require(::stat(root.c_str(), &metadata) == 0 && metadata.st_uid == ::geteuid() && !(metadata.st_mode & 0077), "Fixture must belong privately to this uid");
    const auto base = root / "blocks";
    require(bf::create_directory(base), "Fresh fixture required");
    OnDiskBlockStore2 store(base);
    const auto old = filled(4096, 0x5a);
    const auto id = store.create(old);
    require(!store.tryCreate(id, filled(2048, 0xa5)), "tryCreate overwrote an existing block");
    require(*store.load(id) == old, "No-clobber changed the original block");
    const auto replacement = filled(8192, 0x33);
    store.store(id, replacement);
    require(*store.load(id) == replacement, "Atomic replacement did not publish");

    const auto localState = root / "state" / "vault";
    cpputils::durablefile::createDirectories(localState);
    const auto metadataPath = localState / "metadata";
    const auto key = filled(32, 0x19);
    const auto client = cryfs::LocalStateMetadata::loadOrGenerate(localState, key, false).myClientId();
    const auto savedMetadata = Data::LoadFromFile(metadataPath);
    require(static_cast<bool>(savedMetadata), "CryFS metadata publication was missing");
    require(cryfs::LocalStateMetadata::loadOrGenerate(localState, key, false).myClientId() == client,
            "Existing CryFS client identity changed");
    bool rejected = false;
    try { cryfs::LocalStateMetadata::loadOrGenerate(localState, filled(32, 0xe1), false); }
    catch (const std::exception&) { rejected = true; }
    require(rejected && *Data::LoadFromFile(metadataPath) == *savedMetadata,
            "A different key changed existing CryFS metadata");

    // Real kernel short-write failure, with no host filesystem fill.
    struct rlimit previous{};
    require(::getrlimit(RLIMIT_FSIZE, &previous) == 0, "getrlimit failed");
    struct sigaction ignored{}, oldAction{};
    ignored.sa_handler = SIG_IGN;
    ::sigemptyset(&ignored.sa_mask);
    require(::sigaction(SIGXFSZ, &ignored, &oldAction) == 0, "sigaction failed");
    struct rlimit limited = previous;
    limited.rlim_cur = 128;
    require(::setrlimit(RLIMIT_FSIZE, &limited) == 0, "setrlimit failed");
    bool failed = false;
    try { store.store(id, filled(16384, 0x77)); }
    catch (const std::system_error& error) { failed = error.code().value() == EFBIG; }
    const auto attemptedMetadata = filled(4096, 0x21);
    bool metadataFailed = false;
    try { cpputils::durablefile::store(metadataPath, attemptedMetadata.data(), attemptedMetadata.size()); }
    catch (const std::system_error& error) { metadataFailed = error.code().value() == EFBIG; }
    require(::setrlimit(RLIMIT_FSIZE, &previous) == 0, "restore rlimit failed");
    require(::sigaction(SIGXFSZ, &oldAction, nullptr) == 0, "restore sigaction failed");
    require(failed && *store.load(id) == replacement, "Failed write destroyed committed ciphertext or hid EFBIG");
    require(store.numBlocks() == 1, "Failed write left a temporary block behind");
    require(metadataFailed && *Data::LoadFromFile(metadataPath) == *savedMetadata,
            "Failed write damaged committed CryFS metadata or hid EFBIG");
    require(std::distance(bf::directory_iterator(localState), bf::directory_iterator()) == 1,
            "Failed metadata write left a temporary behind");
    const auto invalidMetadata = filled(4, 0x3f);
    cpputils::durablefile::store(metadataPath, invalidMetadata.data(), invalidMetadata.size());
    rejected = false;
    try { cryfs::LocalStateMetadata::loadOrGenerate(localState, key, false); }
    catch (const std::exception&) { rejected = true; }
    require(rejected && *Data::LoadFromFile(metadataPath) == invalidMetadata,
            "Malformed CryFS metadata silently reset");

    // Distinct encrypted blocks can share a prefix directory. Namespace
    // durability must tolerate concurrent creation/removal in that directory.
    std::atomic<unsigned int> errors{0};
    std::vector<std::thread> workers;
    for (unsigned int worker = 1; worker <= 8; ++worker) {
      workers.emplace_back([&, worker] {
        std::ostringstream name;
        name << "001" << std::hex << std::setw(29) << std::setfill('0') << worker;
        const auto concurrentId = blockstore::BlockId::FromString(name.str());
        try {
          for (unsigned int iteration = 0; iteration < 20; ++iteration) {
            require(store.tryCreate(concurrentId, filled(256, worker)), "Concurrent create failed");
            require(store.remove(concurrentId), "Concurrent remove failed");
          }
        } catch (const std::exception&) { ++errors; }
      });
    }
    for (auto& worker : workers) worker.join();
    require(errors == 0 && store.numBlocks() == 1, "Concurrent prefix-directory mutation failed");

    if (argc == 3) {
      // Refuse to fill an ordinary host filesystem. A tiny administrator-created
      // tmpfs mounted at this private fixture is the only accepted ENOSPC target.
      struct statfs info{};
      require(::statfs(root.c_str(), &info) == 0 && info.f_type == 0x01021994 &&
              static_cast<unsigned long long>(info.f_blocks) * info.f_bsize <= 16 * 1024 * 1024,
              "ENOSPC test requires a tmpfs of at most 16 MiB");
      const auto filler = root / "owned-space-filler";
      const int descriptor = ::open(filler.c_str(), O_WRONLY | O_CREAT | O_EXCL | O_CLOEXEC, 0600);
      require(descriptor >= 0, "Cannot create tmpfs filler");
      const std::vector<char> zeros(65536, 0);
      ssize_t written;
      do { written = ::write(descriptor, zeros.data(), zeros.size()); } while (written > 0 || (written < 0 && errno == EINTR));
      const int fillError = errno;
      ::close(descriptor);
      require(written < 0 && fillError == ENOSPC, "Finite tmpfs did not report ENOSPC");
      failed = false;
      try { store.store(id, filled(16384, 0x99)); }
      catch (const std::system_error& error) { failed = error.code().value() == ENOSPC; }
      require(failed && *store.load(id) == replacement, "ENOSPC destroyed committed ciphertext or was hidden");
      require(store.numBlocks() == 1, "ENOSPC left a temporary block behind");
      require(::unlink(filler.c_str()) == 0, "Cannot remove owned tmpfs filler");
    }
    std::cout << "{\"status\":\"passed\",\"checks\":[\"no-clobber\",\"atomic-replace\",\"short-write-retains-committed-block\",\"temporary-cleanup\",\"concurrent-prefix-mutations\",\"client-identity\",\"key-mismatch-refused\",\"short-write-retains-metadata\",\"malformed-metadata-refused\"],\"enospc\":"
              << (argc == 3 ? "true" : "false") << "}\n";
    return 0;
  } catch (const std::exception& error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
