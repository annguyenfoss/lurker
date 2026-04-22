# LUKS2 cryptsetup commands

Three tuned `luksFormat` commands — one per cipher. All use LUKS2, Argon2id, AES-256-equivalent key strength (`-s 512` in XTS → 256-bit effective), and `/dev/urandom` for key material. Append your target device (e.g. `/dev/sdX` or a loop file) to run.

---

## AES — default pick for almost everything

```sh
cryptsetup -v --type luks2 \
  -c aes-xts-plain64 -s 512 \
  --pbkdf argon2id --pbkdf-memory 1048576 -i 5000 \
  --use-urandom -y \
  luksFormat
```

**Parameters**
- `--type luks2` — modern header format (metadata redundancy, Argon2id support).
- `-c aes-xts-plain64` — AES in XTS mode with 64-bit sector-number tweak. Hardware-accelerated via AES-NI / ARMv8 crypto.
- `-s 512` — XTS splits the key in half → AES-256-XTS.
- `--pbkdf argon2id` — memory-hard KDF, GPU/ASIC resistant.
- `--pbkdf-memory 1048576` — 1 GiB RAM per unlock attempt.
- `-i 5000` — target 5 s KDF time on this machine.
- `--use-urandom` — non-blocking CSPRNG for key material.
- `-y` — verify passphrase at format time.

---

## Serpent — extreme paranoia / maximum cryptanalytic margin

```sh
cryptsetup -v --type luks2 \
  -c serpent-xts-plain64 -s 512 \
  --pbkdf argon2id --pbkdf-memory 2097152 --pbkdf-parallel 4 -i 10000 \
  --use-urandom -y \
  luksFormat
```

**Parameters**
- `-c serpent-xts-plain64 -s 512` — Serpent-256-XTS. 32 rounds vs AES's 14 — largest safety margin of the AES finalists.
- `--pbkdf-memory 2097152` — 2 GiB RAM per unlock attempt (RFC 9106 upper recommendation).
- `--pbkdf-parallel 4` — pin 4 Argon2id lanes explicitly for deterministic behavior across machines.
- `-i 10000` — 10 s KDF budget; ~5× harder brute-force vs default.
- Everything else same as AES.

**Pre-flight:** `cryptsetup benchmark -c serpent-xts-plain64` and a test format on a loop file — LUKS2 + Serpent has had keyslot-encryption regressions historically (cryptsetup issue #499).

---

## Twofish — not-AES alternative, tuned for throughput

```sh
cryptsetup -v --type luks2 \
  -c twofish-xts-plain64 -s 512 \
  --pbkdf argon2id --pbkdf-memory 1048576 --pbkdf-parallel 4 -i 5000 \
  --sector-size 4096 \
  --use-urandom -y \
  luksFormat
```

**Parameters**
- `-c twofish-xts-plain64 -s 512` — Twofish-256-XTS. Feistel structure, distinct design lineage from AES.
- `--pbkdf-memory 1048576` / `-i 5000` — LUKS2-default Argon2id strength; don't compound the cipher slowdown with an extreme KDF.
- `--pbkdf-parallel 4` — explicit lane count.
- **`--sector-size 4096`** — 4 KiB encryption blocks instead of 512 B. 8× fewer XTS tweak operations per MB, matches SSD native sectors, ~10–25% throughput gain. Key Twofish-specific speed tuning.
- `--use-urandom -y` — standard.

**Open-time perf flags** (for `cryptsetup open` or `/etc/crypttab` options, not `luksFormat`):
```
--perf-no_read_workqueue --perf-no_write_workqueue --perf-submit_from_crypt_cpus
```
Bypass kernel workqueues that hurt throughput on fast SSDs. Safe to enable on modern NVMe/SATA SSDs.

**Verify AVX2 Twofish module is active:**
```sh
cat /proc/crypto | grep -A3 twofish
```
Look for `twofish-avx-x86_64` — the 8-way parallel implementation.

---

## Post-format check

For any of the three, confirm the result:
```sh
cryptsetup luksDump /dev/sdX
```
Look for `Cipher: <expected>`, `Cipher key: 512 bits`, `PBKDF: argon2id`, and your `Memory`/`Time cost` values under the keyslot.
