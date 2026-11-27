/* Host compiler-tool compatibility only; use actual OpenBSD endian primitives. */
#ifndef LURKER_OPENBSD_LKL_BYTESWAP_H
#define LURKER_OPENBSD_LKL_BYTESWAP_H
#include <endian.h>
#define bswap_16(value) swap16(value)
#define bswap_32(value) swap32(value)
#define bswap_64(value) swap64(value)
#endif
