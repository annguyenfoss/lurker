/* Private Lurker protocol for the pinned DragonFly libtcplay build. */
#include <sys/types.h>
#include <sys/resource.h>
#include <libdm.h>
#include <stdbool.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

#include "tcplay_api.h"
#include "tcplay_api_internal.h"

#define PROFILE "SHA512-VC"
#define SECTOR_BYTES 512
#define MAPPER_PREFIX "lurker_vc_"
#define UUID_PREFIX "LURKER-VERACRYPT-"

static bool
valid_name(const char *value, const char *prefix, const char *alphabet)
{
    size_t length = strlen(prefix);
    if (strncmp(value, prefix, length) != 0)
        return false;
    value += length;
    return *value != '\0' && strlen(value) <= 64 &&
        strspn(value, alphabet) == strlen(value);
}

static bool
supported_cipher(const char *cipher)
{
    return strcmp(cipher, "AES-256-XTS") == 0 ||
        strcmp(cipher, "SERPENT-256-XTS") == 0 ||
        strcmp(cipher, "TWOFISH-256-XTS") == 0;
}

static int
set_credential(tc_api_task task, const char *kind)
{
    if (strcmp(kind, "keyfile") == 0) {
        if (tc_api_task_set(task, "passphrase", "") != TC_OK)
            return TC_ERR;
        /* This private build reads this path as a bounded credential stream. */
        return tc_api_task_set(task, "keyfiles", "/dev/stdin");
    }
    if (strcmp(kind, "password") != 0)
        return TC_ERR;

    char password[MAX_PASSSZ + 1] = {0};
    size_t size = fread(password, 1, sizeof(password), stdin);
    int result = TC_ERR;
    if (size > 0 && size <= MAX_PASSSZ && feof(stdin) && !ferror(stdin) &&
        memchr(password, '\0', size) == NULL)
        result = tc_api_task_set(task, "passphrase", password);
    explicit_bzero(password, sizeof(password));
    return result;
}

static bool
qualified_volume(const struct tcplay_info *info, uint64_t bytes)
{
    const struct tchdr_dec *header = info->hdr;
    const struct tc_crypto_algo *cipher = info->cipher_chain->cipher;
    return strcmp(info->pbkdf_prf->name, PROFILE) == 0 &&
        info->pbkdf_prf->iteration_count == 500000 &&
        !info->hidden && header->sz_hidvol == 0 && header->flags == 0 &&
        header->tc_ver == 5 && header->sec_sz == SECTOR_BYTES &&
        info->cipher_chain->next == NULL && cipher->klen == 64 &&
        supported_cipher(cipher->name) &&
        header->off_mk_scope == VOL_RSVD_BYTES_START &&
        header->sz_vol == header->sz_mk_scope &&
        header->sz_mk_scope == bytes - MIN_VOL_BYTES &&
        header->sz_mk_scope % SECTOR_BYTES == 0 &&
        info->start == 0 && info->offset == info->skip;
}

/* Suppress libdm diagnostics because target parameters include the key. */
static void
quiet_dm(int level __unused, const char *file __unused, int line __unused,
    const char *format __unused, ...)
{
}

static bool
activate(const struct tcplay_info *info, const char *name, const char *uuid)
{
    char params[256];
    int length = snprintf(params, sizeof(params), "%s %s %" PRIu64 " %s %" PRIu64,
        info->cipher_chain->cipher->dm_crypt_str, info->cipher_chain->dm_key,
        (uint64_t)info->skip, info->dev, (uint64_t)info->offset);
    dm_log_init(quiet_dm);
    struct dm_task *task = dm_task_create(DM_DEVICE_CREATE);
    bool ok = task != NULL && length > 0 && (size_t)length < sizeof(params) &&
        dm_task_set_name(task, name) && dm_task_set_uuid(task, uuid) &&
        dm_task_add_target(task, 0, info->size, "crypt", params) && dm_task_run(task);
    explicit_bzero(params, sizeof(params));
    if (task != NULL)
        dm_task_destroy(task);
    dm_task_update_nodes();
    return ok;
}

int
main(int argc, char **argv)
{
    if (argc == 2 && strcmp(argv[1], "protocol") == 0) {
        puts("lurker-tcplay 1");
        return 0;
    }
    bool create = argc == 5 && strcmp(argv[1], "create") == 0;
    bool map = argc == 6 && strcmp(argv[1], "map") == 0;
    if ((!create && !map) || !valid_name(argv[2], "/dev/vn", "0123456789"))
        return 2;
    const char *cipher = create ? argv[4] : NULL;
    const char *alphanumeric = "0123456789abcdefghijklmnopqrstuvwxyz_";
    if ((create && !supported_cipher(cipher)) ||
        (map && (!valid_name(argv[4], MAPPER_PREFIX, alphanumeric) ||
                 !valid_name(argv[5], UUID_PREFIX, alphanumeric))))
        return 2;

    struct rlimit no_core = {0, 0};
    if (setrlimit(RLIMIT_CORE, &no_core) != 0)
        return 2;
    /* Credential input is never copied into stdio's internal buffer. */
    if (setvbuf(stdin, NULL, _IONBF, 0) != 0)
        return 2;
    if (tc_api_init(0) != TC_OK)
        return 2;
    tc_api_task task = tc_api_task_init(create ? "create" : "info");
    int result = 2;
    disksz_t blocks;
    size_t sector;
    if (task == NULL || get_disk_info(argv[2], &blocks, &sector) != 0 ||
        sector != SECTOR_BYTES || blocks <= MIN_VOL_BYTES / SECTOR_BYTES ||
        blocks > INT64_MAX / SECTOR_BYTES ||
        tc_api_task_set(task, "dev", argv[2]) != TC_OK ||
        tc_api_task_set(task, "interactive", 0) != TC_OK ||
        tc_api_task_set(task, "retries", 1) != TC_OK ||
        set_credential(task, argv[3]) != TC_OK)
        goto done;

    if (create) {
        if (tc_api_task_set(task, "cipher_chain", cipher) == TC_OK &&
            tc_api_task_set(task, "prf_algo", PROFILE) == TC_OK &&
            tc_api_task_set(task, "secure_erase", 1) == TC_OK &&
            tc_api_task_do(task) == TC_OK) {
            puts("created");
            result = 0;
        }
    } else if (tc_api_task_do(task) != TC_OK ||
        task->last_info == NULL || !qualified_volume(task->last_info, blocks * SECTOR_BYTES)) {
        result = 3;
    } else if (!activate(task->last_info, argv[4], argv[5])) {
        result = 4;
    } else {
        printf("mapped %" PRIu64 " %" PRIu64 "\n",
            task->last_info->hdr->off_mk_scope, task->last_info->hdr->sz_mk_scope);
        result = 0;
    }

done:
    if (task != NULL)
        tc_api_task_uninit(task);
    tc_api_uninit();
    if (result == 3)
        fputs("VeraCrypt authentication failed. Check the credential and use a normal SHA-512, PIM 0 volume with AES, Serpent, or Twofish.\n", stderr);
    else if (result == 4)
        fputs("The native device mapper could not activate the authenticated VeraCrypt volume.\n", stderr);
    else if (result != 0)
        fputs("Could not initialize the DragonFly VeraCrypt operation. Check the backing device and available space.\n", stderr);
    return result;
}
