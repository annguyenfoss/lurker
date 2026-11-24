"""Fail-closed ownership tests; never call libvirt or execute product storage."""
import importlib.util
import ipaddress
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch
import xml.etree.ElementTree as ET

SPEC = importlib.util.spec_from_file_location("vm_powerloss", Path(__file__).parents[1] / "test-vm-powerloss.py")
VM = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = VM
SPEC.loader.exec_module(VM)

SOURCE = '''<domain type="kvm"><name>archlinux-core-01</name><uuid>original</uuid>
<os><type>hvm</type></os><devices><emulator>/usr/bin/qemu</emulator>
<disk type="file" device="disk"><driver type="qcow2"/><source file="/original"/>
<target dev="vda"/></disk><interface type="network"><mac address="original"/>
<source network="styx"/></interface><filesystem><source dir="/home"/></filesystem>
<hostdev/><channel><source path="/original-socket"/></channel>
<serial><source path="/original-serial"/></serial><graphics port="5900"/>
<tpm/><rng><backend>/dev/random</backend></rng></devices></domain>'''


def manifest(directory):
    return {"run": "test1234", "os": "archlinux", "source": "archlinux-core-01",
            "name": directory.name + "-archlinux", "uuid": "owned-uuid", "marker": "lurker-disposable-fault:test1234",
            "disk": str(directory / "root.qcow2"), "network": directory.name,
            "network_uuid": "owned-network", "bridge": "lf1234", "mac": "52:54:00:12:34:56",
            "subnet": "192.168.231.0/24"}


class Ownership(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.directory = self.root / "lurker-fault-test1234"
        self.directory.mkdir()
        self.m = manifest(self.directory)
        VM.write_json(self.directory / "manifest.json", self.m)
        self.root_patch = patch.object(VM, "ROOT", self.root)
        self.root_patch.start()
        self.addCleanup(self.root_patch.stop)

    def test_atomic_json_fsync_order_includes_published_directory(self):
        events = []
        original_fsync, original_replace = VM.os.fsync, VM.Path.replace
        def fsync(descriptor):
            events.append("fsync-directory" if VM.stat.S_ISDIR(os.fstat(descriptor).st_mode) else "fsync-file")
            original_fsync(descriptor)
        def replace(path, destination):
            events.append("replace")
            return original_replace(path, destination)
        with patch.object(VM.os, "fsync", side_effect=fsync), patch.object(VM.Path, "replace", autospec=True, side_effect=replace):
            VM.write_json(self.directory / "ack.json", {"ready": True})
        self.assertEqual(events, ["fsync-file", "replace", "fsync-directory"])

    def test_own_manifest_loads(self):
        self.assertEqual(VM.OwnedClone.load(self.directory).manifest, self.m)
        self.assertEqual((self.directory / "manifest.json").stat().st_mode & 0o777, 0o600)

    def test_main_names_and_external_disks_rejected(self):
        for key, value in (("name", "archlinux-core-01"), ("disk", "/original"),
                           ("network", "styx"), ("marker", "foreign"), ("source", "debian-core-01")):
            with self.subTest(key=key):
                VM.write_json(self.directory / "manifest.json", {**self.m, key: value})
                with self.assertRaises(RuntimeError):
                    VM.OwnedClone.load(self.directory)

    def test_clone_sanitizes_all_external_resources(self):
        xml = VM.clone_xml(SOURCE, self.m)
        domain = ET.fromstring(xml)
        self.assertEqual(VM.disk_source(xml), self.m["disk"])
        self.assertEqual(domain.findtext("uuid"), "owned-uuid")
        self.assertEqual(domain.findtext("description"), self.m["marker"])
        for tag in ("hostdev", "filesystem", "channel", "graphics", "tpm", "rng"):
            self.assertIsNone(domain.find("devices/" + tag))
        self.assertNotIn("/original", xml)
        self.assertNotIn('network="styx"', xml)

    def test_existing_job_or_extra_disk_rejected(self):
        for xml in (SOURCE.replace("<target dev=", "<mirror/><target dev="),
                    SOURCE.replace("</devices>", '<disk type="file"/></devices>')):
            with self.assertRaises(RuntimeError):
                VM.disk_source(xml)

    def test_dragonfly_fixture_disk_is_restricted_to_owned_path_and_serial(self):
        source = SOURCE.replace('<target dev="vda"/>', '<target dev="hda" bus="ide"/>')
        owned = {**self.m, "os": "dragonflybsd", "fixture_disk": str(self.directory / "fixture.qcow2")}
        with patch.object(VM, "firmware_policy", return_value=None), patch.object(VM, "validate_firmware_xml"):
            xml = VM.clone_xml(source, owned)
        self.assertEqual(VM.clone_disks(ET.fromstring(xml), owned), owned["disk"])
        for changed in (
            xml.replace(owned["fixture_disk"], "/foreign.qcow2"),
            xml.replace(VM.FIXTURE_DISK_SERIAL, "another-disk"),
            xml.replace("<devices>", '<devices><disk><target dev="vdc"/></disk>'),
        ):
            with self.assertRaises(RuntimeError):
                VM.clone_disks(ET.fromstring(changed), owned)
        with self.assertRaises(RuntimeError):
            VM.fixture_disk({**owned, "os": "archlinux"})

    def test_fixture_disk_replacement_cannot_authorize_boot_or_poweroff(self):
        root = self.directory / "root.qcow2"
        data = self.directory / "fixture.qcow2"
        root.touch()
        data.touch()
        owned = {**self.m, "os": "dragonflybsd", "fixture_disk": str(data)}
        clone = VM.OwnedClone(self.directory, owned)
        VM.write_json(self.directory / "copy-result.json", {
            "disk_identity": {"device": root.stat().st_dev, "inode": root.stat().st_ino},
        })
        VM.write_json(self.directory / "fixture-disk.json", {
            "path": str(data), "device": data.stat().st_dev, "inode": data.stat().st_ino,
        })
        clone.verify_disk()
        data.rename(self.directory / "saved-fixture.qcow2")
        data.touch()
        with patch.object(VM, "virsh") as call:
            with self.assertRaisesRegex(RuntimeError, "Fixture disk inode changed"):
                clone.poweroff()
            call.assert_not_called()

    def test_writable_firmware_rejected(self):
        with self.assertRaises(RuntimeError):
            VM.clone_xml(SOURCE.replace("</os>", "<nvram>/original</nvram></os>"), self.m)

    def test_unreviewed_automatic_uefi_rejected(self):
        with self.assertRaises(RuntimeError):
            VM.clone_xml(SOURCE.replace("<os>", '<os firmware="efi">'), self.m)

    def test_network_has_no_forward(self):
        network = ET.fromstring(VM.network_xml(self.m))
        self.assertIsNone(network.find("forward"))
        self.assertEqual(network.find("dns").get("enable"), "no")
        self.assertEqual(network.find("ip/dhcp/host").get("mac"), self.m["mac"])

    def test_replaced_clone_cannot_authorize_destroy(self):
        clone = VM.OwnedClone.load(self.directory)
        xml = VM.clone_xml(SOURCE, self.m).replace("owned-uuid", "foreign-uuid")
        with patch.object(VM.OwnedClone, "verify_disk"), patch.object(VM, "virsh", return_value=xml) as call:
            with self.assertRaises(RuntimeError):
                clone.poweroff()
        self.assertEqual(call.call_count, 1)
        self.assertEqual(call.call_args.args[0], "dumpxml")

    def test_saved_xml_cannot_add_host_disk(self):
        clone = VM.OwnedClone.load(self.directory)
        (self.directory / "source-inactive.xml").write_text(SOURCE)
        (self.directory / "clone.xml").write_text(SOURCE)
        with patch.object(VM.OwnedClone, "verify_disk"), patch.object(VM.OwnedClone, "verify_network"), patch.object(VM, "virsh") as call:
            with self.assertRaises(RuntimeError):
                clone.create()
        call.assert_not_called()

    def test_lease_outside_isolated_subnet_cannot_authorize_ssh(self):
        clone = VM.OwnedClone.load(self.directory)
        with patch.object(VM.OwnedClone, "verify_domain"), patch.object(VM, "virsh", return_value="10.11.1.18/24"), patch.object(VM, "run") as call:
            with self.assertRaises(RuntimeError):
                clone.ssh("true")
        call.assert_not_called()

    def test_disk_symlink_cannot_authorize_boot_or_poweroff(self):
        clone = VM.OwnedClone.load(self.directory)
        (self.directory / "root.qcow2").symlink_to("/original")
        with patch.object(VM, "virsh") as call:
            for action in (clone.create, clone.poweroff):
                with self.assertRaises(RuntimeError):
                    action()
        call.assert_not_called()

    def test_replaced_disk_inode_rejected(self):
        clone = VM.OwnedClone.load(self.directory)
        (self.directory / "root.qcow2").write_bytes(b"fixture")
        VM.write_json(self.directory / "copy-result.json", {"disk_identity": {"device": -1, "inode": -1}})
        with patch.object(VM, "virsh") as call:
            with self.assertRaisesRegex(RuntimeError, "inode"):
                clone.create()
        call.assert_not_called()

    def test_dangling_copy_destination_rejected_before_libvirt(self):
        clone = VM.OwnedClone.load(self.directory)
        (self.directory / "root.qcow2").symlink_to(self.directory / "missing")
        with patch.object(VM, "virsh") as call:
            with self.assertRaisesRegex(RuntimeError, "symlink"):
                VM.copy_and_boot(clone)
        call.assert_not_called()

    def test_network_policy_replacement_rejected(self):
        clone = VM.OwnedClone.load(self.directory)
        xml = VM.network_xml(self.m)
        for before, after in ((self.m["marker"], "foreign-marker"), ('enable="no"', 'enable="yes"'),
                              (self.m["mac"], "52:54:00:00:00:00"), ("192.168.231.10", "192.168.231.11"),
                              ("255.255.255.0", "255.255.0.0")):
            with self.subTest(field=before), patch.object(VM, "virsh", return_value=xml.replace(before, after)):
                with self.assertRaises(RuntimeError):
                    clone.verify_network()
        with patch.object(VM, "virsh", return_value=xml):
            clone.verify_network()

    def test_partial_copy_cannot_resume(self):
        clone = VM.OwnedClone.load(self.directory)
        (self.directory / "root.qcow2").write_bytes(b"partial")
        (self.directory / "blockcopy.log").write_text("interrupted")
        with patch.object(VM, "virsh") as call:
            with self.assertRaisesRegex(RuntimeError, "completion"):
                VM.finish_copy_and_boot(clone)
        call.assert_not_called()

    def test_unreadable_copy_uses_only_owned_metadata_pool(self):
        clone = VM.OwnedClone.load(self.directory)
        name = self.m["network"] + "-probe"
        pool = f'<pool type="dir"><name>{name}</name><uuid>probe-uuid</uuid><target><path>{self.directory}</path></target></pool>'
        volume = f'<volume><name>root.qcow2</name><capacity>12345</capacity><target><path>{self.m["disk"]}</path><format type="qcow2"/></target><backingStore/></volume>'
        def answer(*args):
            if args[0] == "pool-list":
                return "default iso vmpool01"
            if args[0] == "pool-dumpxml":
                return pool
            if args[0] == "vol-dumpxml":
                return volume
            return ""
        with patch.object(VM.os, "access", return_value=False), patch.object(VM, "virsh", side_effect=answer) as call:
            image = VM.inspect_private_copy(clone)
        self.assertEqual(image["inspection"], "libvirt-directory-pool")
        mutations = [entry.args for entry in call.call_args_list if entry.args[0] in ("pool-create-as", "pool-destroy")]
        self.assertEqual(mutations, [("pool-create-as", name, "dir", "--target", self.directory), ("pool-destroy", "probe-uuid")])
        self.assertTrue((self.directory / "copy-volume.xml").is_file())

    def test_probe_pool_collision_never_touches_existing_pool(self):
        clone = VM.OwnedClone.load(self.directory)
        with patch.object(VM.os, "access", return_value=False), patch.object(VM, "virsh", return_value=self.m["network"] + "-probe") as call:
            with self.assertRaisesRegex(RuntimeError, "already exists"):
                VM.inspect_private_copy(clone)
        self.assertEqual(call.call_count, 1)

    def test_static_transport_rejects_route_and_mac_mismatch(self):
        clone = VM.OwnedClone.load(self.directory)
        route = {"dst": "192.168.231.10", "dev": self.m["bridge"]}
        neighbor = {"dst": route["dst"], "dev": self.m["bridge"], "lladdr": self.m["mac"], "state": ["REACHABLE"]}
        for wrong_route, wrong_neighbor in (({**route, "dev": "virbr-styx"}, neighbor),
                                            ({**route, "gateway": "10.11.1.1"}, neighbor),
                                            (route, {**neighbor, "lladdr": "52:54:00:00:00:00"}),
                                            (route, {**neighbor, "state": ["INCOMPLETE"]})):
            with self.subTest(route=wrong_route, neighbor=wrong_neighbor), patch.object(VM.OwnedClone, "verify_domain"), patch.object(VM, "run", side_effect=[json.dumps([wrong_route]), json.dumps([wrong_neighbor])]), patch.object(VM.subprocess, "run"):
                with self.assertRaises(RuntimeError):
                    clone.static_address()
        with patch.object(VM.OwnedClone, "verify_domain"), patch.object(VM, "run", side_effect=[json.dumps([route]), json.dumps([neighbor])]), patch.object(VM.subprocess, "run"):
            self.assertEqual(clone.static_address(), "192.168.231.10")

    def test_network_stop_rejects_active_connections(self):
        clone = VM.OwnedClone.load(self.directory)
        xml = ET.fromstring(VM.network_xml(self.m));xml.set("connections", "1")
        with patch.object(VM.OwnedClone, "verify_network", return_value=xml), patch.object(VM, "virsh") as call:
            with self.assertRaisesRegex(RuntimeError, "connected"):
                clone.stop_network()
        call.assert_not_called()
    def test_ssh_selects_native_paths_and_keeps_strict_host_key(self):
        clone = VM.OwnedClone.load(self.directory)
        command = "mount\nprintf '%s\\n' 'quoted value'"
        with patch.object(VM.OwnedClone, "lease", return_value="192.168.231.10"), patch.object(VM, "run", return_value="ready") as call:
            self.assertEqual(clone.ssh(command, user="root"), "ready")
        argv = call.call_args.args[0]
        self.assertIn("StrictHostKeyChecking=yes", argv)
        self.assertIn("HostKeyAlias=archlinux-core-01", argv)
        self.assertEqual(argv[-1], "/bin/sh -s")
        path_setup, script = call.call_args.kwargs["input_text"].split("\n", 1)
        self.assertTrue(path_setup.startswith("export PATH="))
        self.assertIn("/sbin", path_setup)
        self.assertEqual(script, command + "\n")

    def dragonfly_fixture(self):
        m = {**self.m, "os": "dragonflybsd", "source": "dragonflybsd-core-01",
             "name": self.directory.name + "-dragonflybsd", "disk_target": "hda",
             "firmware": {"loader": VM.UEFI_CODE, "template": VM.UEFI_VARS,
                          "nvram": str(self.directory / "nvram.fd"),
                          "loader_sha256": "approved", "template_sha256": "approved"}}
        source = SOURCE.replace('<os>', '<os firmware="efi">').replace('</os>',
            '<loader readonly="yes" type="pflash" format="raw">' + VM.UEFI_CODE + '</loader>' +
            '<nvram template="' + VM.UEFI_VARS + '" templateFormat="raw" format="raw">/original-vars</nvram></os>')
        source = source.replace('<target dev="vda"/>', '<target dev="hda" bus="ide"/>')
        return m, source

    def test_reviewed_ide_target_preserves_private_disk_and_firmware(self):
        m, source = self.dragonfly_fixture()
        with self.assertRaises(RuntimeError):
            VM.disk_source(source)
        self.assertEqual(VM.disk_source(source, "hda"), "/original")
        result = VM.clone_xml(source, m)
        root = ET.fromstring(result)
        self.assertEqual(root.findtext("os/nvram"), m["firmware"]["nvram"])
        self.assertEqual(root.find("os/loader").get("readonly"), "yes")
        self.assertEqual(VM.disk_source(result, "hda"), m["disk"])
        self.assertNotIn("/original-vars", result)
        self.assertIsNone(root.find("devices/hostdev"))

    def test_firmware_loader_and_external_nvram_policy_rejected(self):
        m, source = self.dragonfly_fixture()
        for bad in (source.replace('readonly="yes"', 'readonly="no"'), source.replace(VM.UEFI_CODE, "/home/user/firmware")):
            with self.assertRaises(RuntimeError):
                VM.clone_xml(bad, m)
        m["firmware"]["nvram"] = "/original-vars"
        VM.write_json(self.directory / "manifest.json", m)
        with self.assertRaises(RuntimeError):
            VM.OwnedClone.load(self.directory)

    def test_private_nvram_collision_and_replacement_rejected(self):
        m, _ = self.dragonfly_fixture()
        VM.write_json(self.directory / "manifest.json", m)
        clone = VM.OwnedClone.load(self.directory)
        nvram = self.directory / "nvram.fd"
        nvram.symlink_to(self.directory / "missing")
        with patch.object(VM, "trusted_firmware_file", return_value="approved"), patch.object(VM, "virsh") as call:
            with self.assertRaises(RuntimeError):
                clone.verify_firmware_resources(before_boot=True)
        call.assert_not_called()
        nvram.unlink();nvram.write_bytes(b"private vars")
        VM.write_json(self.directory / "firmware-state.json", {"path": str(nvram), "device": -1, "inode": -1})
        with patch.object(VM, "trusted_firmware_file", return_value="approved"):
            with self.assertRaisesRegex(RuntimeError, "inode"):
                clone.verify_firmware_resources()

    def test_untrusted_firmware_ancestor_rejected(self):
        path = self.directory / "firmware.fd";path.write_bytes(b"untrusted")
        with self.assertRaisesRegex(RuntimeError, "administrator controlled"):
            VM.trusted_firmware_file(path)



class GuestFixture(unittest.TestCase):
    def test_freebsd_suj_control_rejects_a_changed_md_identity(self):
        with tempfile.TemporaryDirectory() as directory:
            fixture = Path(directory) / "encrypted data"
            fixture.touch()
            with patch.object(VM, "run", side_effect=["md3", "md0", "wrong identity"]) as native:
                with self.assertRaisesRegex(RuntimeError, "backing identity changed"):
                    VM.freebsd_suj_diagnostic(fixture, "fixture")
                self.assertEqual(native.call_count, 3)

    def test_netbsd_allocated_control_preserves_original_sparse_ciphertext(self):
        with tempfile.TemporaryDirectory() as directory:
            fixture = Path(directory) / "encrypted data"
            with fixture.open("xb") as output:
                output.seek(1024 * 1024 - 1)
                output.write(b"x")
            before = fixture.stat()
            expected = fixture.read_bytes()
            observed = VM.netbsd_allocated_diagnostic(fixture)
            retained = fixture.with_name("sparse ciphertext evidence")
            self.assertEqual(retained.stat().st_ino, before.st_ino)
            self.assertNotEqual(fixture.stat().st_ino, before.st_ino)
            self.assertEqual(fixture.read_bytes(), expected)
            self.assertEqual(retained.read_bytes(), expected)
            self.assertGreaterEqual(observed["copy_bytes_allocated"], len(expected))

    def test_openbsd_sync_control_requires_same_writable_ffs_device(self):
        fixture = Path("/owned/open data")
        before = f"/dev/vnd0a on {fixture} type ffs (local, nodev)"
        command = ["/sbin/mount", "-u", "-o", "sync", str(fixture)]
        synchronized = f"/dev/vnd0a on {fixture} type ffs (local, nodev, synchronous)"
        with patch.object(VM.os.path, "ismount", return_value=True):
            with patch.object(VM, "run", side_effect=[before, "", synchronized]) as native:
                observed = VM.openbsd_sync_diagnostic(fixture)
                self.assertEqual(observed["device"], "/dev/vnd0a")
                self.assertIn("synchronous", observed["workload_flags"])
                native.assert_any_call(command)
            for unsafe in (synchronized.replace("vnd0a", "vnd1a"),
                           synchronized.replace("synchronous", "read-only"),
                           before):
                with self.subTest(unsafe=unsafe), patch.object(VM, "run", side_effect=[before, "", unsafe]):
                    with self.assertRaisesRegex(RuntimeError, "Diagnostic"):
                        VM.openbsd_sync_diagnostic(fixture)

    def test_dragonfly_filesystem_selection_applies_only_at_creation(self):
        manifest = {"backend": "veracrypt", "os": "DragonFly", "source": "/owned/vault",
                    "mountpoint": "/owned/open", "cipher": "aes", "size_gb": "0.5",
                    "create_filesystem": "hammer2"}
        self.assertEqual(VM.guest_volume_arguments("create", manifest),
                         ["create", "--type", "veracrypt", "--cipher", "aes", "--filesystem",
                          "hammer2", "/owned/vault", "0.5"])
        self.assertEqual(VM.guest_volume_arguments("mount", manifest),
                         ["mount", "--type", "veracrypt", "/owned/vault", "/owned/open"])
        with self.assertRaises(RuntimeError):
            VM.guest_volume_arguments("create", {**manifest, "os": "Linux"})

    def test_workload_cannot_rearm_an_existing_worker_log(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            (directory / "worker.log").touch()
            manifest = {"mountpoint": str(directory / "open")}
            with patch.object(VM, "guest_cli") as cli, self.assertRaisesRegex(RuntimeError, "already started"):
                VM.arm_guest_workload(directory, manifest, "fixture")
            cli.assert_not_called()

    def test_raid_workload_reopens_the_same_two_recorded_members(self):
        manifest = {"backend": "raid1c", "source": "/owned/primary", "peer_sources": ["/owned/peer"],
                    "mountpoint": "/owned/open", "size_gb": "0.125", "cipher": "aes"}
        self.assertEqual(VM.guest_volume_arguments("create", manifest),
                         ["create", "--type", "raid1c", "--member", "/owned/peer", "/owned/primary", "0.125"])
        self.assertEqual(VM.guest_volume_arguments("mount", manifest),
                         ["mount", "--type", "raid1c", "--member", "/owned/peer", "/owned/primary", "/owned/open"])
        for peers in ([], ["/owned/primary"], ["/owned/peer", "/owned/third"]):
            with self.subTest(peers=peers), self.assertRaises(RuntimeError):
                VM.guest_volume_arguments("mount", {**manifest, "peer_sources": peers})
        with self.assertRaises(RuntimeError):
            VM.guest_volume_arguments("mount", {**manifest, "backend": "softraid"})

    def test_login_routes_and_fixed_profiles_are_enforced_before_workload(self):
        for system, backend, uid, unprivileged in [
                ("Linux", "cryptomator", 0, False), ("FreeBSD", "gocryptfs", 0, False),
                ("OpenBSD", "raid1c", 1000, False), ("NetBSD", "cgd", 0, True),
                ("DragonFly", "gocryptfs", 1000, False)]:
            with self.subTest(system=system, backend=backend), self.assertRaises(RuntimeError):
                VM.validate_guest_route(backend, unprivileged, "aes", system, uid)
        self.assertEqual(VM.validate_guest_route("veracrypt", True, "twofish", "Linux", 1000), "login-user")
        self.assertEqual(VM.validate_guest_route("raid1c", False, "aes", "OpenBSD", 0), "native-root")
        self.assertEqual(VM.validate_guest_route("veracrypt", False, "serpent", "DragonFly", 0), "native-root")
        with self.assertRaises(RuntimeError):
            VM.validate_guest_route("raid1c", False, "serpent", "OpenBSD", 0)

    def test_owned_persistent_home_marker_and_permissions(self):
        with tempfile.TemporaryDirectory() as temporary, patch.object(VM.Path, "home", return_value=Path(temporary)), patch.object(VM.platform, "system", return_value="FreeBSD"):
            directory = VM.guest_directory("test1234", create=True)
            self.assertEqual(VM.guest_directory("test1234"), directory)
            VM.write_json(directory / "owner.json", {"run": "other123", "uid": os.geteuid()})
            with self.assertRaises(RuntimeError):
                VM.guest_directory("test1234")

    def test_new_guest_fixture_ancestry_is_published_durably(self):
        with tempfile.TemporaryDirectory() as temporary, patch.object(VM.Path, "home", return_value=Path(temporary)), patch.object(VM.platform, "system", return_value="FreeBSD"), patch.object(VM, "fsync_path", wraps=VM.fsync_path) as sync:
            home = Path(temporary)
            VM.guest_directory("test1234", create=True)
            calls = [entry.args[0] for entry in sync.call_args_list]
            self.assertIn(home, calls)
            self.assertIn(home / ".cache", calls)
            self.assertIn(home / ".cache/lurker", calls)

    def test_cache_symlink_rejected_before_fixture_creation(self):
        with tempfile.TemporaryDirectory() as temporary, tempfile.TemporaryDirectory() as other:
            home = Path(temporary)
            (home / ".cache").symlink_to(other)
            with patch.object(VM.Path, "home", return_value=home):
                with self.assertRaises(RuntimeError):
                    VM.guest_directory("test1234", create=True)
            self.assertFalse((Path(other) / "lurker").exists())

    def test_linux_tmpfs_fixture_rejected(self):
        with tempfile.TemporaryDirectory() as temporary, patch.object(VM.Path, "home", return_value=Path(temporary)):
            with patch.object(VM.platform, "system", return_value="Linux"), patch.object(VM.Path, "read_text", return_value="1 0 0:1 / / rw - tmpfs tmpfs rw"):
                # read_text is reached first for owner.json, so provide both.
                with patch.object(VM.json, "loads", return_value={"run": "test1234", "uid": os.geteuid()}):
                    with self.assertRaisesRegex(RuntimeError, "persistent"):
                        VM.guest_directory("test1234", create=True)


if __name__ == "__main__":
    unittest.main()
