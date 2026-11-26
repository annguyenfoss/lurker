# File encryption working notes

The shell functions shared in discussion describe a personal workflow. Reproducing them and integrating `tar`, `pigz`, or `pv` are out of scope.

## GnuPG

- Wrap the installed GnuPG. Select one or more usable encryption keys from the user's local keyring, or choose public key files with the OS or desktop environment's native file picker. Use each selected file directly with `--recipient-file`, without importing it.
- Offer GPG key, password, or both. With both selected, either the matching private key or the password can decrypt the file independently.
- Key creation, import, export, deletion, and other key management are out of scope.

## OpenSSL CMS

- Wrap the installed OpenSSL for CMS file encryption. `openssl enc` compatibility is out of scope.
- Use AES-256-GCM for new CMS files encrypted to certificates.
- Use native OS or desktop file pickers for CMS credentials.
- Encrypt to one or more existing recipient X.509 certificate files. Any selected recipient can decrypt independently.
- Decrypt with an existing matching private key file and its certificate when needed. Prompt for the key's unlock passphrase if it is encrypted.
