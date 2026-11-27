/* Observe the actual native transport without replacing its receive/send ABI.
 * The qualified client has one serialized FUSE session and one attached channel.
 */
#ifndef LKLFUSE_DRAGONFLY_CHANNEL_H
#define LKLFUSE_DRAGONFLY_CHANNEL_H
#include <fuse_kernel.h>
static int df_transport_error, df_init_received, df_init_pending, df_init_sent;
static uint64_t df_init_unique;
static unsigned df_kernel_major, df_kernel_minor;

static int df_channel_enter(struct fuse_chan *outer, struct fuse_chan *inner,
                             struct fuse_session **session)
{
    *session = fuse_chan_session(outer);
    if (!*session || fuse_chan_session(inner)) {
        df_transport_error = 1; return -EIO;
    }
    fuse_session_remove_chan(outer);
    fuse_session_add_chan(*session, inner);
    return 0;
}
static void df_channel_leave(struct fuse_chan *outer, struct fuse_chan *inner,
                              struct fuse_session *session)
{
    fuse_session_remove_chan(inner);
    fuse_session_add_chan(session, outer);
}
static int df_channel_receive(struct fuse_chan **channel, char *buf, size_t size)
{
    struct fuse_chan *outer = *channel, *inner = fuse_chan_data(outer), *received = inner;
    struct fuse_session *session;
    if (df_channel_enter(outer, inner, &session)) return -EIO;
    int ret = fuse_chan_recv(&received, buf, size);
    df_channel_leave(outer, inner, session);
    *channel = outer;
    if (received != inner) { df_transport_error = 1; return -EIO; }
    if (ret < 0 && ret != -EINTR && ret != -EAGAIN) df_transport_error = 1;
    if (ret >= (int)sizeof(struct fuse_in_header)) {
        struct fuse_in_header header; memcpy(&header, buf, sizeof(header));
        if (header.opcode == FUSE_INIT) {
            if (ret < (int)(sizeof(header) + sizeof(struct fuse_init_in)) ||
                df_init_received++) df_transport_error = 1;
            else {
                struct fuse_init_in init; memcpy(&init, buf+sizeof(header), sizeof(init));
                df_kernel_major = init.major; df_kernel_minor = init.minor;
                if (init.major != 7 || init.minor < 23) df_transport_error = 1;
                df_init_unique = header.unique; df_init_pending = 1;
            }
        }
    }
    return ret;
}
static int df_channel_send(struct fuse_chan *outer, const struct iovec iov[], size_t count)
{
    struct fuse_chan *inner = fuse_chan_data(outer);
    struct fuse_session *session;
    int init_reply = 0, valid = 0;
    if (iov && count && iov[0].iov_len == sizeof(struct fuse_out_header)) {
        struct fuse_out_header header; memcpy(&header, iov[0].iov_base, sizeof(header));
        if (df_init_pending && header.unique == df_init_unique) {
            init_reply = 1;
            if (count == 2 && header.error == 0 && header.len == 80 && iov[1].iov_len == 64) {
                struct fuse_init_out prefix; memcpy(&prefix, iov[1].iov_base, sizeof(prefix));
                valid = sizeof(prefix) == 24 && prefix.major == 7 &&
                    prefix.minor == FUSE_KERNEL_MINOR_VERSION &&
                    !(prefix.flags & ~FUSE_EXPORT_SUPPORT);
                for (size_t i = sizeof(prefix); i < 64; i++)
                    if (((const unsigned char *)iov[1].iov_base)[i]) valid = 0;
                fprintf(stderr, "lklfuse: native INIT request=%u.%u reply=%u.%u flags=%u bytes=64 zero-extension=%d\n",
                    df_kernel_major, df_kernel_minor, prefix.major, prefix.minor, prefix.flags, valid);
            }
        }
    }
    if (df_channel_enter(outer, inner, &session)) return -EIO;
    int ret = fuse_chan_send(inner, iov, count);
    df_channel_leave(outer, inner, session);
    if (ret < 0 && ret != -ENOENT) df_transport_error = 1;
    if (init_reply) {
        df_init_pending = 0;
        df_init_sent = ret == 0 && valid;
        if (!df_init_sent) df_transport_error = 1;
        fprintf(stderr, "lklfuse: native INIT actual-send=%d accepted=%d\n", ret, df_init_sent);
    }
    return ret;
}
static void df_channel_destroy(struct fuse_chan *outer)
{
    fuse_chan_destroy(fuse_chan_data(outer));
}
static struct fuse_chan *df_observed_channel(struct fuse_chan *inner)
{
    struct fuse_chan_ops ops = {.receive=df_channel_receive, .send=df_channel_send,
                                .destroy=df_channel_destroy};
    return fuse_chan_new(&ops, fuse_chan_fd(inner), fuse_chan_bufsize(inner), inner);
}
#endif
