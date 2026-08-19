<p align="center">
  <img src="./src/assets/logo.png" width="108" height="108" alt="KhinFetch logo" />
</p>

<h1 align="center">KhinFetch</h1>

<p align="center">
  An unofficial, cross-platform desktop download manager for Khinsider game soundtracks.
</p>

<p align="center">
  <a href="https://github.com/chauyinn/khinfetch/releases"><img src="https://img.shields.io/github/v/release/chauyinn/khinfetch?color=2563eb&label=Release&logo=github&style=flat-square" alt="Latest Release" /></a>
  <a href="https://github.com/chauyinn/khinfetch/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/chauyinn/khinfetch/ci.yml?branch=main&label=CI&logo=githubactions&logoColor=white&style=flat-square" alt="CI Status" /></a>
  <img src="https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-475569?style=flat-square" alt="Platforms" />
  <a href="https://github.com/chauyinn/khinfetch/blob/main/LICENSE"><img src="https://img.shields.io/badge/License-GPL--3.0--or--later-blue?style=flat-square" alt="License: GPL-3.0-or-later" /></a>
</p>

<p align="center">
  <a href="./README.md">English</a>
  ·
  <a href="./README.zh-CN.md">简体中文</a>
</p>

---

> [!NOTE]
> This project was generated with AI assistance.

> [!IMPORTANT]
> **Disclaimer**: KhinFetch is an independent, unofficial client. It is not affiliated with, endorsed by, or maintained by Khinsider. The project does not grant any rights to music, artwork, metadata, or other third-party content. You are responsible for ensuring that your use and downloads are lawful and comply with the applicable website terms.

## ✨ Features

- **🔍 Smart Search & URL Parsing**: Search Khinsider albums directly by keyword or parse album/playlist URLs on the fly.
- **📊 Rich Metadata Preview**: Inspect release year, catalog number, track list, duration, audio formats, and file sizes before downloading.
- **🎧 In-App Audio Preview**: Listen to online MP3 sample previews directly within the app before queuing.
- **🎵 Best Quality Priority**: Automatically select the highest quality format available (e.g. Lossless FLAC) with graceful lossy fallback and clear mixed-quality indicators.
- **📦 Batch Links Import**: Queue multiple album and playlist URLs concurrently.
- **🖼️ Album Artwork Archiving**: Save original high-resolution album cover art alongside downloaded audio tracks.
- **⚡ Persistent Queue Management**: SQLite-backed download engine with pause, resume, retry, cancel, and seamless interrupted download recovery.
- **⚙️ Flexible Customization**: Fine-tune concurrent downloads, quality preferences, file integrity checks, logging levels, bilingual interface, and system-adaptive themes.

## 📸 Screenshots

### Search

![KhinFetch search interface](./docs/images/search.png)

### Download queue

![KhinFetch download queue](./docs/images/downloads.png)

## 🚀 Installation & Releases

KhinFetch provides prebuilt packages for **Windows**, **macOS**, and **Linux** on the [GitHub Releases](https://github.com/chauyinn/khinfetch/releases) page:

- **Windows**: Standard installer (`.exe`) and portable standalone archive (`.zip`, extract and run).
- **macOS / Linux**: System-specific distribution packages.

> [!TIP]
> The portable version stores settings, the SQLite database, and logs in the current user's standard application data directory.

## 🛠️ Development

### Prerequisites

- **Node.js**: `26.x`
- **pnpm**: `11.20.x`
- **Rust**: `1.97` or newer
- Operating system dependencies listed in the [Tauri v2 prerequisites guide](https://v2.tauri.app/start/prerequisites/)

### Getting Started

```bash
# Install frontend dependencies
pnpm install

# Run locally in development mode
pnpm tauri dev
```

### Code Quality & Validation

Please run and pass all validation checks before submitting a pull request:

```bash
# Frontend linting and typechecking/build
pnpm lint
pnpm build

# Rust formatting, Clippy checks, and unit tests
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --locked
```

### Build Production Desktop Application

```bash
pnpm tauri build
```

Unsigned local bundles will be generated under `src-tauri/target/release/bundle/`.

## 📁 Project Structure

```text
khinfetch/
├── src/                    # React 19 + TypeScript frontend
│   ├── api/client.ts       # Unified communication boundary for Tauri backend calls
│   ├── api/generated/      # TypeScript types mirrored from Rust DTOs
│   ├── assets/             # Frontend icons and static assets
│   └── i18n/               # English and Simplified Chinese localizations
├── src-tauri/              # Rust Tauri 2 backend core
│   ├── src/                # Downloader, parser, SQLite persistence, and system logic
│   ├── migrations/         # SQLite database schema migrations
│   └── icons/              # Platform bundle icons
├── docs/images/            # Documentation screenshots
└── .github/workflows/      # CI build/test and release automation
```

## 🤝 Contributing

Issues and pull requests are welcome. Keep changes focused, preserve the unofficial-client positioning, and run the validation commands above before submitting a pull request.

## 💖 Acknowledgements

- [Khinsider](https://downloads.khinsider.com/) provides the website data consumed by the application.
- The implementation was inspired by the open-source [KhinsiderDownloader](https://github.com/weespin/KhinsiderDownloader) project.

## 📄 License

Copyright (C) 2026 KhinFetch contributors.

Unless stated otherwise, the original KhinFetch source code and assets are licensed under the **GNU General Public License, version 3, or (at your option) any later version** (`GPL-3.0-or-later`). See [LICENSE](./LICENSE) for details.

Third-party dependencies and content obtained from external services remain subject to their own licenses, copyrights, and terms.
