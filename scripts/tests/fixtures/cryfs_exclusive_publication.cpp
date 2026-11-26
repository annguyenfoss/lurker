// Native controls for the reviewed Linux CryFS DurableFile header.
#include <algorithm>
#include <boost/filesystem.hpp>
#include <cerrno>
#include <fcntl.h>
#include <fstream>
#include <iostream>
#include <limits>
#include <linux/fs.h>
#include <stdexcept>
#include <string>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <system_error>
#include <unistd.h>
#include <vector>

static int forced_error = 0;
static int link_calls = 0;
static std::vector<std::string> events;

static int publication_fsync(int fd) {
    struct stat metadata{};
    if (::fstat(fd, &metadata) < 0) return -1;
    const int result = ::fsync(fd);
    const int error = errno;
    events.push_back(std::string(S_ISDIR(metadata.st_mode) ? "fsync-directory:" : "fsync-file:") + std::to_string(result));
    errno = error;
    return result;
}
static long publication_syscall(long number, int old_directory, const char* old_path,
                                int new_directory, const char* new_path, unsigned flags) {
    long result;
    if (forced_error) { errno = forced_error; result = -1; }
    else result = ::syscall(number, old_directory, old_path, new_directory, new_path, flags);
    const int error = errno;
    events.push_back("rename-noreplace:" + std::to_string(result == 0 ? 0 : error));
    errno = error;
    return result;
}
static int publication_link(const char* source, const char* target) {
    ++link_calls;
    const int result = ::link(source, target);
    const int error = errno;
    events.push_back("link:" + std::to_string(result == 0 ? 0 : error));
    errno = error;
    return result;
}
static int publication_rename(const char* source, const char* target) {
    const int result = ::rename(source, target);
    const int error = errno;
    events.push_back("rename-replace:" + std::to_string(result == 0 ? 0 : error));
    errno = error;
    return result;
}
#define fsync publication_fsync
#define syscall publication_syscall
#define link publication_link
#define rename publication_rename
#include "src/cpp-utils/io/DurableFile.h"
#undef fsync
#undef syscall
#undef link
#undef rename

using cpputils::durablefile::Publication;
static void require(bool value, const char* message) {
    if (!value) throw std::runtime_error(message);
}
static std::string read(const boost::filesystem::path& file) {
    std::ifstream stream(file.string(), std::ios::binary);
    require(stream.good(), "published file is unreadable");
    return std::string(std::istreambuf_iterator<char>(stream), std::istreambuf_iterator<char>());
}
static bool store(const boost::filesystem::path& file, const std::string& data, Publication mode) {
    return cpputils::durablefile::store(file, data.data(), data.size(), mode);
}
static void durable_publications() {
    for (size_t index = 0; index < events.size(); ++index) {
        if (events[index] != "rename-noreplace:0" && events[index] != "rename-replace:0" && events[index] != "link:0") continue;
        require(index > 0 && events[index - 1] == "fsync-file:0", "publication did not follow successful file fsync");
        require(index + 1 < events.size() && events[index + 1] == "fsync-directory:0", "publication did not precede successful directory fsync");
    }
}
static void normal(const boost::filesystem::path& root) {
    const auto file = root / "publication";
    require(store(file, "initial", Publication::NewFile), "absent target not published");
    require(!store(file, "collision", Publication::NewFile), "collision overwrote target");
    require(read(file) == "initial", "collision changed existing bytes");
    require(store(file, "updated", Publication::Replace), "ordinary replacement failed");
    require(read(file) == "updated", "replacement bytes differ");
    require(link_calls == 0, "supported rename unexpectedly used hardlink fallback");
    durable_publications();
}
static void race(const boost::filesystem::path& root) {
    std::vector<pid_t> children;
    for (int writer = 0; writer < 8; ++writer) {
        const auto pid = ::fork();
        require(pid >= 0, "cannot fork bounded creator race");
        if (pid == 0) {
            events.clear();
            try {
                const bool won = store(root / "race", "writer-" + std::to_string(writer), Publication::NewFile);
                if (won) durable_publications();
                ::_exit(won ? 1 : 0);
            } catch (...) { ::_exit(2); }
        }
        children.push_back(pid);
    }
    int winners = 0;
    for (const auto pid : children) {
        int status;
        while (::waitpid(pid, &status, 0) < 0) require(errno == EINTR, "cannot reap creator");
        require(WIFEXITED(status) && WEXITSTATUS(status) <= 1, "creator race failed");
        winners += WEXITSTATUS(status);
    }
    require(winners == 1, "creator race did not retain exactly one winner");
    require(read(root / "race").rfind("writer-", 0) == 0, "race winner bytes differ");
}
static void error_case(const boost::filesystem::path& root, int error) {
    const auto file = root / "publication";
    if (error == EEXIST) { std::ofstream(file.string()) << "winner"; }
    forced_error = error;
    const bool unsupported = error == ENOSYS || error == EINVAL || error == EOPNOTSUPP;
    try {
        const bool created = store(file, "candidate", Publication::NewFile);
        if (unsupported) {
            require(created && read(file) == "candidate" && link_calls == 1, "unsupported primitive fallback failed");
            // A failed rename precedes the hardlink; validate the same fsync boundaries.
            require(events.front() == "fsync-file:0" && events.back() == "fsync-directory:0", "fallback durability boundaries missing");
        } else {
            require(error == EEXIST && !created && read(file) == "winner" && link_calls == 0, "collision did not preserve winner");
        }
    } catch (const std::system_error& failure) {
        require(!unsupported && error != EEXIST && failure.code().value() == error, "unexpected publication error");
        require(link_calls == 0 && !boost::filesystem::exists(file), "non-capability error attempted fallback or publication");
    }
}
int main(int argc, char** argv) {
    try {
        require(argc >= 3, "usage: control MODE FIXTURE [ERRNO]");
        const std::string mode = argv[1];
        const boost::filesystem::path root = argv[2];
        require(boost::filesystem::is_directory(root), "fixture directory missing");
        if (mode == "normal") normal(root);
        else if (mode == "race") race(root);
        else if (mode == "error") { require(argc == 4, "errno missing"); error_case(root, std::stoi(argv[3])); }
        else if (mode == "verify") {
            require(read(root / "publication") == "updated", "reopened publication bytes differ");
            require(read(root / "race").rfind("writer-", 0) == 0, "reopened race bytes differ");
        } else throw std::runtime_error("unknown control mode");
        std::cout << "{\"status\":\"passed\",\"link_calls\":" << link_calls << ",\"events\":[";
        for (size_t i = 0; i < events.size(); ++i) { if (i) std::cout << ','; std::cout << '"' << events[i] << '"'; }
        std::cout << "]}\n";
        return 0;
    } catch (const std::exception& failure) {
        std::cerr << failure.what() << '\n';
        return 1;
    }
}
