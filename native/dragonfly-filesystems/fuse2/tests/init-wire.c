#include <assert.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#include <fuse_kernel.h>
#include "../dragonfly-init.h"
int main(void)
{
    struct fuse_init_out prefix = {.major=7,.minor=19,.max_readahead=4096,
        .flags=FUSE_EXPORT_SUPPORT,.max_background=0,.congestion_threshold=0,
        .max_write=131072};
    struct dragonfly_fuse_init_out wire = {0};
    wire.prefix = prefix;
    assert(!memcmp(&wire, &prefix, sizeof(prefix)));
    for (size_t i=sizeof(prefix); i<sizeof(wire); i++)
        assert(((const unsigned char *)&wire)[i] == 0);
    printf("{\"prefix_bytes\":%zu,\"wire_bytes\":%zu,\"major\":%u,\"minor\":%u,\"flags\":%u,\"extension_zero_bytes\":%zu,\"major_offset\":%zu,\"minor_offset\":%zu,\"max_readahead_offset\":%zu,\"flags_offset\":%zu,\"max_background_offset\":%zu,\"congestion_threshold_offset\":%zu,\"max_write_offset\":%zu,\"time_gran_offset\":%zu,\"max_pages_offset\":%zu,\"padding_offset\":%zu,\"unused_offset\":%zu}\n",
        sizeof(prefix),sizeof(wire),wire.prefix.major,wire.prefix.minor,wire.prefix.flags,
        sizeof(wire)-sizeof(prefix),offsetof(struct fuse_init_out,major),offsetof(struct fuse_init_out,minor),offsetof(struct fuse_init_out,max_readahead),offsetof(struct fuse_init_out,flags),offsetof(struct fuse_init_out,max_background),offsetof(struct fuse_init_out,congestion_threshold),offsetof(struct fuse_init_out,max_write),offsetof(struct dragonfly_fuse_init_out,time_gran),offsetof(struct dragonfly_fuse_init_out,max_pages),offsetof(struct dragonfly_fuse_init_out,padding),offsetof(struct dragonfly_fuse_init_out,unused));
    return 0;
}
