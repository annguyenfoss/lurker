/* SPDX-License-Identifier: MIT */
#include "../guardian-protocol.h"
#include <stdio.h>
#include <stdlib.h>

static unsigned checks;
#define CHECK(x) do { ++checks; if (!(x)) { \
    fprintf(stderr, "check %u line %u failed: %s\n", checks, __LINE__, #x); \
    exit(1); } } while (0)

static struct lfg_message specimen(void) {
    struct lfg_message m = {0}; size_t i;
    m.kind = LFG_REPLY; m.state = LFG_RELEASED_CLEAN; m.flags = LFG_F_ALL;
    m.required_checks = m.completed_checks = LFG_C_ALL;
    for (i = 0; i < 32; ++i) m.session_nonce[i] = (uint8_t)(i + 1);
    for (i = 0; i < 16; ++i) m.request_nonce[i] = (uint8_t)(0xa0 + i);
    for (i = 0; i < 16; ++i) m.boot_id[i] = (uint8_t)(0x10 + i);
    for (i = 0; i < 32; ++i) m.component_sha256[i] = (uint8_t)(0xc0 + i);
    m.uid = 501; m.guardian_pid = 1234; m.guardian_birth_sec = 1795750000;
    m.guardian_birth_usec = 123456; m.worker_pid = 1235;
    m.worker_birth_sec = 1795750000; m.worker_birth_usec = 234567;
    m.status_dev = m.source_dev = 16777234; m.status_ino = 40000001;
    m.source_ino = 40000002; m.source_size = UINT64_C(536870912);
    m.source_kind = LFG_SOURCE_REGULAR; m.filesystem = 3;
    m.native_id[0] = UINT64_C(0x123456789abcdef0);
    m.native_id[1] = UINT64_C(0x0fedcba987654321);
    m.native_id[2] = 16777234; m.native_id[3] = 40000003;
    m.wait_kind = LFG_WAIT_EXITED; m.sequence = 9;
    return m;
}
static void golden(const char *path, uint8_t *out) {
    FILE *f = fopen(path, "r"); unsigned i, x;
    CHECK(f != NULL);
    for (i = 0; i < LFG_FRAME_SIZE; ++i) {
        CHECK(fscanf(f, "%2x", &x) == 1); out[i] = (uint8_t)x;
    }
    CHECK(fscanf(f, "%2x", &x) == EOF); CHECK(fclose(f) == 0);
}
int main(int argc, char **argv) {
    uint8_t a[LFG_FRAME_SIZE], b[LFG_FRAME_SIZE];
    struct lfg_message m = specimen(), n, sentinel; size_t i;
    CHECK(argc == 3); golden(argv[1], b);
    CHECK(lfg_encode(a, sizeof(a), &m) == LFG_OK);
    CHECK(!memcmp(a, b, sizeof(a)));
    CHECK(lfg_decode(&n, b, sizeof(b)) == LFG_OK);
    CHECK(lfg_clean_candidate(&n)); CHECK(lfg_same_binding(&m, &n));
    CHECK(lfg_encode(a, sizeof(a), &n) == LFG_OK);
    CHECK(!memcmp(a, b, sizeof(a)));
    for (i = 0; i < LFG_FRAME_SIZE; ++i)
        CHECK(lfg_decode(&n, b, i) == LFG_E_LENGTH);
    CHECK(lfg_decode(&n, b, LFG_FRAME_SIZE + 1) == LFG_E_LENGTH);
    CHECK(lfg_encode(a, LFG_FRAME_SIZE - 1, &m) == LFG_E_LENGTH);
    CHECK(lfg_decode(NULL, b, sizeof(b)) == LFG_E_LENGTH);
    CHECK(lfg_decode(&n, NULL, sizeof(b)) == LFG_E_LENGTH);
    memcpy(a, b, sizeof(a)); a[0] ^= 1;
    CHECK(lfg_decode(&n, a, sizeof(a)) == LFG_E_MAGIC);
    memcpy(a, b, sizeof(a)); a[5] = 1;
    CHECK(lfg_decode(&n, a, sizeof(a)) == LFG_E_VERSION);
    memcpy(a, b, sizeof(a)); a[11] = 1;
    CHECK(lfg_decode(&n, a, sizeof(a)) == LFG_E_LENGTH);
    for (i = 0; i < LFG_FRAME_SIZE; ++i) {
        if (i != 14 && i != 15 && i < 312) continue;
        memcpy(a, b, sizeof(a)); a[i] = 1;
        CHECK(lfg_decode(&n, a, sizeof(a)) == LFG_E_RESERVED);
    }
#define BAD(field, value) do { n = m; n.field = (value); \
    CHECK(lfg_encode(a, sizeof(a), &n) != LFG_OK); } while (0)
    BAD(kind, 0); BAD(kind, 6); BAD(kind, 65535); BAD(state, 8);
    BAD(flags, 128); BAD(required_checks, 1024); BAD(completed_checks, 1024);
    BAD(uid, UINT64_C(0x100000000)); BAD(guardian_pid, LFG_MAX_PID + 1);
    BAD(worker_pid, LFG_MAX_PID + 1); BAD(guardian_birth_usec, 1000000);
    BAD(worker_birth_usec, UINT64_MAX); BAD(source_size, UINT64_MAX);
    BAD(source_rdev, 1); BAD(source_kind, 3); BAD(wait_kind, 3);
    BAD(error, UINT32_MAX); BAD(latched_error, UINT32_MAX);
    BAD(error_stage, 65536); BAD(wait_value, 1); BAD(sequence, 0);
    BAD(guardian_pid, 0); BAD(worker_pid, 0); BAD(status_ino, 0);
    BAD(source_ino, 0); BAD(source_size, 0); BAD(filesystem, 0);
    BAD(source_logical_sector, 512); BAD(source_physical_sector, 4096);
    n = m; n.source_kind = LFG_SOURCE_CHAR_PROVIDER; n.source_rdev = 42;
    n.source_logical_sector = 512; n.source_physical_sector = 4096;
    CHECK(lfg_encode(a, sizeof(a), &n) == LFG_OK);
    golden(argv[2], b); CHECK(!memcmp(a, b, sizeof(a)));
    golden(argv[1], b);
    CHECK(lfg_decode(&sentinel, a, sizeof(a)) == LFG_OK);
    CHECK(lfg_same_binding(&n, &sentinel));
    CHECK(lfg_clean_candidate(&sentinel)); /* structural support, not OS admission */
    n.source_physical_sector = 0; CHECK(lfg_validate(&n) == LFG_OK);
    n.source_physical_sector = 513; CHECK(lfg_validate(&n) != LFG_OK);
    n.source_physical_sector = 4096; n.source_size++;
    CHECK(lfg_validate(&n) != LFG_OK);
    n.source_size--; n.source_rdev = 0; CHECK(lfg_validate(&n) != LFG_OK);
    BAD(required_checks, LFG_C_ALL ^ LFG_C_HALT);
    BAD(completed_checks, LFG_C_ALL ^ LFG_C_HALT);
    BAD(flags, LFG_F_ALL ^ LFG_F_STATUS_SYNCED);
    /* Failed status fsync / exit1 cannot be a success candidate, even when a
     * previously written status file contains the unchanged golden frame. */
    n = m; n.state = LFG_RETAINED; n.flags = LFG_F_WORKER_EXITED;
    n.error = n.latched_error = 5; n.error_stage = 7; n.wait_value = 1;
    CHECK(lfg_validate(&n) == LFG_OK); CHECK(!lfg_clean_candidate(&n));
    CHECK(lfg_decode(&sentinel, b, sizeof(b)) == LFG_OK);
    CHECK(lfg_clean_candidate(&sentinel)); /* visible file alone has no trust */
    CHECK(n.wait_value != 0); CHECK(n.error != 0);
    n = m; n.kind = LFG_QUERY; n.state = LFG_REQUEST;
    n.flags = 0; n.completed_checks = 0; n.wait_kind = 0;
    CHECK(lfg_validate(&n) == LFG_OK); CHECK(!lfg_clean_candidate(&n));
    n = m; n.kind = LFG_HELLO; n.state = LFG_REQUEST; n.flags = 0;
    n.completed_checks = 0; n.wait_kind = 0;
    n.worker_pid = n.worker_birth_sec = n.worker_birth_usec = 0;
    memset(n.native_id, 0, sizeof(n.native_id));
    CHECK(lfg_validate(&n) == LFG_OK); CHECK(!lfg_full_binding(&n));
    CHECK(!lfg_clean_candidate(&n));
    n = m; n.kind = LFG_DRAINED; n.state = LFG_WORKER_DRAINED;
    n.flags = 0; n.wait_kind = 0;
    CHECK(lfg_validate(&n) == LFG_OK); CHECK(!lfg_clean_candidate(&n));
    n = m; n.kind = LFG_ERROR; n.state = LFG_RETAINED; n.error = 5;
    CHECK(lfg_validate(&n) == LFG_OK); CHECK(!lfg_clean_candidate(&n));
    n = m; n.kind = LFG_REPLY; n.state = LFG_RETIRED;
    CHECK(lfg_validate(&n) == LFG_OK); CHECK(!lfg_clean_candidate(&n));
    /* Decoder never partially overwrites the consumer context on failure. */
    sentinel = m; memcpy(a, b, sizeof(a)); a[511] = 1;
    CHECK(lfg_decode(&sentinel, a, sizeof(a)) != LFG_OK);
    CHECK(!memcmp(&sentinel, &m, sizeof(m)));
    n = m; n.source_ino++; CHECK(!lfg_same_binding(&m, &n));
    n = m; n.worker_birth_usec++; CHECK(!lfg_same_binding(&m, &n));
    n = m; n.request_nonce[0]++; CHECK(lfg_same_binding(&m, &n));
    CHECK(memcmp(m.request_nonce, n.request_nonce, 16) != 0);
    n = m; n.native_id[0]++; CHECK(lfg_same_binding(&m, &n));
    CHECK(memcmp(m.native_id, n.native_id, sizeof(m.native_id)) != 0);
    printf("{\"protocol\":2,\"frame_bytes\":512,\"checks\":%u,\"result\":\"pass\","
           "\"scope\":\"pure codec only; no live peer or guardian authorization\"}\n", checks);
    return 0;
}
