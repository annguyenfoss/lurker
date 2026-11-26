use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

/// Linux operations either use the kernel's privileged device stack or a
/// file-backed userspace stack. Existing requests retain their original mode.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivilegeMode {
    #[default]
    Privileged,
    Unprivileged,
}

impl PrivilegeMode {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "privileged" => Some(Self::Privileged),
            "unprivileged" => Some(Self::Unprivileged),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

impl ColorMode {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "auto" => Some(Self::Auto),
            "always" => Some(Self::Always),
            "never" => Some(Self::Never),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VolumeType {
    Auto,
    Luks,
    Veracrypt,
    Apple,
    Apfs,
    Luks1,
    Geli,
    Cgd,
    Softraid,
    Raid1c,
    Pefs,
    Gocryptfs,
    Cryfs,
    Cryptomator,
}

impl VolumeType {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "auto" => Some(Self::Auto),
            "luks" => Some(Self::Luks),
            "veracrypt" => Some(Self::Veracrypt),
            "apple" => Some(Self::Apple),
            "apfs" => Some(Self::Apfs),
            "luks1" => Some(Self::Luks1),
            "geli" => Some(Self::Geli),
            "cgd" => Some(Self::Cgd),
            "softraid" => Some(Self::Softraid),
            "raid1c" => Some(Self::Raid1c),
            "pefs" => Some(Self::Pefs),
            "gocryptfs" => Some(Self::Gocryptfs),
            "cryfs" => Some(Self::Cryfs),
            "cryptomator" => Some(Self::Cryptomator),

            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Luks => "luks",
            Self::Veracrypt => "veracrypt",
            Self::Apple => "apple",
            Self::Apfs => "apfs",
            Self::Luks1 => "luks1",
            Self::Geli => "geli",
            Self::Cgd => "cgd",
            Self::Softraid => "softraid",
            Self::Raid1c => "raid1c",
            Self::Pefs => "pefs",
            Self::Gocryptfs => "gocryptfs",
            Self::Cryfs => "cryfs",
            Self::Cryptomator => "cryptomator",
        }
    }
}

impl Display for VolumeType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreateCipher {
    #[default]
    Aes,
    Serpent,
    Twofish,
}

impl CreateCipher {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "aes" => Some(Self::Aes),
            "serpent" => Some(Self::Serpent),
            "twofish" => Some(Self::Twofish),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Aes => "aes",
            Self::Serpent => "serpent",
            Self::Twofish => "twofish",
        }
    }
}

impl Display for CreateCipher {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    File,
    Block,
    Directory,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolvedTargetKind {
    Mapper,
    Mountpoint,
    Block,
    File,
}

impl ResolvedTargetKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mapper => "mapper",
            Self::Mountpoint => "mountpoint",
            Self::Block => "block",
            Self::File => "file",
        }
    }
}

impl Display for ResolvedTargetKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl VolumeType {
    pub fn default_create() -> Self {
        if cfg!(target_os = "macos") {
            Self::Apple
        } else if cfg!(target_os = "freebsd") {
            Self::Geli
        } else if cfg!(target_os = "netbsd") {
            Self::Cgd
        } else if cfg!(target_os = "openbsd") {
            Self::Softraid
        } else if cfg!(target_os = "dragonfly") {
            Self::Luks1
        } else {
            Self::Luks
        }
    }

    pub fn is_directory(self) -> bool {
        matches!(
            self,
            Self::Pefs | Self::Gocryptfs | Self::Cryfs | Self::Cryptomator
        )
    }

    /// Approved scope only; runtime prerequisites and operation support are
    /// checked separately by the backend. Selection never implies availability.
    pub fn supports_platform(self, os: &str) -> bool {
        match self {
            Self::Auto => matches!(
                os,
                "linux" | "macos" | "freebsd" | "dragonfly" | "netbsd" | "openbsd"
            ),
            Self::Apple | Self::Apfs => os == "macos",
            Self::Luks => os == "linux",
            Self::Luks1 => matches!(os, "linux" | "dragonfly"),
            Self::Veracrypt => {
                matches!(os, "linux" | "macos" | "freebsd" | "dragonfly" | "openbsd")
            }
            Self::Geli | Self::Pefs => os == "freebsd",
            Self::Cgd => os == "netbsd",
            Self::Softraid | Self::Raid1c => os == "openbsd",
            Self::Gocryptfs => matches!(os, "linux" | "macos" | "freebsd"),
            Self::Cryfs => matches!(os, "linux" | "macos" | "freebsd"),
            Self::Cryptomator => matches!(os, "linux" | "macos"),
        }
    }
}

#[cfg(test)]
mod scope_tests {
    use super::*;

    #[test]
    fn approved_backend_platform_pairs_are_explicit() {
        let platforms = [
            "macos",
            "linux",
            "freebsd",
            "dragonfly",
            "netbsd",
            "openbsd",
        ];
        let cases = [
            (VolumeType::Apple, [true, false, false, false, false, false]),
            (VolumeType::Apfs, [true, false, false, false, false, false]),
            (VolumeType::Luks, [false, true, false, false, false, false]),
            (VolumeType::Luks1, [false, true, false, true, false, false]),
            (VolumeType::Veracrypt, [true, true, true, true, false, true]),
            (VolumeType::Geli, [false, false, true, false, false, false]),
            (VolumeType::Cgd, [false, false, false, false, true, false]),
            (
                VolumeType::Softraid,
                [false, false, false, false, false, true],
            ),
            (
                VolumeType::Raid1c,
                [false, false, false, false, false, true],
            ),
            (VolumeType::Pefs, [false, false, true, false, false, false]),
            (
                VolumeType::Gocryptfs,
                [true, true, true, false, false, false],
            ),
            (VolumeType::Cryfs, [true, true, true, false, false, false]),
            (
                VolumeType::Cryptomator,
                [true, true, false, false, false, false],
            ),
        ];
        for (backend, allowed) in cases {
            assert_eq!(VolumeType::parse(backend.as_str()), Some(backend));
            for (os, expected) in platforms.into_iter().zip(allowed) {
                assert_eq!(backend.supports_platform(os), expected, "{backend} on {os}");
            }
            assert!(!backend.supports_platform("windows"));
        }
        for excluded in ["age", "encfs", "securefs", "zfs", "fscrypt"] {
            assert_eq!(VolumeType::parse(excluded), None);
        }
    }
}
