# Updater release setup

KhinFetch uses the Tauri v2 updater with signed artifacts published to GitHub Releases.

## Repository secrets

- `TAURI_SIGNING_PRIVATE_KEY`: the full contents of the updater private key.
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: optional. Do not create this secret for the current unencrypted key.

The public key embedded in `src-tauri/tauri.conf.json` must match this private key. Never commit the private key. Keep an offline backup: losing it prevents existing installations from accepting future updates.

The initial private key generated for this repository is stored outside the worktree at:

```text
C:\Users\Engineer\.tauri\khinfetch-updater.key
```

## Release verification

Tag releases continue to use `vMAJOR.MINOR.PATCH`. The release workflow creates signed updater artifacts and merges them into `latest.json`. Its final job verifies these targets before the draft release is ready for publication:

- `linux-x86_64`
- `windows-x86_64`
- `darwin-aarch64`
- `darwin-x86_64`

Publish the draft only after this job succeeds and the release assets contain `latest.json` and `.sig` files.

Windows portable builds and Linux packages other than AppImage only direct users to the latest release page. Windows installer builds, macOS app bundles, and Linux AppImage builds support integrated installation.
