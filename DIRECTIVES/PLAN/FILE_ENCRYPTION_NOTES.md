# File encryption working notes

## File workflow

Prioritize personal use, including transferring secrets between one person's machines and sharing with a few people. Shared passwords and shared keyfiles are the primary credentials. Public-key recipients are an additional option.

OpenSSL CMS, age, and GnuPG each take one input file and produce one output file per operation. Archive creation and extraction, compression, and decompression are out of scope.

All three support multiple public-key recipients per encrypted file. None specifies a fixed public-key recipient count in its CLI documentation or format specification. OS command-line limits still apply when passing recipients as individual arguments.

The shell functions shared in discussion describe a personal workflow. Reproducing them and integrating `tar`, `pigz`, or `pv` are out of scope.

## CLI credential options

Use the same option names across Lurker wherever the operation supports the credential.

- `--key-file FILE` supplies a shared credential from a file. GnuPG reads the first line as the passphrase. OpenSSL CMS reads the AES key material.
- `--passphrase` selects interactive passphrase entry. The option takes no value.
- `--public-key RECIPIENT` selects a public-key recipient for encryption. Repeat it for multiple recipients. Use a local key fingerprint or public-key file for GnuPG, a recipient file for age, and an X.509 certificate file for OpenSSL CMS.
- `--private-key FILE` selects a private key for decryption where the backend accepts a private-key file. Use an age identity file for age or a matching private-key file for OpenSSL CMS. GnuPG resolves the matching private key through the user's existing GnuPG setup.

## age

- Choose either one entered passphrase or one or more public-key recipients. The two credential modes are mutually exclusive for a given file.
- Use the native OS or desktop file picker for public recipient files when encrypting and a matching private identity file when decrypting. A recipient file can list multiple public keys, one per line.
- The age flow has no shared raw keyfile or passphrase-file option.

## GnuPG

- Wrap the installed GnuPG. Select one or more usable encryption keys from the user's local keyring, or choose public key files with the OS or desktop environment's native file picker. Each `--recipient-file` must contain exactly one OpenPGP key and is used without importing it.
- Offer public-key recipients, one shared passphrase, or both. Any matching private key or the shared passphrase can decrypt independently. GnuPG's `--passphrase-file` reads only the first line as that same passphrase. GnuPG has no raw shared-key recipient mode.
- Key creation, import, export, deletion, and other key management are out of scope.

## OpenSSL CMS

- Use one CMS recipient workflow, equivalent to `openssl cms -encrypt` and `-decrypt`, with AES-256-GCM for new files. Direct symmetric CMS `EncryptedData` and `openssl enc` compatibility are out of scope.
- To encrypt, select at least one credential. Allow up to one shared passphrase, one shared AES keyfile containing raw key material, and multiple existing X.509 recipient certificates. Allow any combination. Each selected credential provides independent decryption access.
- Use native OS or desktop file pickers for keyfiles, certificates, and private keys. For certificate-based decryption, use the matching private key file and its certificate when needed. Prompt for the private key's unlock passphrase if it is encrypted.
- Use the installed OpenSSL CLI where credentials can be passed without exposing secrets in process arguments. Choose a secure transport for shared keys and passphrases before implementing those options. The stock CLI accepts one `-secretkey` and one effective `-pwri_password` as argument values and has no file input for those credentials. Its `-passin file:…` option unlocks a private key.
- Limit key handling to selecting existing files. Key and certificate creation, import, export, deletion, and other management are out of scope.
