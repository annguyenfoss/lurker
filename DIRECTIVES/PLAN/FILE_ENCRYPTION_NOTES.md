# File encryption working notes

## GnuPG

- Wrap the installed GnuPG. Discover usable encryption keys in the user's local keyring and offer them in the recipient picker.
- Offer GPG key, password, or both. With both selected, either the matching private key or the password can decrypt the file independently.
- Key handling is limited to selecting existing keys from the user's local keyring. Key creation, import, export, deletion, and other key management are out of scope.
