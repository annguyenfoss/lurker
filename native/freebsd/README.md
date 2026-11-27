# Optional FreeBSD shutdown setup

`lurker_shutdown` is an opt-in native rc.d service for an administrator-selected
list of active Lurker mountpoints. It closes them in the exact order supplied; inner
user directory mounts must precede their outer native volumes. Each operation uses Lurker's
normal identity-checked close. It never force-unmounts, repairs filesystems,
imports pools or obtains credentials.

The service is disabled by default. Packaging puts these dormant files under
`share/lurker/host-setup/freebsd`; it does not install a service in rc.d or enable
it. From that directory, a normal administrator installation is:

```sh
sudo install -o root -g wheel -m 755 lurker_shutdown /usr/local/etc/rc.d/lurker_shutdown
sudo install -d -o root -g wheel -m 755 /usr/local/etc/lurker
sudo install -o root -g wheel -m 600 shutdown-targets /usr/local/etc/lurker/shutdown-targets
sudo service lurker_shutdown oneplan
sudo sysrc lurker_shutdown_enable=YES
```

Prepare `shutdown-targets` before those commands. Each non-comment line contains
an existing account name, one literal tab, and an absolute mountpoint. Spaces and
Unicode in mountpoints are preserved; tabs and newlines cannot occur in a path.
Duplicate mountpoints and extra or trailing tab separators are rejected. List
user directory mounts before their outer mounts.
For example, replace these paths with the actual managed mounts:

```text
adjutant	/data/plain folder
root	/data/outer volume
```

The list, installed CLI, any required sudo executable, and their ancestors must
be canonical, root-owned and not group/world writable. Each action captures at
most 64 KiB in a private, read-only snapshot, validates that complete snapshot,
and uses the same snapshot for execution even if the administrator replaces
the original list. `oneplan` validates and prints the ordered actions without
closing storage. `service lurker_shutdown onestop` explicitly tests that plan
before a machine shutdown. Keep the selected cohort within the host's measured
memory capacity.

User FUSE also needs the ordinary host prerequisites. To persist the installed
native module and user-mount authorization, an administrator can add fusefs to
`kld_list` with `sudo sysrc kld_list+=fusefs` and set `vfs.usermount=1` in
`/etc/sysctl.conf`. Verify the resulting `/dev/fuse` permissions for the intended
account. This is separate from the shutdown service.

A busy or unconfirmed close returns failure and stops this service's list. The
system shutdown mechanism can still continue terminating processes; this service
does not veto shutdown. Already-closed or unowned targets also produce refusal.
Keep the list limited to mounts left open for this shutdown; close busy mounts
normally and remove their entries before requesting shutdown.
The failed stock shutdown and any later explicitly enabled service qualification
must remain separate results in `DIRECTIVES/PLAN/NATIVE_FRESH_20261127/FREEBSD_RESULTS.md`.

Native service references:

- https://man.freebsd.org/cgi/man.cgi?query=rc.subr&sektion=8
- https://man.freebsd.org/cgi/man.cgi?query=rcorder&sektion=8
- https://raw.githubusercontent.com/freebsd/freebsd-src/releng/15.1/libexec/rc/rc.shutdown
