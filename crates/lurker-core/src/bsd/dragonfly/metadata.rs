//! The supported LUKS1 metadata envelope consumed by DragonFly's base reader.
use super::{AppError, AppResult};

pub(super) const SECTOR_BYTES: u64 = 512;
pub(super) const HEADER_BYTES: usize = 592;
const KEY_BYTES: u32 = 64;
const STRIPES: u32 = 4000;
const SLOT_SECTORS: u64 = (KEY_BYTES as u64 * STRIPES as u64).div_ceil(SECTOR_BYTES);
const MAX_METADATA_SECTORS: u64 = 16 * 1024 * 1024 / SECTOR_BYTES;

pub(super) struct Geometry {
    pub(super) metadata: u64,
    pub(super) total: u64,
}

fn invalid() -> AppError {
    AppError::new("Container does not have supported DragonFly LUKS1 metadata.")
}

fn word(header: &[u8; HEADER_BYTES], offset: usize) -> u32 {
    u32::from_be_bytes(header[offset..offset + 4].try_into().unwrap())
}

fn field(bytes: &[u8]) -> Option<&[u8]> {
    let end = bytes.iter().position(|byte| *byte == 0)?;
    Some(&bytes[..end])
}

fn supported_uuid(bytes: &[u8]) -> bool {
    field(bytes).is_some_and(|uuid| {
        uuid.len() == 36
            && uuid.iter().enumerate().all(|(i, byte)| {
                if matches!(i, 8 | 13 | 18 | 23) {
                    *byte == b'-'
                } else {
                    byte.is_ascii_hexdigit()
                }
            })
    })
}

pub(super) fn inspect(header: &[u8; HEADER_BYTES], size: u64) -> AppResult<Geometry> {
    if !header.starts_with(b"LUKS\xba\xbe\x00\x01")
        || !matches!(
            field(&header[8..40]),
            Some(b"aes" | b"serpent" | b"twofish")
        )
        || field(&header[40..72]) != Some(b"xts-plain64")
        || field(&header[72..104]) != Some(b"sha256")
        || word(header, 108) != KEY_BYTES
        || !supported_uuid(&header[168..208])
        || !(1..=i32::MAX as u32).contains(&word(header, 164))
        || size % SECTOR_BYTES != 0
    {
        return Err(invalid());
    }

    let metadata = u64::from(word(header, 104));
    let total = size / SECTOR_BYTES;
    if metadata > MAX_METADATA_SECTORS || metadata >= total {
        return Err(invalid());
    }

    // A keyslot can only refer into the captured metadata, never into the
    // original image. The native implementation uses a fixed stripe length.
    let mut locations = [0; 8];
    let mut active = false;
    for (index, position) in locations.iter_mut().enumerate() {
        let slot = 208 + index * 48;
        match word(header, slot) {
            0x00ac71f3 if (1..=i32::MAX as u32).contains(&word(header, slot + 4)) => {
                active = true;
            }
            0x0000dead => {}
            _ => return Err(invalid()),
        }
        *position = u64::from(word(header, slot + 40));
        if word(header, slot + 44) != STRIPES
            || *position < (HEADER_BYTES as u64).div_ceil(SECTOR_BYTES)
            || *position + SLOT_SECTORS > metadata
        {
            return Err(invalid());
        }
    }
    locations.sort_unstable();
    if !active
        || locations
            .windows(2)
            .any(|pair| pair[0] + SLOT_SECTORS > pair[1])
    {
        return Err(invalid());
    }
    Ok(Geometry { metadata, total })
}
