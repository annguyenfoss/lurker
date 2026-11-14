// Native BSD profiles live at the OS adapter boundary (bsd/): GELI AES-XTS256
// with PBKDF2/600000, CGD AES-XTS512/Argon2id, softraid native CRYPTO /
// two-member RAID1C, and DragonFly LUKS1 XTS512/PBKDF2-SHA256.
// They never inherit Linux LUKS2 flags.
// Directory engines use fixed upstream authenticated formats: gocryptfs
// AES-GCM/EME+scryptN16, CryFS AES-GCM/16KiB, Cryptomator format8 SIV_GCM.
// Native FreeBSD PEFS uses AES-256-XTS/PBKDF2-600000 and verifier-chain metadata.
// CIPHER.md records the complete profiles; generic --cipher is container-only.
use crate::model::CreateCipher;
use std::ffi::OsString;

#[cfg_attr(target_os = "macos", allow(dead_code))]
struct CreateCipherProfile {
    label: &'static str,
    format_args: &'static [&'static str],
    open_args: &'static [&'static str],
    veracrypt_encryption: &'static str,
}

const VERACRYPT_CREATE_ARGS_COMMON: &[&str] = &[
    "-t",
    "--create",
    "--force",
    "--volume-type=normal",
    "--hash=SHA-512",
    "--filesystem=none",
    "--random-source=/dev/urandom",
    "--pim=0",
];

#[cfg(target_os = "linux")]
// VeraCrypt determines the full device size; --size=max is file-only.
const VERACRYPT_CREATE_ARGS_BLOCK: &[&str] = &["--quick"];

// LUKS1 uses the same cipher families and iteration times with PBKDF2/SHA-512;
// Linux dispatch removes LUKS2-only KDF/sector flags for that explicit format.
const LUKS_FORMAT_ARGS_AES: &[&str] = &[
    "--type",
    "luks2",
    "-c",
    "aes-xts-plain64",
    "-s",
    "512",
    "--pbkdf",
    "argon2id",
    "--pbkdf-memory",
    "1048576",
    "-i",
    "5000",
    "--use-urandom",
];

const LUKS_FORMAT_ARGS_SERPENT: &[&str] = &[
    "--type",
    "luks2",
    "-c",
    "serpent-xts-plain64",
    "-s",
    "512",
    "--pbkdf",
    "argon2id",
    "--pbkdf-memory",
    "2097152",
    "--pbkdf-parallel",
    "4",
    "-i",
    "10000",
    "--use-urandom",
];

const LUKS_FORMAT_ARGS_TWOFISH: &[&str] = &[
    "--type",
    "luks2",
    "-c",
    "twofish-xts-plain64",
    "-s",
    "512",
    "--pbkdf",
    "argon2id",
    "--pbkdf-memory",
    "1048576",
    "--pbkdf-parallel",
    "4",
    "-i",
    "5000",
    "--sector-size",
    "4096",
    "--use-urandom",
];

const LUKS_OPEN_ARGS_TWOFISH: &[&str] = &[
    "--perf-no_read_workqueue",
    "--perf-no_write_workqueue",
    "--perf-submit_from_crypt_cpus",
];

const CIPHER_PROFILE_AES: CreateCipherProfile = CreateCipherProfile {
    label: "AES",
    format_args: LUKS_FORMAT_ARGS_AES,
    open_args: &[],
    veracrypt_encryption: "AES",
};

const CIPHER_PROFILE_SERPENT: CreateCipherProfile = CreateCipherProfile {
    label: "Serpent",
    format_args: LUKS_FORMAT_ARGS_SERPENT,
    open_args: &[],
    veracrypt_encryption: "Serpent",
};

const CIPHER_PROFILE_TWOFISH: CreateCipherProfile = CreateCipherProfile {
    label: "Twofish",
    format_args: LUKS_FORMAT_ARGS_TWOFISH,
    open_args: LUKS_OPEN_ARGS_TWOFISH,
    veracrypt_encryption: "Twofish",
};

fn cipher_profile(cipher: CreateCipher) -> &'static CreateCipherProfile {
    match cipher {
        CreateCipher::Aes => &CIPHER_PROFILE_AES,
        CreateCipher::Serpent => &CIPHER_PROFILE_SERPENT,
        CreateCipher::Twofish => &CIPHER_PROFILE_TWOFISH,
    }
}

// Additional LUKS slots keep the create profile's KDF costs without changing
// the existing volume cipher, sector size or volume key.
#[cfg(target_os = "linux")]
pub(crate) fn luks_keyslot_kdf_args(cipher: CreateCipher, version: u16) -> Vec<OsString> {
    let profile = cipher_profile(cipher);
    if version == 1 {
        let time = profile
            .format_args
            .windows(2)
            .find(|pair| pair[0] == "-i")
            .unwrap()[1];
        return ["--pbkdf", "pbkdf2", "--iter-time", time]
            .map(OsString::from)
            .into();
    }
    let mut result = Vec::new();
    for pair in profile.format_args.windows(2) {
        if matches!(
            pair[0],
            "--pbkdf" | "--pbkdf-memory" | "--pbkdf-parallel" | "-i"
        ) {
            result.extend(pair.iter().map(OsString::from));
        }
    }
    result
}

// Linux and native BSD routes share the upstream VeraCrypt header profile.
// Adapters select the native inner filesystem; transport never changes crypto.
#[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "openbsd"))]
pub(crate) fn file_veracrypt_create_args(
    cipher: CreateCipher,
    size_mib: u64,
    keyfile: Option<&std::path::Path>,
) -> crate::AppResult<Vec<OsString>> {
    let mut args: Vec<OsString> = VERACRYPT_CREATE_ARGS_COMMON
        .iter()
        .map(OsString::from)
        .collect();
    args.extend(["--stdin", "--non-interactive"].map(OsString::from));
    args.push("-k".into());
    args.push(crate::credentials::veracrypt_keyfiles(keyfile)?);
    args.push(
        format!(
            "--encryption={}",
            cipher_profile(cipher).veracrypt_encryption
        )
        .into(),
    );
    args.push(format!("--size={size_mib}M").into());
    Ok(args)
}

#[cfg(target_os = "linux")]
pub(crate) fn userspace_veracrypt_create_args(
    cipher: CreateCipher,
    size_mib: u64,
    keyfile: Option<&std::path::Path>,
) -> crate::AppResult<Vec<OsString>> {
    file_veracrypt_create_args(cipher, size_mib, keyfile)
}

#[cfg(target_os = "linux")]
#[path = "linux_workflow.rs"]
mod linux_backend;

#[cfg(target_os = "linux")]
pub use linux_backend::run;

// macOS uses VeraCrypt's native formatter instead of Linux's Btrfs step.
#[cfg(target_os = "macos")]
pub(crate) fn macos_veracrypt_create_args(
    cipher: CreateCipher,
    size_mib: u64,
    keyfile: Option<&std::path::Path>,
) -> crate::AppResult<Vec<OsString>> {
    let mut args: Vec<OsString> = VERACRYPT_CREATE_ARGS_COMMON
        .iter()
        .filter(|arg| **arg != "--filesystem=none")
        .map(OsString::from)
        .collect();
    args.extend([
        OsString::from("--filesystem=exFAT"),
        OsString::from("--stdin"),
        OsString::from("--non-interactive"),
    ]);
    args.push("-k".into());
    args.push(crate::credentials::veracrypt_keyfiles(keyfile)?);
    args.push(
        format!(
            "--encryption={}",
            cipher_profile(cipher).veracrypt_encryption
        )
        .into(),
    );
    args.push(format!("--size={size_mib}M").into());
    Ok(args)
}

#[cfg(target_os = "macos")]
pub(crate) fn macos_apple_create_args(size_mib: u64) -> Vec<OsString> {
    [
        "create",
        "-size",
        &format!("{size_mib}m"),
        "-type",
        "UDIF",
        "-fs",
        "APFS",
        "-volname",
        "Lurker",
        "-encryption",
        "AES-256",
        "-stdinpass",
    ]
    .into_iter()
    .map(OsString::from)
    .collect()
}
