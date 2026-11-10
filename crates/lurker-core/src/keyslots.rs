use crate::linux::{absolute_path, is_block_device_target, source_kind_for_path};
use crate::system::AppContext;
use crate::{AppError, AppResult, CreateCipher, KeyslotOperation, KeyslotsCommand};
use std::ffi::OsString;
use std::fs::File;
use std::io::{IsTerminal, Read};
use std::path::Path;

struct Layout {
    version: u16,
    slots: Vec<u8>,
    bound: Vec<u8>,
    cipher: CreateCipher,
}

fn parse_luks2(bytes: &[u8]) -> AppResult<Layout> {
    let metadata: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|e| AppError::new(format!("Invalid LUKS2 metadata: {e}")))?;
    let entries = metadata["keyslots"]
        .as_object()
        .ok_or_else(|| AppError::new("LUKS2 metadata has no keyslot table."))?;
    let mut slots = Vec::new();
    for id in entries.keys() {
        let slot = id
            .parse::<u8>()
            .ok()
            .filter(|slot| *slot < 32)
            .ok_or_else(|| AppError::new("Invalid LUKS2 keyslot ID."))?;
        slots.push(slot);
    }
    slots.sort_unstable();
    let digests = metadata["digests"]
        .as_object()
        .ok_or_else(|| AppError::new("LUKS2 metadata has no digest table."))?;
    let mut bound = Vec::new();
    for digest in digests.values() {
        if !digest["segments"]
            .as_array()
            .is_some_and(|segments| !segments.is_empty())
        {
            continue;
        }
        for id in digest["keyslots"].as_array().into_iter().flatten() {
            if let Some(slot) = id.as_str().and_then(|id| id.parse::<u8>().ok()) {
                if slots.contains(&slot) && !bound.contains(&slot) {
                    bound.push(slot);
                }
            }
        }
    }
    let encryption = metadata["segments"]
        .as_object()
        .into_iter()
        .flat_map(|segments| segments.values())
        .find_map(|segment| segment["encryption"].as_str())
        .unwrap_or("");
    Ok(Layout {
        version: 2,
        slots,
        bound,
        cipher: cipher(encryption),
    })
}

fn cipher(value: &str) -> CreateCipher {
    if value.starts_with("serpent") {
        CreateCipher::Serpent
    } else if value.starts_with("twofish") {
        CreateCipher::Twofish
    } else {
        CreateCipher::Aes
    }
}

fn parse_luks1(text: &str) -> AppResult<Layout> {
    if !text.lines().any(|line| {
        line.trim()
            .strip_prefix("Version:")
            .is_some_and(|v| v.trim() == "1")
    }) {
        return Err(AppError::new("Source is not a supported LUKS container."));
    }
    let mut slots = Vec::new();
    for line in text.lines() {
        if let Some(line) = line.trim().strip_prefix("Key Slot ") {
            if let Some((id, status)) = line.split_once(':') {
                if status.trim() == "ENABLED" {
                    let id = id
                        .parse::<u8>()
                        .ok()
                        .filter(|id| *id < 8)
                        .ok_or_else(|| AppError::new("Invalid LUKS1 keyslot ID."))?;
                    slots.push(id);
                }
            }
        }
    }
    let encryption = text
        .lines()
        .find_map(|line| line.trim().strip_prefix("Cipher name:"))
        .unwrap_or("")
        .trim();
    Ok(Layout {
        version: 1,
        bound: slots.clone(),
        slots,
        cipher: cipher(encryption),
    })
}

fn inspect(ctx: &mut AppContext, source: &Path) -> AppResult<Layout> {
    let tool = ctx.cryptsetup_path().to_path_buf();
    let elevated = is_block_device_target(source);
    let result = ctx.capture_command(
        &tool,
        &[
            "luksDump".into(),
            "--dump-json-metadata".into(),
            source.as_os_str().into(),
        ],
        elevated,
        "Failed to inspect LUKS keyslots",
    )?;
    if result.status.success() {
        return parse_luks2(&result.stdout);
    }
    let result = ctx.capture_command(
        &tool,
        &["luksDump".into(), source.as_os_str().into()],
        elevated,
        "Failed to inspect LUKS header",
    )?;
    if !result.status.success() {
        return Err(AppError::new(
            "Cannot read a LUKS header from this source. Check the source and permissions.",
        ));
    }
    parse_luks1(&String::from_utf8_lossy(&result.stdout))
}

fn credential(
    supplied: Option<&str>,
    keyfile: Option<&Path>,
    prompt: &str,
    confirm: bool,
) -> AppResult<Vec<u8>> {
    if let Some(path) = keyfile {
        let mut bytes = Vec::new();
        File::open(path)
            .and_then(|file| file.take(8 * 1024 * 1024 + 1).read_to_end(&mut bytes))
            .map_err(|e| AppError::io("Failed to read keyfile", e))?;
        if bytes.is_empty() || bytes.len() > 8 * 1024 * 1024 {
            return Err(AppError::new(
                "LUKS keyfile must contain between 1 byte and 8 MiB.",
            ));
        }
        return Ok(bytes);
    }
    let value = match supplied {
        Some(value) => value.to_owned(),
        None => {
            if !std::io::stdin().is_terminal() {
                return Err(AppError::new("Password entry requires a terminal or the desktop app; alternatively select a keyfile."));
            }
            let value = crate::credentials::prompt_passphrase(prompt)
                .map_err(|e| AppError::io("Failed to read password", e))?;
            if confirm {
                let check = crate::credentials::prompt_passphrase("Confirm new password: ")
                    .map_err(|e| AppError::io("Failed to confirm password", e))?;
                if value != check {
                    return Err(AppError::new("New password confirmation does not match."));
                }
            }
            value
        }
    };
    crate::credentials::validate(Some(&value), None)?;
    Ok(value.into_bytes())
}

pub(crate) fn run(ctx: &mut AppContext, c: KeyslotsCommand) -> AppResult<()> {
    if source_kind_for_path(&c.source).is_none() {
        return Err(AppError::new(
            "LUKS source must be a regular file or block device.",
        ));
    }
    let source = absolute_path(&c.source)?;
    let layout = inspect(ctx, &source)?;
    if c.slot
        .is_some_and(|slot| slot >= if layout.version == 1 { 8 } else { 32 })
    {
        return Err(AppError::new(format!(
            "LUKS{} keyslot is out of range.",
            layout.version
        )));
    }
    if c.operation == KeyslotOperation::List {
        let slots = layout
            .slots
            .iter()
            .map(|slot| {
                if layout.bound.contains(slot) {
                    slot.to_string()
                } else {
                    format!("{slot} (unbound)")
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        ctx.output.msg(&format!(
            "LUKS{} active keyslots: {}",
            layout.version,
            if slots.is_empty() { "none" } else { &slots }
        ));
        return Ok(());
    }
    if c.operation == KeyslotOperation::Add {
        if c.slot.is_some_and(|slot| layout.slots.contains(&slot)) {
            return Err(AppError::new("Selected keyslot is already occupied."));
        }
        if layout.slots.len() >= if layout.version == 1 { 8 } else { 32 } {
            return Err(AppError::new("All LUKS keyslots are occupied."));
        }
    } else {
        let slot = c
            .slot
            .ok_or_else(|| AppError::new("Choose the keyslot to remove."))?;
        if !layout.slots.contains(&slot) {
            return Err(AppError::new("Selected keyslot is not active."));
        }
        if layout.bound.iter().filter(|id| **id != slot).count() == 0 {
            return Err(AppError::new(
                "Refusing to remove the last usable LUKS keyslot.",
            ));
        }
    }
    let mut input = credential(
        c.passphrase.as_deref(),
        c.keyfile.as_deref(),
        "Existing password: ",
        false,
    )?;
    let mut args: Vec<OsString> = vec![
        if c.operation == KeyslotOperation::Add {
            "luksAddKey"
        } else {
            "luksKillSlot"
        }
        .into(),
        "--key-file".into(),
        "-".into(),
        "--keyfile-size".into(),
        input.len().to_string().into(),
    ];
    if c.operation == KeyslotOperation::Add {
        let new = credential(
            c.new_passphrase.as_deref(),
            c.new_keyfile.as_deref(),
            "New password: ",
            true,
        )?;
        args.extend(crate::workflow::luks_keyslot_kdf_args(
            layout.cipher,
            layout.version,
        ));
        args.extend([
            "--new-keyfile".into(),
            "-".into(),
            "--new-keyfile-size".into(),
            new.len().to_string().into(),
        ]);
        if let Some(slot) = c.slot {
            // Compatible with cryptsetup versions predating --new-key-slot.
            args.extend(["--key-slot".into(), slot.to_string().into()]);
        }
        input.extend_from_slice(&new);
    } else {
        // stdin implies batch mode in cryptsetup. Explicit verification keeps
        // removal authenticated and requires a credential for a surviving slot.
        args.push("--verify-passphrase".into());
    }
    args.push(source.as_os_str().into());
    if c.operation == KeyslotOperation::Remove {
        args.push(c.slot.unwrap().to_string().into());
    }
    let tool = ctx.cryptsetup_path().to_path_buf();
    ctx.run_command_with_input(
        &tool,
        &args,
        is_block_device_target(&source),
        &input,
        "Failed to update LUKS keyslots",
    )?;
    ctx.output.success(if c.operation == KeyslotOperation::Add {
        "Added an independent LUKS unlock credential."
    } else {
        "Removed the selected LUKS keyslot."
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_distinguishes_bound_slots_and_luks_versions() {
        let layout = parse_luks2(br#"{"keyslots":{"0":{},"10":{},"2":{}},"digests":{"0":{"keyslots":["0","10"],"segments":["0"]},"1":{"keyslots":["2"],"segments":[]}},"segments":{"0":{"encryption":"twofish-xts-plain64"}}}"#).unwrap();
        assert_eq!(layout.slots, [0, 2, 10]);
        assert_eq!(layout.bound, [0, 10]);
        assert_eq!(layout.cipher, CreateCipher::Twofish);
        let layout = parse_luks1("Version: 1\nCipher name: serpent\nKey Slot 0: ENABLED\nKey Slot 1: DISABLED\nKey Slot 7: ENABLED\n").unwrap();
        assert_eq!(layout.slots, [0, 7]);
        assert_eq!(layout.bound, layout.slots);
        assert_eq!(layout.cipher, CreateCipher::Serpent);
        assert!(parse_luks1("Version: 2").is_err());
        assert!(parse_luks2(br#"{"keyslots":{"32":{}}}"#).is_err());
    }
}
