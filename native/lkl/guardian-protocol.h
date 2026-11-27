/* SPDX-License-Identifier: MIT
 * Host-only guardian protocol v2. This codec performs NO authentication, I/O,
 * process supervision or authorization. A decoded RELEASED_CLEAN record is
 * never sufficient: the consumer must verify the live native peer, fresh
 * request nonce and exact expected binding. Protocol-1 files cannot authorize
 * a protocol-2 close. Integers are unsigned, fixed-width, big-endian bytes.
 */
#ifndef LURKER_LKL_GUARDIAN_PROTOCOL_H
#define LURKER_LKL_GUARDIAN_PROTOCOL_H
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#define LFG_VERSION 2u
#define LFG_FRAME_SIZE 512u
#define LFG_MAX_PID UINT64_C(2147483647)
#define LFG_MAX_OFFSET UINT64_C(9223372036854775807)

enum lfg_kind {
    LFG_HELLO = 1, LFG_BIND = 2, LFG_READY = 3, LFG_DRAINED = 4,
    LFG_ERROR = 5, LFG_QUERY = 16, LFG_CLOSE = 17, LFG_RETIRE = 18,
    LFG_REPLY = 32
};
enum lfg_state {
    LFG_REQUEST = 0, LFG_STARTING = 1, LFG_RUNNING = 2, LFG_DRAINING = 3,
    LFG_WORKER_DRAINED = 4, LFG_RETAINED = 5, LFG_RELEASED_CLEAN = 6,
    LFG_RETIRED = 7
};
enum lfg_wait_kind { LFG_WAIT_UNKNOWN = 0, LFG_WAIT_EXITED = 1,
                     LFG_WAIT_SIGNALED = 2 };
enum lfg_source_kind { LFG_SOURCE_REGULAR = 1, LFG_SOURCE_CHAR_PROVIDER = 2 };
/* These bits describe measured results, never requested policy. */
enum lfg_flags {
    LFG_F_WORKER_EXITED = 1u, LFG_F_WORKER_EXIT_ZERO = 2u,
    LFG_F_WORKER_DRAINED = 4u, LFG_F_NATIVE_CLOSED = 8u,
    LFG_F_CUSTODY_FLUSHED = 16u, LFG_F_CUSTODY_CLOSED = 32u,
    LFG_F_STATUS_SYNCED = 64u
};
#define LFG_F_ALL 127u
/* Stages describe outcomes, not identical native API sequences. BACKEND_RELEASE_OK
 * requires the native binding's documented safe backend release proof; macOS
 * explicitly checks disk removal, while another OS may prove release by its
 * checked halt/source-close lifecycle. No storage check may be skipped.
 * Native bindings must agree on these stage meanings. They may require
 * additional bits only in a future protocol version (unknown bits fail shut). */
enum lfg_checks {
    LFG_C_NATIVE_ABSENT = 1u, LFG_C_FILESYSTEM_SYNC = 2u,
    LFG_C_INNER_UNMOUNT = 4u, LFG_C_BACKING_FLUSH = 8u,
    LFG_C_BACKEND_RELEASE_OK = 16u, LFG_C_HALT = 32u,
    LFG_C_WORKER_BACKING_CLOSED = 64u, LFG_C_CREDENTIALS_RESTORED = 128u,
    LFG_C_NO_IO_ERROR = 256u, LFG_C_SIGNAL_POLICY_OK = 512u
};
#define LFG_C_ALL UINT64_C(1023)

/* Logical native struct; NEVER write sizeof(struct lfg_message) to a socket.
 * native_id is an opaque four-word OS identity, fully interpreted/verified by
 * that binding. filesystem is similarly an OS-binding-owned nonzero tag.
 * birth times and boot_id are captured by native callbacks, not caller input.
 */
struct lfg_message {
    uint16_t kind, state;
    uint32_t flags, error;
    uint64_t required_checks, completed_checks;
    uint8_t session_nonce[32], request_nonce[16], boot_id[16];
    uint64_t uid, guardian_pid, guardian_birth_sec, guardian_birth_usec;
    uint64_t worker_pid, worker_birth_sec, worker_birth_usec;
    uint64_t status_dev, status_ino, source_dev, source_ino, source_size;
    uint64_t source_rdev; /* CHAR_PROVIDER capacity is source_size, not st_size. */
    uint32_t source_kind, filesystem;
    uint64_t native_id[4];
    uint8_t component_sha256[32];
    uint32_t wait_kind, wait_value;
    uint64_t sequence;
    uint32_t error_stage, latched_error;
    uint32_t source_logical_sector, source_physical_sector;
    /* CHAR physical=0 means UNKNOWN; native binding must explicitly permit
     * and persist that policy. Never infer physical sectors from stripe size. */
};

enum lfg_result { LFG_OK = 0, LFG_E_LENGTH = -1, LFG_E_MAGIC = -2,
    LFG_E_VERSION = -3, LFG_E_RESERVED = -4, LFG_E_RANGE = -5,
    LFG_E_STATE = -6 };

static inline uint16_t lfg_get16(const uint8_t *p) {
    return (uint16_t)(((uint16_t)p[0] << 8) | p[1]);
}
static inline uint32_t lfg_get32(const uint8_t *p) {
    return ((uint32_t)p[0] << 24) | ((uint32_t)p[1] << 16) |
           ((uint32_t)p[2] << 8) | p[3];
}
static inline uint64_t lfg_get64(const uint8_t *p) {
    return ((uint64_t)lfg_get32(p) << 32) | lfg_get32(p + 4);
}
static inline void lfg_put16(uint8_t *p, uint16_t x) {
    p[0] = (uint8_t)(x >> 8); p[1] = (uint8_t)x;
}
static inline void lfg_put32(uint8_t *p, uint32_t x) {
    p[0] = (uint8_t)(x >> 24); p[1] = (uint8_t)(x >> 16);
    p[2] = (uint8_t)(x >> 8); p[3] = (uint8_t)x;
}
static inline void lfg_put64(uint8_t *p, uint64_t x) {
    lfg_put32(p, (uint32_t)(x >> 32)); lfg_put32(p + 4, (uint32_t)x);
}
static inline int lfg_nonzero(const uint8_t *p, size_t n) {
    uint8_t x = 0; size_t i;
    for (i = 0; i < n; ++i) x |= p[i];
    return x != 0;
}
static inline int lfg_known_kind(uint16_t k) {
    return (k >= LFG_HELLO && k <= LFG_ERROR) || k == LFG_QUERY ||
           k == LFG_CLOSE || k == LFG_RETIRE || k == LFG_REPLY;
}
static inline int lfg_is_request(uint16_t k) {
    return k == LFG_HELLO || k == LFG_QUERY || k == LFG_CLOSE || k == LFG_RETIRE;
}
/* Full binding is mandatory for worker messages and every success candidate.
 * HELLO/QUERY can discover STARTING metadata but confer no close authority. */
static inline int lfg_full_binding(const struct lfg_message *m) {
    return m->guardian_pid && m->guardian_birth_sec && m->worker_pid &&
        m->worker_birth_sec && m->status_ino && m->source_ino &&
        m->source_size && m->filesystem &&
        lfg_nonzero(m->boot_id, 16) &&
        lfg_nonzero(m->component_sha256, 32);
}
/* Structural success candidate ONLY. Caller must authenticate live peer,
 * correlate the immutable binding and nonce, and trust its own supervision.
 * A file decoded by this function is explicitly NOT completion authority. */
static inline int lfg_clean_candidate(const struct lfg_message *m) {
    return m->kind == LFG_REPLY && m->state == LFG_RELEASED_CLEAN &&
        m->flags == LFG_F_ALL && m->required_checks == LFG_C_ALL &&
        m->completed_checks == LFG_C_ALL && !m->error && !m->latched_error &&
        !m->error_stage && m->wait_kind == LFG_WAIT_EXITED && !m->wait_value &&
        m->sequence && lfg_full_binding(m) &&
        lfg_nonzero(m->request_nonce, 16);
}
static inline int lfg_validate(const struct lfg_message *m) {
    if (!m || !lfg_known_kind(m->kind) || m->state > LFG_RETIRED ||
        (m->flags & ~LFG_F_ALL) ||
        (m->required_checks & ~LFG_C_ALL) ||
        (m->completed_checks & ~m->required_checks) ||
        m->uid > UINT32_MAX || m->guardian_pid > LFG_MAX_PID ||
        m->worker_pid > LFG_MAX_PID || m->guardian_birth_usec >= 1000000 ||
        m->worker_birth_usec >= 1000000 || m->source_size > LFG_MAX_OFFSET ||
        m->guardian_birth_sec > LFG_MAX_OFFSET ||
        m->worker_birth_sec > LFG_MAX_OFFSET ||
        (m->source_kind != LFG_SOURCE_REGULAR && m->source_kind != LFG_SOURCE_CHAR_PROVIDER) ||
        m->wait_kind > LFG_WAIT_SIGNALED ||
        m->error > INT32_MAX || m->latched_error > INT32_MAX ||
        m->error_stage > 65535 || !lfg_nonzero(m->session_nonce, 32))
        return LFG_E_RANGE;
    if (m->source_kind == LFG_SOURCE_REGULAR) {
        if (m->source_rdev || m->source_logical_sector || m->source_physical_sector)
            return LFG_E_RANGE;
    } else {
        uint32_t l = m->source_logical_sector, p = m->source_physical_sector;
        if (!m->source_rdev || !m->source_size || l < 512 || l > 65536 ||
            (l & (l - 1)) || (p && (p < l || p > 65536 || (p & (p - 1)))) ||
            m->source_size % l) return LFG_E_RANGE;
    }
    if ((m->wait_kind == LFG_WAIT_UNKNOWN && m->wait_value) ||
        (m->wait_kind == LFG_WAIT_EXITED && m->wait_value > 255) ||
        (m->wait_kind == LFG_WAIT_SIGNALED && (!m->wait_value || m->wait_value > 127)))
        return LFG_E_RANGE;
    if ((m->flags & LFG_F_WORKER_EXIT_ZERO) &&
        (!(m->flags & LFG_F_WORKER_EXITED) || m->wait_kind != LFG_WAIT_EXITED || m->wait_value))
        return LFG_E_STATE;
    if ((m->flags & LFG_F_CUSTODY_CLOSED) && !(m->flags & LFG_F_CUSTODY_FLUSHED))
        return LFG_E_STATE;
    if (lfg_is_request(m->kind)) {
        if (m->state != LFG_REQUEST || m->flags || m->error || m->latched_error ||
            m->error_stage || m->completed_checks || m->wait_kind || m->wait_value ||
            !lfg_nonzero(m->request_nonce, 16)) return LFG_E_STATE;
        if (m->kind != LFG_HELLO && m->kind != LFG_QUERY && !lfg_full_binding(m))
            return LFG_E_STATE;
    } else if (m->state == LFG_REQUEST) return LFG_E_STATE;
    if ((m->kind == LFG_BIND || m->kind == LFG_READY || m->kind == LFG_DRAINED) &&
        !lfg_full_binding(m)) return LFG_E_STATE;
    if (m->kind == LFG_BIND && m->state != LFG_STARTING) return LFG_E_STATE;
    if (m->kind == LFG_READY && m->state != LFG_RUNNING) return LFG_E_STATE;
    if (m->kind == LFG_DRAINED && (m->state != LFG_WORKER_DRAINED ||
        m->completed_checks != LFG_C_ALL || m->required_checks != LFG_C_ALL ||
        m->error || m->latched_error)) return LFG_E_STATE;
    if (m->kind == LFG_ERROR && (m->state != LFG_RETAINED ||
        (!m->error && !m->latched_error))) return LFG_E_STATE;
    if (m->state == LFG_RELEASED_CLEAN && !lfg_clean_candidate(m)) return LFG_E_STATE;
    if (m->state == LFG_RETIRED && (m->kind != LFG_REPLY ||
        m->flags != LFG_F_ALL || m->error || m->latched_error ||
        m->wait_kind != LFG_WAIT_EXITED || m->wait_value ||
        m->completed_checks != LFG_C_ALL || m->required_checks != LFG_C_ALL ||
        !lfg_full_binding(m) || !lfg_nonzero(m->request_nonce, 16))) return LFG_E_STATE;
    return LFG_OK;
}

/* Fixed public offsets, also documented in tests/golden-v2.json. Reserved
 * bytes 14..15 and 312..511 MUST remain zero. All lengths are exactly 512. */
#define LFG_SCALAR_FIELDS(X16, X32, X64) \
    X16(kind, 6) X16(state, 12) X32(flags, 16) X32(error, 20) \
    X64(required_checks, 24) X64(completed_checks, 32) \
    X64(uid, 104) X64(guardian_pid, 112) X64(guardian_birth_sec, 120) \
    X64(guardian_birth_usec, 128) X64(worker_pid, 136) \
    X64(worker_birth_sec, 144) X64(worker_birth_usec, 152) \
    X64(status_dev, 160) X64(status_ino, 168) X64(source_dev, 176) \
    X64(source_ino, 184) X64(source_size, 192) X64(source_rdev, 200) \
    X32(source_kind, 208) X32(filesystem, 212) \
    X32(wait_kind, 280) X32(wait_value, 284) X64(sequence, 288) \
    X32(error_stage, 296) X32(latched_error, 300) \
    X32(source_logical_sector, 304) X32(source_physical_sector, 308)
static inline int lfg_encode(uint8_t *out, size_t length,
                             const struct lfg_message *m) {
    size_t i; int rc;
    if (!out || length != LFG_FRAME_SIZE) return LFG_E_LENGTH;
    rc = lfg_validate(m); if (rc != LFG_OK) return rc;
    memset(out, 0, length); memcpy(out, "LFG2", 4);
    lfg_put16(out + 4, LFG_VERSION); lfg_put32(out + 8, LFG_FRAME_SIZE);
#define LFG_E16(f, o) lfg_put16(out + o, m->f);
#define LFG_E32(f, o) lfg_put32(out + o, m->f);
#define LFG_E64(f, o) lfg_put64(out + o, m->f);
    LFG_SCALAR_FIELDS(LFG_E16, LFG_E32, LFG_E64)
#undef LFG_E16
#undef LFG_E32
#undef LFG_E64
    memcpy(out + 40, m->session_nonce, 32); memcpy(out + 72, m->request_nonce, 16);
    memcpy(out + 88, m->boot_id, 16); memcpy(out + 248, m->component_sha256, 32);
    for (i = 0; i < 4; ++i) lfg_put64(out + 216 + 8 * i, m->native_id[i]);
    return LFG_OK;
}
static inline int lfg_decode(struct lfg_message *out, const uint8_t *in,
                             size_t length) {
    struct lfg_message m; size_t i; int rc;
    if (!out || !in || length != LFG_FRAME_SIZE) return LFG_E_LENGTH;
    if (memcmp(in, "LFG2", 4)) return LFG_E_MAGIC;
    if (lfg_get16(in + 4) != LFG_VERSION) return LFG_E_VERSION;
    if (lfg_get32(in + 8) != LFG_FRAME_SIZE) return LFG_E_LENGTH;
    if (in[14] || in[15] || lfg_nonzero(in + 312, 200)) return LFG_E_RESERVED;
    memset(&m, 0, sizeof(m));
#define LFG_D16(f, o) m.f = lfg_get16(in + o);
#define LFG_D32(f, o) m.f = lfg_get32(in + o);
#define LFG_D64(f, o) m.f = lfg_get64(in + o);
    LFG_SCALAR_FIELDS(LFG_D16, LFG_D32, LFG_D64)
#undef LFG_D16
#undef LFG_D32
#undef LFG_D64
    memcpy(m.session_nonce, in + 40, 32); memcpy(m.request_nonce, in + 72, 16);
    memcpy(m.boot_id, in + 88, 16); memcpy(m.component_sha256, in + 248, 32);
    for (i = 0; i < 4; ++i) m.native_id[i] = lfg_get64(in + 216 + 8 * i);
    rc = lfg_validate(&m); if (rc == LFG_OK) *out = m;
    return rc;
}
#undef LFG_SCALAR_FIELDS
/* Exact immutable supervision identity. Request nonce, state, wait outcome,
 * sequence, progress and native_id are deliberately separate checks. Native
 * ID becomes immutable after READY; binding verifies it on CLOSE and REPLY. */
static inline int lfg_same_binding(const struct lfg_message *a,
                                   const struct lfg_message *b) {
    return !memcmp(a->session_nonce, b->session_nonce, 32) &&
        !memcmp(a->boot_id, b->boot_id, 16) &&
        !memcmp(a->component_sha256, b->component_sha256, 32) &&
        a->uid == b->uid && a->guardian_pid == b->guardian_pid &&
        a->guardian_birth_sec == b->guardian_birth_sec &&
        a->guardian_birth_usec == b->guardian_birth_usec &&
        a->worker_pid == b->worker_pid && a->worker_birth_sec == b->worker_birth_sec &&
        a->worker_birth_usec == b->worker_birth_usec &&
        a->status_dev == b->status_dev && a->status_ino == b->status_ino &&
        a->source_dev == b->source_dev && a->source_ino == b->source_ino &&
        a->source_size == b->source_size && a->source_rdev == b->source_rdev &&
        a->source_kind == b->source_kind && a->filesystem == b->filesystem &&
        a->required_checks == b->required_checks &&
        a->source_logical_sector == b->source_logical_sector &&
        a->source_physical_sector == b->source_physical_sector;
}
#endif
