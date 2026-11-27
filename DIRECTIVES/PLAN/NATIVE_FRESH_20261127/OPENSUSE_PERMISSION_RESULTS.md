# openSUSE independent caller permission follow-up

Status: **original NTFS3 failure reproduced; fresh-create correction passed its first product gate**. Six representative retained routes passed unrelated-user denial; the seventh exposed a real privacy failure. The planned independent UFS2 sample remains held. Original observations below retain their unchanged e927 identity; the later candidate and its native build have separate evidence.

Product: unchanged installed `e927e40bb235815343302677531d48f1afa4f939`, CLI SHA-256 `37d8ce2814df48218719001f869fcde6b5eaebbac5aa4b80b555856f834df8ab`, helper `2e4407f4fcc7fd3e156dc53b63187a0206d0793c65f4a5dd0435a56566a4eb68`. Native host: Leap 16.1, packaged kernel `7.3.6-lp160.4.gf857b30-default`, unchanged boot `4f2b5d8f-8d91-4705-8dfb-b7776e3ba4ab`. The original 576/678/e927 evidence retains its original scope; these new observations do not relabel prior tests.

## Product observations

The bounded controller was hashed before native launch (`632dffa8269bb7d47e70f2901e8ee2efc617fe72a4798f4300d3af20db704daa`). Strict read-only preflight verified installed hashes, root-controlled executable ancestry, clean RPM inventory, six closed 678 fixtures and unchanged private credential hashes. Actual foreign UID/EUID 65534 had group 65534, no owner group 1300 and zero capabilities. Each foreign process successfully changed into every plaintext ancestor and read a public traversal control before testing the mounted root. Parent paths were newly owned `0711`; retained sources/payloads were not chmodded or chowned.

- **Passed:** retained LUKS2/Btrfs, LUKS1/ext4, VeraCrypt/exFAT, and gocryptfs/CryFS/Cryptomator on ext4. Each passed actual owner UID 1300 list/read/write/create before and after four foreign list/read/write/create refusals. Original appended downloaded payload hashes and the prior ordinary-reboot marker remained unchanged. These samples cover kernel POSIX permissions, exFAT synthetic ownership and all three directory engines, not every filesystem/backend pair.
- **Failed:** a new 128 MiB LUKS1/NTFS3/keyfile fixture mounted read/write with `uid=1300,gid=1300,dmask=0077,fmask=0077,acl`, yet its root was mode `0777`. The foreign process unexpectedly listed the plaintext root; the controller stopped on that first unexpected success. The source/key and original receipts were preserved, and ordinary product unmount succeeded. No timeout or unknown worker occurred.
- **Unstarted:** the planned new LUKS1/UFS2 sample. Its prepared resume controller was never launched. No namespace rename/delete supplement was duplicated; that shared harness remains owned by Arch.

## Matched native controls

Three NEW byte-verified clones of the failed encrypted image were used. Original ciphertext SHA-256 `6fd0d04ea04eb381738e369c3ad09e0a21aa2845a614b444a35f35799b7ca5d8`, size 134217728, and original key SHA-256 `5f66aa01c26ddd174c0253cc46cefff71f1f4e068528ab34ebae2163259817fa` remained unchanged. All three initial clone hashes matched before any native open. Root-owned/protected controller and plan were verified before launch: controller `84b926c7e6f11e29e4d8e96d9d0c3b6317df8f75bac39deb5ce3ff8b332baa03`, plan `6a58dbccd1a7948feac64ea3fbf0f27df73ca9bda8ebbd107668a1eee883a5f8`. The root coordinator performed native setup only; every permission probe ran as a separate actual UID 1300 or 65534 process with zero capabilities and independently proven ancestor traversal.

| New clone | Native observation |
|---|---|
| A: unchanged clone | Ordinary cryptsetup open and native `mount -i -t ntfs3 -o uid=1300,gid=1300,umask=0077` reproduced root 0777 and ACL `user::rwx/group::rwx/other::rwx`. Foreign list and create succeeded; read/write of the separately created owner 0600 probe were denied with EACCES. Owner operations succeeded before/after. No format or chmod occurred. |
| B: unchanged clone plus `noacl` | Native mount rejected the option. The mapper closed normally; no format/chmod or permission pass was claimed. |
| C: new format and private root | Only this new clone was reformatted with the exact native profile `mkntfs -Q -F`. Before initialization it independently reproduced root 0777 and foreign list/create success. Existing held-directory-FD `fchown(1300,1300)`, `fchmod(0700)` and `fsync` changed the fresh root to 0700. All four foreign operations then returned EACCES while owner operations succeeded. Normal unmount/remount with the original options preserved 0700 and the same four denials. |

The kernel module is the installed signed in-tree NTFS3 provider with `CONFIG_NTFS3_FS_POSIX_ACL=y`; module/tool versions, hashes, actual mount records, root filesystem IDs and ACL observations are retained. No kernel code, module, ABI or lifecycle behavior was changed. The official NTFS3 documentation describes masks as defaults for newly created entries, consistent with the formatter root retaining its initial permissions: https://www.kernel.org/doc/html/latest/filesystems/ntfs3.html . These results justify evaluating **fresh-create-only** native root initialization; they do not justify changing an existing filesystem's ownership or permissions on ordinary open. The coordinator owns that shared product change.

All native controls reached known terminal state and used normal unmount/cryptsetup close. No mount/mapper or unknown worker remains. Original and cloned images, key and complete outputs remain retained. Root free space stayed above 10.0 GB and utility above 40.2 GB. Evidence: `target/goal-validation/native-fresh-20261127/hosts/opensuse/build/foreign-uid-proof/`, especially `native-controls-result.json`, `native-evidence/`, `checkpoint.json` and `archive-verification.json`. The verified private archive `original-failure-and-three-native-clones.tar.gz` contains 124 members with matching native before/after hashes, sizes and modes plus full gzip CRC; SHA-256 `e68825314f3c6989b081a20306f35209249a057ee42e85ef969fc97fdc81a237`. Native controls are diagnostics, never a substitute for a product regression pass.

## Fresh-create correction and first product qualification

The shared Linux formatter now initializes only a newly formatted NTFS root
through the existing held-directory ownership path, selects the qualified native
driver explicitly, verifies caller ownership and mode 0700, and synchronizes the
directory before normal initialization unmount. Ordinary open does not change
existing filesystem ownership, permissions or ACLs. Cipher and KDF profiles are
unchanged.

Frozen source is base `e71db1b` plus the single-file overlay SHA-256
`4e9d1aa3704ca341e7c92e002b9bbaa04cce3ed8156bd80a7a6777c6f2e59f6e`;
`linux_workflow.rs` is
`7b2e26feefbe0f6ad6b7d358e67e7336a49caa60b72a9e6ed4f4dfc3c661fe35`.
Native workspace check/test and all three product builds passed: 199 tests,
zero failures, 15 ignored. The initial formatting check failed on 181 existing
baseline regions; none of the three changed hunks appears in that output.
Unrelated formatting was preserved rather than silently reported as passing.

A new administrator-controlled prefix staged only LUKS1, LUKS2 and VeraCrypt.
Three fresh CLI NTFS cases and one helper case passed 28 operations and 12 full
payload opens, normal close/reopen, append and final hashes. Every open reported
root UID/GID 1300 and mode 0700. Actual owner access passed before and after 48
foreign list/read/write/create denials; each foreign process proved ancestor
traversal and zero capabilities. No test-side post-create chmod supplied a pass.

A separate NEW byte-verified clone of the original failed image opened twice
through the candidate with its old root mode 0777, ACL and probe hash unchanged.
Original ciphertext/key hashes remained unchanged. This confirms preservation
of existing metadata, **not** a privacy pass or automatic repair of old images.

Independent audit checked all 636 source files, 201 evidence-archive members,
869 selected-package payload files and 128 raw logs. Its private receipt is
`hosts/debian/build/ntfs-root-independent-audit-1/audit-result.json`, SHA-256
`39d357b5781f7aed1eae5732d5e0d013070e83ab7f37d7af30b6f52dced7a12d`.
The first-product archive is `683a077f8d9470564fe1be99f1d7ad347621f29cd67df321774ceebfd63cb5e0`.
This is the bounded first gate, not qualification of all six backends or all
Linux distributions. Full-component staging, remaining namespace checks and
affected-host qualification remain separate.
