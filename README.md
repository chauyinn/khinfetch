<p align="center">
  <img src="./src/assets/logo.png" width="96" height="96" alt="khinfetch logo" />
</p>

<h1 align="center">khinfetch</h1>

<p align="center">
  An unofficial, cross-platform desktop download manager for Khinsider game soundtracks.
</p>

<p align="center">
  <a href="./README.zh-CN.md">简体中文</a>
  ·
  <a href="./LICENSE">GPL-3.0-or-later</a>
</p>

> [!NOTE]
> This README was generated with AI assistance.

> [!IMPORTANT]
> khinfetch is an independent, unofficial client. It is not affiliated with, endorsed by, or maintained by Khinsider. The project does not grant any rights to music, artwork, metadata, or other third-party content. You are responsible for ensuring that your use and downloads are lawful and comply with the applicable website terms.

## Features

- Search Khinsider albums or open an album or playlist URL directly.
- Select individual tracks or add multiple album and playlist links in a batch.
- Prefer the best available audio format or download MP3 only.
- Optionally save original album artwork alongside audio files.
- Pause, resume, retry, cancel, and inspect persistent download jobs.
- Recover interrupted jobs from the SQLite-backed queue.
- Use the interface in English or Simplified Chinese, with light, dark, and system themes.

## Screenshots

### Search

![khinfetch search interface](./docs/images/search.png)

### Download queue

![khinfetch download queue](./docs/images/downloads.png)

## Status

khinfetch is under active development. Source builds target Windows, macOS, and Linux, and all three platforms are compiled in CI. GitHub Releases may provide prebuilt packages; unless a release states otherwise, those packages are currently unsigned.

## Development

### Prerequisites

- Node.js 26.x
- pnpm 11.20.x
- Rust 1.97 or newer
- The platform prerequisites listed in the [Tauri documentation](https://v2.tauri.app/start/prerequisites/)

### Run locally

```bash
pnpm install --frozen-lockfile
pnpm tauri dev
```

### Validate changes

```bash
pnpm lint
pnpm build
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --locked
```

### Build the desktop application

```bash
pnpm tauri build
```

Unsigned local bundles are written under `src-tauri/target/release/bundle/` when supported by the host platform.

## Project Structure

- `src/`: React and TypeScript frontend
- `src/api/client.ts`: the frontend boundary for all Tauri backend calls
- `src/api/generated/`: TypeScript types mirrored from Rust DTOs
- `src/i18n/`: Simplified Chinese and English interface strings
- `src/assets/`: frontend and documentation assets
- `docs/images/`: screenshots used in the README files
- `src-tauri/src/`: Rust commands, downloads, parsing, storage, logging, and filesystem logic
- `src-tauri/migrations/`: SQLite migrations
- `src-tauri/icons/`: platform-specific Tauri bundle icons
- `.github/workflows/`: continuous integration and release automation

When changing a public Rust DTO in `src-tauri/src/domain/`, update the corresponding generated TypeScript types in `src/api/generated/` in the same change.

## Contributing

Issues and pull requests are welcome. Keep changes focused, preserve the unofficial-client positioning, and run the validation commands above before submitting a pull request.

## Acknowledgements

- [Khinsider](https://downloads.khinsider.com/) provides the website data consumed by the application.
- The implementation was inspired by the open-source [KhinsiderDownloader](https://github.com/weespin/KhinsiderDownloader) project.

## License

Copyright (C) 2026 khinfetch contributors.

Unless a file states otherwise, the original khinfetch source code and project-owned assets in this repository are licensed under the **GNU General Public License, version 3, or (at your option) any later version** (`GPL-3.0-or-later`). You may use, study, modify, and redistribute khinfetch under those terms. Conveying modified versions or binaries requires compliance with the GPL, including the applicable corresponding-source obligations.

khinfetch is distributed **without any warranty**; without even the implied warranties of merchantability or fitness for a particular purpose. See the complete license text in [LICENSE](./LICENSE).

Third-party dependencies and content obtained from external services remain subject to their own licenses, copyrights, and terms. The khinfetch license does not relicense or grant rights to that third-party content.
