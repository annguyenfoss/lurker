# File encryption working notes

## File workflow

Prioritize personal use, including transferring secrets between one person's machines and sharing with a few people. Shared passwords and shared keyfiles are the primary credentials. Public-key recipients are an additional option.

OpenSSL CMS, age, and GnuPG each take one input file and produce one output file per operation. Archive creation and extraction, compression, and decompression are out of scope.

The shell functions shared in discussion describe a personal workflow. Reproducing them and integrating `tar`, `pigz`, or `pv` are out of scope.

## GnuPG

- Wrap the installed GnuPG. Select one or more usable encryption keys from the user's local keyring, or choose public key files with the OS or desktop environment's native file picker. Use each selected file directly with `--recipient-file`, without importing it.
- Offer GPG key, password, or both. With both selected, either the matching private key or the password can decrypt the file independently.
- Key creation, import, export, deletion, and other key management are out of scope.

## OpenSSL CMS

- Use one CMS recipient workflow, equivalent to `openssl cms -encrypt` and `-decrypt`, with AES-256-GCM for new files. Direct symmetric CMS `EncryptedData` and `openssl enc` compatibility are out of scope.
- To encrypt, select at least one credential: a passphrase, a shared AES keyfile containing raw key material, or one or more existing X.509 recipient certificates. Allow any combination. Each selected credential provides independent decryption access.
- Use native OS or desktop file pickers for keyfiles, certificates, and private keys. For certificate-based decryption, use the matching private key file and its certificate when needed. Prompt for the private key's unlock passphrase if it is encrypted.
- Use the installed OpenSSL CLI where credentials can be passed without exposing secrets in process arguments. Choose a secure transport for shared keys and passphrases before implementing those options. The stock CLI accepts `-secretkey` and `-pwri_password` as argument values.
- Limit key handling to selecting existing files. Key and certificate creation, import, export, deletion, and other management are out of scope.
