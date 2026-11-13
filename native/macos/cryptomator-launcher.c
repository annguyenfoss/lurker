/* Keep the private Java image in sealed Resources, with native code in Helpers.
 * No PATH lookup, shell interpolation, or credential handling belongs here. */
#include <mach-o/dyld.h>
#include <errno.h>
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

int main(int argc, char **argv) {
    if (argc < 1) return 1;
    char executable[PATH_MAX];
    uint32_t length = sizeof executable;
    if (_NSGetExecutablePath(executable, &length) != 0) {
        fputs("Lurker backend executable path is too long.\n", stderr);
        return 1;
    }
    char *canonical = realpath(executable, NULL);
    if (canonical == NULL) {
        perror("Resolve Lurker backend executable");
        return 1;
    }
    char *separator = strrchr(canonical, '/');
    if (separator == NULL) {
        free(canonical);
        return 1;
    }
    *separator = '\0';
    char launcher[PATH_MAX];
    int size = snprintf(launcher, sizeof launcher,
                        "%s/../Resources/backends/cryptomator/lurker-cryptomator", canonical);
    free(canonical);
    if (size < 0 || (size_t)size >= sizeof launcher) {
        fputs("Lurker private Java path is too long.\n", stderr);
        return 1;
    }
    char **arguments = calloc((size_t)argc + 1, sizeof *arguments);
    if (arguments == NULL) {
        perror("Allocate Lurker backend arguments");
        return 1;
    }
    arguments[0] = launcher;
    for (int i = 1; i < argc; ++i) arguments[i] = argv[i];
    execv(launcher, arguments);
    perror("Start Lurker private Java backend");
    free(arguments);
    return 1;
}
