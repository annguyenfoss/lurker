use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

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
}

impl VolumeType {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "auto" => Some(Self::Auto),
            "luks" => Some(Self::Luks),
            "veracrypt" => Some(Self::Veracrypt),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Luks => "luks",
            Self::Veracrypt => "veracrypt",
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
