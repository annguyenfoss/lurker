# File encryption working notes

## GnuPG

- Wrap the installed GnuPG. Discover usable encryption keys in the user's local keyring and offer them in the recipient picker.
- Offer GPG key, password, or both. With both selected, either the matching private key or the password can decrypt the file independently.
- Key creation and management are open for discussion. Start with key selection and consider a guided creation flow if users often arrive without an encryption key.
