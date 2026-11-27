#include <stddef.h>
#include <stdio.h>
#include FUSE_WIRE_HEADER
int main(void)
{
 printf("{\"wire_bytes\":%zu,\"major_offset\":%zu,\"minor_offset\":%zu,\"max_readahead_offset\":%zu,\"flags_offset\":%zu,\"max_background_offset\":%zu,\"congestion_threshold_offset\":%zu,\"max_write_offset\":%zu,\"time_gran_offset\":%zu,\"max_pages_offset\":%zu,\"padding_offset\":%zu,\"unused_offset\":%zu}\n",sizeof(struct fuse_init_out),offsetof(struct fuse_init_out,major),offsetof(struct fuse_init_out,minor),offsetof(struct fuse_init_out,max_readahead),offsetof(struct fuse_init_out,flags),offsetof(struct fuse_init_out,max_background),offsetof(struct fuse_init_out,congestion_threshold),offsetof(struct fuse_init_out,max_write),offsetof(struct fuse_init_out,time_gran),offsetof(struct fuse_init_out,max_pages),offsetof(struct fuse_init_out,padding),offsetof(struct fuse_init_out,unused));return 0;
}
