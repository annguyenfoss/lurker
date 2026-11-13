//! FreeBSD's native file-backed GELI adapter.
//!
//! The shared BSD lifecycle owns staging, publication, mount identity and
//! recovery. This module only operates on md devices returned by mdconfig;
//! it deliberately has no physical-device, forced-detach or boot integration.
use super::{Credential, args, exec};
use crate::{AppError, AppResult, CreateCipher};
use std::path::Path;

const MDCONFIG: &str = "/sbin/mdconfig";
const GELI: &str = "/sbin/geli";
const NEWFS: &str = "/sbin/newfs";

/// Unlike cryptsetup, GELI names the AES component key size here: AES-XTS
/// accepts 128 or 256, not cryptsetup's combined XTS key size of 512.
const FORMAT_ARGS: &[&str] = &[
    "init", "-B", "none", "-e", "AES-XTS", "-l", "256", "-s", "4096",
];
const PASSPHRASE_ITERATIONS: &str = "600000";

pub(super) fn attach_file(path: &Path, readonly: bool) -> AppResult<String> {
    let mut argv = args(&["-a", "-t", "vnode"]);
    if readonly {
        argv.extend(args(&["-o", "readonly"]));
    }
    argv.push("-f".into());
    argv.push(path.as_os_str().to_owned());
    let value = exec(MDCONFIG, &argv, None)?;
    let name = value.trim();
    md_unit(name)?;
    Ok(format!("/dev/{name}"))
}

pub(super) fn detach_file(device: &str) -> AppResult<()> {
    let unit = md_unit(device)?;
    exec(MDCONFIG, &args(&["-d", "-u", unit]), None)?;
    Ok(())
}

/// md unit numbers may be reused after another process detaches a device.
/// Require mdconfig's conjunction of unit and backing filename before cleanup.
pub(super) fn verify_backing(device: &str, path: &Path) -> AppResult<()> {
    let unit = md_unit(device)?;
    let mut argv = args(&["-l", "-u", unit, "-f"]);
    argv.push(path.as_os_str().to_owned());
    if exec(MDCONFIG, &argv, None)?.trim() != unit {
        return Err(AppError::new(
            "The FreeBSD md backing file changed; refusing cleanup.",
        ));
    }
    Ok(())
}

pub(super) fn verify_mapping(mapped: &str, backing: &str) -> AppResult<()> {
    let unit = md_unit(backing)?;
    if mapped != format!("/dev/{unit}.eli") {
        return Err(AppError::new(
            "The GELI provider does not match the recorded md device.",
        ));
    }
    let listing = exec(GELI, &args(&["list", &format!("{unit}.eli")]), None)?;
    if !mapping_matches(&listing, unit) {
        return Err(AppError::new("The GELI mapping changed; refusing cleanup."));
    }
    Ok(())
}

fn mapping_matches(listing: &str, unit: &str) -> bool {
    let geom = format!("Geom name: {unit}.eli");
    let consumer = format!("1. Name: {unit}");
    let Some((header, consumers)) = listing.split_once("Consumers:\n") else {
        return false;
    };
    header.lines().any(|line| line == geom)
        && header.lines().any(|line| line == "State: ACTIVE")
        && consumers
            .lines()
            .next()
            .is_some_and(|line| line == consumer)
}

pub(super) fn format(device: &str, cipher: CreateCipher, credential: &Credential) -> AppResult<()> {
    md_unit(device)?;
    if cipher != CreateCipher::Aes {
        return Err(AppError::new(
            "FreeBSD GELI uses the AES-XTS profile; Serpent and Twofish are unavailable.",
        ));
    }
    let mut argv = args(FORMAT_ARGS);
    let input = match credential {
        Credential::Password(password) => {
            argv.extend(args(&["-i", PASSPHRASE_ITERATIONS, "-J", "-"]));
            Some(password_line(password))
        }
        Credential::Keyfile(path) => {
            argv.extend(args(&["-P", "-K"]));
            argv.push(path.as_os_str().to_owned());
            None
        }
    };
    argv.push(device.into());
    exec(GELI, &argv, input.as_deref())?;
    Ok(())
}

pub(super) fn unlock(device: &str, readonly: bool, credential: &Credential) -> AppResult<String> {
    md_unit(device)?;
    let mut argv = args(&["attach"]);
    if readonly {
        argv.push("-r".into());
    }
    let input = match credential {
        Credential::Password(password) => {
            argv.extend(args(&["-j", "-"]));
            Some(password_line(password))
        }
        Credential::Keyfile(path) => {
            argv.extend(args(&["-p", "-k"]));
            argv.push(path.as_os_str().to_owned());
            None
        }
    };
    argv.push(device.into());
    exec(GELI, &argv, input.as_deref())?;
    Ok(format!("{device}.eli"))
}

pub(super) fn close(device: &str) -> AppResult<()> {
    let backing = device
        .strip_suffix(".eli")
        .ok_or_else(|| AppError::new("Expected a GELI md provider."))?;
    md_unit(backing)?;
    exec(GELI, &args(&["detach", backing]), None)?;
    Ok(())
}

pub(super) fn make_filesystem(device: &str) -> AppResult<()> {
    let backing = device
        .strip_suffix(".eli")
        .ok_or_else(|| AppError::new("Expected a GELI md provider."))?;
    md_unit(backing)?;
    exec(NEWFS, &args(&["-U", device]), None)?;
    Ok(())
}

fn md_unit(value: &str) -> AppResult<&str> {
    let name = value.strip_prefix("/dev/").unwrap_or(value);
    if name
        .strip_prefix("md")
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
    {
        Ok(name)
    } else {
        Err(AppError::new("Expected a file-backed FreeBSD md device."))
    }
}

fn password_line(password: &[u8]) -> Vec<u8> {
    let mut input = password.to_vec();
    input.push(b'\n');
    input
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_md_device_names_reach_native_tools() {
        for value in ["md0", "/dev/md123"] {
            assert!(md_unit(value).is_ok());
        }
        for value in [
            "md",
            "ada0",
            "/dev/ada0",
            "md0.eli",
            "md0\nmd1",
            "md0/../ada0",
        ] {
            assert!(md_unit(value).is_err(), "accepted {value}");
        }
    }

    #[test]
    fn password_transport_preserves_spaces_and_uses_one_line() {
        assert_eq!(password_line(b"  password  "), b"  password  \n");
    }

    #[test]
    fn mapping_identity_requires_active_geom_and_matching_consumer() {
        let value = "Geom name: md42.eli\nState: ACTIVE\nProviders:\n1. Name: md42.eli\nConsumers:\n1. Name: md42\n";
        assert!(mapping_matches(value, "md42"));
        assert!(!mapping_matches(value, "md4"));
        assert!(!mapping_matches(
            &value.replace("ACTIVE", "SUSPENDED"),
            "md42"
        ));
        assert!(!mapping_matches(
            &value.replace("1. Name: md42\n", "1. Name: md43\n"),
            "md42"
        ));
    }
}
