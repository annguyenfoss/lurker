#!/usr/bin/env python3
"""Create isolated owned KVM copies for explicit abrupt-poweroff tests.

Never stop, pivot, redefine or repair a source guest. All virsh calls select the
system connection. State and full private evidence remain in the run directory.
"""
from __future__ import annotations

import argparse
from dataclasses import dataclass
import hashlib
import importlib.util
import ipaddress
import json
import os
import platform
from pathlib import Path
import re
import secrets
import shutil
import sqlite3
import stat
import sys
import subprocess
import time
import uuid
import xml.etree.ElementTree as ET

URI = "qemu:///system"
ROOT = Path("/mnt/virt")
SOURCES = {name: name + "-core-01" for name in
           ("archlinux", "freebsd", "netbsd", "openbsd", "dragonflybsd")}

DISK_TARGETS = {name: "hda" if name == "dragonflybsd" else "vda" for name in SOURCES}
FIXTURE_DISK_TARGET = "vdb"
FIXTURE_DISK_SERIAL = "LURKERFAULTDATA"
UEFI_CODE = "/usr/share/edk2/x64/OVMF_CODE.4m.fd"
UEFI_VARS = "/usr/share/edk2/x64/OVMF_VARS.4m.fd"

DIRECTORIES = {"gocryptfs", "cryfs", "cryptomator", "pefs"}
LOGIN_BACKENDS = {"gocryptfs", "cryfs", "cryptomator"}
ALLOWED = {"Linux": {"luks", "luks1", "veracrypt", "gocryptfs", "cryfs", "cryptomator"},
           "FreeBSD": {"geli", "veracrypt", "pefs", "gocryptfs"},
           "NetBSD": {"cgd"}, "OpenBSD": {"softraid", "raid1c", "veracrypt"},
           "DragonFly": {"luks1"}}


def validate_guest_route(backend, unprivileged, cipher, system=None, uid=None):
    system = platform.system() if system is None else system
    uid = os.geteuid() if uid is None else uid
    require(backend in ALLOWED.get(system, set()), "Backend is outside this VM OS's locked matrix")
    require(not unprivileged or (system == "Linux" and backend == "veracrypt"),
            "Userspace selection is Linux VeraCrypt only")
    login = backend in LOGIN_BACKENDS or unprivileged
    require((uid != 0) if login else (uid == 0),
            "This case requires the login user" if login else "This native case requires root inside its owned clone")
    require(backend in {"luks", "luks1", "veracrypt"} or cipher == "aes",
            "This backend has a fixed create profile; alternate cipher not tested")
    return "login-user" if login else "native-root"


def verify_staged_package(cli, commit, backend):
    require(not cli.is_symlink(), "CLI entry must be a regular staged package file")
    cli = cli.resolve(strict=True)
    root = cli.parent.parent
    require(cli == root / "bin/lurker", "Use the package's bin/lurker entry")
    for path in (root, *root.parents):
        info = path.lstat()
        require(info.st_uid == 0 and not info.st_mode & 0o022 and not path.is_symlink(),
                "Stage package under a root-controlled persistent prefix")
    manifest_path = root / "share/lurker/manifest.json"
    require(not manifest_path.is_symlink(), "Package manifest symlink rejected")
    manifest = json.loads(manifest_path.read_text())
    require(manifest["source_commit"] == commit and manifest["os"] == platform.system(),
            "Staged package source/OS does not match requested candidate")
    require(backend in manifest["selected_backends"], "Backend is absent from the staged package")
    for name, entry in manifest["files"].items():
        relative = Path(name)
        require(not relative.is_absolute() and ".." not in relative.parts, "Unsafe package manifest path")
        path = root / relative
        require(path.resolve(strict=True).is_relative_to(root), "Package file escapes private root")
        if "sha256" in entry:
            require(not path.is_symlink() and hashlib.sha256(path.read_bytes()).hexdigest() == entry["sha256"],
                    "Staged package bytes changed: " + name)
        else:
            require(path.is_symlink() and os.readlink(path) == entry["symlink"], "Package symlink changed: " + name)
    require(manifest["files"]["bin/lurker"]["sha256"] == hashlib.sha256(cli.read_bytes()).hexdigest(),
            "CLI is missing from verified package manifest")
    return manifest, hashlib.sha256(manifest_path.read_bytes()).hexdigest()


def guest_prerequisites(package, backend, unprivileged):
    for requirement in package.get("host_tool_requirements", {}).get(backend, []):
        choices = requirement if isinstance(requirement, list) else [requirement]
        require(any(shutil.which(name) for name in choices), "Missing native tool prerequisite: " + "/".join(choices))
    system = platform.system()
    fuse = backend in LOGIN_BACKENDS or unprivileged or (backend == "veracrypt" and system in {"FreeBSD", "OpenBSD"})
    if fuse:
        require(any(os.access(path, os.R_OK | os.W_OK) for path in Path("/dev").glob("fuse*")),
                "No accessible FUSE device for the selected user; explicit clone prerequisite setup required")
    if system == "FreeBSD" and backend == "gocryptfs":
        require(run(["/sbin/sysctl", "-n", "vfs.usermount"]).strip() == "1",
                "FreeBSD login-user FUSE requires vfs.usermount=1 in the disposable clone")
    if backend == "pefs":
        require(Path("/boot/modules/pefs.ko").is_file(), "Missing matching FreeBSD PEFS kernel module")


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def run(argv, timeout=60, *, input_text=None):
    process = subprocess.run(list(map(str, argv)), input=input_text, capture_output=True, text=True, timeout=timeout,
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
    descriptor = os.open(path.parent, os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def disk_source(xml, target="vda"):
    disks = ET.fromstring(xml).findall("./devices/disk")
    require(len(disks) == 1, "Source must have exactly one disk; extra media require explicit review")
    disk = disks[0]
    require(disk.get("type") == "file" and disk.get("device") == "disk", "Only file-backed source disks")
    require(target in ("vda", "hda") and disk.find("target").get("dev") == target, "Reviewed source disk target changed")
    if target == "hda":
        require(disk.find("target").get("bus") == "ide", "Reviewed hda must use IDE")
    require(disk.find("driver").get("type") == "qcow2", "Source format must be qcow2")
    require(disk.find("mirror") is None, "An existing block job must never be interrupted")
    return disk.find("source").get("file")



def disk_target(manifest):
    expected = DISK_TARGETS[manifest["os"]]
    require(manifest.get("disk_target", expected) == expected, "Unreviewed disk target for this OS")
    return expected


def fixture_disk(manifest):
    if "fixture_disk" not in manifest:
        return None
    require(manifest["os"] == "dragonflybsd", "Dedicated fixture disk is reviewed for DragonFly only")
    expected = Path(manifest["disk"]).with_name("fixture.qcow2")
    require(manifest["fixture_disk"] == str(expected), "Fixture disk path changed")
    return expected


def clone_disks(domain, manifest):
    if fixture_disk(manifest) is not None:
        disks = domain.findall("./devices/disk")
        require(len(disks) == 2, "DragonFly clone must have only its root and fixture disks")
        candidates = [disk for disk in disks if disk.find("target").get("dev") == FIXTURE_DISK_TARGET]
        require(len(candidates) == 1, "Owned fixture disk missing or ambiguous")
        disk = candidates[0]
        source, target, driver = (disk.find(tag) for tag in ("source", "target", "driver"))
        require(disk.get("type") == "file" and disk.get("device") == "disk"
                and target.get("bus") == "virtio" and disk.findtext("serial") == FIXTURE_DISK_SERIAL
                and driver is not None and driver.get("type") == "qcow2"
                and source is not None and source.get("file") == manifest["fixture_disk"]
                and disk.find("mirror") is None and disk.find("backingStore/source") is None,
                "Owned fixture disk definition changed")
        domain.find("devices").remove(disk)
    return disk_source(ET.tostring(domain, encoding="unicode"), disk_target(manifest))


def trusted_firmware_file(path):
    path = Path(path)
    for entry in (path, *path.parents):
        info = entry.lstat()
        require(not entry.is_symlink() and info.st_uid == 0 and not info.st_mode & 0o022,
                "Firmware paths must be administrator controlled without symlinks")
    require(path.is_file(), "Firmware must be a regular file")
    return hashlib.sha256(path.read_bytes()).hexdigest()


def firmware_policy(manifest):
    firmware = manifest.get("firmware")
    if firmware is None:
        require(manifest["os"] != "dragonflybsd", "DragonFly requires reviewed private UEFI state")
        return None
    require(manifest["os"] == "dragonflybsd" and firmware["loader"] == UEFI_CODE and
            firmware["template"] == UEFI_VARS and firmware["nvram"] ==
            str(Path(manifest["disk"]).parent / "nvram.fd"), "Firmware ownership policy changed")
    return firmware


def validate_firmware_xml(domain, firmware, private=False):
    loader, nvram = domain.find("os/loader"), domain.find("os/nvram")
    if firmware is None:
        require(domain.find("os").get("firmware") in (None, "bios") and
                loader is None and nvram is None and domain.find("os/varstore") is None,
                "Firmware writable paths require a separately reviewed private copy")
        return
    require(domain.find("os").get("firmware") == "efi" and loader is not None and nvram is not None,
            "Reviewed DragonFly UEFI definition missing")
    require(loader.text == UEFI_CODE and loader.get("readonly") == "yes" and
            loader.get("type") == "pflash" and loader.get("format") == "raw", "Unreviewed firmware loader")
    require(not list(nvram) and nvram.get("template") == UEFI_VARS and
            nvram.get("templateFormat") == "raw" and nvram.get("format") == "raw" and
            domain.find("os/varstore") is None, "Unreviewed NVRAM storage")
    if private:
        require(nvram.text == firmware["nvram"], "Clone NVRAM path changed")

def clone_xml(source, manifest):
    domain = ET.fromstring(source)
    firmware = firmware_policy(manifest)
    validate_firmware_xml(domain, firmware)
    if firmware is not None:
        domain.find("os/nvram").text = firmware["nvram"]
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
    if fixture_disk(manifest) is not None:
        fixture = ET.SubElement(devices, "disk", {"type": "file", "device": "disk"})
        ET.SubElement(fixture, "driver", {"name": "qemu", "type": "qcow2", "cache": "none"})
        ET.SubElement(fixture, "source", {"file": manifest["fixture_disk"]})
        ET.SubElement(fixture, "target", {"dev": FIXTURE_DISK_TARGET, "bus": "virtio"})
        ET.SubElement(fixture, "serial").text = FIXTURE_DISK_SERIAL
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
        disk_target(manifest)
        fixture_disk(manifest)
        firmware_policy(manifest)
        return cls(directory, manifest)

    def verify_network(self):
        xml = ET.fromstring(virsh("net-dumpxml", self.manifest["network_uuid"]))
        require(xml.findtext("name") == self.manifest["network"], "Network name changed")
        require(xml.findtext("uuid") == self.manifest["network_uuid"], "Network UUID changed")
        require(xml.findtext("description") == self.manifest["marker"], "Network ownership marker changed")
        require(xml.find("dns") is not None and xml.find("dns").get("enable") == "no", "Network DNS must remain disabled")
        require(xml.find("forward") is None, "Fault network must remain isolated")
        require(xml.find("bridge").get("name") == self.manifest["bridge"], "Bridge changed")
        subnet = ipaddress.ip_network(self.manifest["subnet"])
        require(xml.find("ip").get("address") == str(subnet[1]) and
                xml.find("ip").get("netmask") == str(subnet.netmask), "Network subnet changed")
        addresses = xml.findall("ip")
        require(len(addresses) == 1, "Network must have exactly one reviewed subnet")
        hosts = xml.findall("ip/dhcp/host")
        require(len(hosts) == 1 and hosts[0].get("mac") == self.manifest["mac"] and
                hosts[0].get("ip") == str(subnet[10]), "Clone DHCP identity changed")
        return xml

    def stop_network(self):
        network = self.verify_network()
        require(int(network.get("connections", "0")) == 0, "Owned network still has connected guests")
        virsh("net-destroy", self.manifest["network_uuid"])
        self.event("network-stop", method="libvirt-net-destroy", network_uuid=self.manifest["network_uuid"])

    def verify_domain(self):
        domain = ET.fromstring(virsh("dumpxml", self.manifest["uuid"]))
        require(domain.findtext("uuid") == self.manifest["uuid"], "Clone UUID changed")
        require(domain.findtext("name") == self.manifest["name"], "Clone name changed")
        require(domain.findtext("description") == self.manifest["marker"], "Clone marker changed")
        require(clone_disks(domain, self.manifest) == self.manifest["disk"], "Clone disk changed")
        interfaces = domain.findall("devices/interface")
        require(len(interfaces) == 1, "Clone must have only the isolated NIC")
        require(interfaces[0].find("source").get("network") == self.manifest["network"], "Clone network changed")
        require(interfaces[0].find("mac").get("address") == self.manifest["mac"], "Clone MAC changed")
        validate_firmware_xml(domain, firmware_policy(self.manifest), private=True)
        self.verify_firmware_resources()
        self.verify_network()

    def verify_disk(self):
        path = Path(self.manifest["disk"])
        metadata = path.lstat()
        require(stat.S_ISREG(metadata.st_mode) and not path.is_symlink(), "Clone disk is not a regular private file")
        recorded = json.loads((self.directory / "copy-result.json").read_text())
        require(recorded["disk_identity"] == {"device": metadata.st_dev, "inode": metadata.st_ino},
                "Clone disk inode changed")
        fixture = fixture_disk(self.manifest)
        if fixture is not None:
            identity = fixture.lstat()
            require(stat.S_ISREG(identity.st_mode), "Fixture disk must be a regular private file")
            recorded_fixture = json.loads((self.directory / "fixture-disk.json").read_text())
            require(recorded_fixture == {"path": str(fixture), "device": identity.st_dev, "inode": identity.st_ino},
                    "Fixture disk inode changed")

    def provision_fixture_disk(self):
        path = fixture_disk(self.manifest)
        require(path is not None, "Run has no dedicated fixture disk")
        require(self.manifest["uuid"] not in virsh("list", "--all", "--uuid").split(),
                "Cannot provision storage while the clone is defined")
        # Reserving the path exclusively prevents replacing any earlier fixture.
        with path.open("xb"):
            pass
        run(["qemu-img", "create", "-f", "qcow2", str(path), "1G"])
        identity = path.lstat()
        write_json(self.directory / "fixture-disk.json",
                   {"path": str(path), "device": identity.st_dev, "inode": identity.st_ino})

    def verify_firmware_resources(self, before_boot=False):
        firmware = firmware_policy(self.manifest)
        if firmware is None:
            return
        require(trusted_firmware_file(UEFI_CODE) == firmware["loader_sha256"] and
                trusted_firmware_file(UEFI_VARS) == firmware["template_sha256"], "Trusted firmware bytes changed")
        path = Path(firmware["nvram"])
        state_file = self.directory / "firmware-state.json"
        if not state_file.exists():
            require(before_boot and not path.exists() and not path.is_symlink(),
                    "Initial NVRAM destination exists or lacks an ownership record")
            return
        info = path.lstat()
        require(stat.S_ISREG(info.st_mode) and not path.is_symlink(), "Private NVRAM is not a regular file")
        state = json.loads(state_file.read_text())
        require(state == {"path": str(path), "device": info.st_dev, "inode": info.st_ino},
                "Private NVRAM inode changed")

    def record_initial_firmware(self):
        firmware = firmware_policy(self.manifest)
        if firmware is None or (self.directory / "firmware-state.json").exists():
            return
        domain = ET.fromstring(virsh("dumpxml", self.manifest["uuid"]))
        require(domain.findtext("uuid") == self.manifest["uuid"] and
                domain.findtext("name") == self.manifest["name"] and
                domain.findtext("description") == self.manifest["marker"], "Created firmware domain identity changed")
        validate_firmware_xml(domain, firmware, private=True)
        path = Path(firmware["nvram"])
        info = path.lstat()
        require(stat.S_ISREG(info.st_mode) and not path.is_symlink(), "Created NVRAM is not a regular private file")
        write_json(self.directory / "firmware-state.json", {"path": str(path), "device": info.st_dev, "inode": info.st_ino})

    def event(self, operation, **details):
        event = {"schema": 1, "run": self.manifest["run"], "domain_uuid": self.manifest["uuid"],
                 "operation": operation, "time_ns": time.time_ns(), **details}
        path = self.directory / "hypervisor-events.jsonl"
        with open(path, "a", opener=lambda p, f: os.open(p, f, 0o600)) as output:
            output.write(json.dumps(event) + "\n")
            output.flush()
            os.fsync(output.fileno())

    def create(self):
        self.verify_disk()
        self.verify_network()
        self.verify_firmware_resources(before_boot=True)
        xml = (self.directory / "clone.xml").read_text()
        # Compare the saved definition to a fresh deterministic derivation, so
        # editing the XML cannot smuggle in an external disk or host device.
        expected = clone_xml((self.directory / "source-inactive.xml").read_text(), self.manifest)
        require(xml == expected, "Saved clone XML changed")
        virsh("create", self.directory / "clone.xml")
        self.record_initial_firmware()
        self.verify_domain()
        self.event("boot", method="libvirt-create")

    def poweroff(self):
        self.verify_disk()
        self.verify_domain()
        self.event("poweroff-intent", method="libvirt-destroy", disk=self.manifest["disk"])
        virsh("destroy", self.manifest["uuid"])
        require(self.manifest["uuid"] not in virsh("list", "--all", "--uuid").split(), "Transient clone remains defined")
        self.event("poweroff-complete", method="libvirt-destroy")

    def static_address(self):
        self.verify_domain()
        address = str(ipaddress.ip_network(self.manifest["subnet"])[10])
        route = json.loads(run(["ip", "-j", "route", "get", address]))
        require(len(route) == 1 and route[0].get("dev") == self.manifest["bridge"] and
                route[0].get("dst") == address and not route[0].get("gateway"), "Static clone route is not its owned bridge")
        # ICMP need not succeed, but the owned subnet ARP must identify exactly
        # the fresh clone MAC before any SSH connection is authorized.
        subprocess.run(["ping", "-c", "1", "-W", "1", address], capture_output=True, timeout=3)
        neighbors = json.loads(run(["ip", "-j", "neigh", "show", "to", address]))
        require(len(neighbors) == 1 and neighbors[0].get("dst") == address and
                neighbors[0].get("dev") == self.manifest["bridge"] and
                neighbors[0].get("lladdr", "").lower() == self.manifest["mac"] and
                not {"FAILED", "INCOMPLETE"}.intersection(neighbors[0].get("state", [])), "Static clone neighbor MAC is unconfirmed")
        return address

    def static_marker(self):
        return {"run": self.manifest["run"], "address": str(ipaddress.ip_network(self.manifest["subnet"])[10]),
                "bridge": self.manifest["bridge"], "mac": self.manifest["mac"],
                "method": "explicit-clone-console-static-assignment"}

    def register_static_transport(self):
        self.static_address()
        write_json(self.directory / "static-transport.json", self.static_marker())

    def lease(self):
        self.verify_domain()
        leases = virsh("net-dhcp-leases", self.manifest["network_uuid"], "--mac", self.manifest["mac"])
        addresses = re.findall(r"\b((?:\d+\.){3}\d+/\d+)\b", leases)
        if not addresses and (self.directory / "static-transport.json").exists():
            require(json.loads((self.directory / "static-transport.json").read_text()) == self.static_marker(),
                    "Static transport ownership record changed")
            return self.static_address()
        require(len(addresses) == 1, "Expected exactly one clone DHCP lease")
        address = ipaddress.ip_interface(addresses[0]).ip
        subnet = ipaddress.ip_network(self.manifest["subnet"])
        require(address in subnet and address not in (subnet[0], subnet[1], subnet[-1]), "Unsafe clone address")
        require(not address.is_loopback and address not in ipaddress.ip_network("10.11.1.0/24"),
                "Original guest address forbidden")
        return str(address)

    def ssh(self, command, user="adjutant", timeout=60):
        address = self.lease()
        # Send multiline scripts to sh over stdin so BSD csh login shells
        # only have to parse the fixed invocation, never the script itself.
        script = "export PATH=/usr/local/sbin:/usr/local/bin:/usr/pkg/sbin:/usr/pkg/bin:/usr/sbin:/usr/bin:/sbin:/bin\n" + command + "\n"
        # The copied host key must already be trusted for the original domain.
        return run(["ssh", "-F", "/dev/null", "-o", "StrictHostKeyChecking=yes", "-o", "CheckHostIP=no",
                    "-o", "UpdateHostKeys=no", "-o", "BatchMode=yes", "-o", "ConnectTimeout=5",
                    "-o", "HostKeyAlias=" + self.manifest["source"], user + "@" + address, "/bin/sh -s"],
                   timeout, input_text=script)


def prepare(os_name, run_id, subnet, bandwidth, dragonfly_fixture_disk=False):
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
    target = DISK_TARGETS[os_name]
    source_disk = disk_source(live, target)
    require(disk_source(inactive, target) == source_disk, "Live/persistent source disks differ")
    require(virsh("domstate", source).strip() == "running", "Source must remain running")
    require("No current block job" in virsh("blockjob", source, target, "--info"), "Preexisting block job")
    info = virsh("domblkinfo", source, target)
    capacity = int(re.search(r"Capacity:\s+(\d+)", info).group(1))
    require(os.statvfs(ROOT).f_bavail * os.statvfs(ROOT).f_frsize > capacity + 10 * 1024**3,
            "Insufficient room for a full standalone source copy")
    directory = ROOT / ("lurker-fault-" + run_id)
    require(not directory.exists(), "Run already exists; use its manifest")
    manifest = {"schema": 1, "run": run_id, "os": os_name, "source": source,
                "source_uuid": ET.fromstring(inactive).findtext("uuid"), "source_disk": source_disk, "disk_target": target,
                "name": directory.name + "-" + os_name, "uuid": str(uuid.uuid4()),
                "marker": "lurker-disposable-fault:" + run_id, "disk": str(directory / "root.qcow2"),
                "network": directory.name, "network_uuid": str(uuid.uuid4()), "subnet": str(network),
                "bridge": "lf" + secrets.token_hex(5), "mac": "52:54:00:" + ":".join(secrets.token_hex(1) for _ in range(3)),
                "bandwidth_mib": bandwidth, "source_xml_sha256": hashlib.sha256(inactive.encode()).hexdigest()}
    if os_name == "dragonflybsd":
        manifest["firmware"] = {"loader": UEFI_CODE, "template": UEFI_VARS,
                                "nvram": str(directory / "nvram.fd"),
                                "loader_sha256": trusted_firmware_file(UEFI_CODE),
                                "template_sha256": trusted_firmware_file(UEFI_VARS)}
    if dragonfly_fixture_disk:
        require(os_name == "dragonflybsd", "Dedicated fixture disk is reviewed for DragonFly only")
        manifest["fixture_disk"] = str(directory / "fixture.qcow2")
    # Validate before allocating any plan directory.
    clone_xml(inactive, manifest)
    # Traverse-only access allows libvirt's QEMU uid to use its relabeled image;
    # all manifests, XML and logs remain private to the invoking user.
    directory.mkdir(mode=0o711)
    directory.chmod(0o711)
    write_json(directory / "manifest.json", manifest)
    for name, text in (("source-inactive.xml", inactive), ("source-live.xml", live),
                       ("clone.xml", clone_xml(inactive, manifest)), ("network.xml", network_xml(manifest))):
        path = directory / name
        path.write_text(text)
        path.chmod(0o600)
    return OwnedClone.load(directory)


def copy_and_boot(clone):
    m = clone.manifest
    destination = Path(m["disk"])
    require(not destination.exists() and not destination.is_symlink(), "Copy destination already exists or is a symlink")
    require(virsh("dumpxml", m["source"], "--inactive") == (clone.directory / "source-inactive.xml").read_text(), "Source XML changed")
    require(disk_source(virsh("dumpxml", m["source"]), disk_target(m)) == m["source_disk"], "Source disk changed")
    require("No current block job" in virsh("blockjob", m["source"], disk_target(m), "--info"), "Preexisting block job")
    # --finish stops mirroring when ready and leaves the ORIGINAL disk active.
    # Never --pivot, --shallow, --reuse-external, or direct reads of live qcow2.
    command = ["virsh", "-c", URI, "blockcopy", m["source"], disk_target(m), "--dest", m["disk"],
               "--format", "qcow2", "--bandwidth", str(m["bandwidth_mib"]), "--wait", "--finish",
               "--transient-job", "--timeout", "3600"]
    with (clone.directory / "blockcopy.log").open("w") as log:
        code = subprocess.run(command, stdout=log, stderr=subprocess.STDOUT).returncode
    require(code == 0, "Live copy failed; inspect blockcopy.log; do not abort any unidentified job")
    finish_copy_and_boot(clone)



def inspect_private_copy(clone):
    """Inspect unreadable daemon-owned copies through an owned metadata pool."""
    disk = clone.manifest["disk"]
    if os.access(disk, os.R_OK):
        image = json.loads(run(["qemu-img", "info", "--output=json", disk]))
        require(not image.get("backing-filename"), "Clone image must have no backing file")
        return {"inspection": "offline-qemu-img", **image}
    pool_name = clone.manifest["network"] + "-probe"
    require(pool_name not in virsh("pool-list", "--all", "--name").split(), "Probe pool already exists; reconcile its ownership")
    # A transient directory pool registers metadata for this already-owned
    # directory. No --build, pool-delete, vol-create, vol-delete or permissions
    # change is performed. pool-destroy only stops this metadata registration.
    virsh("pool-create-as", pool_name, "dir", "--target", clone.directory)
    pool_uuid = None

    def verified_pool(identifier):
        xml = ET.fromstring(virsh("pool-dumpxml", identifier))
        require(xml.tag == "pool" and xml.get("type") == "dir" and xml.findtext("name") == pool_name,
                "Probe pool identity changed")
        require(xml.findtext("target/path") == str(clone.directory), "Probe pool target changed")
        if pool_uuid is not None:
            require(xml.findtext("uuid") == pool_uuid, "Probe pool UUID changed")
        return xml

    try:
        pool = verified_pool(pool_name)
        pool_uuid = pool.findtext("uuid")
        require(pool_uuid, "Probe pool has no UUID")
        write_json(clone.directory / "probe-pool.json", {"name": pool_name, "uuid": pool_uuid,
                   "target": str(clone.directory), "operation": "metadata-only"})
        virsh("pool-refresh", pool_uuid)
        volume_xml = virsh("vol-dumpxml", "root.qcow2", pool_uuid)
        volume = ET.fromstring(volume_xml)
        require(volume.findtext("name") == "root.qcow2" and volume.findtext("target/path") == disk,
                "Probe returned a different image")
        require(volume.find("target/format").get("type") == "qcow2", "Copied format is not qcow2")
        require(not volume.findall(".//backingStore/path"), "Copied image has a backing file")
        require(int(volume.findtext("capacity")) > 0, "Copied capacity missing")
        (clone.directory / "copy-volume.xml").write_text(volume_xml)
        return {"inspection": "libvirt-directory-pool", "format": "qcow2", "backing_file": None,
                "virtual_size": int(volume.findtext("capacity")), "pool_uuid": pool_uuid}
    finally:
        if pool_uuid is not None:
            verified_pool(pool_uuid)
            virsh("pool-destroy", pool_uuid)

def finish_copy_and_boot(clone):
    """Resume only a positively completed, disconnected owned copy job."""
    m = clone.manifest
    destination = Path(m["disk"])
    require(destination.is_file() and not destination.is_symlink(), "Completed copy must be a regular private file")
    require((clone.directory / "blockcopy.log").read_text().strip() == "Successfully copied",
            "Copy completion is not confirmed; never resume a partial copy")
    after = virsh("dumpxml", m["source"], "--inactive")
    (clone.directory / "source-after.xml").write_text(after)
    require(after == (clone.directory / "source-inactive.xml").read_text(), "Original definition changed")
    require(disk_source(virsh("dumpxml", m["source"]), disk_target(m)) == m["source_disk"], "Original source disk changed")
    require(virsh("domstate", m["source"]).strip() == "running", "Original stopped unexpectedly")
    require("No current block job" in virsh("blockjob", m["source"], disk_target(m), "--info"), "Copy job still active")
    image = inspect_private_copy(clone)
    disk_metadata = Path(m["disk"]).lstat()
    require(stat.S_ISREG(disk_metadata.st_mode), "Copied disk is not a regular file")
    write_json(clone.directory / "copy-result.json", {"status": "passed", "source_unchanged": True, "image": image,
               "disk_identity": {"device": disk_metadata.st_dev, "inode": disk_metadata.st_ino}})
    virsh("net-create", clone.directory / "network.xml")
    clone.create()
    print(json.dumps({"status": "booting", "directory": str(clone.directory), "name": m["name"]}), flush=True)



def fsync_path(path):
    descriptor = os.open(path, os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def guest_directory(run_id, create=False):
    require(re.fullmatch(r"[a-z0-9]{8,20}", run_id), "Invalid guest run ID")
    home = Path.home()
    # Resolve before creating anything: a redirected cache could be tmpfs or an
    # unrelated directory. Do not follow symlinks in the owned fixture ancestry.
    parent = home / ".cache" / "lurker"
    for path in (home, home / ".cache", parent):
        require(not path.is_symlink(), "Guest fixture ancestry contains symlink")
        if create and not path.exists():
            path.mkdir(mode=0o700)
            fsync_path(path.parent)
            fsync_path(path)
        require(path.is_dir() and path.stat().st_uid == os.geteuid(), "Guest cache is not owned")
    directory = parent / ("fault-" + run_id)
    if create:
        directory.mkdir(mode=0o700)
        fsync_path(parent)
        write_json(directory / "owner.json", {"run": run_id, "uid": os.geteuid()})
    require(not directory.is_symlink(), "Guest fixture symlink forbidden")
    state = directory.stat()
    require(state.st_uid == os.geteuid() and not state.st_mode & 0o077, "Guest fixture owner/mode changed")
    require(json.loads((directory / "owner.json").read_text()) == {"run": run_id, "uid": os.geteuid()},
            "Guest fixture ownership marker changed")
    # A home cache path alone does not establish persistent backing storage.
    # Require a mount-table assertion by the caller and record it in evidence;
    # Linux tmpfs/ramfs are additionally rejected automatically.
    if platform.system() == "Linux":
        path = str(directory.resolve())
        matches = []
        for line in Path("/proc/self/mountinfo").read_text().splitlines():
            before, after = line.split(" - ", 1)
            mountpoint = before.split()[4].replace("\\040", " ")
            if path == mountpoint or path.startswith(mountpoint.rstrip("/") + "/"):
                matches.append((len(mountpoint), after.split()[0]))
        require(matches and max(matches)[1] not in ("tmpfs", "ramfs"), "Guest fixtures must be persistent")
    return directory


def guest_cli(directory, arguments, secret):
    specification = importlib.util.spec_from_file_location("powerloss_cli", Path(__file__).with_name("test-backends.py"))
    module = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(module)
    manifest = json.loads((directory / "guest.json").read_text())
    cli = Path(manifest["cli"])
    require(hashlib.sha256(cli.read_bytes()).hexdigest() == manifest["cli_sha256"], "Candidate CLI changed")
    log = directory / ("cli-" + str(time.time_ns()) + ".log")
    args = [str(cli), "--color=never", str(arguments[0])]
    if manifest["unprivileged"]:
        args.append("--unprivileged")
    args += list(map(str, arguments[1:]))
    result = module.terminal(args, secret, 600, log)
    require(result["exit_code"] == 0 and not result["timed_out"], "Candidate CLI failed; retained log=" + str(log))
    return result


def guest_volume_arguments(action, manifest):
    require(action in {"create", "mount"}, "Unknown guest volume action")
    backend = manifest["backend"]
    peers = manifest.get("peer_sources", [])
    require(isinstance(peers, list) and len(peers) == (1 if backend == "raid1c" else 0),
            "RAID1C requires its recorded second member; other backends forbid peers")
    command = [action, "--type", backend]
    if peers:
        require(peers[0] != manifest["source"], "Mirror members must be distinct")
        command += ["--member", peers[0]]
    if action == "create" and backend in {"luks", "luks1", "veracrypt"}:
        command += ["--cipher", manifest["cipher"]]
    command.append(manifest["source"])
    if action == "mount":
        command.append(manifest["mountpoint"])
    elif backend not in DIRECTORIES:
        require(manifest["size_gb"] is not None, "File container size missing")
        command.append(manifest["size_gb"])
    return command


def netbsd_sync_diagnostic(mountpoint):
    """Remount the test fixture before issuing any measured application writes."""
    # NetBSD <sys/statvfs.h> ST_LOG (MNT_LOG). Python only exports ST_SYNCHRONOUS.
    journal = 0x02000000
    before = os.statvfs(mountpoint)
    require(os.path.ismount(mountpoint) and before.f_flag & journal
            and not before.f_flag & os.ST_RDONLY,
            "Diagnostic requires an existing writable journaled CGD fixture")
    command = ["/sbin/mount", "-u", "-o", "sync,log", str(mountpoint)]
    run(command)
    after = os.statvfs(mountpoint)
    require(os.path.ismount(mountpoint) and after.f_fsid == before.f_fsid
            and after.f_flag & journal and after.f_flag & os.ST_SYNCHRONOUS
            and not after.f_flag & os.ST_RDONLY,
            "Diagnostic remount did not establish synchronous journaled mode on the fixture")
    return {"command": command, "fsid": before.f_fsid, "before_flags": before.f_flag,
            "workload_flags": after.f_flag, "journaled": True, "synchronous": True}


def openbsd_sync_diagnostic(mountpoint):
    """Make regular-file writes synchronous on the test fixture before measurement."""
    mount_row = re.compile(r"(/dev/(?:sd[1-9][0-9]*|vnd[0-3])a) on "
                           + re.escape(str(mountpoint)) + r" type ffs \(([^)]+)\)")

    def mounted_ffs():
        matches = [match.groups() for line in run(["/sbin/mount"]).splitlines()
                   if (match := mount_row.fullmatch(line))]
        require(os.path.ismount(mountpoint) and len(matches) == 1,
                "Diagnostic requires a single identified native OpenBSD FFS fixture")
        device, flags = matches[0]
        flags = flags.split(", ")
        require("read-only" not in flags and "asynchronous" not in flags,
                "Diagnostic requires a writable OpenBSD FFS fixture")
        return device, flags

    device, before = mounted_ffs()
    command = ["/sbin/mount", "-u", "-o", "sync", str(mountpoint)]
    run(command)
    after_device, after = mounted_ffs()
    require(after_device == device and "synchronous" in after,
            "Diagnostic remount did not establish synchronous mode on the same fixture")
    return {"command": command, "device": device, "before_flags": before, "workload_flags": after}


def netbsd_allocated_diagnostic(source):
    """Use an allocated, byte-identical copy of the newly created CGD fixture."""
    before = source.lstat()
    require(stat.S_ISREG(before.st_mode) and before.st_uid == os.geteuid() and before.st_nlink == 1,
            "Allocated control requires an owned, newly created regular fixture")
    sparse_evidence = source.with_name("sparse ciphertext evidence")
    replacement = source.with_name("allocated ciphertext temporary")
    with source.open("rb") as original, replacement.open("xb") as allocated:
        shutil.copyfileobj(original, allocated, length=1024 * 1024)
        allocated.flush()
        os.fsync(allocated.fileno())
    after = replacement.stat()
    require(after.st_size == before.st_size and after.st_blocks * 512 >= after.st_size,
            "Backing filesystem did not allocate the complete ciphertext copy")
    with source.open("rb") as original, replacement.open("rb") as allocated:
        digest = hashlib.file_digest(original, "sha256").hexdigest()
        require(hashlib.file_digest(allocated, "sha256").hexdigest() == digest,
                "Allocated ciphertext differs from the source fixture")
    os.link(source, sparse_evidence, follow_symlinks=False)
    os.replace(replacement, source)
    fsync_path(source.parent)
    return {"sparse_bytes_allocated": before.st_blocks * 512,
            "copy_bytes_allocated": after.st_blocks * 512,
            "logical_size": after.st_size, "sha256": digest}


def freebsd_suj_diagnostic(source, secret):
    """Enable UFS soft-updates journaling on a fresh, unmounted GELI fixture."""
    identity = source.lstat()
    require(stat.S_ISREG(identity.st_mode) and identity.st_uid == os.geteuid()
            and identity.st_nlink == 1, "SUJ control requires an owned, new regular fixture")
    existing = set(run(["/sbin/mdconfig", "-l"]).split())
    # The candidate's new GELI file sits on 512-byte md sectors. GELI's
    # 4096-byte encrypted sectors describe the provider above that md device.
    unit = run(["/sbin/mdconfig", "-a", "-t", "vnode", "-S", "512", "-f", str(source)]).strip()
    require(re.fullmatch(r"md(?:0|[1-9][0-9]*)", unit) and unit not in existing,
            "SUJ control requires a newly attached device")
    raw = "/dev/" + unit
    provider = raw + ".eli"

    def verify_backing():
        listing = run(["/sbin/geom", "md", "list", unit])
        expected = f"ident: MD-DEV{identity.st_dev}-INO{identity.st_ino}"
        require(expected in [line.strip() for line in listing.splitlines()],
                "SUJ control backing identity changed")

    verify_backing()
    attached = False
    try:
        run(["/sbin/geli", "attach", "-j", "-", raw], input_text=secret + "\n")
        attached = True
        command = ["/sbin/tunefs", "-j", "enable", provider]
        run(command)
        result = subprocess.run(["/sbin/tunefs", "-p", provider], capture_output=True,
                                text=True, timeout=60, check=True)
        report = result.stdout + result.stderr
        require(re.search(r"soft updates journaling:\s+(?:\(-j\)\s+)?enabled\b", report)
                is not None, "SUJ was not enabled on the test filesystem")
        return {"command": command, "filesystem": report}
    finally:
        verify_backing()
        if attached:
            run(["/sbin/geli", "detach", raw])
        run(["/sbin/mdconfig", "-d", "-u", unit])


def guest_setup(args):
    require(args.persistent_home_confirmed, "Confirm the home filesystem is persistent, not tmpfs")
    require(re.fullmatch(r"[0-9a-f]{40}", args.commit), "Exact candidate Git commit required")
    try:
        route = validate_guest_route(args.backend, args.unprivileged, args.cipher)
        if args.netbsd_sync_diagnostic or args.netbsd_allocated_diagnostic:
            require(platform.system() == "NetBSD" and args.backend == "cgd" and os.geteuid() == 0,
                    "NetBSD diagnostics require a native CGD container")
        if args.openbsd_sync_diagnostic:
            require(platform.system() == "OpenBSD" and args.backend in {"veracrypt", "softraid", "raid1c"}
                    and os.geteuid() == 0,
                    "Synchronous FFS diagnostic requires a native OpenBSD container")
        if args.freebsd_suj_diagnostic:
            require(platform.system() == "FreeBSD" and args.backend == "geli" and os.geteuid() == 0,
                    "SUJ diagnostic requires a native FreeBSD GELI container")
        package, manifest_digest = verify_staged_package(args.cli, args.commit, args.backend)
        guest_prerequisites(package, args.backend, args.unprivileged)
    except Exception as error:
        print(json.dumps({"status": "blocked", "stage": "prerequisites", "backend": args.backend,
                          "source_commit": args.commit, "reason": str(error)[:2048]}), flush=True)
        return 2
    directory = guest_directory(args.run, create=True)
    cli = args.cli.resolve(strict=True)
    secret = secrets.token_urlsafe(48)
    password = directory / "test-password"
    with open(password, "w", opener=lambda p, f: os.open(p, f, 0o600)) as output:
        output.write(secret)
        output.flush()
        os.fsync(output.fileno())
    source, mountpoint = directory / "encrypted data", directory / "open data"
    size_gb = None
    if args.backend not in ("gocryptfs", "cryfs", "cryptomator", "pefs"):
        size_gb = "0.125" if (args.backend in ("geli", "cgd", "softraid", "raid1c") or platform.system() == "DragonFly") else "0.35"
    mountpoint.mkdir(mode=0o700)
    manifest = {"schema": 1, "run": args.run, "cli": str(cli), "source_commit": args.commit,
                "cli_sha256": hashlib.sha256(cli.read_bytes()).hexdigest(), "backend": args.backend,
                "package_manifest_sha256": manifest_digest, "privilege_route": route,
                "selected_package_backends": package["selected_backends"],
                "runner_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                "credential_transport_sha256": hashlib.sha256(Path(__file__).with_name("test-backends.py").read_bytes()).hexdigest(),
                "cipher": args.cipher, "size_gb": size_gb, "unprivileged": args.unprivileged, "source": str(source),
                "peer_sources": [str(directory / "encrypted peer")] if args.backend == "raid1c" else [],
                "mountpoint": str(mountpoint), "os": platform.system(), "kernel": platform.release(),
                "architecture": platform.machine(), "python_version": platform.python_version(),
                "sqlite_version": sqlite3.sqlite_version, "persistent_home_confirmed": True,
                "mount_table": run(["mount"]), "repair": False}
    write_json(directory / "guest.json", manifest)
    guest_cli(directory, guest_volume_arguments("create", manifest), secret)
    if args.netbsd_allocated_diagnostic:
        manifest["netbsd_allocated_diagnostic"] = netbsd_allocated_diagnostic(source)
        write_json(directory / "guest.json", manifest)
    if args.freebsd_suj_diagnostic:
        manifest["freebsd_suj_diagnostic"] = freebsd_suj_diagnostic(source, secret)
        write_json(directory / "guest.json", manifest)
    guest_cli(directory, guest_volume_arguments("mount", manifest), secret)
    require(os.path.ismount(mountpoint), "CLI reported success without an identified mounted filesystem")
    if args.netbsd_sync_diagnostic:
        manifest["netbsd_sync_diagnostic"] = netbsd_sync_diagnostic(mountpoint)
        write_json(directory / "guest.json", manifest)
    if args.openbsd_sync_diagnostic:
        manifest["openbsd_sync_diagnostic"] = openbsd_sync_diagnostic(mountpoint)
        write_json(directory / "guest.json", manifest)
    # Persist only test credentials/recovery metadata before the measured
    # workload. No global sync and no clean detach occurs before poweroff.
    fsync_path(directory)
    with (directory / "worker.log").open("wb") as output:
        worker = subprocess.Popen([sys.executable, str(Path(__file__).resolve()), "guest-worker", "--run", args.run],
                                  stdin=subprocess.DEVNULL, stdout=output, stderr=subprocess.STDOUT,
                                  start_new_session=True)
    write_json(directory / "worker-process.json", {"pid": worker.pid, "run": args.run})
    print(json.dumps({"status": "workload-started", "directory": str(directory), "pid": worker.pid}), flush=True)
    return 0


def guest_worker(run_id):
    directory = guest_directory(run_id)
    manifest = json.loads((directory / "guest.json").read_text())
    mountpoint = Path(manifest["mountpoint"])
    require(mountpoint == directory / "open data", "Mountpoint ownership changed")
    require(os.path.ismount(mountpoint), "Workload must execute inside the encrypted mount")
    payload = secrets.token_bytes(65536)
    temporary = mountpoint / "atomic.tmp"
    with temporary.open("wb") as output:
        output.write(payload)
        output.flush()
        os.fsync(output.fileno())
    temporary.rename(mountpoint / "durable record")
    fsync_path(mountpoint)
    connection = sqlite3.connect(mountpoint / "profile.sqlite")
    require(connection.execute("PRAGMA journal_mode=WAL").fetchone()[0] == "wal", "SQLite WAL unavailable")
    connection.execute("PRAGMA synchronous=FULL")
    connection.execute("PRAGMA wal_autocheckpoint=0")
    require(connection.execute("PRAGMA synchronous").fetchone()[0] == 2, "SQLite FULL unavailable")
    connection.execute("CREATE TABLE committed (id INTEGER PRIMARY KEY, value BLOB)")
    connection.execute("INSERT INTO committed VALUES (1, ?)", (payload,))
    connection.commit()
    database_files = ("profile.sqlite", "profile.sqlite-wal")
    for name in database_files:
        fsync_path(mountpoint / name)
    fsync_path(mountpoint)
    # Native encrypted-directory engines can expose reserved names in readdir
    # that do not resolve through the decrypted view. Measure only our files.
    sizes = {name: (mountpoint / name).stat().st_size
             for name in ("durable record", *database_files, "profile.sqlite-shm")}
    # Start an intentionally uncommitted transaction. Its visibility after
    # recovery is characterized separately and never counted as durable data.
    connection.execute("INSERT INTO committed VALUES (2, ?)", (secrets.token_bytes(8192),))
    dirty = (mountpoint / "unacknowledged writes").open("wb", buffering=0)
    dirty.write(secrets.token_bytes(32768))
    write_json(directory / "ready.json", {"status": "ready-for-abrupt-poweroff", "schema": 1,
               "run": run_id, "sha256": hashlib.sha256(payload).hexdigest(), "sizes": sizes,
               "durable": ["file-fsync", "rename-directory-fsync", "sqlite-FULL-WAL-commit"],
               "sqlite_connection_open": True, "global_sync": False, "clean_unmount": False,
               "created_ns": time.time_ns()})
    # Keep SQLite's connection, SHM and WAL alive. Never close/checkpoint in a
    # finally block: only abrupt destruction of this disposable VM ends it.
    while True:
        dirty.write(secrets.token_bytes(4096))
        dirty.seek(0)
        time.sleep(0.05)


def guest_verify(run_id):
    directory = guest_directory(run_id)
    manifest = json.loads((directory / "guest.json").read_text())
    ready = json.loads((directory / "ready.json").read_text())
    require(ready["status"] == "ready-for-abrupt-poweroff" and ready["run"] == run_id, "No durable acknowledgement")
    secret = (directory / "test-password").read_text()
    source, mountpoint = Path(manifest["source"]), Path(manifest["mountpoint"])
    require(source == directory / "encrypted data" and mountpoint == directory / "open data", "Fixture paths changed")
    require(manifest.get("peer_sources", []) ==
            ([str(directory / "encrypted peer")] if manifest["backend"] == "raid1c" else []),
            "Recorded mirror member paths changed")
    result = {"schema": 1, "status": "failed", "suite": "isolated-vm-abrupt-poweroff", **manifest,
              "ready": ready, "repair": False, "poweroff_method": "libvirt-destroy"}
    phase = "verify-recovered-package"
    try:
        _, digest = verify_staged_package(Path(manifest["cli"]), manifest["source_commit"], manifest["backend"])
        require(digest == manifest["package_manifest_sha256"], "Staged package manifest changed after poweroff")
        phase = "reopen-through-candidate-cli"
        guest_cli(directory, guest_volume_arguments("mount", manifest), secret)
        require(os.path.ismount(mountpoint), "Recovery CLI did not expose the encrypted filesystem")
        phase = "capture-raw-WAL-evidence"
        evidence = directory / "recovered-evidence"
        evidence.mkdir(mode=0o700)
        for name in ("profile.sqlite", "profile.sqlite-wal", "profile.sqlite-shm"):
            path = mountpoint / name
            if path.exists():
                shutil.copyfile(path, evidence / name)
        phase = "durable-record-and-SQLite-recovery"
        payload = (mountpoint / "durable record").read_bytes()
        require(hashlib.sha256(payload).hexdigest() == ready["sha256"], "Acknowledged durable record changed")
        connection = sqlite3.connect((mountpoint / "profile.sqlite").as_uri() + "?mode=rw", uri=True)
        try:
            require(connection.execute("PRAGMA integrity_check").fetchone()[0] == "ok", "SQLite integrity failed")
            require(connection.execute("SELECT value FROM committed WHERE id=1").fetchone()[0] == payload,
                    "Acknowledged SQLite row changed")
            result["uncommitted_row_visible"] = connection.execute("SELECT count(*) FROM committed WHERE id=2").fetchone()[0]
            require(result["uncommitted_row_visible"] == 0, "Uncommitted SQLite row became visible")
        finally:
            connection.close()
        result["unacknowledged_file_size"] = (mountpoint / "unacknowledged writes").stat().st_size if (mountpoint / "unacknowledged writes").exists() else None
        phase = "normal-cli-close"
        guest_cli(directory, ["unmount", str(mountpoint)], secret)
        require(not os.path.ismount(mountpoint), "Recovery CLI reported close while the mount remains attached")
        result.update(status="passed", checks=5)
    except Exception as error:
        result.update(status="failed", failed_stage=phase, error=str(error).replace(secret, "<redacted>")[:2048])
    write_json(directory / "result.json", result)
    print(json.dumps({"status": result["status"], "result": str(directory / "result.json"),
                      "failed_stage": result.get("failed_stage")}), flush=True)
    return 0 if result["status"] == "passed" else 1

def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="action", required=True)
    plan = commands.add_parser("prepare", help="Read-only hypervisor preflight; save concrete private plan")
    plan.add_argument("--os", choices=SOURCES, required=True)
    plan.add_argument("--run", required=True)
    plan.add_argument("--subnet", required=True)
    plan.add_argument("--bandwidth", type=int, default=32)
    plan.add_argument("--dragonfly-fixture-disk", action="store_true")
    for action in ("copy-boot", "finish-copy-boot", "provision-fixture-disk", "static-transport", "network-stop", "inspect", "poweroff", "boot", "ssh"):
        command = commands.add_parser(action)
        command.add_argument("directory", type=Path)
        if action == "ssh":
            command.add_argument("command")
            command.add_argument("--user", default="adjutant", choices=("adjutant", "root"))
    setup = commands.add_parser("guest-setup", help="Inside an owned clone only; prepare persistent workload")
    setup.add_argument("--run", required=True)
    setup.add_argument("--cli", type=Path, required=True)
    setup.add_argument("--backend", choices=("luks", "luks1", "veracrypt", "geli", "cgd", "softraid", "raid1c", "gocryptfs", "cryfs", "cryptomator", "pefs"), required=True)
    setup.add_argument("--cipher", choices=("aes", "serpent", "twofish"), default="aes")
    setup.add_argument("--commit", required=True)
    setup.add_argument("--unprivileged", action="store_true")
    setup.add_argument("--persistent-home-confirmed", action="store_true")
    setup.add_argument("--netbsd-sync-diagnostic", action="store_true",
                       help="For NetBSD CGD, require a synchronous journaled fixture before the workload")
    setup.add_argument("--openbsd-sync-diagnostic", action="store_true",
                       help="For an OpenBSD container, require synchronous FFS before the workload")
    setup.add_argument("--netbsd-allocated-diagnostic", action="store_true",
                       help="For NetBSD CGD, mount an allocated copy of the new fixture")
    setup.add_argument("--freebsd-suj-diagnostic", action="store_true",
                       help="For FreeBSD GELI, enable soft-updates journaling on the new fixture")
    for action in ("guest-worker", "guest-verify"):
        guest = commands.add_parser(action)
        guest.add_argument("--run", required=True)
    args = parser.parse_args()
    if args.action == "guest-setup":
        return guest_setup(args)
    if args.action == "guest-worker":
        guest_worker(args.run)
        return 0
    if args.action == "guest-verify":
        return guest_verify(args.run)
    if args.action == "prepare":
        require(1 <= args.bandwidth <= 64, "Bandwidth must be 1–64 MiB/s")
        clone = prepare(args.os, args.run, args.subnet, args.bandwidth, args.dragonfly_fixture_disk)
        print(json.dumps(clone.manifest, indent=2))
        return 0
    clone = OwnedClone.load(args.directory)
    if args.action == "copy-boot":
        copy_and_boot(clone)
    elif args.action == "finish-copy-boot":
        finish_copy_and_boot(clone)
    elif args.action == "provision-fixture-disk":
        clone.provision_fixture_disk()
    elif args.action == "static-transport":
        clone.register_static_transport()
    elif args.action == "network-stop":
        clone.stop_network()
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
