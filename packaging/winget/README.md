# winget

Templates for the [winget-pkgs](https://github.com/microsoft/winget-pkgs) manifests of `amigo-labs.pincel`, as in amigo-blinzel.

After a release is published:

1. Fill `{{VERSION}}` and `{{SHA256_X64}}` (the `pincel-windows-x64-setup.exe` line of the release's `SHA256SUMS`).
2. Validate: `winget validate --manifest packaging/winget` and test locally: `winget install --manifest packaging/winget`.
3. Submit the three files to winget-pkgs under `manifests/a/amigo-labs/pincel/<version>/`.

Later versions: `wingetcreate update amigo-labs.pincel --version <v> --urls <x64-url> --submit` updates the previous manifest automatically.
