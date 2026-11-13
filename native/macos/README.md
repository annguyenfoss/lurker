# macOS backend qualification patches

`cryfs-fuse-t.patch` is an **experimental, unapplied** patch for CryFS 1.0.3
commit `5d0cc36785d93e12250696a3ffc928042f2e8d61`. It selects the published
FUSE-T pkg-config module, supplies the removed libc++ visibility annotation for
range-v3 when needed, and adds the same byte ordering already used by
`std::less<IdWrapper>` for newer libc++ transparent comparisons. It changes no
encryption primitive or storage format.

Native arm64 testing with Apple Clang 21 / SDK 27.2 and FUSE-T 1.2.9 built and
mounted the backend. File write, fsync, rename and truncation worked, but SQLite
could not enable WAL. **This runtime combination is not qualified for Lurker's
application/profile use case and must not become a production build default.**
The owned test mount was closed gracefully. A compatible macFUSE setup still
needs separate native qualification.

Related upstream compiler issue:
https://github.com/ericniebler/range-v3/issues/1866
