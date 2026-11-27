/* Native DragonFly FUSE ABI 7.28 audits the complete 64-byte INIT reply even
 * when FUSE2 negotiates minor19. Zero-extend the existing response; preserve
 * its negotiated version/features and advertise no additional capability.
 */
#ifndef FUSE2_DRAGONFLY_INIT_H
#define FUSE2_DRAGONFLY_INIT_H
#include <stddef.h>
struct dragonfly_fuse_init_out {
    struct fuse_init_out prefix;
    __u32 time_gran;
    __u16 max_pages;
    __u16 padding;
    __u32 unused[8];
};
_Static_assert(sizeof(struct fuse_init_out) == 24, "Expected pinned FUSE2 INIT prefix");
_Static_assert(sizeof(struct dragonfly_fuse_init_out) == 64, "Native INIT reply size");
_Static_assert(offsetof(struct dragonfly_fuse_init_out, time_gran) == 24, "Native time_gran offset");
_Static_assert(offsetof(struct dragonfly_fuse_init_out, max_pages) == 28, "Native max_pages offset");
_Static_assert(offsetof(struct dragonfly_fuse_init_out, padding) == 30, "Native padding offset");
_Static_assert(offsetof(struct dragonfly_fuse_init_out, unused) == 32, "Native reserved fields offset");
#endif
