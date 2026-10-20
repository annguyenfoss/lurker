// Block-device enumeration for the DevicePicker dropdown.
//
// Shells out to `lsblk -J -o PATH,LABEL,MODEL,SIZE,FSTYPE,TRAN,TYPE` and
// flattens the tree into a flat Vec<BlockDevice>. This is read-only — no
// sudo, no persistence, no caching. Returns an empty list on any error
// (the picker shows a "No devices detected" hint).

use serde::Deserialize;
use std::process::Command;

pub struct BlockDevice {
    pub path: String,
    pub label: String, // human name: MODEL or LABEL
    pub sub: String,   // meta: TRAN · SIZE · FSTYPE
    pub kind: String,  // "usb" | "ssd" | "map" | "disk"
}

#[derive(Deserialize, Debug)]
struct LsblkJson {
    blockdevices: Vec<LsblkNode>,
}

#[derive(Deserialize, Debug, Clone)]
struct LsblkNode {
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    size: Option<String>,
    #[serde(default)]
    fstype: Option<String>,
    #[serde(default)]
    tran: Option<String>,
    #[serde(default, rename = "type")]
    type_: Option<String>,
    #[serde(default)]
    children: Option<Vec<LsblkNode>>,
}

pub fn list_block_devices() -> Vec<BlockDevice> {
    let output = match Command::new("lsblk")
        .args(["-J", "-o", "PATH,LABEL,MODEL,SIZE,FSTYPE,TRAN,TYPE"])
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };
    let Ok(parsed) = serde_json::from_slice::<LsblkJson>(&output.stdout) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for root in parsed.blockdevices {
        flatten(&root, &mut out);
    }
    out
}

fn flatten(node: &LsblkNode, out: &mut Vec<BlockDevice>) {
    // Only include real paths. Skip containers (disks that have partitions)
    // — the user wants to target partitions / leaves / crypt mappers, not
    // raw whole disks.
    let leaf = node.children.as_ref().map(|c| c.is_empty()).unwrap_or(true);
    let type_ = node.type_.as_deref().unwrap_or("");
    let include = leaf && type_ != "rom" && type_ != "loop";

    if include {
        if let Some(path) = node.path.clone() {
            let label = node
                .label
                .clone()
                .or_else(|| node.model.clone())
                .unwrap_or_else(|| "(unlabeled)".to_string());
            let tran = node.tran.clone().unwrap_or_default().to_ascii_uppercase();
            let size = node.size.clone().unwrap_or_default();
            let fstype = node
                .fstype
                .clone()
                .unwrap_or_else(|| "unallocated".to_string());
            let sub = [tran.clone(), size, fstype]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" · ");
            let kind = if type_ == "crypt" || path.starts_with("/dev/mapper/") {
                "map"
            } else if tran.eq_ignore_ascii_case("USB") {
                "usb"
            } else if tran.eq_ignore_ascii_case("NVME") || type_ == "part" {
                "ssd"
            } else {
                "disk"
            }
            .to_string();

            out.push(BlockDevice {
                path,
                label,
                sub,
                kind,
            });
        }
    }

    if let Some(children) = &node.children {
        for child in children {
            flatten(child, out);
        }
    }
}
