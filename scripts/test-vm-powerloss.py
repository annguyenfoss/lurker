#!/usr/bin/env python3
"""Create isolated owned KVM copies for explicit abrupt-poweroff tests.

Never stop, pivot, redefine or repair a source guest. All virsh calls select the
system connection. State and full private evidence remain in the run directory.
"""
from __future__ import annotations

import argparse
from dataclasses import dataclass
import hashlib
import ipaddress
import json
import os
from pathlib import Path
import re
import secrets
import subprocess
import time
import uuid
import xml.etree.ElementTree as ET

URI = "qemu:///system"
ROOT = Path("/mnt/virt")
SOURCES = {name: name + "-core-01" for name in
           ("archlinux", "freebsd", "netbsd", "openbsd", "dragonflybsd")}


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def run(argv, timeout=60):
    process = subprocess.run(list(map(str, argv)), capture_output=True, text=True, timeout=timeout,
                             env={**os.environ, "LC_ALL": "C"})
    require(process.returncode == 0, f"Command failed ({process.returncode}): {argv}: {process.stderr[-2000:]}")
    return process.stdout


def virsh(*argv, timeout=60):
    return run(["virsh", "-c", URI, *argv], timeout)


def write_json(path, value):
    temporary = path.with_suffix(".tmp")
    with open(temporary, "w", opener=lambda p, f: os.open(p, f, 0o600)) as stream:
        json.dump(value, stream, indent=2)
        stream.write("\n")
        stream.flush()
        os.fsync(stream.fileno())
    temporary.replace(path)


def disk_source(xml):
    disks = ET.fromstring(xml).findall("./devices/disk")
    require(len(disks) == 1, "Source must have exactly one disk; extra media require explicit review")
    disk = disks[0]
    require(disk.get("type") == "file" and disk.get("device") == "disk", "Only file-backed source disks")
    require(disk.find("target").get("dev") == "vda", "Reviewed source disk must be vda")
    require(disk.find("driver").get("type") == "qcow2", "Source format must be qcow2")
    require(disk.find("mirror") is None, "An existing block job must never be interrupted")
    return disk.find("source").get("file")


def clone_xml(source, manifest):
    domain = ET.fromstring(source)
    require(domain.find("os/nvram") is None and domain.find("os/loader") is None,
            "Firmware writable paths require a separately reviewed private copy")
    domain.attrib.pop("id", None)
    for tag in ("uuid", "description", "metadata", "seclabel", "resource", "sysinfo", "genid", "qemu:commandline"):
        for element in domain.findall(tag):
            domain.remove(element)
    domain.find("name").text = manifest["name"]
    ET.SubElement(domain, "uuid").text = manifest["uuid"]
    ET.SubElement(domain, "description").text = manifest["marker"]
    devices = domain.find("devices")
    allowed = {"emulator", "disk", "controller", "input", "video", "memballoon", "interface"}
    for device in list(devices):
        if device.tag not in allowed:
            devices.remove(device)
    disks = devices.findall("disk")
    require(len(disks) == 1, "Clone requires exactly one private disk")
    disk = disks[0]
    for tag in ("source", "backingStore", "mirror", "auth", "iotune"):
        for element in disk.findall(tag):
            disk.remove(element)
    ET.SubElement(disk, "source", {"file": manifest["disk"]})
    for interface in devices.findall("interface"):
        devices.remove(interface)
    interface = ET.SubElement(devices, "interface", {"type": "network"})
    ET.SubElement(interface, "mac", {"address": manifest["mac"]})
    ET.SubElement(interface, "source", {"network": manifest["network"]})
    ET.SubElement(interface, "model", {"type": "virtio"})
    # Generated PTYs contain no host paths; keep a serial console for boot diagnosis.
    serial = ET.SubElement(devices, "serial", {"type": "pty"})
    ET.SubElement(serial, "target", {"port": "0"})
    console = ET.SubElement(devices, "console", {"type": "pty"})
    ET.SubElement(console, "target", {"type": "serial", "port": "0"})
    for element in domain.iter():
        for child in list(element):
            if child.tag == "alias":
                element.remove(child)
    require(all("}" not in element.tag for element in domain.iter()), "Unreviewed XML extension")
    return ET.tostring(domain, encoding="unicode")


def network_xml(manifest):
    network = ET.Element("network", {"ipv6": "no"})
    ET.SubElement(network, "name").text = manifest["network"]
    ET.SubElement(network, "uuid").text = manifest["network_uuid"]
    ET.SubElement(network, "description").text = manifest["marker"]
    ET.SubElement(network, "bridge", {"name": manifest["bridge"], "stp": "on", "delay": "0"})
    ET.SubElement(network, "dns", {"enable": "no"})
    subnet = ipaddress.ip_network(manifest["subnet"])
    address = ET.SubElement(network, "ip", {"address": str(subnet[1]), "netmask": str(subnet.netmask)})
    dhcp = ET.SubElement(address, "dhcp")
    ET.SubElement(dhcp, "range", {"start": str(subnet[2]), "end": str(subnet[250])})
    ET.SubElement(dhcp, "host", {"mac": manifest["mac"], "name": "fault-clone", "ip": str(subnet[10])})
    return ET.tostring(network, encoding="unicode")


@dataclass(frozen=True)
class OwnedClone:
    directory: Path
    manifest: dict

    @classmethod
    def load(cls, directory):
        directory = Path(directory)
        require(directory.parent == ROOT and not directory.is_symlink(), "Unowned run directory")
        require(directory.stat().st_uid == os.geteuid(), "Run directory owner changed")
        manifest = json.loads((directory / "manifest.json").read_text())
        require(re.fullmatch(r"[a-z0-9]{8,20}", manifest["run"]), "Invalid run ID")
        require(directory.name == "lurker-fault-" + manifest["run"], "Run path mismatch")
        require(manifest["os"] in SOURCES, "Unreviewed OS")
        require(manifest["source"] == SOURCES[manifest["os"]], "Source identity mismatch")
        require(manifest["name"] == directory.name + "-" + manifest["os"], "Clone name mismatch")
        require(manifest["name"] not in SOURCES.values(), "Original guests cannot be owned")
        require(manifest["disk"] == str(directory / "root.qcow2"), "Disk ownership mismatch")
        require(manifest["network"] == directory.name, "Network ownership mismatch")
        require(manifest["marker"] == "lurker-disposable-fault:" + manifest["run"], "Marker mismatch")
        return cls(directory, manifest)

    def verify_network(self):
        xml = ET.fromstring(virsh("net-dumpxml", self.manifest["network_uuid"]))
        require(xml.findtext("name") == self.manifest["network"], "Network name changed")
        require(xml.findtext("uuid") == self.manifest["network_uuid"], "Network UUID changed")
        require(xml.find("forward") is None, "Fault network must remain isolated")
        require(xml.find("bridge").get("name") == self.manifest["bridge"], "Bridge changed")
        require(xml.find("ip").get("address") == str(ipaddress.ip_network(self.manifest["subnet"])[1]),
                "Network address changed")
        return xml

    def verify_domain(self):
        domain = ET.fromstring(virsh("dumpxml", self.manifest["uuid"]))
        require(domain.findtext("uuid") == self.manifest["uuid"], "Clone UUID changed")
        require(domain.findtext("name") == self.manifest["name"], "Clone name changed")
        require(domain.findtext("description") == self.manifest["marker"], "Clone marker changed")
        require(disk_source(ET.tostring(domain, encoding="unicode")) == self.manifest["disk"], "Clone disk changed")
        interfaces = domain.findall("devices/interface")
        require(len(interfaces) == 1, "Clone must have only the isolated NIC")
        require(interfaces[0].find("source").get("network") == self.manifest["network"], "Clone network changed")
        require(interfaces[0].find("mac").get("address") == self.manifest["mac"], "Clone MAC changed")
        self.verify_network()

    def create(self):
        self.verify_network()
        xml = (self.directory / "clone.xml").read_text()
        # Compare the saved definition to a fresh deterministic derivation, so
        # editing the XML cannot smuggle in an external disk or host device.
        expected = clone_xml((self.directory / "source-inactive.xml").read_text(), self.manifest)
        require(xml == expected, "Saved clone XML changed")
        virsh("create", self.directory / "clone.xml")
        self.verify_domain()

    def poweroff(self):
        self.verify_domain()
        virsh("destroy", self.manifest["uuid"])

    def lease(self):
        self.verify_domain()
        leases = virsh("net-dhcp-leases", self.manifest["network_uuid"], "--mac", self.manifest["mac"])
        addresses = re.findall(r"\b((?:\d+\.){3}\d+/\d+)\b", leases)
        require(len(addresses) == 1, "Expected exactly one clone DHCP lease")
        address = ipaddress.ip_interface(addresses[0]).ip
        subnet = ipaddress.ip_network(self.manifest["subnet"])
        require(address in subnet and address not in (subnet[0], subnet[1], subnet[-1]), "Unsafe clone address")
        require(not address.is_loopback and address not in ipaddress.ip_network("10.11.1.0/24"),
                "Original guest address forbidden")
        return str(address)

    def ssh(self, command, user="adjutant", timeout=60):
        address = self.lease()
        # The copied host key must already be trusted for the original domain.
        return run(["ssh", "-F", "/dev/null", "-o", "StrictHostKeyChecking=yes", "-o", "CheckHostIP=no",
                    "-o", "UpdateHostKeys=no", "-o", "BatchMode=yes", "-o", "ConnectTimeout=5",
                    "-o", "HostKeyAlias=" + self.manifest["source"], user + "@" + address, command], timeout)


def prepare(os_name, run_id, subnet, bandwidth):
    require(re.fullmatch(r"[a-z0-9]{8,20}", run_id), "Run ID must be 8–20 lowercase letters/digits")
    network = ipaddress.ip_network(subnet)
    require(network.version == 4 and network.prefixlen == 24 and network.is_private, "Private IPv4 /24 required")
    require(not network.overlaps(ipaddress.ip_network("10.11.1.0/24")), "Original guest network forbidden")
    routes = json.loads(run(["ip", "-j", "route"]))
    for route in routes:
        if route.get("dst", "default") != "default":
            require(not network.overlaps(ipaddress.ip_network(route["dst"])), "Subnet overlaps a host route")
    for name in virsh("net-list", "--all", "--name").split():
        for address in ET.fromstring(virsh("net-dumpxml", name)).findall("ip"):
            existing = ipaddress.ip_network(address.get("address") + "/" + (address.get("netmask") or address.get("prefix")), strict=False)
            require(not network.overlaps(existing), "Subnet overlaps a libvirt network")
    source = SOURCES[os_name]
    inactive = virsh("dumpxml", source, "--inactive")
    live = virsh("dumpxml", source)
    source_disk = disk_source(live)
    require(disk_source(inactive) == source_disk, "Live/persistent source disks differ")
    require(virsh("domstate", source).strip() == "running", "Source must remain running")
    require("No current block job" in virsh("blockjob", source, "vda", "--info"), "Preexisting block job")
    info = virsh("domblkinfo", source, "vda")
    capacity = int(re.search(r"Capacity:\s+(\d+)", info).group(1))
    require(os.statvfs(ROOT).f_bavail * os.statvfs(ROOT).f_frsize > capacity + 10 * 1024**3,
            "Insufficient room for a full standalone source copy")
    directory = ROOT / ("lurker-fault-" + run_id)
    require(not directory.exists(), "Run already exists; use its manifest")
    manifest = {"schema": 1, "run": run_id, "os": os_name, "source": source,
                "source_uuid": ET.fromstring(inactive).findtext("uuid"), "source_disk": source_disk,
                "name": directory.name + "-" + os_name, "uuid": str(uuid.uuid4()),
                "marker": "lurker-disposable-fault:" + run_id, "disk": str(directory / "root.qcow2"),
                "network": directory.name, "network_uuid": str(uuid.uuid4()), "subnet": str(network),
                "bridge": "lf" + secrets.token_hex(5), "mac": "52:54:00:" + ":".join(secrets.token_hex(1) for _ in range(3)),
                "bandwidth_mib": bandwidth, "source_xml_sha256": hashlib.sha256(inactive.encode()).hexdigest()}
    # Traverse-only access allows libvirt's QEMU uid to use its relabeled image;
    # all manifests, XML and logs remain private to the invoking user.
    directory.mkdir(mode=0o711)
    write_json(directory / "manifest.json", manifest)
    for name, text in (("source-inactive.xml", inactive), ("source-live.xml", live),
                       ("clone.xml", clone_xml(inactive, manifest)), ("network.xml", network_xml(manifest))):
        path = directory / name
        path.write_text(text)
        path.chmod(0o600)
    return OwnedClone.load(directory)


def copy_and_boot(clone):
    m = clone.manifest
    require(not Path(m["disk"]).exists(), "Copy destination already exists")
    require(virsh("dumpxml", m["source"], "--inactive") == (clone.directory / "source-inactive.xml").read_text(), "Source XML changed")
    require(disk_source(virsh("dumpxml", m["source"])) == m["source_disk"], "Source disk changed")
    require("No current block job" in virsh("blockjob", m["source"], "vda", "--info"), "Preexisting block job")
    # --finish stops mirroring when ready and leaves the ORIGINAL disk active.
    # Never --pivot, --shallow, --reuse-external, or direct reads of live qcow2.
    command = ["virsh", "-c", URI, "blockcopy", m["source"], "vda", "--dest", m["disk"],
               "--format", "qcow2", "--bandwidth", str(m["bandwidth_mib"]), "--wait", "--finish",
               "--transient-job", "--timeout", "3600"]
    with (clone.directory / "blockcopy.log").open("w") as log:
        code = subprocess.run(command, stdout=log, stderr=subprocess.STDOUT).returncode
    require(code == 0, "Live copy failed; inspect blockcopy.log; do not abort any unidentified job")
    after = virsh("dumpxml", m["source"], "--inactive")
    (clone.directory / "source-after.xml").write_text(after)
    require(after == (clone.directory / "source-inactive.xml").read_text(), "Original definition changed")
    require(disk_source(virsh("dumpxml", m["source"])) == m["source_disk"], "Original source disk changed")
    require(virsh("domstate", m["source"]).strip() == "running", "Original stopped unexpectedly")
    require("No current block job" in virsh("blockjob", m["source"], "vda", "--info"), "Copy job still active")
    image = json.loads(run(["qemu-img", "info", "--output=json", m["disk"]]))
    require(not image.get("backing-filename"), "Clone image must have no backing file")
    write_json(clone.directory / "copy-result.json", {"status": "passed", "source_unchanged": True, "image": image})
    virsh("net-create", clone.directory / "network.xml")
    clone.create()
    print(json.dumps({"status": "booting", "directory": str(clone.directory), "name": m["name"]}), flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="action", required=True)
    plan = commands.add_parser("prepare", help="Read-only hypervisor preflight; save concrete private plan")
    plan.add_argument("--os", choices=SOURCES, required=True)
    plan.add_argument("--run", required=True)
    plan.add_argument("--subnet", required=True)
    plan.add_argument("--bandwidth", type=int, default=32)
    for action in ("copy-boot", "inspect", "poweroff", "boot", "ssh"):
        command = commands.add_parser(action)
        command.add_argument("directory", type=Path)
        if action == "ssh":
            command.add_argument("command")
            command.add_argument("--user", default="adjutant", choices=("adjutant", "root"))
    args = parser.parse_args()
    if args.action == "prepare":
        require(1 <= args.bandwidth <= 64, "Bandwidth must be 1–64 MiB/s")
        clone = prepare(args.os, args.run, args.subnet, args.bandwidth)
        print(json.dumps(clone.manifest, indent=2))
        return 0
    clone = OwnedClone.load(args.directory)
    if args.action == "copy-boot":
        copy_and_boot(clone)
    elif args.action == "poweroff":
        clone.poweroff()
    elif args.action == "boot":
        clone.create()
    elif args.action == "ssh":
        print(clone.ssh(args.command, args.user), end="")
    elif args.action == "inspect":
        clone.verify_domain()
        print(json.dumps({"status": "owned-running", "address": clone.lease(), "manifest": clone.manifest}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
