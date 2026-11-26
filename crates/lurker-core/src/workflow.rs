// Native BSD profiles live at the OS adapter boundary (bsd/): GELI AES-XTS256
// with PBKDF2/600000, CGD AES-XTS512/Argon2id, softraid native CRYPTO /
// two-member RAID1C, and DragonFly LUKS1 XTS512/PBKDF2-SHA256.
// They never inherit Linux LUKS2 flags.
// Directory engines use fixed upstream authenticated formats: gocryptfs
// AES-GCM/EME+scryptN16, CryFS AES-GCM/16KiB, Cryptomator format8 SIV_GCM.
// Native FreeBSD PEFS uses AES-256-XTS/PBKDF2-600000 and verifier-chain metadata.
// ENCRYPTION.md records the complete profiles; generic --cipher is container-only.
// Newly formatted Linux/BSD filesystems initialize a private root for the
// authorized invoking user. Unlocking existing filesystems preserves ownership.
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

// The native Linux 44bsd driver probes UFS1 in 512-byte units. Choose compatible
// device geometry when creating it; cipher, key size and KDF costs stay fixed.
#[cfg(target_os = "linux")]
fn luks2_format_args(
    profile: &CreateCipherProfile,
    filesystem: Option<crate::CreateFilesystem>,
) -> Vec<OsString> {
    let mut args: Vec<_> = profile.format_args.iter().map(OsString::from).collect();
    if filesystem == Some(crate::CreateFilesystem::Ufs1) {
        if let Some(index) = args.iter().position(|arg| arg == "--sector-size") {
            args[index + 1] = "512".into();
        } else {
            args.extend(["--sector-size".into(), "512".into()]);
        }
    }
    args
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

#[cfg(target_os = "dragonfly")]
pub(crate) fn dragonfly_veracrypt_cipher(cipher: CreateCipher) -> String {
    // DragonFly uses the same normal VeraCrypt XTS256 profile via libtcplay.
    format!(
        "{}-256-XTS",
        cipher_profile(cipher)
            .veracrypt_encryption
            .to_ascii_uppercase()
    )
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

// Native APFS volume profiles are delegated to diskutil apfs with a Disk user password.
// macOS uses VeraCrypt's native formatter instead of Linux's Btrfs step.
#[cfg(target_os = "macos")]
pub(crate) fn macos_veracrypt_create_args(
    cipher: CreateCipher,
    size_mib: u64,
    keyfile: Option<&std::path::Path>,
    filesystem: Option<crate::CreateFilesystem>,
) -> crate::AppResult<Vec<OsString>> {
    let mut args: Vec<OsString> = VERACRYPT_CREATE_ARGS_COMMON
        .iter()
        .filter(|arg| **arg != "--filesystem=none")
        .map(OsString::from)
        .collect();
    args.extend([
        OsString::from(format!(
            "--filesystem={}",
            macos_filesystem(filesystem, "exFAT")?
        )),
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
pub(crate) fn macos_apple_create_args(
    size_mib: u64,
    filesystem: Option<crate::CreateFilesystem>,
) -> crate::AppResult<Vec<OsString>> {
    use crate::CreateFilesystem;
    let mut args: Vec<OsString> = vec!["create".into()];
    if matches!(
        filesystem,
        Some(CreateFilesystem::Ext4 | CreateFilesystem::Ntfs | CreateFilesystem::Zfs)
    ) {
        // The adapter supplies only an identity-checked zero-only native source
        // device. Crypto arguments stay in this fixed profile.
        args.extend(["-format".into(), "UDRW".into()]);
    } else {
        args.extend([
            "-size".into(),
            format!("{size_mib}m").into(),
            "-type".into(),
            "UDIF".into(),
            "-fs".into(),
            match filesystem {
                Some(CreateFilesystem::Fat32) => "MS-DOS FAT32",
                _ => macos_filesystem(filesystem, "APFS")?,
            }
            .into(),
            "-volname".into(),
            "Lurker".into(),
        ]);
    }
    args.extend(["-encryption".into(), "AES-256".into(), "-stdinpass".into()]);
    Ok(args)
}

#[cfg(target_os = "macos")]
fn macos_filesystem(
    filesystem: Option<crate::CreateFilesystem>,
    default: &'static str,
) -> crate::AppResult<&'static str> {
    use crate::CreateFilesystem;
    match filesystem {
        None => Ok(default),
        Some(CreateFilesystem::Apfs) => Ok("APFS"),
        Some(CreateFilesystem::Exfat) => Ok("exFAT"),
        Some(CreateFilesystem::Fat32) => Ok("FAT"),
        Some(other) => Err(crate::AppError::new(format!(
            "Filesystem {} is not available through the native macOS container formatter.",
            other.as_str()
        ))),
    }
}

#[cfg(all(test, target_os = "macos"))]
mod macos_profile_tests {
    use super::*;

    #[test]
    fn apple_native_and_external_filesystems_keep_one_fixed_crypto_profile() {
        for filesystem in [
            None,
            Some(crate::CreateFilesystem::Exfat),
            Some(crate::CreateFilesystem::Ext4),
            Some(crate::CreateFilesystem::Ntfs),
            Some(crate::CreateFilesystem::Zfs),
        ] {
            let args = macos_apple_create_args(768, filesystem).unwrap();
            let encryption = args.iter().position(|arg| arg == "-encryption").unwrap();
            assert_eq!(args[encryption + 1], "AES-256");
            assert_eq!(args.last().unwrap(), "-stdinpass");
            assert_eq!(args.iter().filter(|arg| *arg == "-encryption").count(), 1);
            assert!(!args.iter().any(|arg| arg == "-srcdevice"));
            assert_eq!(
                args.iter().any(|arg| arg == "-fs"),
                filesystem.is_none() || filesystem == Some(crate::CreateFilesystem::Exfat)
            );
        }
        assert!(macos_apple_create_args(768, Some(crate::CreateFilesystem::Xfs)).is_err());
    }
}
