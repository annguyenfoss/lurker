/* SPDX-License-Identifier: GPL-2.0-only */
/* OpenBSD FUSE result boundary; Linux errno values stay internal to LKL.
 * Symbol names are derived from the pinned LKL generated errno UAPI headers.
 */
#ifndef LURKER_LKL_OPENBSD_ERRNO_H
#define LURKER_LKL_OPENBSD_ERRNO_H
#include <errno.h>
#include <stdio.h>
#include <lkl.h>

static long ob_lkl_result(long result, int attribute)
{
    if (result >= 0)
        return result;
    if (attribute && result == -LKL_ENODATA)
        return -ENOATTR;
#if defined(EPERM) && defined(LKL_EPERM)
    if (result == -LKL_EPERM) return -EPERM;
#endif
#if defined(ENOENT) && defined(LKL_ENOENT)
    if (result == -LKL_ENOENT) return -ENOENT;
#endif
#if defined(ESRCH) && defined(LKL_ESRCH)
    if (result == -LKL_ESRCH) return -ESRCH;
#endif
#if defined(EINTR) && defined(LKL_EINTR)
    if (result == -LKL_EINTR) return -EINTR;
#endif
#if defined(EIO) && defined(LKL_EIO)
    if (result == -LKL_EIO) return -EIO;
#endif
#if defined(ENXIO) && defined(LKL_ENXIO)
    if (result == -LKL_ENXIO) return -ENXIO;
#endif
#if defined(E2BIG) && defined(LKL_E2BIG)
    if (result == -LKL_E2BIG) return -E2BIG;
#endif
#if defined(ENOEXEC) && defined(LKL_ENOEXEC)
    if (result == -LKL_ENOEXEC) return -ENOEXEC;
#endif
#if defined(EBADF) && defined(LKL_EBADF)
    if (result == -LKL_EBADF) return -EBADF;
#endif
#if defined(ECHILD) && defined(LKL_ECHILD)
    if (result == -LKL_ECHILD) return -ECHILD;
#endif
#if defined(EAGAIN) && defined(LKL_EAGAIN)
    if (result == -LKL_EAGAIN) return -EAGAIN;
#endif
#if defined(ENOMEM) && defined(LKL_ENOMEM)
    if (result == -LKL_ENOMEM) return -ENOMEM;
#endif
#if defined(EACCES) && defined(LKL_EACCES)
    if (result == -LKL_EACCES) return -EACCES;
#endif
#if defined(EFAULT) && defined(LKL_EFAULT)
    if (result == -LKL_EFAULT) return -EFAULT;
#endif
#if defined(ENOTBLK) && defined(LKL_ENOTBLK)
    if (result == -LKL_ENOTBLK) return -ENOTBLK;
#endif
#if defined(EBUSY) && defined(LKL_EBUSY)
    if (result == -LKL_EBUSY) return -EBUSY;
#endif
#if defined(EEXIST) && defined(LKL_EEXIST)
    if (result == -LKL_EEXIST) return -EEXIST;
#endif
#if defined(EXDEV) && defined(LKL_EXDEV)
    if (result == -LKL_EXDEV) return -EXDEV;
#endif
#if defined(ENODEV) && defined(LKL_ENODEV)
    if (result == -LKL_ENODEV) return -ENODEV;
#endif
#if defined(ENOTDIR) && defined(LKL_ENOTDIR)
    if (result == -LKL_ENOTDIR) return -ENOTDIR;
#endif
#if defined(EISDIR) && defined(LKL_EISDIR)
    if (result == -LKL_EISDIR) return -EISDIR;
#endif
#if defined(EINVAL) && defined(LKL_EINVAL)
    if (result == -LKL_EINVAL) return -EINVAL;
#endif
#if defined(ENFILE) && defined(LKL_ENFILE)
    if (result == -LKL_ENFILE) return -ENFILE;
#endif
#if defined(EMFILE) && defined(LKL_EMFILE)
    if (result == -LKL_EMFILE) return -EMFILE;
#endif
#if defined(ENOTTY) && defined(LKL_ENOTTY)
    if (result == -LKL_ENOTTY) return -ENOTTY;
#endif
#if defined(ETXTBSY) && defined(LKL_ETXTBSY)
    if (result == -LKL_ETXTBSY) return -ETXTBSY;
#endif
#if defined(EFBIG) && defined(LKL_EFBIG)
    if (result == -LKL_EFBIG) return -EFBIG;
#endif
#if defined(ENOSPC) && defined(LKL_ENOSPC)
    if (result == -LKL_ENOSPC) return -ENOSPC;
#endif
#if defined(ESPIPE) && defined(LKL_ESPIPE)
    if (result == -LKL_ESPIPE) return -ESPIPE;
#endif
#if defined(EROFS) && defined(LKL_EROFS)
    if (result == -LKL_EROFS) return -EROFS;
#endif
#if defined(EMLINK) && defined(LKL_EMLINK)
    if (result == -LKL_EMLINK) return -EMLINK;
#endif
#if defined(EPIPE) && defined(LKL_EPIPE)
    if (result == -LKL_EPIPE) return -EPIPE;
#endif
#if defined(EDOM) && defined(LKL_EDOM)
    if (result == -LKL_EDOM) return -EDOM;
#endif
#if defined(ERANGE) && defined(LKL_ERANGE)
    if (result == -LKL_ERANGE) return -ERANGE;
#endif
#if defined(EDEADLK) && defined(LKL_EDEADLK)
    if (result == -LKL_EDEADLK) return -EDEADLK;
#endif
#if defined(ENAMETOOLONG) && defined(LKL_ENAMETOOLONG)
    if (result == -LKL_ENAMETOOLONG) return -ENAMETOOLONG;
#endif
#if defined(ENOLCK) && defined(LKL_ENOLCK)
    if (result == -LKL_ENOLCK) return -ENOLCK;
#endif
#if defined(ENOSYS) && defined(LKL_ENOSYS)
    if (result == -LKL_ENOSYS) return -ENOSYS;
#endif
#if defined(ENOTEMPTY) && defined(LKL_ENOTEMPTY)
    if (result == -LKL_ENOTEMPTY) return -ENOTEMPTY;
#endif
#if defined(ELOOP) && defined(LKL_ELOOP)
    if (result == -LKL_ELOOP) return -ELOOP;
#endif
#if defined(ENOMSG) && defined(LKL_ENOMSG)
    if (result == -LKL_ENOMSG) return -ENOMSG;
#endif
#if defined(EIDRM) && defined(LKL_EIDRM)
    if (result == -LKL_EIDRM) return -EIDRM;
#endif
#if defined(ECHRNG) && defined(LKL_ECHRNG)
    if (result == -LKL_ECHRNG) return -ECHRNG;
#endif
#if defined(EL2NSYNC) && defined(LKL_EL2NSYNC)
    if (result == -LKL_EL2NSYNC) return -EL2NSYNC;
#endif
#if defined(EL3HLT) && defined(LKL_EL3HLT)
    if (result == -LKL_EL3HLT) return -EL3HLT;
#endif
#if defined(EL3RST) && defined(LKL_EL3RST)
    if (result == -LKL_EL3RST) return -EL3RST;
#endif
#if defined(ELNRNG) && defined(LKL_ELNRNG)
    if (result == -LKL_ELNRNG) return -ELNRNG;
#endif
#if defined(EUNATCH) && defined(LKL_EUNATCH)
    if (result == -LKL_EUNATCH) return -EUNATCH;
#endif
#if defined(ENOCSI) && defined(LKL_ENOCSI)
    if (result == -LKL_ENOCSI) return -ENOCSI;
#endif
#if defined(EL2HLT) && defined(LKL_EL2HLT)
    if (result == -LKL_EL2HLT) return -EL2HLT;
#endif
#if defined(EBADE) && defined(LKL_EBADE)
    if (result == -LKL_EBADE) return -EBADE;
#endif
#if defined(EBADR) && defined(LKL_EBADR)
    if (result == -LKL_EBADR) return -EBADR;
#endif
#if defined(EXFULL) && defined(LKL_EXFULL)
    if (result == -LKL_EXFULL) return -EXFULL;
#endif
#if defined(ENOANO) && defined(LKL_ENOANO)
    if (result == -LKL_ENOANO) return -ENOANO;
#endif
#if defined(EBADRQC) && defined(LKL_EBADRQC)
    if (result == -LKL_EBADRQC) return -EBADRQC;
#endif
#if defined(EBADSLT) && defined(LKL_EBADSLT)
    if (result == -LKL_EBADSLT) return -EBADSLT;
#endif
#if defined(EBFONT) && defined(LKL_EBFONT)
    if (result == -LKL_EBFONT) return -EBFONT;
#endif
#if defined(ENOSTR) && defined(LKL_ENOSTR)
    if (result == -LKL_ENOSTR) return -ENOSTR;
#endif
#if defined(ENODATA) && defined(LKL_ENODATA)
    if (result == -LKL_ENODATA) return -ENODATA;
#endif
#if defined(ETIME) && defined(LKL_ETIME)
    if (result == -LKL_ETIME) return -ETIME;
#endif
#if defined(ENOSR) && defined(LKL_ENOSR)
    if (result == -LKL_ENOSR) return -ENOSR;
#endif
#if defined(ENONET) && defined(LKL_ENONET)
    if (result == -LKL_ENONET) return -ENONET;
#endif
#if defined(ENOPKG) && defined(LKL_ENOPKG)
    if (result == -LKL_ENOPKG) return -ENOPKG;
#endif
#if defined(EREMOTE) && defined(LKL_EREMOTE)
    if (result == -LKL_EREMOTE) return -EREMOTE;
#endif
#if defined(ENOLINK) && defined(LKL_ENOLINK)
    if (result == -LKL_ENOLINK) return -ENOLINK;
#endif
#if defined(EADV) && defined(LKL_EADV)
    if (result == -LKL_EADV) return -EADV;
#endif
#if defined(ESRMNT) && defined(LKL_ESRMNT)
    if (result == -LKL_ESRMNT) return -ESRMNT;
#endif
#if defined(ECOMM) && defined(LKL_ECOMM)
    if (result == -LKL_ECOMM) return -ECOMM;
#endif
#if defined(EPROTO) && defined(LKL_EPROTO)
    if (result == -LKL_EPROTO) return -EPROTO;
#endif
#if defined(EMULTIHOP) && defined(LKL_EMULTIHOP)
    if (result == -LKL_EMULTIHOP) return -EMULTIHOP;
#endif
#if defined(EDOTDOT) && defined(LKL_EDOTDOT)
    if (result == -LKL_EDOTDOT) return -EDOTDOT;
#endif
#if defined(EBADMSG) && defined(LKL_EBADMSG)
    if (result == -LKL_EBADMSG) return -EBADMSG;
#endif
#if defined(EOVERFLOW) && defined(LKL_EOVERFLOW)
    if (result == -LKL_EOVERFLOW) return -EOVERFLOW;
#endif
#if defined(ENOTUNIQ) && defined(LKL_ENOTUNIQ)
    if (result == -LKL_ENOTUNIQ) return -ENOTUNIQ;
#endif
#if defined(EBADFD) && defined(LKL_EBADFD)
    if (result == -LKL_EBADFD) return -EBADFD;
#endif
#if defined(EREMCHG) && defined(LKL_EREMCHG)
    if (result == -LKL_EREMCHG) return -EREMCHG;
#endif
#if defined(ELIBACC) && defined(LKL_ELIBACC)
    if (result == -LKL_ELIBACC) return -ELIBACC;
#endif
#if defined(ELIBBAD) && defined(LKL_ELIBBAD)
    if (result == -LKL_ELIBBAD) return -ELIBBAD;
#endif
#if defined(ELIBSCN) && defined(LKL_ELIBSCN)
    if (result == -LKL_ELIBSCN) return -ELIBSCN;
#endif
#if defined(ELIBMAX) && defined(LKL_ELIBMAX)
    if (result == -LKL_ELIBMAX) return -ELIBMAX;
#endif
#if defined(ELIBEXEC) && defined(LKL_ELIBEXEC)
    if (result == -LKL_ELIBEXEC) return -ELIBEXEC;
#endif
#if defined(EILSEQ) && defined(LKL_EILSEQ)
    if (result == -LKL_EILSEQ) return -EILSEQ;
#endif
#if defined(ERESTART) && defined(LKL_ERESTART)
    if (result == -LKL_ERESTART) return -ERESTART;
#endif
#if defined(ESTRPIPE) && defined(LKL_ESTRPIPE)
    if (result == -LKL_ESTRPIPE) return -ESTRPIPE;
#endif
#if defined(EUSERS) && defined(LKL_EUSERS)
    if (result == -LKL_EUSERS) return -EUSERS;
#endif
#if defined(ENOTSOCK) && defined(LKL_ENOTSOCK)
    if (result == -LKL_ENOTSOCK) return -ENOTSOCK;
#endif
#if defined(EDESTADDRREQ) && defined(LKL_EDESTADDRREQ)
    if (result == -LKL_EDESTADDRREQ) return -EDESTADDRREQ;
#endif
#if defined(EMSGSIZE) && defined(LKL_EMSGSIZE)
    if (result == -LKL_EMSGSIZE) return -EMSGSIZE;
#endif
#if defined(EPROTOTYPE) && defined(LKL_EPROTOTYPE)
    if (result == -LKL_EPROTOTYPE) return -EPROTOTYPE;
#endif
#if defined(ENOPROTOOPT) && defined(LKL_ENOPROTOOPT)
    if (result == -LKL_ENOPROTOOPT) return -ENOPROTOOPT;
#endif
#if defined(EPROTONOSUPPORT) && defined(LKL_EPROTONOSUPPORT)
    if (result == -LKL_EPROTONOSUPPORT) return -EPROTONOSUPPORT;
#endif
#if defined(ESOCKTNOSUPPORT) && defined(LKL_ESOCKTNOSUPPORT)
    if (result == -LKL_ESOCKTNOSUPPORT) return -ESOCKTNOSUPPORT;
#endif
#if defined(EOPNOTSUPP) && defined(LKL_EOPNOTSUPP)
    if (result == -LKL_EOPNOTSUPP) return -EOPNOTSUPP;
#endif
#if defined(EPFNOSUPPORT) && defined(LKL_EPFNOSUPPORT)
    if (result == -LKL_EPFNOSUPPORT) return -EPFNOSUPPORT;
#endif
#if defined(EAFNOSUPPORT) && defined(LKL_EAFNOSUPPORT)
    if (result == -LKL_EAFNOSUPPORT) return -EAFNOSUPPORT;
#endif
#if defined(EADDRINUSE) && defined(LKL_EADDRINUSE)
    if (result == -LKL_EADDRINUSE) return -EADDRINUSE;
#endif
#if defined(EADDRNOTAVAIL) && defined(LKL_EADDRNOTAVAIL)
    if (result == -LKL_EADDRNOTAVAIL) return -EADDRNOTAVAIL;
#endif
#if defined(ENETDOWN) && defined(LKL_ENETDOWN)
    if (result == -LKL_ENETDOWN) return -ENETDOWN;
#endif
#if defined(ENETUNREACH) && defined(LKL_ENETUNREACH)
    if (result == -LKL_ENETUNREACH) return -ENETUNREACH;
#endif
#if defined(ENETRESET) && defined(LKL_ENETRESET)
    if (result == -LKL_ENETRESET) return -ENETRESET;
#endif
#if defined(ECONNABORTED) && defined(LKL_ECONNABORTED)
    if (result == -LKL_ECONNABORTED) return -ECONNABORTED;
#endif
#if defined(ECONNRESET) && defined(LKL_ECONNRESET)
    if (result == -LKL_ECONNRESET) return -ECONNRESET;
#endif
#if defined(ENOBUFS) && defined(LKL_ENOBUFS)
    if (result == -LKL_ENOBUFS) return -ENOBUFS;
#endif
#if defined(EISCONN) && defined(LKL_EISCONN)
    if (result == -LKL_EISCONN) return -EISCONN;
#endif
#if defined(ENOTCONN) && defined(LKL_ENOTCONN)
    if (result == -LKL_ENOTCONN) return -ENOTCONN;
#endif
#if defined(ESHUTDOWN) && defined(LKL_ESHUTDOWN)
    if (result == -LKL_ESHUTDOWN) return -ESHUTDOWN;
#endif
#if defined(ETOOMANYREFS) && defined(LKL_ETOOMANYREFS)
    if (result == -LKL_ETOOMANYREFS) return -ETOOMANYREFS;
#endif
#if defined(ETIMEDOUT) && defined(LKL_ETIMEDOUT)
    if (result == -LKL_ETIMEDOUT) return -ETIMEDOUT;
#endif
#if defined(ECONNREFUSED) && defined(LKL_ECONNREFUSED)
    if (result == -LKL_ECONNREFUSED) return -ECONNREFUSED;
#endif
#if defined(EHOSTDOWN) && defined(LKL_EHOSTDOWN)
    if (result == -LKL_EHOSTDOWN) return -EHOSTDOWN;
#endif
#if defined(EHOSTUNREACH) && defined(LKL_EHOSTUNREACH)
    if (result == -LKL_EHOSTUNREACH) return -EHOSTUNREACH;
#endif
#if defined(EALREADY) && defined(LKL_EALREADY)
    if (result == -LKL_EALREADY) return -EALREADY;
#endif
#if defined(EINPROGRESS) && defined(LKL_EINPROGRESS)
    if (result == -LKL_EINPROGRESS) return -EINPROGRESS;
#endif
#if defined(ESTALE) && defined(LKL_ESTALE)
    if (result == -LKL_ESTALE) return -ESTALE;
#endif
#if defined(EUCLEAN) && defined(LKL_EUCLEAN)
    if (result == -LKL_EUCLEAN) return -EUCLEAN;
#endif
#if defined(ENOTNAM) && defined(LKL_ENOTNAM)
    if (result == -LKL_ENOTNAM) return -ENOTNAM;
#endif
#if defined(ENAVAIL) && defined(LKL_ENAVAIL)
    if (result == -LKL_ENAVAIL) return -ENAVAIL;
#endif
#if defined(EISNAM) && defined(LKL_EISNAM)
    if (result == -LKL_EISNAM) return -EISNAM;
#endif
#if defined(EREMOTEIO) && defined(LKL_EREMOTEIO)
    if (result == -LKL_EREMOTEIO) return -EREMOTEIO;
#endif
#if defined(EDQUOT) && defined(LKL_EDQUOT)
    if (result == -LKL_EDQUOT) return -EDQUOT;
#endif
#if defined(ENOMEDIUM) && defined(LKL_ENOMEDIUM)
    if (result == -LKL_ENOMEDIUM) return -ENOMEDIUM;
#endif
#if defined(EMEDIUMTYPE) && defined(LKL_EMEDIUMTYPE)
    if (result == -LKL_EMEDIUMTYPE) return -EMEDIUMTYPE;
#endif
#if defined(ECANCELED) && defined(LKL_ECANCELED)
    if (result == -LKL_ECANCELED) return -ECANCELED;
#endif
#if defined(ENOKEY) && defined(LKL_ENOKEY)
    if (result == -LKL_ENOKEY) return -ENOKEY;
#endif
#if defined(EKEYEXPIRED) && defined(LKL_EKEYEXPIRED)
    if (result == -LKL_EKEYEXPIRED) return -EKEYEXPIRED;
#endif
#if defined(EKEYREVOKED) && defined(LKL_EKEYREVOKED)
    if (result == -LKL_EKEYREVOKED) return -EKEYREVOKED;
#endif
#if defined(EKEYREJECTED) && defined(LKL_EKEYREJECTED)
    if (result == -LKL_EKEYREJECTED) return -EKEYREJECTED;
#endif
#if defined(EOWNERDEAD) && defined(LKL_EOWNERDEAD)
    if (result == -LKL_EOWNERDEAD) return -EOWNERDEAD;
#endif
#if defined(ENOTRECOVERABLE) && defined(LKL_ENOTRECOVERABLE)
    if (result == -LKL_ENOTRECOVERABLE) return -ENOTRECOVERABLE;
#endif
#if defined(ERFKILL) && defined(LKL_ERFKILL)
    if (result == -LKL_ERFKILL) return -ERFKILL;
#endif
#if defined(EHWPOISON) && defined(LKL_EHWPOISON)
    if (result == -LKL_EHWPOISON) return -EHWPOISON;
#endif
    fprintf(stderr, "lklfuse: unmapped Linux errno result %ld (attribute=%d)\n", result, attribute);
    return -EIO;
}
#endif
