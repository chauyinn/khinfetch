<p align="center">
  <img src="./src/assets/logo.png" width="108" height="108" alt="KhinFetch 标志" />
</p>

<h1 align="center">KhinFetch</h1>

<p align="center">
  面向 Khinsider 游戏音乐原声资源的非官方跨平台桌面下载管理器。
</p>

<p align="center">
  <a href="https://github.com/chauyinn/khinfetch/releases"><img src="https://img.shields.io/github/v/release/chauyinn/khinfetch?color=2563eb&label=Release&logo=github&style=flat-square" alt="最新发布" /></a>
  <a href="https://github.com/chauyinn/khinfetch/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/chauyinn/khinfetch/ci.yml?branch=main&label=CI&logo=githubactions&logoColor=white&style=flat-square" alt="CI 构建状态" /></a>
  <img src="https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-475569?style=flat-square" alt="支持平台" />
  <a href="https://github.com/chauyinn/khinfetch/blob/main/LICENSE"><img src="https://img.shields.io/badge/License-GPL--3.0--or--later-blue?style=flat-square" alt="开源许可证" /></a>
</p>

<p align="center">
  <a href="./README.md">English</a>
  ·
  <a href="./README.zh-CN.md">简体中文</a>
</p>

---

> [!NOTE]
> 本项目由 AI 辅助生成。

> [!IMPORTANT]
> **免责声明**：KhinFetch 是独立开发的非官方客户端，与 Khinsider 不存在从属、授权、认可或维护关系。本项目不会授予用户对音乐、封面、元数据或其他第三方内容的任何权利。用户有责任确保其使用和下载行为合法，并遵守相关网站条款。

## ✨ 功能特性

- **🔍 智能搜索与链接解析**：支持按关键词搜索 Khinsider 专辑，或直接粘贴专辑与播放列表链接进行解析。
- **📊 完整元数据预览**：下载前直观查看专辑年份、编号、曲目列表、时长、格式及文件大小。
- **🎧 在线即时试听**：下载前支持直接在线播放 MP3 试听片段。
- **🎵 最高音质优先**：智能为每首曲目匹配最高可用音质（无损 FLAC / 高音质 MP3），格式缺失时自动回退并清晰标示混合音质。
- **📦 批量链接导入**：支持一次性粘贴多个专辑和播放列表链接加入队列。
- **🖼️ 专辑封面保存**：可选择在下载音频的同时，将原画质专辑封面保存至同级目录。
- **⚡ 持久化下载管理**：基于 SQLite 构建下载任务队列，支持暂停、恢复、重试、取消，意外中断后随时断点恢复。
- **⚙️ 丰富个性化配置**：自由设置并发下载数、音质偏好、文件完整性校验、日志级别、中英双语及深浅色主题。

## 📸 界面预览

### 搜索

![KhinFetch 搜索界面](./docs/images/search.png)

### 下载队列

![KhinFetch 下载队列](./docs/images/downloads.png)

## 🚀 获取与运行

KhinFetch 源码构建面向 **Windows**、**macOS** 和 **Linux** 平台。最新预构建包可在 [GitHub Releases](https://github.com/chauyinn/khinfetch/releases) 下载：

- **Windows**：提供安装包与免安装便携版（Portable ZIP，解压即用）。
- **macOS / Linux**：提供对应系统格式分发包。

> [!TIP]
> 便携版仍会将设置、数据库和日志安全存储在当前用户的系统应用数据目录中。

## 🛠️ 本地开发

### 环境要求

- **Node.js**：`26.x`
- **pnpm**：`11.20.x`
- **Rust**：`1.97` 或更高版本
- 目标系统平台所需的 [Tauri 2 依赖环境](https://v2.tauri.app/start/prerequisites/)

### 快速开始

```bash
# 安装前端依赖
pnpm install

# 启动本地开发环境
pnpm tauri dev
```

### 代码检查与测试验证

在提交代码或创建 Pull Request 前，请确保运行并通过以下验证命令：

```bash
# 前端 Lint 与构建检查
pnpm lint
pnpm build

# Rust 代码格式化、Clippy 检查与单元测试
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --locked
```

### 构建桌面应用

```bash
pnpm tauri build
```

构建生成的本地安装包将输出至 `src-tauri/target/release/bundle/` 目录。

## 📁 项目结构

```text
khinfetch/
├── src/                    # React 19 + TypeScript 前端
│   ├── api/client.ts       # 前端调用 Tauri 后端的统一通信边界
│   ├── api/generated/      # 与 Rust DTO 对应的 TypeScript 类型定义
│   ├── assets/             # 前端图标与静态资源
│   └── i18n/               # 简体中文与英文界面文本
├── src-tauri/              # Rust Tauri 2 后端核心
│   ├── src/                # 下载器、解析引擎、SQLite 存储及系统逻辑
│   ├── migrations/         # SQLite 数据库迁移文件
│   └── icons/              # 各平台打包图标
├── docs/images/            # 文档截图资源
└── .github/workflows/      # CI 编译测试与自动化发布工作流
```

## 🤝 参与贡献

欢迎提交 Issue 和 Pull Request，在提交 Pull Request 前运行上述验证命令。

## 💖 致谢

- [Khinsider](https://downloads.khinsider.com/) 提供本应用所读取的网站数据。
- 项目实现参考了开源项目 [KhinsiderDownloader](https://github.com/weespin/KhinsiderDownloader)。

## 📄 许可证

版权所有 (C) 2026 KhinFetch contributors。

除非文件中另有说明，本仓库内 KhinFetch 的原创源代码及项目自有资产均依据 **GNU 通用公共许可证第 3 版或任何后续版本**（`GPL-3.0-or-later`）授权。详情请参阅 [LICENSE](./LICENSE)。

第三方依赖以及从外部服务取得的内容仍分别受其自身许可证、著作权和条款约束。KhinFetch 的开源许可证不会对这些第三方内容重新授权，也不会授予用户相关内容权利。
