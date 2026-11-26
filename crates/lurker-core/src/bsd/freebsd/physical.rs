//! GEOM inspection never opens an active encrypted backing provider.
use super::{args, exec};
use crate::{AppError, AppResult};
use std::collections::HashSet;
use std::fs;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::Path;

#[derive(Debug, PartialEq)]
struct Node {
    class: String,
    name: String,
    bytes: u64,
    sector: u32,
    parent: Option<usize>,
}

fn invalid() -> AppError {
    AppError::new("Unrecognized or ambiguous native FreeBSD GEOM identity.")
}

fn provider_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('-')
        && name.len() <= 255
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-./".contains(&b))
        && name.split('/').all(|part| !matches!(part, "" | "." | ".."))
}

fn topology(text: &str) -> AppResult<Vec<Node>> {
    let mut nodes = Vec::new();
    let mut stack = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let words: Vec<_> = line.split_whitespace().collect();
        if words.len() < 5 || !provider_name(words[2]) {
            return Err(invalid());
        }
        let depth = words[0].parse::<usize>().map_err(|_| invalid())?;
        if depth > stack.len() || depth > 64 {
            return Err(invalid());
        }
        stack.truncate(depth);
        nodes.push(Node {
            class: words[1].into(),
            name: words[2].into(),
            bytes: words[3].parse().map_err(|_| invalid())?,
            sector: words[4].parse().map_err(|_| invalid())?,
            parent: stack.last().copied(),
        });
        stack.push(nodes.len() - 1);
    }
    if nodes.is_empty() {
        return Err(invalid());
    }
    Ok(nodes)
}

fn selected<'a>(nodes: &'a [Node], name: &str) -> AppResult<(usize, &'a Node, &'a Node)> {
    let matches: Vec<_> = nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.name == name)
        .collect();
    let [(index, node)] = matches.as_slice() else {
        return Err(invalid());
    };
    if !matches!(node.class.as_str(), "DISK" | "PART")
        || node.bytes == 0
        || !(512..=65536).contains(&node.sector)
        || !node.sector.is_power_of_two()
        || node.bytes % u64::from(node.sector) != 0
    {
        return Err(invalid());
    }
    let mut root = *node;
    while let Some(parent) = root.parent {
        if root.class != "PART" {
            return Err(invalid());
        }
        root = &nodes[parent];
    }
    if root.class != "DISK" {
        return Err(invalid());
    }
    // Never format an entire disk through an existing partition table.
    if node.class == "DISK"
        && nodes
            .iter()
            .any(|n| n.class == "PART" && n.parent == Some(*index))
    {
        return Err(AppError::new(
            "Select a FreeBSD data partition; the whole disk has a partition table.",
        ));
    }
    Ok((*index, node, root))
}

fn section<'a>(listing: &'a str, name: &str) -> AppResult<(&'a str, &'a str)> {
    let (head, rest) = listing.split_once("Providers:\n").ok_or_else(invalid)?;
    let providers = rest.split_once("Consumers:\n").map_or(rest, |(p, _)| p);
    let mut selected = None;
    let mut current_start = None;
    for (offset, line) in providers.split_inclusive('\n').scan(0, |offset, line| {
        let start = *offset;
        *offset += line.len();
        Some((start, line))
    }) {
        let line = line.trim();
        if let Some((number, value)) = line.split_once(". Name: ") {
            if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
                return Err(invalid());
            }
            if let Some((start, matched)) = current_start.take() {
                if matched {
                    if selected.replace(&providers[start..offset]).is_some() {
                        return Err(invalid());
                    }
                }
            }
            current_start = Some((offset, value == name));
        }
    }
    if let Some((start, true)) = current_start {
        if selected.replace(&providers[start..]).is_some() {
            return Err(invalid());
        }
    }
    Ok((head, selected.ok_or_else(invalid)?))
}

fn field<'a>(text: &'a str, key: &str) -> AppResult<Option<&'a str>> {
    let prefix = format!("{key}:");
    let values: Vec<_> = text
        .lines()
        .filter_map(|line| line.trim().strip_prefix(&prefix).map(str::trim))
        .collect();
    match values.as_slice() {
        [] => Ok(None),
        [value] => Ok(Some(*value)),
        _ => Err(invalid()),
    }
}

fn details(listing: &str, node: &Node) -> AppResult<(String, String, Option<String>)> {
    let (head, body) = section(listing, &node.name)?;
    if field(head, "Geom class")? != Some(node.class.as_str()) {
        return Err(invalid());
    }
    let bytes = field(body, "Mediasize")?
        .and_then(|s| s.split_whitespace().next())
        .and_then(|s| s.parse::<u64>().ok());
    let sector = field(body, "Sectorsize")?.and_then(|s| s.parse::<u32>().ok());
    if bytes != Some(node.bytes) || sector != Some(node.sector) {
        return Err(invalid());
    }
    let mode = field(body, "Mode")?.ok_or_else(invalid)?;
    let Some((read, rest)) = mode.strip_prefix('r').and_then(|s| s.split_once('w')) else {
        return Err(invalid());
    };
    let Some((write, exclusive)) = rest.split_once('e') else {
        return Err(invalid());
    };
    if [read, write, exclusive]
        .iter()
        .any(|s| s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()))
    {
        return Err(invalid());
    }
    let id = if node.class == "PART" {
        let uuid = field(body, "rawuuid")?.filter(|s| !s.is_empty());
        if uuid.is_some_and(|s| {
            s.len() != 36
                || !s.bytes().enumerate().all(|(i, b)| {
                    if [8, 13, 18, 23].contains(&i) {
                        b == b'-'
                    } else {
                        b.is_ascii_hexdigit()
                    }
                })
        }) {
            return Err(invalid());
        }
        uuid.map(|s| format!("gpt:{}", s.to_ascii_lowercase()))
    } else {
        field(body, "ident")?
            .filter(|s| !s.is_empty())
            .map(|s| format!("disk:{s}"))
    };
    Ok((mode.into(), field(body, "offset")?.unwrap_or("").into(), id))
}

fn snapshot() -> AppResult<Vec<Node>> {
    topology(&exec(
        "/sbin/sysctl",
        &args(&["-n", "kern.geom.conftxt"]),
        None,
    )?)
}

fn listing(name: &str) -> AppResult<String> {
    if !provider_name(name) {
        return Err(invalid());
    }
    exec("/sbin/geom", &args(&["-p", name]), None)
}

pub(in super::super) fn physical_device(source: &Path) -> AppResult<super::super::PhysicalDevice> {
    let source = fs::canonicalize(source)
        .map_err(|e| AppError::io("Resolve FreeBSD physical provider", e))?;
    let name = source
        .strip_prefix("/dev")
        .ok()
        .and_then(Path::to_str)
        .filter(|name| provider_name(name) && !name.contains('/'))
        .ok_or_else(|| {
            AppError::new("Select a canonical FreeBSD DISK or PART provider under /dev.")
        })?;
    let before = fs::symlink_metadata(&source)
        .map_err(|e| AppError::io("Inspect FreeBSD physical provider", e))?;
    if !before.file_type().is_char_device() || before.uid() != 0 {
        return Err(AppError::new(
            "The FreeBSD physical source must be a root-owned native character device.",
        ));
    }
    let nodes = snapshot()?;
    let (_, node, root) = selected(&nodes, name)?;
    let info = details(&listing(name)?, node)?;
    let disk_id = details(&listing(&root.name)?, root)?.2;
    let confirmed_nodes = snapshot()?;
    let (_, confirmed, confirmed_root) = selected(&confirmed_nodes, name)?;
    let confirmed_info = details(&listing(name)?, confirmed)?;
    let confirmed_disk_id = details(&listing(&confirmed_root.name)?, confirmed_root)?.2;
    let after = fs::symlink_metadata(&source)
        .map_err(|e| AppError::io("Recheck FreeBSD physical provider", e))?;
    if (before.dev(), before.ino(), before.rdev()) != (after.dev(), after.ino(), after.rdev())
        || (
            node.class.as_str(),
            node.bytes,
            node.sector,
            root.name.as_str(),
        ) != (
            confirmed.class.as_str(),
            confirmed.bytes,
            confirmed.sector,
            confirmed_root.name.as_str(),
        )
        || disk_id != confirmed_disk_id
        || (info.1.as_str(), &info.2) != (confirmed_info.1.as_str(), &confirmed_info.2)
    {
        return Err(AppError::new(
            "The FreeBSD physical provider changed during inspection.",
        ));
    }
    let metadata_id = match (info.2, disk_id) {
        (Some(part), Some(disk)) if node.class == "PART" => Some(format!("{part};{disk}")),
        (Some(id), _) | (_, Some(id)) => Some(id),
        _ => None,
    }
    .map(|id| format!("{id};offset:{};sector:{}", info.1, node.sector));
    Ok(super::super::PhysicalDevice {
        source,
        bytes: node.bytes,
        metadata_id,
    })
}

fn root_disks(
    name: &str,
    found: &mut HashSet<String>,
    seen: &mut HashSet<String>,
) -> AppResult<()> {
    if !seen.insert(name.into()) {
        return Ok(());
    }
    if seen.len() > 128 {
        return Err(invalid());
    }
    let text = listing(name)?;
    let (head, _) = section(&text, name)?;
    if field(head, "Geom class")? == Some("DISK") {
        found.insert(name.into());
        return Ok(());
    }
    let (_, consumers) = text.split_once("Consumers:\n").ok_or_else(invalid)?;
    let names: Vec<_> = consumers
        .lines()
        .filter_map(|line| line.trim().split_once(". Name: ").map(|(_, value)| value))
        .collect();
    if names.is_empty() {
        return Err(invalid());
    }
    for parent in names {
        root_disks(parent, found, seen)?;
    }
    Ok(())
}

pub(in super::super) fn ensure_physical_idle(source: &Path) -> AppResult<()> {
    let physical = physical_device(source)?;
    let name = physical
        .source
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(invalid)?;
    let nodes = snapshot()?;
    let (_, node, root) = selected(&nodes, name)?;
    if details(&listing(name)?, node)?.0 != "r0w0e0" {
        return Err(AppError::new(
            "The FreeBSD physical provider is mounted, mapped, or open by another process.",
        ));
    }
    let mut protected = HashSet::new();
    let mut seen = HashSet::new();
    let mut pools = HashSet::new();
    for mount in super::super::mounts()? {
        let target = mount.target.to_str().unwrap_or("");
        if !matches!(
            target,
            "/" | "/usr" | "/usr/local" | "/var" | "/home" | "/tmp"
        ) && !mount.target.starts_with("/boot")
        {
            continue;
        }
        if let Ok(path) = mount.source.strip_prefix("/dev") {
            root_disks(
                path.to_str().ok_or_else(invalid)?,
                &mut protected,
                &mut seen,
            )?;
        } else if mount.filesystem == "zfs" {
            let pool = mount
                .source
                .to_str()
                .and_then(|s| s.split('/').next())
                .ok_or_else(invalid)?;
            if pools.insert(pool.to_owned()) {
                let text = exec("/sbin/zpool", &args(&["status", "-P", pool]), None)?;
                let paths: Vec<_> = text
                    .lines()
                    .filter_map(|line| line.split_whitespace().next())
                    .filter_map(|p| p.strip_prefix("/dev/"))
                    .collect();
                if paths.is_empty() {
                    return Err(invalid());
                }
                for path in paths {
                    root_disks(path, &mut protected, &mut seen)?;
                }
            }
        }
    }
    let swaps = exec("/usr/sbin/swapinfo", &args(&["-k"]), None)?;
    for name in swaps
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .filter_map(|p| p.strip_prefix("/dev/"))
    {
        root_disks(name, &mut protected, &mut seen)?;
    }
    if protected.contains(&root.name) {
        return Err(AppError::new(
            "The physical provider belongs to an operating-system, boot, or swap disk.",
        ));
    }
    // Check native write access without changing any bytes. Close this descriptor
    // before GELI takes exclusive GEOM ownership; O_EXCL is not a FreeBSD lease.
    let writable = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&physical.source)
        .map_err(|e| AppError::io("The FreeBSD physical provider is not writable", e))?;
    drop(writable);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const TREE: &str =
        "0 DISK da7 1073741824 512\n1 PART da7p1 536870912 512 i 1\n0 DISK nda2 2147483648 4096\n";
    #[test]
    fn only_real_disks_and_partitions_with_unambiguous_geometry_are_accepted() {
        let nodes = topology(TREE).unwrap();
        assert_eq!(selected(&nodes, "da7p1").unwrap().2.name, "da7");
        assert!(selected(&nodes, "da7").is_err());
        assert!(selected(&nodes, "nda2").is_ok());
        assert!(
            selected(
                &topology(&(TREE.to_owned() + "0 MD md0 1024 512\n")).unwrap(),
                "md0"
            )
            .is_err()
        );
        assert!(topology("2 PART da7p1 1024 512\n").is_err());
        assert!(
            selected(
                &topology(&(TREE.to_owned() + "0 DISK nda2 2048 512\n")).unwrap(),
                "nda2"
            )
            .is_err()
        );
        assert!(selected(&topology(&TREE.replace("4096", "513")).unwrap(), "nda2").is_err());
    }
    #[test]
    fn geometry_and_mode_must_belong_to_the_exact_provider() {
        let nodes = topology(TREE).unwrap();
        let (_, part, _) = selected(&nodes, "da7p1").unwrap();
        let text = "Geom class: PART\nGeom name: da7\nProviders:\n1. Name: da7p2\n Mediasize: 999\n Sectorsize: 4096\n Mode: r1w1e1\n2. Name: da7p1\n Mediasize: 536870912 (512M)\n Sectorsize: 512\n Mode: r0w0e0\n rawuuid: 12345678-1234-1234-1234-123456789abc\n offset: 1048576\nConsumers:\n1. Name: da7\n";
        let info = details(text, part).unwrap();
        assert_eq!(info.0, "r0w0e0");
        assert_eq!(info.1, "1048576");
        assert!(info.2.unwrap().starts_with("gpt:"));
        assert!(details(&text.replace(" Sectorsize: 512", " Sectorsize: 513"), part).is_err());
        assert!(
            details(
                &text.replace(" Mode: r0w0e0", " Mode: r0w0e0\n Mode: r1w1e1"),
                part
            )
            .is_err()
        );
    }
    #[test]
    #[ignore = "requires the verified disposable FreeBSD physical partition"]
    fn native_physical_identity_is_read_only_and_rejects_system_devices() {
        let source = std::path::PathBuf::from(
            std::env::var_os("LURKER_FREEBSD_PHYSICAL_TEST_DEVICE").unwrap(),
        );
        let info = physical_device(&source).unwrap();
        assert_eq!(info.source, source);
        assert_eq!(info.bytes, 512 * 1024 * 1024);
        assert!(info.metadata_id.as_ref().unwrap().contains("gpt:"));
        ensure_physical_idle(&source).unwrap();
        if let Some(key) = std::env::var_os("LURKER_FREEBSD_PHYSICAL_TEST_KEY") {
            assert_eq!(super::super::probe_image(&source).unwrap(), Some(512));
            let credential = super::super::Credential::Keyfile(key.into());
            let device = source.to_str().unwrap();
            let mapped = super::super::unlock(device, false, &credential).unwrap();
            let active = physical_device(&source);
            let busy = ensure_physical_idle(&source);
            let mapping = super::super::verify_mapping(&mapped, device);
            let backing = super::super::verify_backing(device, &source);
            let detached = super::super::close(&mapped);
            let active = active.unwrap();
            assert_eq!(active.bytes, info.bytes);
            assert_eq!(active.metadata_id, info.metadata_id);
            assert!(busy.is_err());
            mapping.unwrap();
            backing.unwrap();
            detached.unwrap();
            ensure_physical_idle(&source).unwrap();
        }
        assert!(physical_device(Path::new("/dev/null")).is_err());
        assert!(ensure_physical_idle(Path::new("/dev/vtbd0p1")).is_err());
        assert!(ensure_physical_idle(Path::new("/dev/vtbd1")).is_err());
    }
}
