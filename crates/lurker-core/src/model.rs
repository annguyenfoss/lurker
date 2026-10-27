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
}

impl VolumeType {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "auto" => Some(Self::Auto),
            "luks" => Some(Self::Luks),
            "veracrypt" => Some(Self::Veracrypt),
            "apple" => Some(Self::Apple),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Luks => "luks",
            Self::Veracrypt => "veracrypt",
            Self::Apple => "apple",
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
        } else {
            Self::Luks
        }
    }
}
